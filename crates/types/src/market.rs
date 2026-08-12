use crate::{AccountId, AmountMicros, AssetId, AssetUnits};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Open,
    Partial,
    Filled,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub asset_id: AssetId,
    pub trader: AccountId,
    pub side: Side,
    /// Limit price per asset unit, in EURC micros.
    pub price: AmountMicros,
    pub remaining: AssetUnits,
    pub filled: AssetUnits,
    pub status: OrderStatus,
    /// Global sequence for time priority (lower = older).
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeFill {
    pub id: [u8; 32],
    pub asset_id: AssetId,
    pub buy_order: OrderId,
    pub sell_order: OrderId,
    pub buyer: AccountId,
    pub seller: AccountId,
    pub units: AssetUnits,
    pub price: AmountMicros,
    pub notional: AmountMicros,
}
