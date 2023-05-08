use std::any::Any;
use std::ops::Deref;
use std::sync::Arc;
use std::vec;

use bincode::{deserialize, serialize};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sled::{Db, IVec, Tree};

use crate::record::Record;
use crate::result::DbResult;
use crate::subscriber::{self, Subscriber};
use crate::table::TableType;

pub trait IndexType: Serialize + DeserializeOwned {}
impl<T: Serialize + DeserializeOwned> IndexType for T {}

/// An index of a Table.
///
/// # Type Parameters
///
/// * `T` - The type of the value to be stored in the table.
/// * `I` - The type of the index key.
pub struct Index<T: TableType, I: IndexType>(pub(crate) Arc<IndexInner<T, I>>);

impl<T: TableType, I: IndexType> Clone for Index<T, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: TableType, I: IndexType> Deref for Index<T, I> {
    type Target = Arc<IndexInner<T, I>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct IndexInner<T: TableType, I: IndexType> {
    table_data: Tree,
    /// Function which will be used to compute the key per insert.
    key_func: Box<dyn Fn(&T) -> I + Send + Sync>,
    /// Built index, each key can have multiple matching records.
    indexed_data: Tree,
    /// Reference to uncommitted operation log.
    subscriber: Subscriber<T>,
}

impl<T: TableType, I: IndexType> IndexInner<T, I> {
    pub(crate) fn new(
        idx_name: &str,
        engine: &Db,
        table_data: &Tree,
        key_func: impl Fn(&T) -> I + Send + Sync + 'static,
        subscriber: Subscriber<T>,
    ) -> DbResult<Self> {
        let need_sync = !engine.tree_names().contains(&IVec::from(idx_name));

        let new_index = Self {
            table_data: table_data.clone(),
            key_func: Box::new(key_func),
            indexed_data: engine.open_tree(idx_name)?,
            subscriber,
        };

        // Index is new, sync data
        if need_sync {
            new_index.sync()?;
        }

        Ok(new_index)
    }

    /// Resync index to be up to date with table.
    pub fn sync(&self) -> DbResult<()> {
        self.indexed_data.clear()?;
        for key in self.table_data.iter().keys() {
            // This should always succeed
            if let Some(data) = self.table_data.get(&key.clone()?)? {
                self.insert(&Record {
                    id: deserialize(&key?)?,
                    data: deserialize(&data)?,
                })?;
            }
        }

        Ok(())
    }

    fn commit_log(&self) -> DbResult<()> {
        // Commit log of events on the main table.
        while let Ok(event) = self.subscriber.rx.try_recv() {
            match event {
                subscriber::Event::Remove(record) => self.delete(&record)?,
                subscriber::Event::Insert(record) => self.insert(&record)?,
                subscriber::Event::Update {
                    id,
                    old_data,
                    new_data,
                } => {
                    self.delete(&Record { id, data: old_data })?;
                    self.insert(&Record { id, data: new_data })?;
                }
            }
        }

        Ok(())
    }

    /// Insert a record into the index.
    fn insert(&self, record: &Record<T>) -> DbResult<()> {
        let key = serialize(&(self.key_func)(&record.data))?;

        if let Some(data) = self.indexed_data.get(&key)? {
            let mut vec: Vec<u64> = deserialize(&data)?;
            vec.push(record.id);
            self.indexed_data.insert(key, serialize(&vec)?)?;
        } else {
            self.indexed_data
                .insert(key, serialize(&vec![record.id])?)?;
        }

        Ok(())
    }

    /// Delete record from index.
    fn delete(&self, record: &Record<T>) -> DbResult<()> {
        let key = serialize(&(self.key_func)(&record.data))?;

        if let Some(data) = self.indexed_data.get(&key)? {
            let mut index_values: Vec<u64> = deserialize(&data)?;

            // We can remove the entire node here since its one element.
            if index_values.len() < 2 {
                self.indexed_data.remove(&key)?;
            } else {
                // Remove the single ID from here.
                if let Some(pos) = index_values.iter().position(|id| *id == record.id) {
                    index_values.remove(pos);
                    // Replace the row with one that doesn't have the element.
                    self.indexed_data.insert(&key, serialize(&index_values)?)?;
                }
            }
        }

        Ok(())
    }

    /// Query by index key.
    ///
    /// This method searches for multiple [`Record`]'s that match the index key provided.
    ///
    /// # Arguments
    ///
    /// * `query` - A reference to the query key.
    ///
    /// # Errors
    ///
    /// Returns an error if the query could not be performed.
    pub fn select(&self, query: &I) -> DbResult<Vec<Record<T>>> {
        self.commit_log()?;

        Ok(
            if let Ok(Some(bytes)) = self.indexed_data.get(serialize(&query)?) {
                let ids: Vec<u64> = deserialize(&bytes)?;

                let mut results = vec![];
                for id in ids {
                    let encoded_data = self.table_data.get(serialize(&id)?)?;
                    if let Some(encoded_data) = encoded_data {
                        results.push(Record {
                            id,
                            data: deserialize::<T>(&encoded_data)?,
                        })
                    }
                }

                results
            } else {
                Vec::new()
            },
        )
    }

    pub fn update(&self, query: &I, value: T) -> DbResult<Vec<Record<T>>> {
        self.commit_log()?;

        let mut new_data = vec![];

        if let Ok(Some(bytes)) = self.indexed_data.get(serialize(&query)?) {
            let ids: Vec<u64> = deserialize(&bytes)?;
            let new_value = serialize(&value)?;

            for id in ids {
                self.table_data.insert(serialize(&id)?, new_value.clone())?;

                new_data.push(Record {
                    id,
                    data: value.clone(),
                })
            }
        }

        Ok(new_data)
    }

    /// Check if a record matches the built index key.
    pub fn exists_record(&self, record: &Record<T>) -> DbResult<bool> {
        self.exists((self.key_func)(&record.data))
    }

    /// Check if a record exists by the key.
    pub fn exists(&self, key: I) -> DbResult<bool> {
        Ok(!self.select(&key)?.is_empty())
    }

    pub fn index_name(&self) -> String {
        std::str::from_utf8(&self.indexed_data.name())
            .unwrap()
            .to_string()
    }
}

/// Type which [`Index`] can be casted to which doesn't require the `I` type parameter.
pub trait AnyIndex<T: TableType> {
    fn record_exists(&self, record: &Record<T>) -> DbResult<bool>;
    fn search(&self, value: Box<dyn Any>) -> DbResult<Vec<Record<T>>>;
    fn idx_name(&self) -> String;
}

impl<T, I> AnyIndex<T> for Index<T, I>
where
    T: TableType,
    I: IndexType + 'static,
{
    fn search(&self, value: Box<dyn Any>) -> DbResult<Vec<Record<T>>> {
        let i = *value.downcast::<I>().unwrap();
        self.select(&i)
    }

    fn idx_name(&self) -> String {
        self.index_name()
    }

    fn record_exists(&self, record: &Record<T>) -> DbResult<bool> {
        self.exists_record(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Table, TinyBase};

    #[test]
    fn index_sync() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Insert a string value into the table
        let id = table.insert("value1".to_string()).unwrap();
        let id2 = table.insert("value2".to_string()).unwrap();

        // Create an index for the table
        let index = table.create_index("length", |value| value.len()).unwrap();

        assert!(index.sync().is_ok());

        let results = index.select(&6).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, id);
        assert_eq!(results[1].id, id2);
    }

    #[test]
    fn index_select() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Insert a string value into the table
        table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        // Create an index for the table
        let index = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        let record: Vec<Record<String>> =
            index.select(&"value1".to_string()).expect("Select failed");

        assert_eq!(record.len(), 1);
        assert_eq!(record[0].data, "value1");

        let record_2 = index
            .select(&"non_existent_value".to_string())
            .expect("Select failed");

        assert_eq!(record_2.len(), 0);
    }

    #[test]
    fn index_update() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the table
        let index: Index<String, String> = table
            .create_index("index_name", |value| value.to_owned())
            .unwrap();

        // Insert string values into the table
        let id1 = table.insert("initial_value_1".to_string()).unwrap();
        table.insert("initial_value_2".to_string()).unwrap();

        // Update records with matching key
        let updated_records = index
            .update(&"initial_value_1".to_string(), "updated_value".to_string())
            .expect("Update failed");

        assert_eq!(updated_records.len(), 1);
        assert_eq!(updated_records[0].id, id1);
        assert_eq!(updated_records[0].data, "updated_value");
    }

    #[test]
    fn index_exists() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the table
        let index = table
            .create_index("index_name", |value| value.to_owned())
            .unwrap();

        // Insert a string value into the table
        let id = table.insert("value1".to_string()).unwrap();

        let record = Record {
            id,
            data: "value1".to_string(),
        };

        assert!(index.exists_record(&record).expect("Exists check failed"));

        let record_not_exist = Record {
            id: 999,
            data: "non_existent_value".to_string(),
        };

        assert!(!index
            .exists_record(&record_not_exist)
            .expect("Exists check failed"));
    }
}
