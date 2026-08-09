use crate::{AccountId, AmountMicros, Epoch, ShardId, WitnessedCheckpoint};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum L1Tx {
    GenesisFund {
        account: AccountId,
        amount: AmountMicros,
    },
    Stake {
        account: AccountId,
        amount: AmountMicros,
    },
    Unstake {
        account: AccountId,
        amount: AmountMicros,
    },
    AllocateShardAllowance {
        account: AccountId,
        shard_id: ShardId,
        amount: AmountMicros,
    },
    /// Apply a witnessed shard checkpoint: burns `exposure` from shard outstanding allowance.
    CheckpointFinalized {
        checkpoint: WitnessedCheckpoint,
        /// Account that escrowed the shard allowance (settlement counterparty).
        funding_account: AccountId,
    },
    /// Record a conflicting checkpoint attempt (slash stub).
    SlashConflict {
        shard_id: ShardId,
        epoch: Epoch,
        height: u64,
        offender: AccountId,
        amount: AmountMicros,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedL1Tx {
    pub tx: L1Tx,
    pub proposer: [u8; 32],
    pub signature: Vec<u8>,
}
