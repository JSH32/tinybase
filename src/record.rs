/// A single record in a table.
#[derive(Debug, Clone)]
pub struct Record<T> {
    /// Unique ID of a record.
    pub id: u64,
    pub data: T,
}
