use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};

use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use uuid::Uuid;

use crate::index::Index;
use crate::result::DbResult;
use crate::subscriber::{Event, Subscriber};
/// A single record in a table.
#[derive(Debug, Clone)]
pub struct Record<T> {
    /// Unique required ID of a record.
    pub id: Uuid,
    pub data: T,
}

pub(crate) type SenderMap<T> = Arc<RwLock<HashMap<Uuid, Sender<T>>>>;

/// Represents a table in the database.
///
/// [`Table`] provides methods for interacting with a table in the database, such as inserting records
/// and creating indexes.
///
/// # Type Parameters
///
/// * `T` - The type of the value to be stored in the table. Must implement [`Serialize`], [`Deserialize`], and [`Debug`].
pub struct Table<T>
where
    T: Serialize + Debug + Clone,
    for<'de> T: Deserialize<'de>,
{
    pub(crate) engine: Db,
    root: Tree,
    name: String,
    _table_type: PhantomData<T>,
    senders: SenderMap<Event<T>>,
}

impl<T> Table<T>
where
    T: Serialize + Debug + Clone,
    for<'de> T: Deserialize<'de>,
{
    /// Creates a new table with the given engine and name.
    ///
    /// This method is intended for internal use and should not be called directly. Instead, use the
    /// [`crate::TinyDb`]'s `open_table()` method.
    ///
    /// # Arguments
    ///
    /// * `engine` - A reference to the `Db` engine.
    /// * `name` - The name of the table.
    ///
    /// # Errors
    ///
    /// Returns an error if the table could not be created.
    pub(crate) fn new(engine: &Db, name: &str) -> DbResult<Self> {
        let root = engine.open_tree(name)?;

        Ok(Self {
            engine: engine.clone(),
            root,
            name: name.to_owned(),
            _table_type: PhantomData::default(),
            senders: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Inserts a new record into the table.
    ///
    /// This method generates a new UUID for the record and serializes it, along with the value, before
    /// inserting them into the table. Returns a [`DbResult`] containing the ID if the insert
    /// is successful or an error if it fails.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be inserted into the table.
    ///
    /// # Errors
    ///
    /// Returns an error if the record could not be inserted.
    ///
    /// # Example
    ///
    /// ```
    /// use tinydb::{TinyDb, Table, Record};
    ///
    /// let db = TinyDb::new(Some("path/to/db"), false);
    /// let mut table: Table<String> = db.open_table("my_table").unwrap();
    /// let id = table.insert("my_value".to_string()).unwrap();
    /// ```
    pub fn insert(&mut self, value: T) -> DbResult<Uuid> {
        let uuid: Uuid = Uuid::new_v4();
        self.root.insert(serialize(&uuid)?, serialize(&value)?)?;
        self.dispatch_event(Event::Insert(Record {
            id: uuid,
            data: value,
        }));

        Ok(uuid)
    }

    pub fn delete(&mut self, id: Uuid) -> DbResult<Option<Record<T>>> {
        let serialized_id = serialize(&id)?;
        if let Some(serialized) = self.root.remove(serialized_id)? {
            let record = Record {
                id,
                data: deserialize(&serialized)?,
            };

            self.dispatch_event(Event::Remove(record.clone()));

            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    pub fn update(&mut self, ids: &[Uuid], value: T) -> DbResult<Vec<Record<T>>> {
        let serialized_value = serialize(&value)?;

        let mut updated = vec![];

        for id in ids {
            self.root.update_and_fetch(serialize(&id)?, |old_value| {
                if let Some(old_value) = old_value {
                    updated.push(Record {
                        id: id.clone(),
                        data: value.clone(),
                    });

                    self.dispatch_event(Event::Update {
                        id: id.clone(),
                        old_data: deserialize(old_value).unwrap(),
                        new_data: value.clone(),
                    });

                    Some(serialized_value.clone())
                } else {
                    None
                }
            })?;
        }

        Ok(updated)
    }

    /// Creates a new index for the table.
    ///
    /// This method takes a name for the index and a function that defines how to construct the index key
    /// from a record in the table. It returns a [`DbResult`] containing the new [`Index`] if successful
    /// or an error if it fails.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the index.
    /// * `key_func` - A function that takes a reference to a record of type `T` and generates the index key
    ///   for each record in the table.
    ///
    /// # Type Parameters
    ///
    /// * `I` - The type of the index key.
    ///
    /// # Errors
    ///
    /// Returns an error if the index could not be created.
    ///
    /// # Example
    ///
    /// ```
    /// use tinydb::{TinyDb, Table, Index};
    ///
    /// let db = TinyDb::new(Some("path/to/db"), false);
    /// let mut table: Table<String> = db.open_table("my_table").unwrap();
    /// let index: Index<String, Vec<u8>> = table.create_index("my_index", |value| value.to_owned()).unwrap();
    /// ```
    pub fn create_index<I: AsRef<[u8]>>(
        &mut self,
        name: &str,
        key_func: impl Fn(&T) -> I + Send + Sync + 'static,
    ) -> DbResult<Index<T, I>> {
        let sender_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel();

        let subscriber = Subscriber::new(sender_id, rx, self.senders.clone());
        self.senders.write().unwrap().insert(sender_id, tx);

        Index::new(
            &format!("{}_idx_{}", self.name, name),
            &self.engine,
            &self.root,
            key_func,
            subscriber,
        )
    }

    fn dispatch_event(&self, event: Event<T>) {
        for sender in self.senders.read().unwrap().values() {
            sender.send(event.clone()).unwrap();
        }
    }
}
