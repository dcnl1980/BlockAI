# BlockAI Asset Compliance Hooks (Plan 8) Design

**Date:** 2026-08-12  
**Depends on:** Plans 6–7

## 1. Goal

Add **issuer-controlled compliance hooks** for tokenized assets: freeze an instrument and optional **allowlists** for holders/counterparties. Still **not** full KYC/AML or a regulated transfer agent.

## 2. Controls

| Control | Effect |
|---|---|
| `frozen` | Block mint/transfer/spot/place-order for that asset |
| `allowlist_enabled` | Only allowlisted accounts may receive/hold-move/trade the asset |
| allowlist membership | Issuer add/remove accounts |

## 3. L1 txs

- `SetAssetFrozen { asset_id, issuer, frozen }`
- `SetAssetAllowlistEnabled { asset_id, issuer, enabled }`
- `SetAssetAllowlistMember { asset_id, issuer, account, allowed }`

## 4. Enforcement points

`MintAsset` (to), `TransferAsset` (from+to), `SpotTrade` (buyer+seller), `PlaceLimitOrder` (trader). Fail closed with clear errors.

## 5. Non-goals

Identity providers, accreditation, jurisdiction rules, court orders automation, privacy-preserving credentials.
