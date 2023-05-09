use std::sync::Arc;

use sled::Config;

pub mod index;
pub use index::Index;

pub mod query_builder;
pub use query_builder::{ConditionBuilder, QueryBuilder};

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
    /// Create a new instance of `TinyBase`.
    ///
    /// # Arguments
    ///
    /// * `path` - An optional path to the database file. If `None`, an in-memory database is created.
    /// * `temporary` - If `true`, the database file will be deleted on close.
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

    /// Open a table for a given type.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the table.
    ///
    /// # Returns
    ///
    /// A `Table` instance for the given type.
    pub fn open_table<T: TableType>(&self, name: &str) -> DbResult<Table<T>> {
        Ok(Table(Arc::new(TableInner::new(&self.engine, name)?)))
    }
}
