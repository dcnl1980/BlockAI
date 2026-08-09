use crate::{AmountMicros, Epoch, ShardId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointHeader {
    pub shard_id: ShardId,
    pub epoch: Epoch,
    pub root: [u8; 32],
    pub height: u64,
    pub tx_count: u64,
    pub exposure: AmountMicros,
    pub sealed_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedCheckpoint {
    pub header: CheckpointHeader,
    pub shard_signer_pubkey: [u8; 32],
    pub shard_signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessSig {
    pub witness_pubkey: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessedCheckpoint {
    pub checkpoint: SignedCheckpoint,
    pub witness_sigs: Vec<WitnessSig>,
}
