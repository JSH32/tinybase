use serde::{Deserialize, Serialize};
use tinybase::TinyBase;
use tinybase_derive::TinyBaseTable;

#[derive(TinyBaseTable, Serialize, Deserialize, Debug, Clone)]
struct TestTable {
    #[index]
    pub name: String,
    pub age: u8,
}

fn main() {
    let db = TinyBase::new(Some("./people"), true);
    let test = TestTable::init(&db, "people").unwrap();

    let results = test.by_name("Hello".to_owned()).unwrap();
    println!("{:#?}", results);
}
