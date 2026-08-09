# SEEF Checkpoints, Witnesses & Receipts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist three-party payment receipts, seal Merkle checkpoints from the shard log, and collect independent witness countersignatures ready for later L1 ingest.

**Architecture:** Extend Plan 1 with `receipt` types, service-side receipt signing, a shard `ReceiptLog` + Merkle sealer, and a `blockai-witness` crate. No global L1 yet — checkpoints are verified locally and exported as `WitnessedCheckpoint` objects.

**Tech Stack:** Existing Rust workspace; `blake3` Merkle; ed25519 via `blockai-crypto`; tokio test harness.

**Spec:** `docs/superpowers/specs/2026-08-09-blockai-seef-design.md` §§4.6, 5.6, 6.3–6.4 (checkpoint object shape)

## Global Constraints

- From-scratch BlockAI; no chain forks.
- Commit-before-exec unchanged from Plan 1.
- Checkpoint seal triggers: first of N txs, exposure micros, or explicit `seal_now` (time trigger optional in tests).
- Witnesses are independent keys; rewriting history needs shard quorum + witnesses.
- Amounts remain `AmountMicros(u128)` / `"EURC"`.

## File map

```text
crates/types/src/receipt.rs
crates/types/src/checkpoint.rs
crates/crypto/src/receipt_sign.rs
crates/shard/src/merkle.rs
crates/shard/src/receipt_log.rs
crates/shard/src/checkpoint.rs
crates/witness/Cargo.toml
crates/witness/src/lib.rs
crates/tools/src/bin/checkpoint_sim.rs
```

---

### Task 1: Receipt and checkpoint types

**Files:**
- Create: `crates/types/src/receipt.rs`
- Create: `crates/types/src/checkpoint.rs`
- Modify: `crates/types/src/lib.rs`
- Test: `crates/types/tests/receipt_leaf_hash.rs`

**Interfaces:**
- Produces:
  - `AgentAuthorization { pay_cbor_hash, agent_signature }` (A)
  - `EdgeAcceptance { agent_auth_hash, commit_index, tx_id, edge_signature }` (E)
  - `ServiceReceipt { edge_accept_hash, execution_hash, actual_amount, service_signature }` (S)
  - `PaymentProof { agent, edge, service }`
  - `fn receipt_leaf_hash(proof: &PaymentProof) -> [u8; 32]`
  - `CheckpointHeader { shard_id, epoch, root, height, tx_count, exposure, sealed_at_unix_ms }`
  - `SignedCheckpoint { header, shard_signer_pubkey, shard_signature }`
  - `WitnessedCheckpoint { checkpoint, witness_sigs: Vec<( [u8;32], Vec<u8>)> }`

- [ ] **Step 1: Write failing test** `receipt_leaf_hash` changes when `execution_hash` changes
- [ ] **Step 2: Run fail**
- [ ] **Step 3: Implement types + BLAKE3 leaf hash over CBOR of `PaymentProof`**
- [ ] **Step 4: Pass**
- [ ] **Step 5: Commit** `feat(types): add payment receipts and checkpoint headers`

---

### Task 2: Sign/verify EdgeAcceptance and ServiceReceipt

**Files:**
- Create: `crates/crypto/src/receipt_sign.rs`
- Modify: `crates/crypto/src/lib.rs`
- Test: `crates/crypto/tests/receipt_sign_verify.rs`

**Interfaces:**
- `sign_edge_acceptance(edge_kp, &EdgeAcceptance) -> sig`
- `verify_edge_acceptance(vk, &EdgeAcceptance)`
- `sign_service_receipt(service_kp, &ServiceReceipt) -> sig`
- `verify_service_receipt(vk, &ServiceReceipt)`
- Domain tags `"EDGE_ACCEPT"` / `"SERVICE_RECEIPT"`

- [ ] **Step 1–5:** TDD roundtrip + tamper fails; commit `feat(crypto): sign edge and service receipts`

---

### Task 3: Merkle tree

**Files:**
- Create: `crates/shard/src/merkle.rs`
- Modify: `crates/shard/src/lib.rs`
- Test: `crates/shard/tests/merkle_proof.rs`

**Interfaces:**
- `fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32]`
- `fn merkle_proof(leaves, index) -> MerkleProof`
- `fn verify_merkle_proof(leaf, proof, root) -> bool`
- Pairing: hash(left||right); odd last leaf duplicated

- [ ] **Step 1–5:** TDD inclusion proof; commit `feat(shard): add Merkle root and proofs`

---

### Task 4: Receipt log + checkpoint sealer on shard

**Files:**
- Create: `crates/shard/src/receipt_log.rs`
- Create: `crates/shard/src/checkpoint.rs`
- Modify: `crates/shard/src/engine.rs` (hook after EdgeAccept path via new helper API)
- Modify: `crates/shard/src/lib.rs`
- Test: `crates/shard/tests/checkpoint_seal.rs`

**Interfaces:**
- `ReceiptLog::append(PaymentProof)`
- `CheckpointSealer { max_txs, max_exposure }`
- `fn maybe_seal(log, sealer, shard_kp, shard_id, epoch, now) -> Option<SignedCheckpoint>`
- `fn force_seal(...) -> SignedCheckpoint`
- Engine helper: `complete_payment(pay, edge_accept, execution_hash, actual_amount, service_kp) -> PaymentProof` builds A/E/S and appends to log

- [ ] **Step 1:** Test seals after N=2 payments with correct tx_count/exposure
- [ ] **Step 2–4:** Implement; verify Merkle root matches leaves
- [ ] **Step 5:** Commit `feat(shard): receipt log and checkpoint sealing`

---

### Task 5: Witness crate

**Files:**
- Create: `crates/witness/` crate
- Modify: root `Cargo.toml` members
- Test: `crates/witness/tests/countersign.rs`

**Interfaces:**
- `struct Witness { key: Keypair }`
- `fn Witness::countersign(&self, checkpoint: &SignedCheckpoint) -> (pubkey, sig)`
- `fn verify_witnessed(checkpoint, witnesses_required: usize) -> Result<()>`
- Reject: bad shard sig, duplicate witness, insufficient witnesses, conflicting root for same `(shard, epoch, height)` tracked in `WitnessSet::accept`

- [ ] **Step 1–5:** TDD 2-of-3 witnesses; commit `feat(witness): countersign sealed checkpoints`

---

### Task 6: End-to-end checkpoint_sim + integration test

**Files:**
- Create: `crates/tools/src/bin/checkpoint_sim.rs`
- Create: `crates/shard/tests/receipt_checkpoint_e2e.rs`
- Modify: `crates/tools/Cargo.toml`

**Flow:**
1. Plan 1 cluster pays twice
2. Build service receipts
3. Seal checkpoint (N=2)
4. 3 witnesses countersign
5. Verify `WitnessedCheckpoint` and a Merkle proof for pay #1

- [ ] **Step 1–5:** TDD e2e + CLI `checkpoint_sim`; commit `feat(tools): checkpoint_sim end-to-end path`

---

## Self-review

| Spec item | Task |
|---|---|
| Three-party receipts A/E/S | 1–2, 4 |
| Merkle checkpoint | 3–4 |
| Witness countersign | 5 |
| Checkpoint object fields | 1, 4 |
| Verifiability path | 6 |
| No L1 apply yet | Explicit deferral to Plan 3 |
