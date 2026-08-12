# SEEF Plan 10 — Full PQ Dual-Sign

> **For agentic workers:** Implement task-by-task; keep local BFT classical.

**Goal:** Hybrid Ed25519 + ML-DSA-65 dual-sign on PAY, checkpoints, witnesses, receipts, and HSM root shares.

**Architecture:** Extend `blockai-crypto` hybrid helpers; optional PQ wire fields with serde defaults; seal APIs take `Option<&PqKeypair>`; shard/witness/hsm verify PQ when alg/material present.

**Tech Stack:** Rust workspace, `ml-dsa` (ML-DSA-65), CBOR domain-separated bodies, Ed25519 dalek.

## Global Constraints

- Edition / toolchain: Rust 1.97+ (ml-dsa / edition2024 deps as already required)
- No PQ on local BFT vote digests
- Fail closed when `HybridEd25519MlDsa65` but PQ pubkey/sig empty
- Exhaustive `AlgorithmId` switches with `never` default

---

### Task 1: Types wire fields + Default helpers

**Files:**
- Modify: `crates/types/src/pay.rs`, `checkpoint.rs`, `receipt.rs`
- Modify: `crates/hsm/src/lib.rs` (`ShareSig`)

- [ ] Add optional PQ fields + `Default` for `Pay` (so literals can `..Default::default()`)
- [ ] Add serde-default PQ fields on checkpoint/witness/receipt/share types
- [ ] `cargo test -p blockai-types`

### Task 2: Crypto hybrid seal/verify

**Files:**
- Modify: `crates/crypto/src/hybrid.rs`, `lib.rs`, `sign.rs`, `receipt_sign.rs`
- Test: `crates/crypto/tests/hybrid_pay.rs`, extend receipt tests

- [ ] `seal_pay_hybrid` / `verify_pay_hybrid`
- [ ] Checkpoint / witness / edge / service / root-op hybrid helpers (or thin wrappers)
- [ ] Classical `sign_pay`/`verify_pay` unchanged for Ed25519

### Task 3: Wire into shard / witness / payment / hsm

**Files:**
- Modify: `crates/shard/src/engine.rs`, `checkpoint.rs`, `payment.rs`
- Modify: `crates/witness/src/lib.rs`
- Modify: `crates/hsm/src/lib.rs`

- [ ] `validate_pay` exhaustive alg match
- [ ] Checkpoint sealer accepts optional PQ key
- [ ] Witness countersign optional PQ
- [ ] SoftHsm optional PQ shares
- [ ] Update Pay literals with `..Default::default()`

### Task 4: Sim + docs + ship

**Files:**
- Create: `crates/tools/src/bin/pq_sim.rs`
- Modify: `crates/tools/Cargo.toml`, `README.md`

- [ ] `pq_sim` dual-signs PAY + checkpoint + HSM root op
- [ ] README / plan links
- [ ] `cargo test` + commit + PR
