use blockai_types::{
    Account, AccountId, AgentId, AmountMicros, Dispute, DisputeStatus, L1Tx, WitnessedCheckpoint,
};
use blockai_wasm::{code_hash, WasmRuntime};
use blockai_witness::verify_witnessed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecuteError {
    #[error("unknown account")]
    UnknownAccount,
    #[error("insufficient available balance")]
    InsufficientAvailable,
    #[error("insufficient stake")]
    InsufficientStake,
    #[error("insufficient shard allowance")]
    InsufficientShardAllowance,
    #[error("conservation broken")]
    ConservationBroken,
    #[error("invalid checkpoint witnesses")]
    InvalidCheckpoint,
    #[error("checkpoint already finalized")]
    CheckpointAlreadyFinalized,
    #[error("conflicting checkpoint")]
    ConflictingCheckpoint,
    #[error("account suspended")]
    Suspended,
    #[error("agent already registered")]
    AgentExists,
    #[error("unknown agent")]
    UnknownAgent,
    #[error("unknown contract")]
    UnknownContract,
    #[error("code hash mismatch")]
    CodeHashMismatch,
    #[error("wasm error: {0}")]
    Wasm(String),
    #[error("unknown dispute")]
    UnknownDispute,
    #[error("dispute not open")]
    DisputeNotOpen,
    #[error("dispute exists")]
    DisputeExists,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRecord {
    pub account: AccountId,
    pub agent_id: AgentId,
    pub metadata_hash: [u8; 32],
    pub suspended: bool,
    pub reputation: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GlobalState {
    pub accounts: HashMap<AccountId, Account>,
    pub total_supply: AmountMicros,
    pub shard_outstanding: HashMap<(String, AccountId), AmountMicros>,
    pub finalized: HashMap<(String, u64, u64), [u8; 32]>,
    pub agents: HashMap<AgentId, AgentRecord>,
    pub contracts: HashMap<[u8; 32], Vec<u8>>,
    pub contract_deployers: HashMap<[u8; 32], AccountId>,
    pub disputes: HashMap<[u8; 32], Dispute>,
    pub last_call_result: Option<i32>,
    pub min_witnesses: usize,
    pub default_fuel: u64,
    pub events: Vec<String>,
}

impl GlobalState {
    pub fn new(min_witnesses: usize) -> Self {
        Self {
            min_witnesses,
            default_fuel: 10_000,
            ..Default::default()
        }
    }

    pub fn locked_sum(&self) -> AmountMicros {
        let account_locked: u128 = self
            .accounts
            .values()
            .map(|a| a.balance_locked.0 + a.stake.0)
            .sum();
        let dispute_bonds: u128 = self
            .disputes
            .values()
            .filter(|d| d.status == DisputeStatus::Open)
            .map(|d| d.bond.0)
            .sum();
        AmountMicros(account_locked + dispute_bonds)
    }

    pub fn available_sum(&self) -> AmountMicros {
        AmountMicros(self.accounts.values().map(|a| a.balance_available.0).sum())
    }

    pub fn shard_outstanding_sum(&self) -> AmountMicros {
        AmountMicros(self.shard_outstanding.values().map(|a| a.0).sum())
    }

    pub fn check_conservation(&self) -> Result<(), ExecuteError> {
        let lhs = self.available_sum().0
            + self.shard_outstanding_sum().0
            + self.locked_sum().0;
        if lhs != self.total_supply.0 {
            return Err(ExecuteError::ConservationBroken);
        }
        Ok(())
    }

    pub fn apply(&mut self, tx: &L1Tx) -> Result<(), ExecuteError> {
        match tx {
            L1Tx::GenesisFund { account, amount } => {
                let acc = self
                    .accounts
                    .entry(*account)
                    .or_insert_with(|| Account::new_human(*account, AmountMicros(0)));
                acc.balance_available.0 += amount.0;
                self.total_supply.0 += amount.0;
                self.events
                    .push(format!("GenesisFund {:?}/{}", account.0[0], amount.0));
            }
            L1Tx::Stake { account, amount } => {
                let acc = self
                    .accounts
                    .get_mut(account)
                    .ok_or(ExecuteError::UnknownAccount)?;
                if acc.suspended {
                    return Err(ExecuteError::Suspended);
                }
                if acc.balance_available.0 < amount.0 {
                    return Err(ExecuteError::InsufficientAvailable);
                }
                acc.balance_available.0 -= amount.0;
                acc.stake.0 += amount.0;
                self.events.push(format!("Stake {}", amount.0));
            }
            L1Tx::Unstake { account, amount } => {
                let acc = self
                    .accounts
                    .get_mut(account)
                    .ok_or(ExecuteError::UnknownAccount)?;
                if acc.stake.0 < amount.0 {
                    return Err(ExecuteError::InsufficientStake);
                }
                acc.stake.0 -= amount.0;
                acc.balance_available.0 += amount.0;
                self.events.push(format!("Unstake {}", amount.0));
            }
            L1Tx::AllocateShardAllowance {
                account,
                shard_id,
                amount,
            } => {
                let acc = self
                    .accounts
                    .get_mut(account)
                    .ok_or(ExecuteError::UnknownAccount)?;
                if acc.balance_available.0 < amount.0 {
                    return Err(ExecuteError::InsufficientAvailable);
                }
                acc.balance_available.0 -= amount.0;
                let key = (shard_id.as_str().to_string(), *account);
                let entry = self
                    .shard_outstanding
                    .entry(key)
                    .or_insert(AmountMicros(0));
                entry.0 += amount.0;
                self.events.push(format!(
                    "AllocateShardAllowance {} -> {}",
                    shard_id.as_str(),
                    amount.0
                ));
            }
            L1Tx::CheckpointFinalized {
                checkpoint,
                funding_account,
            } => {
                self.apply_checkpoint(checkpoint, *funding_account)?;
            }
            L1Tx::SlashConflict {
                shard_id,
                epoch,
                height,
                offender,
                amount,
            } => {
                let acc = self
                    .accounts
                    .get_mut(offender)
                    .ok_or(ExecuteError::UnknownAccount)?;
                let slash = amount.0.min(acc.stake.0);
                acc.stake.0 -= slash;
                // burned from supply (penalty)
                self.total_supply.0 = self.total_supply.0.saturating_sub(slash);
                self.events.push(format!(
                    "SlashConflict {} epoch={} height={} amount={}",
                    shard_id.as_str(),
                    epoch.0,
                    height,
                    slash
                ));
            }
            L1Tx::RegisterAgent {
                account,
                agent_id,
                metadata_hash,
            } => {
                if self.agents.contains_key(agent_id) {
                    return Err(ExecuteError::AgentExists);
                }
                self.accounts
                    .entry(*account)
                    .or_insert_with(|| Account::new_agent(*account, *agent_id, AmountMicros(0)));
                if let Some(acc) = self.accounts.get_mut(account) {
                    acc.agent_id = Some(*agent_id);
                }
                self.agents.insert(
                    *agent_id,
                    AgentRecord {
                        account: *account,
                        agent_id: *agent_id,
                        metadata_hash: *metadata_hash,
                        suspended: false,
                        reputation: 0,
                    },
                );
                self.events
                    .push(format!("RegisterAgent {}", agent_id.0[0]));
            }
            L1Tx::SuspendAgent { agent_id } => {
                let rec = self
                    .agents
                    .get_mut(agent_id)
                    .ok_or(ExecuteError::UnknownAgent)?;
                rec.suspended = true;
                if let Some(acc) = self.accounts.get_mut(&rec.account) {
                    acc.suspended = true;
                }
                self.events.push(format!("SuspendAgent {}", agent_id.0[0]));
            }
            L1Tx::UpdateReputation {
                agent_id,
                delta,
                reason_hash,
            } => {
                let rec = self
                    .agents
                    .get_mut(agent_id)
                    .ok_or(ExecuteError::UnknownAgent)?;
                rec.reputation = rec.reputation.saturating_add(*delta);
                if let Some(acc) = self.accounts.get_mut(&rec.account) {
                    acc.reputation = rec.reputation;
                }
                self.events.push(format!(
                    "UpdateReputation {} delta={} reason={}",
                    agent_id.0[0],
                    delta,
                    reason_hash[0]
                ));
            }
            L1Tx::DeployContract {
                deployer,
                code_hash: expected,
                code,
            } => {
                if !self.accounts.contains_key(deployer) {
                    return Err(ExecuteError::UnknownAccount);
                }
                let runtime = WasmRuntime::new();
                let compiled = runtime
                    .compile(code)
                    .map_err(|e| ExecuteError::Wasm(e.to_string()))?;
                let hash = code_hash(&compiled);
                if hash != *expected {
                    // allow expected to be hash of original input if WAT
                    let input_hash = code_hash(code);
                    if *expected != input_hash && *expected != hash {
                        return Err(ExecuteError::CodeHashMismatch);
                    }
                }
                let store_hash = if self.contracts.contains_key(expected) {
                    *expected
                } else {
                    hash
                };
                self.contracts.insert(store_hash, compiled.clone());
                // also index by caller-provided hash for convenience
                self.contracts.insert(*expected, compiled);
                self.contract_deployers.insert(*expected, *deployer);
                self.contract_deployers.insert(store_hash, *deployer);
                self.events
                    .push(format!("DeployContract {}", expected[0]));
            }
            L1Tx::CallContract {
                caller,
                code_hash,
                export,
                args,
                fuel,
            } => {
                if !self.accounts.contains_key(caller) {
                    return Err(ExecuteError::UnknownAccount);
                }
                let code = self
                    .contracts
                    .get(code_hash)
                    .ok_or(ExecuteError::UnknownContract)?
                    .clone();
                let runtime = WasmRuntime::new();
                let fuel = if *fuel == 0 { self.default_fuel } else { *fuel };
                let result = runtime
                    .call_i32_i32(&code, export, args.0, args.1, fuel)
                    .map_err(|e| ExecuteError::Wasm(e.to_string()))?;
                self.last_call_result = Some(result);
                self.events
                    .push(format!("CallContract {} -> {}", export, result));
            }
            L1Tx::OpenDispute {
                id,
                plaintiff,
                defendant,
                bond,
                evidence_hash,
            } => {
                if self.disputes.contains_key(id) {
                    return Err(ExecuteError::DisputeExists);
                }
                let acc = self
                    .accounts
                    .get_mut(plaintiff)
                    .ok_or(ExecuteError::UnknownAccount)?;
                if acc.balance_available.0 < bond.0 {
                    return Err(ExecuteError::InsufficientAvailable);
                }
                acc.balance_available.0 -= bond.0;
                // bond held in dispute map (counted in locked_sum)
                self.disputes.insert(
                    *id,
                    Dispute {
                        id: *id,
                        plaintiff: *plaintiff,
                        defendant: *defendant,
                        bond: *bond,
                        status: DisputeStatus::Open,
                        evidence_hash: *evidence_hash,
                    },
                );
                self.events.push(format!("OpenDispute {}", id[0]));
            }
            L1Tx::ResolveDispute { id, for_plaintiff } => {
                let dispute = self
                    .disputes
                    .get_mut(id)
                    .ok_or(ExecuteError::UnknownDispute)?;
                if dispute.status != DisputeStatus::Open {
                    return Err(ExecuteError::DisputeNotOpen);
                }
                let winner = if *for_plaintiff {
                    dispute.plaintiff
                } else {
                    dispute.defendant
                };
                let bond = dispute.bond;
                dispute.status = if *for_plaintiff {
                    DisputeStatus::ResolvedForPlaintiff
                } else {
                    DisputeStatus::ResolvedForDefendant
                };
                let winner_acc = self
                    .accounts
                    .get_mut(&winner)
                    .ok_or(ExecuteError::UnknownAccount)?;
                winner_acc.balance_available.0 += bond.0;
                // reputation nudge
                if let Some(agent_id) = winner_acc.agent_id {
                    if let Some(rec) = self.agents.get_mut(&agent_id) {
                        rec.reputation = rec.reputation.saturating_add(1);
                        winner_acc.reputation = rec.reputation;
                    }
                }
                self.events.push(format!(
                    "ResolveDispute {} for_plaintiff={}",
                    id[0], for_plaintiff
                ));
            }
        }
        self.check_conservation()?;
        Ok(())
    }

    fn apply_checkpoint(
        &mut self,
        checkpoint: &WitnessedCheckpoint,
        funding_account: AccountId,
    ) -> Result<(), ExecuteError> {
        verify_witnessed(checkpoint, self.min_witnesses)
            .map_err(|_| ExecuteError::InvalidCheckpoint)?;
        let h = &checkpoint.checkpoint.header;
        let key = (h.shard_id.as_str().to_string(), h.epoch.0, h.height);
        if let Some(existing) = self.finalized.get(&key) {
            if *existing != h.root {
                return Err(ExecuteError::ConflictingCheckpoint);
            }
            return Err(ExecuteError::CheckpointAlreadyFinalized);
        }

        let allow_key = (h.shard_id.as_str().to_string(), funding_account);
        let outstanding = self
            .shard_outstanding
            .get_mut(&allow_key)
            .ok_or(ExecuteError::InsufficientShardAllowance)?;
        if outstanding.0 < h.exposure.0 {
            return Err(ExecuteError::InsufficientShardAllowance);
        }
        outstanding.0 -= h.exposure.0;

        // Settlement credit: exposure moves back to funding available as settled spend
        // accounting — for Plan 3 we treat exposure as consumed (burned from outstanding
        // into a locked settlement sink owned by the funding account's locked bucket
        // representing "spent to services").
        let acc = self
            .accounts
            .get_mut(&funding_account)
            .ok_or(ExecuteError::UnknownAccount)?;
        acc.balance_locked.0 += h.exposure.0;

        self.finalized.insert(key, h.root);
        self.events.push(format!(
            "CheckpointFinalized {} h={} txs={} exposure={}",
            h.shard_id.as_str(),
            h.height,
            h.tx_count,
            h.exposure.0
        ));
        Ok(())
    }

    pub fn ensure_agent(&mut self, id: AccountId, agent_id: AgentId) {
        self.accounts
            .entry(id)
            .or_insert_with(|| Account::new_agent(id, agent_id, AmountMicros(0)));
    }
}
