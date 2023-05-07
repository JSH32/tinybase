use std::{any::Any, fmt::Debug};

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::{
    index::{AnyIndex, Index},
    result::DbResult,
    table::{Record, Table},
};

pub enum QueryOperator {
    And,
    Or,
}

pub struct QueryBuilder<'qb, T>
where
    T: Serialize + DeserializeOwned + Debug + Clone + 'static,
{
    table: &'qb Table<T>,
    search_conditions: Vec<(Box<&'qb dyn AnyIndex<T>>, Box<dyn Any>)>,
}

impl<'qb, T> QueryBuilder<'qb, T>
where
    T: Serialize + DeserializeOwned + Debug + Clone,
{
    pub fn new(table: &'qb Table<T>) -> Self {
        Self {
            table,
            search_conditions: Vec::new(),
        }
    }

    pub fn by<I: AsRef<[u8]>>(mut self, index: &'qb Index<T, I>, value: I) -> Self
    where
        I: Ord + 'static,
    {
        self.search_conditions
            .push((Box::new(index), Box::new(value)));

        self
    }

    pub fn select(self, op: QueryOperator) -> DbResult<Vec<Record<T>>> {
        Self::static_select(self.search_conditions, op)
    }

    pub fn update(self, op: QueryOperator, value: T) -> DbResult<Vec<Record<T>>> {
        let ids: Vec<Uuid> = Self::static_select(self.search_conditions, op)?
            .iter()
            .map(|record| record.id)
            .collect();

        self.table.update(&ids, value)
    }

    /// Actual functionality for select. Used to prevent unnecessary move.
    fn static_select(
        search_conditions: Vec<(Box<&'qb dyn AnyIndex<T>>, Box<dyn Any>)>,
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
