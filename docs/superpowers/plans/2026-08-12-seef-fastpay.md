# SEEF Plan 12 — FastPay Regional Middle Tier

> **For agentic workers:** Keep PAY hot path unchanged; certificates only for regional ops.

**Goal:** FastPay-style consistent broadcast for cross-shard reallocation and capability top-ups.

**Architecture:** New `blockai-fastpay` committee + certs; authority/shard apply paths; optional L1 outstanding rebalance.

**Tech Stack:** Ed25519 committee signatures, CBOR digests, existing Authority / ShardEngine / GlobalState.

## Global Constraints

- Quorum **3-of-4** regional authorities  
- No PAY authorization via FastPay  
- Conservation on authority float and L1 `shard_outstanding`  
- Exhaustive `L1Tx` / `RegionalOp` matches

---

### Task 1: `blockai-fastpay` + types

- Create crate with `RegionalOp`, `RegionalCertificate`, `RegionalCommittee`
- Tests: quorum seal/verify, reject short quorum

### Task 2: Authority + shard + L1 apply

- `Authority::reallocate`, `debit_for_top_up`, allowance getters
- `ShardEngine::top_up_capability` + WAL `TopUpCapability`
- `L1Tx::ReallocateShardOutstanding` + execute

### Task 3: Sim + ship

- `fastpay_sim`, README, commit, PR
