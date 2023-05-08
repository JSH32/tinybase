use std::sync::Arc;

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
use table::{TableInner, TableType};

pub mod constraint;
pub use constraint::Constraint;

mod encoding;
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
    ///   `TinyBase` instance is dropped.
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
    pub fn open_table<T: TableType>(&self, name: &str) -> DbResult<Table<T>> {
        Ok(Table(Arc::new(TableInner::new(&self.engine, name)?)))
    }
}
