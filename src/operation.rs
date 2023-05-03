use std::collections::VecDeque;

use uuid::Uuid;

use crate::table::Record;

/// Modifying operation applied to table.
#[derive(PartialEq, Clone)]
pub enum Operation {
    Insert,
    Delete,
    Update,
}

/// Queue of operations.
pub(crate) struct OperationQueue<T> {
    /// The actual queue. The third element represents the old record.
    /// This will be present on updates.
    pub queue: VecDeque<(Operation, Uuid, Option<Record<T>>)>,
}
