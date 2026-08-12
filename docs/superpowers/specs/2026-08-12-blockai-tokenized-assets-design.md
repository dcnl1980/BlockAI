# BlockAI Tokenized Assets + Spot Trade (Plan 6) Design

**Status:** Approved direction via user request (tokenizeizing / digital stocks trading on SEEF).  
**Date:** 2026-08-12

## 1. Goal

Add a first-class L1 path to **register tokenized instruments**, **mint/transfer units**, and execute **atomic spot trades against EURC** — so agents/humans can demo “issue a digital stock and buy/sell it” on BlockAI.

## 2. Non-goals (explicit)

- Not a regulated securities exchange, broker-dealer, or ATS.
- No KYC/AML, accreditation, transfer restrictions, or corporate actions.
- No continuous order book / matching engine / cancels / partial fills.
- No oracle price feeds, margin, shorting, or derivatives.
- No claim of legal “stock” status — instruments are **ledger assets** labeled by the issuer.

## 3. Model

### 3.1 Instrument

```text
Asset {
  asset_id,
  symbol,          // e.g. "ACME"
  name,
  issuer,          // AccountId that may mint
  decimals,        // display hint; balances are integer units
  max_supply,      // hard cap
  minted,          // units issued so far
}
```

### 3.2 Holdings

`holdings[(account, asset_id)] -> units: u128`

Invariant per asset: `Σ holdings == minted ≤ max_supply`.

### 3.3 Cash

EURC remains `Account.balance_available` / conservation rules from Plan 3. Spot trades debit buyer EURC and credit seller EURC atomically with the asset transfer.

### 3.4 Spot trade (atomic)

```text
SpotTrade {
  buyer, seller,
  asset_id,
  asset_units,     // shares transferred seller → buyer
  price_total,     // EURC micros buyer → seller
}
```

All-or-nothing: insufficient EURC, insufficient units, unknown asset, or suspended party → fail closed, no partial apply.

## 4. L1 transactions

| Tx | Who | Effect |
|---|---|---|
| `RegisterAsset` | issuer account exists | create asset metadata, minted=0 |
| `MintAsset` | issuer | increase minted + holder balance (≤ max_supply) |
| `TransferAsset` | from | move units from→to |
| `SpotTrade` | atomic | move units seller→buyer and EURC buyer→seller |

## 5. Security / economics fit with SEEF

- Token ops settle on **global L1** (not local PAY shards). Micropay shards stay for agent API spend.
- No new payment keys; reuse account identity stubs already on L1.
- Conservation for EURC unchanged; asset conservation checked after each asset-affecting tx.
- Fail closed on unknown asset, over-mint, over-spend, suspended accounts.

## 6. Demo success criteria

1. Register `ACME` with max 1_000 units.
2. Mint 100 to issuer.
3. Buyer funded with EURC buys 10 units for a fixed price.
4. Balances: buyer holds 10 ACME; seller holds 90; EURC conserved.
5. `trade_sim` CLI prints a single-line OK summary.

## 7. Follow-ons (deferred)

Order book, RFQ, compliance hooks, restricted lists, dividends, custody attestations, secondary venue discovery.
