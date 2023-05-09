use thiserror::Error;

#[derive(Error, Debug)]
pub enum TinyBaseError {
    #[error("sled error")]
    Sled(#[from] sled::Error),
    #[error("serializer error")]
    Serializer(#[from] bincode::Error),
    #[error("record failed to match unique constraint")]
    Exists { constraint: String, id: u64 },
    #[error("a condition check was not met")]
    Condition,
    #[error("query builder error")]
    QueryBuilder(String),
    #[error("batch operation violates constraints")]
    BatchOperationConstraints,
}

pub type DbResult<T> = Result<T, TinyBaseError>;
