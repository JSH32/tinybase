use serde::{Deserialize, Serialize};
use sled::Config;

use crate::table::Table;

mod index;
mod operation;
mod query_builder;
mod table;

pub struct TinyDb {
    engine: sled::Db,
}

impl TinyDb {
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

    pub fn open_table<T>(&self, name: &str) -> Table<T>
    where
        T: Serialize + core::fmt::Debug,
        for<'de> T: Deserialize<'de>,
    {
        Table::new(&self.engine, name)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Data {
    name: String,
    last_name: String,
}

fn main() {
    let db: TinyDb = TinyDb::new(Some("./people"), true);
    let mut person_table: Table<Data> = db.open_table("people");

    let mut name_idx = person_table.create_index("name", |record| record.name.to_owned());
    let mut lastname_idx =
        person_table.create_index("last_name", |record| record.last_name.to_owned());

    person_table.insert(Data {
        name: "Name".to_string(),
        last_name: "LastName".to_string(),
    });

    person_table.insert(Data {
        name: "BrotherName".to_string(),
        last_name: "LastName".to_string(),
    });

    person_table.insert(Data {
        name: "Someone".to_string(),
        last_name: "Else".to_string(),
    });

    println!("{:#?}", name_idx.query(&"Someone".to_owned()))
    // println!(
    //     "{:#?}",
    //     QueryBuilder::new(&person_table)
    //         .by(&lastname_idx, "LastName".to_owned())
    //         .by(&name_idx, "Name".to_owned())
    //         .select(QueryOperator::And)
    // );
}
