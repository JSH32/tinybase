use std::any::Any;

use crate::{
    index::{AnyIndex, Index, IndexType},
    result::DbResult,
    table::{Table, TableType},
    Record,
};

pub enum QueryOperator {
    And,
    Or,
}

pub struct QueryBuilder<T>
where
    T: TableType + 'static,
{
    table: Table<T>,
    search_conditions: Vec<(Box<dyn AnyIndex<T>>, Box<dyn Any>)>,
}

impl<T> QueryBuilder<T>
where
    T: TableType,
{
    pub fn new(table: &Table<T>) -> Self {
        Self {
            table: table.clone(),
            search_conditions: Vec::new(),
        }
    }

    pub fn by<I: IndexType + 'static>(mut self, index: &Index<T, I>, value: I) -> Self {
        self.search_conditions
            .push((Box::new(index.clone()), Box::new(value)));

        self
    }

    pub fn select(self, op: QueryOperator) -> DbResult<Vec<Record<T>>> {
        Self::static_select(self.search_conditions, op)
    }

    pub fn update(self, op: QueryOperator, value: T) -> DbResult<Vec<Record<T>>> {
        let ids: Vec<u64> = Self::static_select(self.search_conditions, op)?
            .iter()
            .map(|record| record.id)
            .collect();

        self.table.update(&ids, value)
    }

    pub fn delete(self, op: QueryOperator) -> DbResult<Vec<Record<T>>> {
        let selected = Self::static_select(self.search_conditions, op)?;

        let mut removed = vec![];

        for record in &selected {
            if let Some(record) = self.table.delete(record.id)? {
                removed.push(record);
            }
        }

        Ok(selected)
    }

    /// Actual functionality for select. Used to prevent unnecessary move.
    fn static_select(
        search_conditions: Vec<(Box<dyn AnyIndex<T>>, Box<dyn Any>)>,
        op: QueryOperator,
    ) -> DbResult<Vec<Record<T>>> {
        let result_list = search_conditions
            .into_iter()
            .map(|(index, value)| index.search(value))
            .collect::<DbResult<Vec<Vec<Record<T>>>>>()?;

        match op {
            QueryOperator::And => {
                let mut intersection: Vec<Record<T>> = result_list[0].clone();
                for other_result in result_list.into_iter().skip(1) {
                    intersection.retain(|record| {
                        other_result
                            .iter()
                            .any(|other_record| record.id == other_record.id)
                    });
                }
                Ok(intersection)
            }
            QueryOperator::Or => {
                let mut records: Vec<Record<T>> = result_list.into_iter().flatten().collect();

                let mut seen = Vec::new();
                records.retain(|item| {
                    if seen.contains(&item.id) {
                        false
                    } else {
                        seen.push(item.id);
                        true
                    }
                });

                Ok(records)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TinyBase;

    #[test]
    fn query_builder_select_and() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the table
        let index = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        // Insert string values into the table
        table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        let selected_records = QueryBuilder::new(&table)
            .by(&index, "value1".to_string())
            .by(&index, "value2".to_string())
            .select(QueryOperator::And)
            .expect("Select failed");

        assert_eq!(selected_records.len(), 0);
    }

    #[test]
    fn query_builder_select_or() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the table
        let index = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        // Insert string values into the table
        table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        let selected_records = QueryBuilder::new(&table)
            .by(&index, "value1".to_string())
            .by(&index, "value2".to_string())
            .select(QueryOperator::Or)
            .expect("Select failed");

        assert_eq!(selected_records.len(), 2);
    }

    #[test]
    fn query_builder_update() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the table
        let index = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        let length = table.create_index("length", |value| value.len()).unwrap();

        // Insert string values into the table
        table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        let updated_records = QueryBuilder::new(&table)
            .by(&index, "value1".to_string())
            .by(&length, 6)
            .update(QueryOperator::And, "updated_value".to_string())
            .expect("Update failed");

        assert_eq!(updated_records.len(), 1);
        assert_eq!(updated_records[0].data, "updated_value");
    }

    #[test]
    fn query_builder_delete() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Insert string values into the table
        table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        // Create an index for the table
        let index = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        let deleted_records = QueryBuilder::new(&table)
            .by(&index, "value1".to_string())
            .delete(QueryOperator::And)
            .expect("Delete failed");

        assert_eq!(deleted_records.len(), 1);

        // Check if record is really deleted
        let records = index.select(&"value1".to_string()).expect("Select failed");
        assert_eq!(records.len(), 0);
    }
}
