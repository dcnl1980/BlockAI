use crate::{AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Sequence, ShardId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendCapability {
    pub capability_id: CapabilityId,
    pub account_id: AccountId,
    pub agent_id: AgentId,
    pub shard_id: ShardId,
    pub epoch: Epoch,
    pub currency: String,
    pub maximum_total: AmountMicros,
    pub maximum_per_call: AmountMicros,
    pub service_scope: Vec<String>,
    pub policy_hash: [u8; 32],
    pub sequence_start: Sequence,
    pub sequence_end: Sequence,
    pub valid_from_unix_ms: u64,
    pub valid_until_unix_ms: u64,
    pub region: String,
    pub issuer_pubkey: [u8; 32],
    pub issuer_signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpochState {
    Active,
    Fenced,
    Expired,
}
