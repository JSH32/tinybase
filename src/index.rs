use std::fmt::Debug;
use std::time::Duration;

use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use sled::{Db, Subscriber, Tree};
use uuid::Uuid;

use crate::table::Record;

/// An index of a Table.
pub struct Index<T, I>
where
    T: Serialize + Debug,
    for<'de> T: Deserialize<'de>,
    I: AsRef<[u8]>,
{
    table_data: Tree,
    /// Function which will be used to compute the key per insert.
    key_func: Box<dyn Fn(&T) -> I + Send + Sync>,
    /// Built index, each key can have multiple matching records.
    indexed_data: Tree,
    /// Reference to uncomitted operation log.
    subscriber: Subscriber,
}

impl<T, I> Index<T, I>
where
    T: Serialize + Debug,
    for<'de> T: Deserialize<'de>,
    I: AsRef<[u8]>,
{
    pub(crate) fn new(
        idx_name: &str,
        engine: &Db,
        table_data: &Tree,
        key_func: impl Fn(&T) -> I + Send + Sync + 'static,
    ) -> Self {
        let new_index = Self {
            table_data: table_data.clone(),
            key_func: Box::new(key_func),
            indexed_data: engine.open_tree(idx_name).unwrap(),
            subscriber: table_data.watch_prefix(vec![]), // log
        };

        new_index
    }

    fn commit_log(&mut self) {
        // Commit log of events on the main table.
        while let Ok(event) = self.subscriber.next_timeout(Duration::new(0, 0)) {
            match event {
                sled::Event::Insert { key, value } => self.insert(&Record {
                    id: deserialize(&key).unwrap(),
                    data: deserialize(&value).unwrap(),
                }),
                sled::Event::Remove { key } => self.delete(&Record {
                    id: deserialize(&key).unwrap(),
                    data: deserialize(&self.table_data.get(&key).unwrap().unwrap()).unwrap(),
                }),
            }
        }
    }

    /// Insert a record into the index.
    fn insert(&self, record: &Record<T>) {
        let key = (self.key_func)(&record.data);

        if let Some(data) = self.indexed_data.get(&key).unwrap() {
            let mut vec: Vec<Uuid> = deserialize(&data).unwrap();
            vec.push(record.id);
            self.indexed_data
                .insert(key, serialize(&vec).unwrap())
                .unwrap();
        } else {
            self.indexed_data
                .insert(key, serialize(&vec![record.id]).unwrap())
                .unwrap();
        }
    }

    /// Delete record from index.
    fn delete(&self, record: &Record<T>) {
        let key = (self.key_func)(&record.data);

        if let Some(data) = self.indexed_data.get(&key).unwrap() {
            let mut primary_vec: Vec<Uuid> = deserialize(&data).unwrap();

            // We can remove the entire node here.
            if primary_vec.len() < 2 {
                self.indexed_data.remove(&key).unwrap();
            } else {
                let pos = primary_vec.iter().position(|id| *id == record.id).unwrap();
                primary_vec.remove(pos);
                self.indexed_data.remove(&key).unwrap();
            }
        }
    }

    /// Query by key.
    pub fn query(&mut self, query: &I) -> Vec<Record<T>> {
        self.commit_log();

        if let Ok(Some(bytes)) = self.indexed_data.get(query) {
            let uuids: Vec<Uuid> = deserialize(&bytes).unwrap();

            uuids
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        self.table_data.get(serialize(&uuids[0]).unwrap()).unwrap(),
                    )
                })
                .filter(|(_, x)| x.is_some())
                .map(|(uuid, result)| Record {
                    id: uuid,
                    data: deserialize::<T>(&result.unwrap()).unwrap(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}
