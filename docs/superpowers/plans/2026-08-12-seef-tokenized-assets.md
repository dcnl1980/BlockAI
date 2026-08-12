# SEEF Plan 6 — Tokenized Assets + Atomic Spot Trade

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let issuers register/mint ledger assets and counterparties execute atomic EURC↔asset spot trades on global L1.

**Architecture:** Extend `blockai-types` with `AssetId` / `Asset` / holdings keys; add L1 txs; apply in `blockai-execute::GlobalState` with EURC + per-asset conservation. CLI `trade_sim` demos issue→mint→trade.

**Tech Stack:** Existing execute/consensus/types; no new crates required.

**Spec:** `docs/superpowers/specs/2026-08-12-blockai-tokenized-assets-design.md`

## Global Constraints

- Not a regulated exchange; no order book.
- EURC conservation from Plan 3 must hold.
- Per-asset: sum(holdings) == minted ≤ max_supply.
- Suspended accounts cannot trade/transfer/mint.
- Fail closed; no partial trades.

## File map

```text
docs/superpowers/specs/2026-08-12-blockai-tokenized-assets-design.md
docs/superpowers/plans/2026-08-12-seef-tokenized-assets.md
crates/types/src/asset.rs
crates/types/src/ids.rs          # AssetId
crates/types/src/l1_tx.rs        # Register/Mint/Transfer/SpotTrade
crates/types/src/lib.rs
crates/execute/src/lib.rs
crates/execute/tests/tokenized_trade.rs
crates/tools/src/bin/trade_sim.rs
crates/tools/Cargo.toml
README.md
```

### Tasks
1. Types + L1 txs
2. Execute apply + conservation
3. Tests + `trade_sim` + docs
