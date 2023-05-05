use thiserror::Error;

#[derive(Error, Debug)]
pub enum TinyDbError {
    #[error("sled error")]
    Sled(#[from] sled::Error),
    #[error("serializer error")]
    Serializer(#[from] bincode::Error),
}

pub type DbResult<T> = Result<T, TinyDbError>;
