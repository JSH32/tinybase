use serde::{Deserialize, Serialize};
use tinybase::{ConditionBuilder, QueryBuilder, Table, TinyBase};
use tinybase_derive::Repository;

#[derive(Repository, Serialize, Deserialize, Debug, Clone)]
struct Person {
    #[index]
    #[unique]
    pub name: String,
    #[index]
    pub last_name: String,
    pub age: u8,
}

fn main() {
    let db = TinyBase::new(Some("./people"), true);
    let people = Person::init(&db, "people").unwrap();

    init_example_data(&people);

    println!(
        "Found all the Smith's:\n{:#?}",
        people.find_by_last_name("Smith".to_owned()).unwrap()
    );

    println!(
        "Replaced lastnames with Brown:\n{:#?}",
        QueryBuilder::new(&people)
            .with_condition(ConditionBuilder::or(
                ConditionBuilder::by(&people.name, "John".to_string()),
                ConditionBuilder::by(&people.last_name, "Jones".to_string()),
            ))
            .update(|record| Person {
                last_name: "Brown".to_owned(),
                ..record
            })
            .unwrap()
    );
}

fn init_example_data(person_table: &Table<Person>) {
    person_table
        .insert(Person {
            name: "John".to_string(),
            last_name: "Smith".to_string(),
            age: 18,
        })
        .unwrap();

    person_table
        .insert(Person {
            name: "Bill".to_string(),
            last_name: "Smith".to_string(),
            age: 40,
        })
        .unwrap();

    person_table
        .insert(Person {
            name: "Coraline".to_string(),
            last_name: "Jones".to_string(),
            age: 16,
        })
        .unwrap();
}
