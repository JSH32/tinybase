use uuid::Uuid;

/// A single record in a table.
#[derive(Debug, Clone)]
pub struct Record<T> {
    /// Unique required ID of a record.
    pub id: Uuid,
    pub data: T,
}
