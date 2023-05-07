use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use sled::Config;

pub mod index;
pub use index::Index;

pub mod operation;
pub use operation::Operation;

pub mod query_builder;
pub use query_builder::{QueryBuilder, QueryOperator};

pub mod result;
pub use result::DbResult;

pub mod record;
pub use record::Record;

pub mod table;
pub use table::Table;
use table::TableInner;

pub mod constraint;
pub use constraint::Constraint;

mod subscriber;

/// A tiny structured database based on sled.
pub struct TinyBase {
    engine: sled::Db,
}

impl TinyBase {
    /// Creates a new `TinyBase` instance.
    ///
    /// # Arguments
    ///
    /// * `path` - An optional path to the database. If not provided, an
    ///   in-memory database will be used.
    /// * `temporary` - If true, the database will be removed when the
    ///   `TinyDb` instance is dropped.
    ///
    /// # Example
    ///
    /// ```
    /// use tinybase::TinyBase;
    ///
    /// let db = TinyBase::new(Some("path/to/db"), false);
    /// ```
    pub fn new(path: Option<&str>, temporary: bool) -> Self {
        Self {
            engine: if let Some(path) = path {
                Config::new().path(path).temporary(temporary)
            } else {
                Config::new().temporary(temporary)
            }
            .open()
            .unwrap(),
        }
    }

    /// Opens a table with the specified name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the table to open.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the value to be stored in the table. Must implement
    ///   `Serialize`, `Deserialize`, and `Debug`.
    ///
    /// # Errors
    ///
    /// Returns an error if the table could not be opened.
    ///
    /// # Example
    ///
    /// ```
    /// use tinydb::{TinyDb, Table};
    ///
    /// let db = TinyDb::new(Some("path/to/db"), false);
    /// let table: Table<String> = db.open_table("my_table").unwrap();
    /// ```
    pub fn open_table<T>(&self, name: &str) -> DbResult<Table<T>>
    where
        T: Serialize + DeserializeOwned + Clone + core::fmt::Debug,
    {
        Ok(Table(Arc::new(TableInner::new(&self.engine, name)?)))
    }
}
