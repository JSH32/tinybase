// use std::any::Any;

// use crate::{
//     index::Index,
//     table::{Record, Table},
// };

// trait AnyIndex<T> {
//     fn search(&self, value: Box<dyn Any>) -> Vec<Record<T>>;
// }

// impl<T: Clone, I: Ord + 'static> AnyIndex<T> for Index<T, I> {
//     fn search(&self, value: Box<dyn Any>) -> Vec<Record<T>> {
//         let i = *value.downcast::<I>().unwrap();
//         self.query(&i)
//     }
// }

// pub enum QueryOperator {
//     And,
//     Or,
// }

// pub struct QueryBuilder<'qb, T: Clone> {
//     table: &'qb Table<T>,
//     search_conditions: Vec<(Box<&'qb dyn AnyIndex<T>>, Box<dyn Any>)>,
// }

// impl<'qb, T: Clone> QueryBuilder<'qb, T> {
//     pub fn new(table: &'qb Table<T>) -> Self {
//         Self {
//             table,
//             search_conditions: Vec::new(),
//         }
//     }

//     pub fn by<I>(mut self, index: &'qb Index<T, I>, value: I) -> Self
//     where
//         I: Ord + 'static,
//     {
//         self.search_conditions
//             .push((Box::new(index), Box::new(value)));

//         self
//     }

//     pub fn select(self, op: QueryOperator) -> Vec<Record<T>> {
//         let result_list: Vec<Vec<Record<T>>> = self
//             .search_conditions
//             .into_iter()
//             .map(|(index, value)| index.search(value))
//             .collect();

//         match op {
//             QueryOperator::And => {
//                 let mut intersection: Vec<Record<T>> = result_list[0].clone();
//                 for other_result in result_list.into_iter().skip(1) {
//                     intersection.retain(|record| {
//                         other_result
//                             .iter()
//                             .any(|other_record| record.id == other_record.id)
//                     });
//                 }
//                 intersection
//             }
//             QueryOperator::Or => {
//                 let mut records: Vec<Record<T>> = result_list.into_iter().flatten().collect();

//                 let mut seen = Vec::new();
//                 records.retain(|item| {
//                     if seen.contains(&item.id) {
//                         false
//                     } else {
//                         seen.push(item.id);
//                         true
//                     }
//                 });

//                 records
//             }
//         }
//     }
// }
