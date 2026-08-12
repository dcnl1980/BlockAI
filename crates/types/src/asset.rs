use crate::AccountId;
use serde::{Deserialize, Serialize};

/// Integer holding units for a tokenized instrument (1 unit = one share at decimals=0).
pub type AssetUnits = u128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub asset_id: crate::AssetId,
    pub symbol: String,
    pub name: String,
    pub issuer: AccountId,
    /// Display decimals; balances are still integer `AssetUnits`.
    pub decimals: u8,
    pub max_supply: AssetUnits,
    pub minted: AssetUnits,
    /// When true, mint/transfer/trade/orders for this asset fail closed.
    #[serde(default)]
    pub frozen: bool,
    /// When true, only allowlisted accounts may participate.
    #[serde(default)]
    pub allowlist_enabled: bool,
}
