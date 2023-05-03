use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use bincode::serialize;
use serde::{Deserialize, Serialize};
use sled::{Config, Db, Tree};
use uuid::Uuid;

use crate::index::Index;
use crate::operation::OperationQueue;

/// A single record in a table.
#[derive(Debug, Clone)]
pub struct Record<T> {
    /// Unique required ID of a record.
    pub id: Uuid,
    pub data: T,
}

pub struct Table<T>
where
    T: Serialize + Debug,
    for<'de> T: Deserialize<'de>,
{
    pub(crate) engine: Db,
    root: Tree,
    name: String,
    _table_type: PhantomData<T>,
}

impl<T> Table<T>
where
    T: Serialize + Debug,
    for<'de> T: Deserialize<'de>,
{
    pub(crate) fn new(engine: &Db, name: &str) -> Self {
        let root = engine.open_tree(name).unwrap();

        Self {
            engine: engine.clone(),
            root,
            name: name.to_owned(),
            _table_type: PhantomData::default(),
        }
    }

    // /// Push to all queues.
    // fn push_queues(&self, operation: Operation, uuid: Uuid) {
    //     let message = (
    //         operation.clone(),
    //         uuid,
    //         if operation == Operation::Update {
    //             Some({
    //                 let data = self.engine.get(&uuid).unwrap();
    //                 Record {
    //                     id: uuid,
    //                     data: data.clone(),
    //                 }
    //             })
    //         } else {
    //             None
    //         },
    //     );

    //     for queue in reader.queues.iter() {
    //         queue.write().unwrap().queue.push_back(message.clone())
    //     }
    // }

    pub fn insert(&mut self, value: T) {
        let uuid = Uuid::new_v4();
        self.root
            .insert(serialize(&uuid).unwrap(), serialize(&value).unwrap())
            .unwrap();

        // Inform indexes logs.
        // self.push_queues(Operation::Insert, uuid);
    }

    pub fn create_index<I: AsRef<[u8]>>(
        &mut self,
        name: &str,
        key_func: impl Fn(&T) -> I + Send + Sync + 'static,
    ) -> Index<T, I> {
        // let operation_queue = Arc::new(RwLock::new(OperationQueue {
        //     queue: VecDeque::new(),
        // }));

        // self.inner
        //     .write()
        //     .unwrap()
        //     .queues
        //     .push(operation_queue.clone());

        Index::new(
            &format!("{}_idx_{}", self.name, name),
            &self.engine,
            &self.root,
            key_func,
        )
    }
}
