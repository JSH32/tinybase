use std::any::Any;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::vec;

use serde::de::DeserializeOwned;
use serde::Serialize;
use sled::{Db, Tree};

use crate::encoding::{decode, encode};
use crate::record::Record;
use crate::result::DbResult;
use crate::subscriber::{self, Subscriber};
use crate::table::{TableInner, TableType};

use self::private::AnyIndexInternal;

pub trait IndexType: Serialize + DeserializeOwned {}
impl<T: Serialize + DeserializeOwned> IndexType for T {}

/// Provides methods for interacting with an index on a typed table.
pub struct Index<T: TableType + 'static, I: IndexType>(pub(crate) Arc<IndexInner<T, I>>);

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

/// Inner state of an index on a typed table.
pub struct IndexInner<T: TableType + 'static, I: IndexType> {
    table: Weak<TableInner<T>>,
    /// Function which will be used to compute the key per insert.
    key_func: Box<dyn Fn(&T) -> I + Send + Sync>,
    /// Built index, each key can have multiple matching records.
    indexed_data: Tree,
    /// Reference to uncommitted operation log.
    subscriber: Subscriber<T>,
}

impl<T: TableType, I: IndexType> IndexInner<T, I> {
    /// Creates a new index with the given name, engine, table data, key function, and subscriber.
    ///
    /// This method is intended for internal use and should not be called directly. Instead, use the
    /// [`crate::Table`]'s `create_index()` method.
    ///
    /// # Arguments
    ///
    /// * `idx_name` - The name of the index.
    /// * `engine` - The database engine.
    /// * `table` - A weak pointer to the table.
    /// * `key_func` - A function which computes the index key for each record.
    /// * `subscriber` - A subscriber to uncommitted operation log.
    ///
    /// # Returns
    ///
    /// The new [`IndexInner`] instance.
    pub(crate) fn new(
        idx_name: &str,
        engine: &Db,
        table: Weak<TableInner<T>>,
        key_func: impl Fn(&T) -> I + Send + Sync + 'static,
        subscriber: Subscriber<T>,
    ) -> DbResult<Self> {
        let new_index = Self {
            table,
            key_func: Box::new(key_func),
            indexed_data: engine.open_tree(idx_name)?,
            subscriber,
        };

        new_index.sync()?;

        Ok(new_index)
    }

    /// Resync index to be up to date with table.
    pub fn sync(&self) -> DbResult<()> {
        self.indexed_data.clear()?;

        let table = self.table.upgrade().unwrap();
        let root = table.root.write().unwrap();
        for key in root.iter().keys() {
            // This should always succeed
            if let Some(data) = root.get(&key.clone()?)? {
                self.insert(&Record {
                    id: decode(&key?)?,
                    data: decode(&data)?,
                })?;
            }
        }

        Ok(())
    }

    /// Commits the received events from the main table to the index.
    fn commit_log(&self) -> DbResult<()> {
        // Commit log of events on the main table.
        while let Ok(event) = self.subscriber.rx.try_recv() {
            match event {
                subscriber::Event::Remove(record) => self.remove(&record)?,
                subscriber::Event::Insert(record) => self.insert(&record)?,
                subscriber::Event::Update {
                    id,
                    old_data,
                    new_data,
                } => {
                    self.remove(&Record { id, data: old_data })?;
                    self.insert(&Record { id, data: new_data })?;
                }
            }
        }

        Ok(())
    }

    /// Insert a record into the index. The index key will be computed.
    ///
    /// # Arguments
    ///
    /// * `record` - The record to insert.
    fn insert(&self, record: &Record<T>) -> DbResult<()> {
        let key = encode(&(self.key_func)(&record.data))?;

        if let Some(data) = self.indexed_data.get(&key)? {
            let mut vec: Vec<u64> = decode(&data)?;
            vec.push(record.id);
            self.indexed_data.insert(key, encode(&vec)?)?;
        } else {
            self.indexed_data.insert(key, encode(&vec![record.id])?)?;
        }

        Ok(())
    }

    /// Delete a record from the index.
    /// The record will compute an index key to delete by.
    ///
    /// # Arguments
    ///
    /// * `record` - The record to delete.
    fn remove(&self, record: &Record<T>) -> DbResult<()> {
        let key = encode(&(self.key_func)(&record.data))?;

        if let Some(data) = self.indexed_data.get(&key)? {
            let mut index_values: Vec<u64> = decode(&data)?;

            // We can remove the entire node here since its one element.
            if index_values.len() < 2 {
                self.indexed_data.remove(&key)?;
            } else {
                // Remove the single ID from here.
                if let Some(pos) = index_values.iter().position(|id| *id == record.id) {
                    index_values.remove(pos);
                    // Replace the row with one that doesn't have the element.
                    self.indexed_data.insert(&key, encode(&index_values)?)?;
                }
            }
        }

        Ok(())
    }

    /// Delete records from the table and the index based on the given query.
    ///
    /// # Arguments
    ///
    /// * `query` - A reference to the query key.
    ///
    /// # Returns
    ///
    /// All the deleted [`Record`] instances.
    pub fn delete(&self, query: &I) -> DbResult<Vec<Record<T>>> {
        let records = self.select(query)?;

        let table = self.table.upgrade().unwrap();

        for record in &records {
            table.delete(record.id)?;
        }

        Ok(records)
    }

    /// Select records from the table based on the given query.
    ///
    /// # Arguments
    ///
    /// * `query` - A reference to the query key.
    ///
    /// # Returns
    ///
    /// All selected [`Record`] instances.
    pub fn select(&self, query: &I) -> DbResult<Vec<Record<T>>> {
        self.commit_log()?;

        let table = self.table.upgrade().unwrap();

        Ok(
            if let Ok(Some(bytes)) = self.indexed_data.get(encode(&query)?) {
                let ids: Vec<u64> = decode(&bytes)?;

                let mut results = vec![];
                for id in ids {
                    if let Some(record) = table.select(id)? {
                        results.push(record);
                    }
                }

                results
            } else {
                Vec::new()
            },
        )
    }

    /// Static select that doesn't obtain a read lock.
    fn tree_select(&self, tree: &Tree, query: &I) -> DbResult<Vec<Record<T>>> {
        self.commit_log()?;

        let table = self.table.upgrade().unwrap();

        Ok(
            if let Ok(Some(bytes)) = self.indexed_data.get(encode(&query)?) {
                let ids: Vec<u64> = decode(&bytes)?;

                let mut results = vec![];
                for id in ids {
                    if let Some(record) = table.tree_select(tree, id)? {
                        results.push(record);
                    }
                }

                results
            } else {
                Vec::new()
            },
        )
    }

    /// Update records in the table and the index based on the given query and new value.
    ///
    /// # Arguments
    ///
    /// * `query` - A reference to the query key.
    /// * `updater` - Closure to generate the new data based on the old data.
    ///
    /// # Returns
    ///
    /// All updated [`Record`] instances.
    pub fn update(&self, query: &I, updater: fn(T) -> T) -> DbResult<Vec<Record<T>>> {
        self.commit_log()?;

        let table = self.table.upgrade().unwrap();

        if let Ok(Some(bytes)) = self.indexed_data.get(encode(&query)?) {
            let ids: Vec<u64> = decode(&bytes)?;
            table.update(&ids, updater)
        } else {
            Ok(vec![])
        }
    }

    pub fn index_name(&self) -> String {
        std::str::from_utf8(&self.indexed_data.name())
            .unwrap()
            .to_string()
    }

    pub fn generate_key(&self, data: &T) -> DbResult<Vec<u8>> {
        encode(&(self.key_func)(&data))
    }
}

pub(crate) mod private {
    use super::*;

    /// Additional methods for index which are only for internal use.
    pub trait AnyIndexInternal<T: TableType> {
        fn tree_exists(&self, tree: &Tree, record: &Record<T>) -> DbResult<Vec<u64>>;
    }
}

impl<T, I> private::AnyIndexInternal<T> for Index<T, I>
where
    T: TableType,
    I: IndexType + 'static,
{
    fn tree_exists(&self, tree: &Tree, record: &Record<T>) -> DbResult<Vec<u64>> {
        let i = (self.key_func)(&record.data);

        Ok(self
            .tree_select(tree, &i)?
            .iter()
            .map(|record: &Record<T>| record.id)
            .collect())
    }
}

/// Type which [`Index`] can be casted to which doesn't require the `I` type parameter.
pub trait AnyIndex<T: TableType>: private::AnyIndexInternal<T> {
    /// Check if a record exists by the index key.
    ///
    /// # Arguments
    ///
    /// * `record` - The record to check for existence.
    fn exists(&self, record: &Record<T>) -> DbResult<Vec<u64>>;
    /// Select which allows any type.
    fn search(&self, value: Box<dyn Any>) -> DbResult<Vec<Record<T>>>;
    /// Alias for `index_name`.
    fn idx_name(&self) -> String;
    /// Generate a key and return encoded value.
    fn gen_key(&self, data: &T) -> DbResult<Vec<u8>>;
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

    fn exists(&self, record: &Record<T>) -> DbResult<Vec<u64>> {
        self.tree_exists(&self.table.upgrade().unwrap().root.read().unwrap(), record)
    }

    fn gen_key(&self, data: &T) -> DbResult<Vec<u8>> {
        self.generate_key(data)
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
            .update(&"initial_value_1".to_string(), |_| {
                "updated_value".to_string()
            })
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

        assert!(!index
            .exists(&record)
            .expect("Exists check failed")
            .is_empty());

        let record_not_exist = Record {
            id: 999,
            data: "non_existent_value".to_string(),
        };

        assert!(index
            .exists(&record_not_exist)
            .expect("Exists check failed")
            .is_empty());
    }
}
