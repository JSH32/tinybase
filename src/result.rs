use thiserror::Error;

#[derive(Error, Debug)]
pub enum TinyDbError {
    #[error("sled error")]
    Sled(#[from] sled::Error),
    #[error("serializer error")]
    Serializer(#[from] bincode::Error),
    #[error("record failed to match unique constraint")]
    Exists {
        constraint: String,
        record_id: uuid::Uuid,
    },
    #[error("a condition check was not met")]
    Condition,
}

pub type DbResult<T> = Result<T, TinyDbError>;
