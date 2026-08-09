use crate::{AccountId, AgentId, AmountMicros, Epoch, ShardId, WitnessedCheckpoint};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeStatus {
    Open,
    ResolvedForPlaintiff,
    ResolvedForDefendant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dispute {
    pub id: [u8; 32],
    pub plaintiff: AccountId,
    pub defendant: AccountId,
    pub bond: AmountMicros,
    pub status: DisputeStatus,
    pub evidence_hash: [u8; 32],
}

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
    RegisterAgent {
        account: AccountId,
        agent_id: AgentId,
        metadata_hash: [u8; 32],
    },
    SuspendAgent {
        agent_id: AgentId,
    },
    UpdateReputation {
        agent_id: AgentId,
        delta: i64,
        reason_hash: [u8; 32],
    },
    DeployContract {
        deployer: AccountId,
        code_hash: [u8; 32],
        /// WASM or WAT bytes (stored off hot path in ContractStore; L1 records hash + deployer).
        code: Vec<u8>,
    },
    CallContract {
        caller: AccountId,
        code_hash: [u8; 32],
        export: String,
        args: (i32, i32),
        fuel: u64,
    },
    OpenDispute {
        id: [u8; 32],
        plaintiff: AccountId,
        defendant: AccountId,
        bond: AmountMicros,
        evidence_hash: [u8; 32],
    },
    ResolveDispute {
        id: [u8; 32],
        for_plaintiff: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedL1Tx {
    pub tx: L1Tx,
    pub proposer: [u8; 32],
    pub signature: Vec<u8>,
}
