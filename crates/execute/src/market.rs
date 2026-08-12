use crate::{ExecuteError, GlobalState};
use blockai_types::{
    AmountMicros, AssetId, AssetUnits, Order, OrderId, OrderStatus, Side, TradeFill,
};

impl GlobalState {
    pub fn order_cash_escrow_sum(&self) -> AmountMicros {
        AmountMicros(self.order_cash_escrow.values().copied().sum())
    }

    pub fn asset_escrow_units(&self, asset_id: AssetId) -> AssetUnits {
        self.order_asset_escrow
            .values()
            .filter(|(id, _)| *id == asset_id)
            .map(|(_, u)| *u)
            .sum()
    }

    pub(crate) fn place_limit_order(
        &mut self,
        order_id: OrderId,
        asset_id: AssetId,
        trader: blockai_types::AccountId,
        side: Side,
        price: AmountMicros,
        units: AssetUnits,
    ) -> Result<(), ExecuteError> {
        if units == 0 || price.0 == 0 {
            return Err(ExecuteError::ZeroAssetAmount);
        }
        if self.orders.contains_key(&order_id) {
            return Err(ExecuteError::OrderExists);
        }
        if !self.assets.contains_key(&asset_id) {
            return Err(ExecuteError::UnknownAsset);
        }
        self.ensure_active(&trader)?;

        let notional = price
            .0
            .checked_mul(units)
            .ok_or(ExecuteError::EscrowOverflow)?;

        match side {
            Side::Buy => {
                let acc = self
                    .accounts
                    .get_mut(&trader)
                    .ok_or(ExecuteError::UnknownAccount)?;
                if acc.balance_available.0 < notional {
                    return Err(ExecuteError::InsufficientAvailable);
                }
                acc.balance_available.0 -= notional;
                self.order_cash_escrow.insert(order_id, notional);
            }
            Side::Sell => {
                self.debit_holding(trader, asset_id, units)?;
                self.order_asset_escrow.insert(order_id, (asset_id, units));
            }
        }

        let seq = self.next_order_seq;
        self.next_order_seq += 1;
        self.orders.insert(
            order_id,
            Order {
                id: order_id,
                asset_id,
                trader,
                side,
                price,
                remaining: units,
                filled: 0,
                status: OrderStatus::Open,
                seq,
            },
        );
        self.events.push(format!(
            "PlaceLimitOrder {:?} {} price={} units={}",
            side, order_id.0[0], price.0, units
        ));
        self.match_asset(asset_id)?;
        Ok(())
    }

    pub(crate) fn cancel_order(
        &mut self,
        order_id: OrderId,
        trader: blockai_types::AccountId,
    ) -> Result<(), ExecuteError> {
        let order = self
            .orders
            .get(&order_id)
            .ok_or(ExecuteError::UnknownOrder)?
            .clone();
        if order.trader != trader {
            return Err(ExecuteError::NotOrderOwner);
        }
        if matches!(order.status, OrderStatus::Filled | OrderStatus::Cancelled) {
            return Err(ExecuteError::OrderNotOpen);
        }
        self.release_order_escrow(&order)?;
        if let Some(o) = self.orders.get_mut(&order_id) {
            o.status = OrderStatus::Cancelled;
            o.remaining = 0;
        }
        self.events
            .push(format!("CancelOrder {}", order_id.0[0]));
        Ok(())
    }

    fn release_order_escrow(&mut self, order: &Order) -> Result<(), ExecuteError> {
        match order.side {
            Side::Buy => {
                let locked = self.order_cash_escrow.remove(&order.id).unwrap_or(0);
                // Refund unused cash: escrow held limit*original; remaining portion =
                // locked cash should equal price * remaining for an open order.
                let refund = order.price.0.saturating_mul(order.remaining).min(locked);
                let dust = locked.saturating_sub(refund);
                let acc = self
                    .accounts
                    .get_mut(&order.trader)
                    .ok_or(ExecuteError::UnknownAccount)?;
                acc.balance_available.0 += refund + dust;
            }
            Side::Sell => {
                if let Some((asset_id, units)) = self.order_asset_escrow.remove(&order.id) {
                    let give_back = units.min(order.remaining);
                    self.credit_holding(order.trader, asset_id, give_back);
                }
            }
        }
        Ok(())
    }

    fn match_asset(&mut self, asset_id: AssetId) -> Result<(), ExecuteError> {
        loop {
            let best_buy = self.best_order(asset_id, Side::Buy);
            let best_sell = self.best_order(asset_id, Side::Sell);
            let (Some(buy_id), Some(sell_id)) = (best_buy, best_sell) else {
                break;
            };
            let buy = self.orders.get(&buy_id).unwrap().clone();
            let sell = self.orders.get(&sell_id).unwrap().clone();
            if buy.price.0 < sell.price.0 {
                break;
            }
            if buy.trader == sell.trader {
                // Skip self-trade by cancelling cannot; leave resting and stop aggressor match.
                // Prefer: do not match; break to avoid locking the book forever.
                break;
            }
            let units = buy.remaining.min(sell.remaining);
            if units == 0 {
                break;
            }
            // Maker is the older order (lower seq).
            let trade_price = if buy.seq < sell.seq {
                buy.price
            } else {
                sell.price
            };
            let notional = trade_price
                .0
                .checked_mul(units)
                .ok_or(ExecuteError::EscrowOverflow)?;

            // Debit buyer cash escrow (may have locked higher limit).
            let buyer_escrow = self
                .order_cash_escrow
                .get_mut(&buy.id)
                .ok_or(ExecuteError::InsufficientAvailable)?;
            if *buyer_escrow < notional {
                return Err(ExecuteError::InsufficientAvailable);
            }
            *buyer_escrow -= notional;

            // Credit seller available EURC.
            let seller_acc = self
                .accounts
                .get_mut(&sell.trader)
                .ok_or(ExecuteError::UnknownAccount)?;
            seller_acc.balance_available.0 += notional;

            // Move asset from sell escrow to buyer holdings.
            let sell_escrow = self
                .order_asset_escrow
                .get_mut(&sell.id)
                .ok_or(ExecuteError::InsufficientAssetUnits)?;
            if sell_escrow.1 < units {
                return Err(ExecuteError::InsufficientAssetUnits);
            }
            sell_escrow.1 -= units;
            self.credit_holding(buy.trader, asset_id, units);

            // Update orders.
            self.apply_fill_to_order(buy_id, units)?;
            self.apply_fill_to_order(sell_id, units)?;

            // If buy filled at better than limit, unused escrow remains until cancel/fill remainder.
            // When buy fully filled, refund leftover cash escrow.
            if self.orders.get(&buy_id).map(|o| o.status) == Some(OrderStatus::Filled) {
                if let Some(left) = self.order_cash_escrow.remove(&buy_id) {
                    if left > 0 {
                        let acc = self.accounts.get_mut(&buy.trader).unwrap();
                        acc.balance_available.0 += left;
                    }
                }
            }
            if self.orders.get(&sell_id).map(|o| o.status) == Some(OrderStatus::Filled) {
                self.order_asset_escrow.remove(&sell_id);
            }

            let mut hasher = blake3::Hasher::new();
            hasher.update(&buy_id.0);
            hasher.update(&sell_id.0);
            hasher.update(&units.to_le_bytes());
            hasher.update(&self.fills.len().to_le_bytes());
            let fill_id = *hasher.finalize().as_bytes();
            self.fills.push(TradeFill {
                id: fill_id,
                asset_id,
                buy_order: buy_id,
                sell_order: sell_id,
                buyer: buy.trader,
                seller: sell.trader,
                units,
                price: trade_price,
                notional: AmountMicros(notional),
            });
            self.events.push(format!(
                "Fill units={} price={} buy={} sell={}",
                units, trade_price.0, buy_id.0[0], sell_id.0[0]
            ));
        }
        Ok(())
    }

    fn best_order(&self, asset_id: AssetId, side: Side) -> Option<OrderId> {
        let open = self.orders.values().filter(|o| {
            o.asset_id == asset_id
                && o.side == side
                && o.remaining > 0
                && matches!(o.status, OrderStatus::Open | OrderStatus::Partial)
        });
        match side {
            Side::Buy => open
                .max_by(|a, b| {
                    a.price
                        .0
                        .cmp(&b.price.0)
                        .then_with(|| b.seq.cmp(&a.seq)) // higher price, then older (lower seq)
                })
                .map(|o| o.id),
            Side::Sell => open
                .min_by(|a, b| a.price.0.cmp(&b.price.0).then_with(|| a.seq.cmp(&b.seq)))
                .map(|o| o.id),
        }
    }

    fn apply_fill_to_order(&mut self, id: OrderId, units: AssetUnits) -> Result<(), ExecuteError> {
        let o = self.orders.get_mut(&id).ok_or(ExecuteError::UnknownOrder)?;
        if o.remaining < units {
            return Err(ExecuteError::InsufficientAssetUnits);
        }
        o.remaining -= units;
        o.filled += units;
        o.status = if o.remaining == 0 {
            OrderStatus::Filled
        } else {
            OrderStatus::Partial
        };
        Ok(())
    }
}
