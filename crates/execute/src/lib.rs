mod market;

use blockai_types::{
    Account, AccountId, AgentId, AmountMicros, Asset, AssetId, AssetUnits, Dispute, DisputeStatus,
    L1Tx, Order, OrderId, TradeFill, WitnessedCheckpoint,
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
    #[error("asset already registered")]
    AssetExists,
    #[error("unknown asset")]
    UnknownAsset,
    #[error("asset symbol taken")]
    AssetSymbolTaken,
    #[error("not asset issuer")]
    NotAssetIssuer,
    #[error("exceeds max supply")]
    ExceedsMaxSupply,
    #[error("insufficient asset units")]
    InsufficientAssetUnits,
    #[error("zero asset amount")]
    ZeroAssetAmount,
    #[error("asset conservation broken")]
    AssetConservationBroken,
    #[error("self trade forbidden")]
    SelfTrade,
    #[error("order already exists")]
    OrderExists,
    #[error("unknown order")]
    UnknownOrder,
    #[error("order not open")]
    OrderNotOpen,
    #[error("not order owner")]
    NotOrderOwner,
    #[error("escrow overflow")]
    EscrowOverflow,
    #[error("asset frozen")]
    AssetFrozen,
    #[error("account not allowlisted for asset")]
    NotAllowlisted,
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
    pub assets: HashMap<AssetId, Asset>,
    pub asset_symbols: HashMap<String, AssetId>,
    /// Allowlisted (asset, account) pairs when asset.allowlist_enabled.
    pub asset_allowlist: HashMap<(AssetId, AccountId), ()>,
    /// Holdings keyed by (account, asset_id).
    pub holdings: HashMap<(AccountId, AssetId), AssetUnits>,
    pub orders: HashMap<OrderId, Order>,
    /// EURC micros locked for buy orders.
    pub order_cash_escrow: HashMap<OrderId, u128>,
    /// Asset units locked for sell orders.
    pub order_asset_escrow: HashMap<OrderId, (AssetId, AssetUnits)>,
    pub fills: Vec<TradeFill>,
    pub next_order_seq: u64,
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
        AmountMicros(account_locked + dispute_bonds + self.order_cash_escrow_sum().0)
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
        self.check_asset_conservation()?;
        Ok(())
    }

    pub fn holding(&self, account: AccountId, asset_id: AssetId) -> AssetUnits {
        self.holdings
            .get(&(account, asset_id))
            .copied()
            .unwrap_or(0)
    }

    pub fn check_asset_conservation(&self) -> Result<(), ExecuteError> {
        for (asset_id, asset) in &self.assets {
            let held: AssetUnits = self
                .holdings
                .iter()
                .filter(|((_, id), _)| id == asset_id)
                .map(|(_, u)| *u)
                .sum();
            let escrowed = self.asset_escrow_units(*asset_id);
            if held + escrowed != asset.minted || asset.minted > asset.max_supply {
                return Err(ExecuteError::AssetConservationBroken);
            }
        }
        Ok(())
    }

    pub(crate) fn ensure_active(&self, account: &AccountId) -> Result<(), ExecuteError> {
        let acc = self
            .accounts
            .get(account)
            .ok_or(ExecuteError::UnknownAccount)?;
        if acc.suspended {
            return Err(ExecuteError::Suspended);
        }
        Ok(())
    }

    pub(crate) fn ensure_asset_active(&self, asset_id: &AssetId) -> Result<(), ExecuteError> {
        let asset = self
            .assets
            .get(asset_id)
            .ok_or(ExecuteError::UnknownAsset)?;
        if asset.frozen {
            return Err(ExecuteError::AssetFrozen);
        }
        Ok(())
    }

    pub(crate) fn ensure_asset_participant(
        &self,
        asset_id: &AssetId,
        account: &AccountId,
    ) -> Result<(), ExecuteError> {
        let asset = self
            .assets
            .get(asset_id)
            .ok_or(ExecuteError::UnknownAsset)?;
        if asset.allowlist_enabled && !self.asset_allowlist.contains_key(&(*asset_id, *account)) {
            return Err(ExecuteError::NotAllowlisted);
        }
        Ok(())
    }

    fn ensure_issuer(asset: &Asset, issuer: &AccountId) -> Result<(), ExecuteError> {
        if asset.issuer != *issuer {
            return Err(ExecuteError::NotAssetIssuer);
        }
        Ok(())
    }

    pub(crate) fn credit_holding(&mut self, account: AccountId, asset_id: AssetId, units: AssetUnits) {
        let entry = self.holdings.entry((account, asset_id)).or_insert(0);
        *entry = entry.saturating_add(units);
    }

    pub(crate) fn debit_holding(
        &mut self,
        account: AccountId,
        asset_id: AssetId,
        units: AssetUnits,
    ) -> Result<(), ExecuteError> {
        let bal = self.holding(account, asset_id);
        if bal < units {
            return Err(ExecuteError::InsufficientAssetUnits);
        }
        let next = bal - units;
        if next == 0 {
            self.holdings.remove(&(account, asset_id));
        } else {
            self.holdings.insert((account, asset_id), next);
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
            L1Tx::RegisterAsset {
                asset_id,
                issuer,
                symbol,
                name,
                decimals,
                max_supply,
            } => {
                self.ensure_active(issuer)?;
                if self.assets.contains_key(asset_id) {
                    return Err(ExecuteError::AssetExists);
                }
                let sym = symbol.to_ascii_uppercase();
                if sym.is_empty() || self.asset_symbols.contains_key(&sym) {
                    return Err(ExecuteError::AssetSymbolTaken);
                }
                if *max_supply == 0 {
                    return Err(ExecuteError::ZeroAssetAmount);
                }
                self.assets.insert(
                    *asset_id,
                    Asset {
                        asset_id: *asset_id,
                        symbol: sym.clone(),
                        name: name.clone(),
                        issuer: *issuer,
                        decimals: *decimals,
                        max_supply: *max_supply,
                        minted: 0,
                        frozen: false,
                        allowlist_enabled: false,
                    },
                );
                self.asset_symbols.insert(sym.clone(), *asset_id);
                self.events
                    .push(format!("RegisterAsset {} max={}", sym, max_supply));
            }
            L1Tx::MintAsset {
                asset_id,
                issuer,
                to,
                units,
            } => {
                if *units == 0 {
                    return Err(ExecuteError::ZeroAssetAmount);
                }
                self.ensure_active(issuer)?;
                self.ensure_active(to)?;
                self.ensure_asset_active(asset_id)?;
                let asset = self
                    .assets
                    .get(asset_id)
                    .ok_or(ExecuteError::UnknownAsset)?
                    .clone();
                Self::ensure_issuer(&asset, issuer)?;
                self.ensure_asset_participant(asset_id, to)?;
                if asset.minted.saturating_add(*units) > asset.max_supply {
                    return Err(ExecuteError::ExceedsMaxSupply);
                }
                {
                    let asset_mut = self.assets.get_mut(asset_id).unwrap();
                    asset_mut.minted += *units;
                }
                self.credit_holding(*to, *asset_id, *units);
                let symbol = self.assets.get(asset_id).unwrap().symbol.clone();
                self.events
                    .push(format!("MintAsset {} -> {} units={}", symbol, to.0[0], units));
            }
            L1Tx::TransferAsset {
                asset_id,
                from,
                to,
                units,
            } => {
                if *units == 0 {
                    return Err(ExecuteError::ZeroAssetAmount);
                }
                self.ensure_asset_active(asset_id)?;
                self.ensure_active(from)?;
                self.ensure_active(to)?;
                self.ensure_asset_participant(asset_id, from)?;
                self.ensure_asset_participant(asset_id, to)?;
                self.debit_holding(*from, *asset_id, *units)?;
                self.credit_holding(*to, *asset_id, *units);
                self.events.push(format!(
                    "TransferAsset {} units={} from={} to={}",
                    asset_id.0[0], units, from.0[0], to.0[0]
                ));
            }
            L1Tx::SpotTrade {
                asset_id,
                buyer,
                seller,
                asset_units,
                price_total,
            } => {
                if *asset_units == 0 {
                    return Err(ExecuteError::ZeroAssetAmount);
                }
                if buyer == seller {
                    return Err(ExecuteError::SelfTrade);
                }
                self.ensure_asset_active(asset_id)?;
                self.ensure_active(buyer)?;
                self.ensure_active(seller)?;
                self.ensure_asset_participant(asset_id, buyer)?;
                self.ensure_asset_participant(asset_id, seller)?;
                {
                    let buyer_acc = self.accounts.get(buyer).unwrap();
                    if buyer_acc.balance_available.0 < price_total.0 {
                        return Err(ExecuteError::InsufficientAvailable);
                    }
                }
                if self.holding(*seller, *asset_id) < *asset_units {
                    return Err(ExecuteError::InsufficientAssetUnits);
                }
                // Atomic: EURC then units (both fail-closed already checked).
                {
                    let buyer_acc = self.accounts.get_mut(buyer).unwrap();
                    buyer_acc.balance_available.0 -= price_total.0;
                }
                {
                    let seller_acc = self.accounts.get_mut(seller).unwrap();
                    seller_acc.balance_available.0 += price_total.0;
                }
                self.debit_holding(*seller, *asset_id, *asset_units)?;
                self.credit_holding(*buyer, *asset_id, *asset_units);
                self.events.push(format!(
                    "SpotTrade asset={} units={} price={} buyer={} seller={}",
                    asset_id.0[0], asset_units, price_total.0, buyer.0[0], seller.0[0]
                ));
            }
            L1Tx::PlaceLimitOrder {
                order_id,
                asset_id,
                trader,
                side,
                price,
                units,
            } => {
                self.place_limit_order(*order_id, *asset_id, *trader, *side, *price, *units)?;
            }
            L1Tx::CancelOrder { order_id, trader } => {
                self.cancel_order(*order_id, *trader)?;
            }
            L1Tx::SetAssetFrozen {
                asset_id,
                issuer,
                frozen,
            } => {
                self.ensure_active(issuer)?;
                let asset = self
                    .assets
                    .get_mut(asset_id)
                    .ok_or(ExecuteError::UnknownAsset)?;
                Self::ensure_issuer(asset, issuer)?;
                asset.frozen = *frozen;
                self.events
                    .push(format!("SetAssetFrozen {} frozen={}", asset_id.0[0], frozen));
            }
            L1Tx::SetAssetAllowlistEnabled {
                asset_id,
                issuer,
                enabled,
            } => {
                self.ensure_active(issuer)?;
                let asset = self
                    .assets
                    .get_mut(asset_id)
                    .ok_or(ExecuteError::UnknownAsset)?;
                Self::ensure_issuer(asset, issuer)?;
                asset.allowlist_enabled = *enabled;
                self.events.push(format!(
                    "SetAssetAllowlistEnabled {} enabled={}",
                    asset_id.0[0], enabled
                ));
            }
            L1Tx::SetAssetAllowlistMember {
                asset_id,
                issuer,
                account,
                allowed,
            } => {
                self.ensure_active(issuer)?;
                let asset = self
                    .assets
                    .get(asset_id)
                    .ok_or(ExecuteError::UnknownAsset)?
                    .clone();
                Self::ensure_issuer(&asset, issuer)?;
                if *allowed {
                    self.asset_allowlist.insert((*asset_id, *account), ());
                } else {
                    self.asset_allowlist.remove(&(*asset_id, *account));
                }
                self.events.push(format!(
                    "SetAssetAllowlistMember asset={} account={} allowed={}",
                    asset_id.0[0], account.0[0], allowed
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
