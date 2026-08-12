# BlockAI Limit Order Book (Plan 7) Design

**Date:** 2026-08-12  
**Depends on:** Plan 6 tokenized assets + SpotTrade

## 1. Goal

Add a **per-asset limit order book** against EURC with place, cancel, **partial fills**, price-time priority matching, and append-only **trade/fill history**.

## 2. Non-goals

- Not a regulated exchange / ATS.
- No stop/market-only special types beyond limit (market = limit crossing book).
- No hidden orders, icebergs, or maker rebates.
- No cross-asset books (only Asset ↔ EURC).

## 3. Model

- `Order { id, asset_id, trader, side: Buy|Sell, price_per_unit, remaining, filled, status, seq }`
- Buy escrow: lock `price * units` EURC from available into `order_cash_escrow`
- Sell escrow: lock `units` from holdings into `order_asset_escrow`
- Match when best bid ≥ best ask; trade price = **resting (maker) price**
- Partial fill reduces `remaining`; status `Partial` / `Filled`
- Cancel returns unused escrow to trader; only owner may cancel
- `fills: Vec<TradeFill>` is the trade history

## 4. Invariants

- EURC: available + shard_outstanding + locked + dispute_bonds + order_cash_escrow = total_supply
- Asset: holdings + order_asset_escrow = minted ≤ max_supply
- No self-trade (same trader both sides)
- Fail closed on insufficient escrow, unknown order, bad owner

## 5. L1 txs

- `PlaceLimitOrder`
- `CancelOrder`

(Matching runs inside `PlaceLimitOrder` apply.)

## 6. Demo

Seller places ask; buyer places bid that crosses → fill(s) recorded; balances update; cancel remainder works.
