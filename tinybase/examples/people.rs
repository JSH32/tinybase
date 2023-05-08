use serde::{Deserialize, Serialize};
use tinybase::{Constraint, QueryBuilder, QueryOperator, Table, TinyBase};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    last_name: String,
}

fn main() {
    let db = TinyBase::new(Some("./people"), true);
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
        .constraint(Constraint::check(|person| !person.name.contains(".")))
        .unwrap();

    init_example_data(&person_table);

    println!(
        "Found all the Smith's:\n{:#?}",
        lastname_idx.select(&"Smith".to_owned())
    );

    println!(
        "Replaced name of John OR lastname Jones with Kevin Spacey:\n{:#?}",
        QueryBuilder::new(&person_table)
            .by(&name_idx, "John".to_string())
            .by(&lastname_idx, "Jones".to_string())
            .update(
                QueryOperator::Or,
                Person {
                    name: "Kevin".to_string(),
                    last_name: "Spacey".to_string()
                }
            )
            .unwrap()
    );
}

fn init_example_data(person_table: &Table<Person>) {
    person_table
        .insert(Person {
            name: "John".to_string(),
            last_name: "Smith".to_string(),
        })
        .unwrap();

    person_table
        .insert(Person {
            name: "Bill".to_string(),
            last_name: "Smith".to_string(),
        })
        .unwrap();

    person_table
        .insert(Person {
            name: "Coraline".to_string(),
            last_name: "Jones".to_string(),
        })
        .unwrap();
}
