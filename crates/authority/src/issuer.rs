use blockai_crypto::{sign_capability, Keypair};
use blockai_types::{
    AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Sequence, ShardId, SpendCapability,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("unknown account")]
    UnknownAccount,
    #[error("insufficient reserve")]
    InsufficientReserve,
    #[error("insufficient shard allowance")]
    InsufficientShardAllowance,
    #[error("unknown shard allocation")]
    UnknownShardAllocation,
}

#[derive(Clone, Debug)]
pub struct AccountFloat {
    pub total: AmountMicros,
    pub reserve: AmountMicros,
    pub shard_allowances: HashMap<ShardId, AmountMicros>,
}

pub struct Authority {
    issuer: Keypair,
    accounts: HashMap<AccountId, AccountFloat>,
    outstanding: HashMap<CapabilityId, AmountMicros>,
    fenced_epochs: HashMap<ShardId, Epoch>,
    next_cap_counter: u64,
}

#[derive(Clone, Debug)]
pub struct IssueRequest {
    pub account_id: AccountId,
    pub agent_id: AgentId,
    pub shard_id: ShardId,
    pub epoch: Epoch,
    pub maximum_total: AmountMicros,
    pub maximum_per_call: AmountMicros,
    pub service_scope: Vec<String>,
    pub policy_hash: [u8; 32],
    pub sequence_start: Sequence,
    pub sequence_end: Sequence,
    pub ttl_ms: u64,
    pub region: String,
    pub now_unix_ms: u64,
}

impl Authority {
    pub fn new_for_tests() -> Self {
        Self {
            issuer: Keypair::generate(),
            accounts: HashMap::new(),
            outstanding: HashMap::new(),
            fenced_epochs: HashMap::new(),
            next_cap_counter: 1,
        }
    }

    pub fn issuer_verifying_key_bytes(&self) -> [u8; 32] {
        self.issuer.verifying_key_bytes()
    }

    pub fn issuer_signing_bytes_for_tests(&self) -> [u8; 32] {
        self.issuer.signing_key().to_bytes()
    }

    pub fn fund(&mut self, account: AccountId, total: AmountMicros) -> Result<(), AuthorityError> {
        self.accounts.insert(
            account,
            AccountFloat {
                total,
                reserve: total,
                shard_allowances: HashMap::new(),
            },
        );
        Ok(())
    }

    pub fn allocate(
        &mut self,
        account: AccountId,
        shard: ShardId,
        amount: AmountMicros,
    ) -> Result<(), AuthorityError> {
        let float = self
            .accounts
            .get_mut(&account)
            .ok_or(AuthorityError::UnknownAccount)?;
        if float.reserve.0 < amount.0 {
            return Err(AuthorityError::InsufficientReserve);
        }
        float.reserve = AmountMicros(float.reserve.0 - amount.0);
        let entry = float
            .shard_allowances
            .entry(shard)
            .or_insert(AmountMicros(0));
        entry.0 += amount.0;
        Ok(())
    }

    pub fn issue_capability(
        &mut self,
        req: IssueRequest,
    ) -> Result<SpendCapability, AuthorityError> {
        let float = self
            .accounts
            .get_mut(&req.account_id)
            .ok_or(AuthorityError::UnknownAccount)?;
        let allowance = float
            .shard_allowances
            .get_mut(&req.shard_id)
            .ok_or(AuthorityError::UnknownShardAllocation)?;
        if allowance.0 < req.maximum_total.0 {
            return Err(AuthorityError::InsufficientShardAllowance);
        }
        allowance.0 -= req.maximum_total.0;

        let mut hasher = blake3::Hasher::new();
        hasher.update(&req.account_id.0);
        hasher.update(&req.agent_id.0);
        hasher.update(req.shard_id.as_str().as_bytes());
        hasher.update(&req.epoch.0.to_le_bytes());
        hasher.update(&self.next_cap_counter.to_le_bytes());
        self.next_cap_counter += 1;
        let capability_id = CapabilityId(*hasher.finalize().as_bytes());

        let mut cap = SpendCapability {
            capability_id,
            account_id: req.account_id,
            agent_id: req.agent_id,
            shard_id: req.shard_id,
            epoch: req.epoch,
            currency: "EURC".into(),
            maximum_total: req.maximum_total,
            maximum_per_call: req.maximum_per_call,
            service_scope: req.service_scope,
            policy_hash: req.policy_hash,
            sequence_start: req.sequence_start,
            sequence_end: req.sequence_end,
            valid_from_unix_ms: req.now_unix_ms,
            valid_until_unix_ms: req.now_unix_ms.saturating_add(req.ttl_ms),
            region: req.region,
            issuer_pubkey: self.issuer.verifying_key_bytes(),
            issuer_signature: vec![],
        };
        cap.issuer_signature = sign_capability(&self.issuer, &cap);
        self.outstanding.insert(capability_id, req.maximum_total);
        Ok(cap)
    }

    pub fn fence_epoch(&mut self, shard: ShardId, epoch: Epoch) {
        self.fenced_epochs.insert(shard, epoch);
    }

    pub fn is_epoch_fenced(&self, shard: &ShardId, epoch: Epoch) -> bool {
        self.fenced_epochs
            .get(shard)
            .map(|e| e.0 >= epoch.0)
            .unwrap_or(false)
    }
}
