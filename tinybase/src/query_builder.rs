use std::any::Any;

use crate::{
    index::{AnyIndex, Index, IndexType},
    result::DbResult,
    table::{Table, TableType},
    Record,
};

/// A single query condition.
pub enum QueryCondition<T>
where
    T: TableType + 'static,
{
    By(Box<dyn AnyIndex<T>>, Box<dyn Any>),
    And(Box<QueryCondition<T>>, Box<QueryCondition<T>>),
    Or(Box<QueryCondition<T>>, Box<QueryCondition<T>>),
}

/// For building and chaining query conditions.
pub struct ConditionBuilder<T: TableType + 'static>(QueryCondition<T>);

impl<T: TableType + 'static> ConditionBuilder<T> {
    /// Creates a new query condition using the specified index and value.
    ///
    /// # Arguments
    ///
    /// * `index` - The index to use for the query.
    /// * `value` - The value to search for in the index.
    pub fn by<I: IndexType + 'static>(index: &Index<T, I>, value: I) -> Self {
        Self(QueryCondition::By(Box::new(index.clone()), Box::new(value)))
    }

    /// Creates a new query condition representing the logical AND of two existing conditions.
    ///
    /// # Arguments
    ///
    /// * `left` - The left-hand side condition.
    /// * `right` - The right-hand side condition.
    pub fn and(left: Self, right: Self) -> Self {
        Self(QueryCondition::And(Box::new(left.0), Box::new(right.0)))
    }

    /// Creates a new query condition representing the logical OR of two existing conditions.
    ///
    /// # Arguments
    ///
    /// * `left` - The left-hand side condition.
    /// * `right` - The right-hand side condition.
    pub fn or(left: Self, right: Self) -> Self {
        Self(QueryCondition::Or(Box::new(left.0), Box::new(right.0)))
    }

    /// Builds the final query condition.
    ///
    /// # Returns
    ///
    /// A new [`QueryCondition`].
    pub fn build(self) -> QueryCondition<T> {
        self.0
    }
}

impl<T: TableType + 'static> Into<QueryCondition<T>> for ConditionBuilder<T> {
    fn into(self) -> QueryCondition<T> {
        self.build()
    }
}

/// Builder for building and executing queries.
pub struct QueryBuilder<T>
where
    T: TableType + 'static,
{
    table: Table<T>,
    condition: Option<QueryCondition<T>>,
}

impl<T> QueryBuilder<T>
where
    T: TableType,
{
    /// Creates a new query builder for the given table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table to build the query for.
    pub fn new(table: &Table<T>) -> Self {
        Self {
            table: table.clone(),
            condition: None,
        }
    }

    /// Adds a query condition to the query builder.
    /// This will overwrite the previous condition (if set).
    ///
    /// # Arguments
    ///
    /// * `condition` - The condition to add to the query builder.
    pub fn with_condition<C: Into<QueryCondition<T>>>(mut self, condition: C) -> Self {
        self.condition = Some(condition.into());
        self
    }

    /// Validates the query builder's state.
    fn check_valid(&self) -> DbResult<()> {
        match &self.condition {
            Some(_) => Ok(()),
            None => Err(crate::result::TinyBaseError::QueryBuilder(
                "No search condition provided".into(),
            )),
        }
    }

    /// Executes the query and returns the selected records.
    ///
    /// # Returns
    ///
    /// All selected [`Record`] instances.
    pub fn select(self) -> DbResult<Vec<Record<T>>> {
        self.check_valid()?;
        Self::select_recursive(self.condition.unwrap())
    }

    /// Updates the records in the table based on the query condition and new value.
    ///
    /// # Arguments
    ///
    /// * `value` - The new value to set for the updated records.
    ///
    /// # Returns
    ///
    /// All updated [`Record`] instances.
    pub fn update(self, value: T) -> DbResult<Vec<Record<T>>> {
        self.check_valid()?;
        let ids: Vec<u64> = Self::select_recursive(self.condition.unwrap())?
            .iter()
            .map(|record| record.id)
            .collect();

        self.table.update(&ids, value)
    }

    /// Deletes the records from the table based on the query condition.
    ///
    /// # Returns
    ///
    /// All deleted [`Record`] instances.
    pub fn delete(self) -> DbResult<Vec<Record<T>>> {
        self.check_valid()?;
        let selected = Self::select_recursive(self.condition.unwrap())?;

        let mut removed = vec![];

        for record in &selected {
            if let Some(record) = self.table.delete(record.id)? {
                removed.push(record);
            }
        }

        Ok(removed)
    }

    /// Recursively processes the query conditions and returns the selected records.
    fn select_recursive(condition: QueryCondition<T>) -> DbResult<Vec<Record<T>>> {
        match condition {
            QueryCondition::By(index, value) => index.search(value),
            QueryCondition::And(left, right) => {
                let left_records = Self::select_recursive(*left)?;
                let right_records = Self::select_recursive(*right)?;

                let mut intersection: Vec<Record<T>> = left_records.clone();
                intersection.retain(|record| {
                    right_records
                        .iter()
                        .any(|other_record| record.id == other_record.id)
                });

                Ok(intersection)
            }
            QueryCondition::Or(left, right) => {
                let mut records: Vec<Record<T>> =
                    Self::select_recursive(*left)?.into_iter().collect();
                records.extend(Self::select_recursive(*right)?.into_iter());

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

        let length = table.create_index("length", |value| value.len()).unwrap();

        // Insert string values into the table
        let value1 = table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        let result_1 = QueryBuilder::new(&table)
            .with_condition(ConditionBuilder::and(
                ConditionBuilder::by(&index, "value1".to_string()),
                ConditionBuilder::by(&index, "value2".to_string()),
            ))
            .select()
            .expect("Select failed");

        assert_eq!(result_1.len(), 0);

        let result_2 = QueryBuilder::new(&table)
            .with_condition(ConditionBuilder::and(
                ConditionBuilder::by(&index, "value1".to_string()),
                ConditionBuilder::by(&length, 6),
            ))
            .select()
            .expect("Select failed");

        assert_eq!(result_2.len(), 1);
        assert_eq!(result_2[0].id, value1);
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
            .with_condition(ConditionBuilder::or(
                ConditionBuilder::by(&index, "value1".to_string()),
                ConditionBuilder::by(&index, "value2".to_string()),
            ))
            .select()
            .expect("Select failed");

        assert_eq!(selected_records.len(), 2);
    }

    #[test]
    fn query_builder_select_combined() {
        let db = TinyBase::new(None, true);
        let table: Table<String> = db.open_table("test_table").unwrap();

        // Create an index for the table
        let name = table
            .create_index("name", |value| value.to_owned())
            .unwrap();

        let length = table.create_index("length", |value| value.len()).unwrap();

        // Insert string values into the table
        table.insert("value1".to_string()).unwrap();
        table.insert("value2".to_string()).unwrap();

        let selected_records = QueryBuilder::new(&table)
            .with_condition(ConditionBuilder::and(
                ConditionBuilder::or(
                    ConditionBuilder::by(&name, "value1".to_owned()),
                    ConditionBuilder::by(&name, "value2".to_owned()),
                ),
                ConditionBuilder::by(&length, 6),
            ))
            .select()
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
            .with_condition(ConditionBuilder::and(
                ConditionBuilder::by(&index, "value1".to_string()),
                ConditionBuilder::by(&length, 6),
            ))
            .update("updated_value".to_string())
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
            .with_condition(ConditionBuilder::by(&index, "value1".to_string()))
            .delete()
            .expect("Update failed");

        assert_eq!(deleted_records.len(), 1);

        // Check if record is really deleted
        let records = index.select(&"value1".to_string()).expect("Select failed");
        assert_eq!(records.len(), 0);
    }
}
