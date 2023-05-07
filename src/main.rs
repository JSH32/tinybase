use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sled::Config;

pub mod index;
pub use index::Index;

pub mod operation;
pub use operation::Operation;

pub mod query_builder;
pub use query_builder::{QueryBuilder, QueryOperator};

pub mod result;
pub use result::DbResult;

pub mod table;
pub use table::{Record, Table};

use crate::constraint::Constraint;

mod constraint;
mod subscriber;

/// A tiny structured database based on sled.
pub struct TinyDb {
    engine: sled::Db,
}

impl TinyDb {
    /// Creates a new `TinyDb` instance.
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
    /// use tinydb::TinyDb;
    ///
    /// let db = TinyDb::new(Some("path/to/db"), false);
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
        Table::new(&self.engine, name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    last_name: String,
}

fn main() {
    let db: TinyDb = TinyDb::new(Some("./people"), true);
    let person_table: Table<Person> = db.open_table("people").unwrap();

    let name_idx = person_table
        .create_index("name", |record| record.name.to_owned())
        .unwrap();

    let lastname_idx = person_table
        .create_index("last_name", |record| record.last_name.to_owned())
        .unwrap();

    person_table
        .constraint(Constraint::unique(&name_idx))
        .unwrap();

    person_table
        .constraint(Constraint::Check(|person| !person.name.contains(".")))
        .unwrap();

    init_example_data(&person_table);

    println!(
        "{:#?}",
        QueryBuilder::new(&person_table)
            .by(&name_idx, "Name".to_string())
            .by(&lastname_idx, "Else".to_string())
            .update(
                QueryOperator::Or,
                Person {
                    name: "Replacement".to_string(),
                    last_name: "LastName".to_string()
                }
            )
            .unwrap()
    );

    println!("{:#?}", name_idx.select(&"Name".to_string()).unwrap())
}

fn init_example_data(person_table: &Table<Person>) {
    person_table
        .insert(Person {
            name: "Name".to_string(),
            last_name: "LastName".to_string(),
        })
        .unwrap();

    person_table
        .insert(Person {
            name: "BrotherName".to_string(),
            last_name: "LastName".to_string(),
        })
        .unwrap();

    person_table
        .insert(Person {
            name: "Someone".to_string(),
            last_name: "Else".to_string(),
        })
        .unwrap();
}
