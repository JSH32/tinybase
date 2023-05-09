<p align="center">
	<img width="550" src="https://raw.githubusercontent.com/JSH32/tinybase/master/.github/banner.png"><br>
	<img src="https://img.shields.io/badge/contributions-welcome-orange.svg">
	<img src="https://img.shields.io/badge/Made%20with-%E2%9D%A4-ff69b4?logo=love">
</p>

# TinyBase

TinyBase is an in-memory database built with Rust, based on the [sled](https://github.com/spacejam/sled) embedded key-value store. It supports indexing and constraints, allowing you to create efficient queries and ensure data consistency.

## Features
- In-memory storage for fast access.
- Built on top of sled for a reliable key-value store.
- Indexing support for efficient querying.
- Constraints to ensure data consistency.

## Installation & Setup

To use TinyBase in your Rust project, add the following line to your Cargo.toml file's `[dependencies]` section.:

```toml
tinybase = { version = "0.1.2", features = ["derive"] }
```

## Usage Example

Here's a simple example demonstrating how to use TinyBase with a `Person` struct.

- [Full example](https://github.com/JSH32/tinybase/blob/master/tinybase/examples/people_derive.rs)
- [Without derive](https://github.com/JSH32/tinybase/blob/master/tinybase/examples/people.rs)

```rust
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
        "Replaced name of John OR lastname Jones with Kevin Spacey:\n{:#?}",
        QueryBuilder::new(&people)
            .by(&people.name, "John".to_string())
            .by(&people.last_name, "Jones".to_string())
            .update(
                QueryOperator::Or,
                Person {
                    name: "Kevin".to_string(),
                    last_name: "Spacey".to_string(),
                    age: 63
                }
            )
            .unwrap()
    );
}
```

This example demonstrates how to create a new TinyBase instance, open a table (or create one if it doesn't exist), add indexes and constraints, and perform basic operations (insert/select).

You can view more examples in [examples](https://github.com/JSH32/tinybase/tree/master/examples)
