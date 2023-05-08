use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};

use bincode::{deserialize, serialize};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sled::{Db, Tree};

use crate::constraint::{Constraint, ConstraintInner};
use crate::index::{Index, IndexInner, IndexType};
use crate::record::Record;
use crate::result::DbResult;
use crate::subscriber::{Event, Subscriber};

pub(crate) type SenderMap<T> = Arc<RwLock<HashMap<u64, Sender<T>>>>;

pub trait TableType: Serialize + DeserializeOwned + Clone + Debug {}
impl<T: Serialize + DeserializeOwned + Debug + Clone> TableType for T {}

pub struct Table<T: TableType + 'static>(pub(crate) Arc<TableInner<T>>);

impl<T: TableType> Clone for Table<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: TableType> Deref for Table<T> {
    type Target = Arc<TableInner<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Represents a table in the database.
///
/// [`Table`] provides methods for interacting with a table in the database, such as inserting records
/// and creating indexes.
///
/// # Type Parameters
///
/// * `T` - The type of the value to be stored in the table.
pub struct TableInner<T>
where
    T: TableType + 'static,
{
    pub(crate) engine: Db,
    root: Tree,
    name: String,
    senders: SenderMap<Event<T>>,
    constraints: RwLock<Vec<Constraint<T>>>,
}

impl<T> TableInner<T>
where
    T: TableType,
{
    /// Creates a new table with the given engine and name.
    ///
    /// This method is intended for internal use and should not be called directly. Instead, use the
    /// [`crate::TinyBase`]'s `open_table()` method.
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
            senders: Arc::new(RwLock::new(HashMap::new())),
            constraints: RwLock::new(Vec::new()),
        })
    }

    /// Inserts a new record into the table.
    ///
    /// This method generates a new ID for the record and serializes it, along with the value, before
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
    pub fn insert(&self, value: T) -> DbResult<u64> {
        let record = Record {
            id: self.engine.generate_id()?,
            data: value.clone(),
        };

        // Check for unique
        for constraint in self.constraints.read().unwrap().iter() {
            match &constraint.0 {
                ConstraintInner::Unique(index) => {
                    if index.record_exists(&record)? {
                        return Err(crate::result::TinyBaseError::Exists {
                            constraint: index.idx_name(),
                            id: record.id,
                        });
                    }
                }
                ConstraintInner::Check(condition) => {
                    if !condition(&value) {
                        return Err(crate::result::TinyBaseError::Condition);
                    }
                }
            };
        }

        self.root
            .insert(serialize(&record.id)?, serialize(&value)?)?;
        self.dispatch_event(Event::Insert(record.clone()));

        Ok(record.id)
    }

    pub fn select(&self, id: u64) -> DbResult<Option<Record<T>>> {
        if let Some(serialized) = self.root.get(serialize(&id)?)? {
            Ok(Some(Record {
                id,
                data: deserialize(&serialized)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete(&self, id: u64) -> DbResult<Option<Record<T>>> {
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

    pub fn update(&self, ids: &[u64], value: T) -> DbResult<Vec<Record<T>>> {
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
    pub fn create_index<I: IndexType>(
        &self,
        name: &str,
        key_func: impl Fn(&T) -> I + Send + Sync + 'static,
    ) -> DbResult<Index<T, I>> {
        let sender_id = self.engine.generate_id()?;
        let (tx, rx) = mpsc::channel();

        let subscriber = Subscriber::new(sender_id, rx, self.senders.clone());
        self.senders.write().unwrap().insert(sender_id, tx);

        Ok(Index(Arc::new(IndexInner::new(
            &format!("{}_idx_{}", self.name, name),
            &self.engine,
            &self.root,
            key_func,
            subscriber,
        )?)))
    }

    pub fn constraint(&self, constraint: Constraint<T>) -> DbResult<()> {
        let mut constraint_map = self.constraints.write().unwrap();

        match &constraint.0 {
            // Check if index has already been added if constraint is unique.
            ConstraintInner::Unique(index) => {
                let index_name = index.idx_name();

                if constraint_map
                    .iter()
                    .find(|idx| {
                        if let ConstraintInner::Unique(unique) = &idx.0 {
                            unique.idx_name() == index_name
                        } else {
                            false
                        }
                    })
                    .is_none()
                {
                    constraint_map.push(constraint);
                }
            }
            ConstraintInner::Check(_) => constraint_map.push(constraint),
        };

        Ok(())
    }

    fn dispatch_event(&self, event: Event<T>) {
        for sender in self.senders.read().unwrap().values() {
            sender.send(event.clone()).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TinyBase;

    #[test]
    fn table_insert_and_select() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Insert a string value into the table
        let id = table.insert("test_value".to_string()).unwrap();
        let record = table.select(id).unwrap().expect("Record not found");

        assert_eq!(record.id, id);
        assert_eq!(record.data, "test_value");
    }

    #[test]
    fn table_delete() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Insert a string value into the table
        let id = table.insert("test_value".to_string()).unwrap();

        // Delete the record with the given ID
        let deleted_record = table.delete(id).unwrap().expect("Record not found");

        assert_eq!(deleted_record.id, id);
        assert_eq!(deleted_record.data, "test_value");

        // Check if the record is really deleted
        assert!(table.select(id).unwrap().is_none());
    }

    #[test]
    fn table_update() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Insert a string value into the table
        let id1 = table.insert("value1".to_string()).unwrap();
        let id2 = table.insert("value2".to_string()).unwrap();

        // Update the records with new values
        let updated_records = table
            .update(&[id1, id2], "updated_value".to_string())
            .expect("Update failed");

        assert_eq!(updated_records.len(), 2);
        assert_eq!(updated_records[0].id, id1);
        assert_eq!(updated_records[0].data, "updated_value");

        assert_eq!(updated_records[1].id, id2);
        assert_eq!(updated_records[1].data, "updated_value");
    }
}
