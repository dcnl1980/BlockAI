# SEEF Plan 5 — QUIC Dataplane, Attestation, PQ Agility, Benches

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the Plan 5 security/dataplane slice: persistent QUIC PAY transport that forbids 0-RTT PAY, software attestation stubs that fail closed on issuance, crypto-agility with at least one ML-DSA path, and a PAY-authorize latency bench.

**Architecture:** New `blockai-net` (QUIC frames + quinn loop) and `blockai-attest` (RATS-style stub verifier). Extend `blockai-crypto` with `AlgorithmId` + hybrid Ed25519/ML-DSA-65 seals for capabilities. Gate `Authority::issue_capability` on attestation evidence. Criterion bench over in-process shard authorize.

**Tech Stack:** `quinn` 0.11, `rustls`/`rcgen` for test TLS, `ml-dsa` 0.1 (ML-DSA-65), `criterion`, existing authority/shard crates.

**Spec:** `docs/superpowers/specs/2026-08-09-blockai-seef-design.md` §§4.7, 5.5, 7.2–7.4, 10.1–10.3

## Global Constraints

- No QUIC 0-RTT for PAY / TRANSFER / PURCHASE / WITHDRAW (fail closed).
- 0-RTT allowed only for idempotent reads.
- QUIC/TLS keys never used as payment keys.
- No / failed attestation → no new capabilities.
- Hot-path PAY remains classical Ed25519; capabilities support hybrid classical+PQ.
- Algorithms identified by envelope IDs; economic state machine unchanged.
- AF_XDP, multipath QUIC, production HSM, real hardware attestation remain deferred.

## File map

```text
docs/superpowers/plans/2026-08-09-seef-quic-attest-pq.md
Cargo.toml                              # members: net, attest
crates/crypto/src/alg.rs                # AlgorithmId
crates/crypto/src/pq.rs                 # MlDsa65 keygen/sign/verify
crates/crypto/src/hybrid.rs             # hybrid capability seal
crates/types/src/capability.rs          # optional PQ fields + alg id
crates/attest/Cargo.toml
crates/attest/src/lib.rs
crates/attest/tests/fail_closed.rs
crates/authority/src/issuer.rs          # require attestation
crates/net/Cargo.toml
crates/net/src/lib.rs
crates/net/src/frame.rs                 # Pay vs IdempotentRead
crates/net/src/policy.rs                # 0-RTT admission
crates/net/src/quic.rs                  # quinn server/client helpers
crates/net/tests/zero_rtt_pay_rejected.rs
crates/tools/src/bin/quic_sim.rs
crates/shard/benches/pay_authorize.rs
README.md
```

---

### Task 1: Algorithm IDs + ML-DSA-65 + hybrid capability seal

**Files:**
- Create: `crates/crypto/src/alg.rs`, `crates/crypto/src/pq.rs`, `crates/crypto/src/hybrid.rs`
- Modify: `crates/crypto/Cargo.toml`, `crates/crypto/src/lib.rs`, `crates/types/src/capability.rs`
- Test: `crates/crypto/tests/hybrid_capability.rs`

**Interfaces:**
- Produces: `AlgorithmId::{Ed25519, MlDsa65, HybridEd25519MlDsa65}`, `PqKeypair`, `sign_capability_hybrid`, `verify_capability_hybrid`

- [ ] **Step 1:** Add `ml-dsa` dependency with default features; implement PQ + hybrid modules; extend `SpendCapability` with `issuer_alg: u16`, `issuer_pq_pubkey: Vec<u8>`, `issuer_pq_signature: Vec<u8>` (empty = classical-only).
- [ ] **Step 2:** Test hybrid sign/verify roundtrip and reject when PQ half is tampered.
- [ ] **Step 3:** Commit.

### Task 2: Attestation stub + fail-closed issuance

**Files:**
- Create: `crates/attest/**`
- Modify: `crates/authority/src/issuer.rs`, authority tests, any callers of `issue_capability`
- Test: `crates/attest/tests/fail_closed.rs`, update `crates/authority/tests/partition_issue.rs`

**Interfaces:**
- Produces: `AttestationEvidence`, `AttestationPolicy`, `verify_evidence(policy, evidence) -> Result<(), AttestError>`
- Consumes: Authority issues only after verify succeeds

- [ ] **Step 1:** Implement software “platform” signed evidence + approved hash sets.
- [ ] **Step 2:** Change `Authority::issue_capability` to take `&AttestationEvidence` and return `AuthorityError::AttestationFailed` on mismatch.
- [ ] **Step 3:** Tests: good evidence issues; bad binary hash / missing sig fail closed.
- [ ] **Step 4:** Commit.

### Task 3: QUIC dataplane (no 0-RTT PAY)

**Files:**
- Create: `crates/net/**`, `crates/tools/src/bin/quic_sim.rs`
- Modify: workspace `Cargo.toml`, `crates/tools/Cargo.toml`, `README.md`
- Test: `crates/net/tests/zero_rtt_pay_rejected.rs`, `crates/net/tests/pay_1rtt_ok.rs`

**Interfaces:**
- Produces: `AppFrame::{Pay, IdempotentRead}`, `admit_frame(is_early_data, frame)`, quinn bind/connect helpers that never treat TLS keys as payment keys

- [ ] **Step 1:** Frame codec (length-prefixed CBOR) + admission policy unit tests.
- [ ] **Step 2:** Quinn localhost server/client; tag 0-RTT vs 1-RTT; reject PAY on early data; accept PAY on 1-RTT; allow IdempotentRead on 0-RTT.
- [ ] **Step 3:** `quic_sim` CLI happy path.
- [ ] **Step 4:** Commit.

### Task 4: PAY authorize bench + docs

**Files:**
- Create: `crates/shard/benches/pay_authorize.rs`
- Modify: `crates/shard/Cargo.toml`, README

- [ ] **Step 1:** Criterion bench: activate capability + `handle_pay` on 4-validator in-process cluster; document how to run.
- [ ] **Step 2:** `cargo test` green; commit; open PR.

## Spec coverage

| Spec item | Task |
|---|---|
| Persistent QUIC; no 0-RTT PAY | Task 3 |
| 0-RTT only idempotent reads | Task 3 |
| Attestation fail-closed issuance | Task 2 |
| Crypto-agility + one PQ path | Task 1 |
| Key separation (transport ≠ payment) | Task 3 (documented + separate key domains) |
| Same-DC PAY p50 bench hook | Task 4 |
