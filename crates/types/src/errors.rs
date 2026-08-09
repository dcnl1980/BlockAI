use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TypesError {
    #[error("empty shard id")]
    EmptyShardId,
    #[error("unsupported currency: {0}")]
    UnsupportedCurrency(String),
    #[error("cbor encode failed")]
    CborEncode,
    #[error("cbor decode failed")]
    CborDecode,
}
