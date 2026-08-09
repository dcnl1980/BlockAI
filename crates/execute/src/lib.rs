use blockai_types::{Account, AccountId, AmountMicros, L1Tx, WitnessedCheckpoint};
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
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GlobalState {
    pub accounts: HashMap<AccountId, Account>,
    pub total_supply: AmountMicros,
    pub shard_outstanding: HashMap<(String, AccountId), AmountMicros>,
    pub finalized: HashMap<(String, u64, u64), [u8; 32]>,
    pub min_witnesses: usize,
    pub events: Vec<String>,
}

impl GlobalState {
    pub fn new(min_witnesses: usize) -> Self {
        Self {
            min_witnesses,
            ..Default::default()
        }
    }

    pub fn locked_sum(&self) -> AmountMicros {
        AmountMicros(
            self.accounts
                .values()
                .map(|a| a.balance_locked.0 + a.stake.0)
                .sum(),
        )
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
                let acc = self.accounts.entry(*account).or_insert_with(|| {
                    Account::new_human(*account, AmountMicros(0))
                });
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
        let key = (
            h.shard_id.as_str().to_string(),
            h.epoch.0,
            h.height,
        );
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

    pub fn ensure_agent(&mut self, id: AccountId, agent_id: blockai_types::AgentId) {
        self.accounts
            .entry(id)
            .or_insert_with(|| Account::new_agent(id, agent_id, AmountMicros(0)));
    }
}
