use bincode::Options;

use crate::DbResult;

pub(crate) fn encode<S: ?Sized + serde::Serialize>(item: &S) -> DbResult<Vec<u8>> {
    Ok(bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .serialize(&item)?)
}

pub(crate) fn decode<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> DbResult<T> {
    Ok(bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize(bytes)?)
}
