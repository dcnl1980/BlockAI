# SEEF Plan 11 — Assurance Suite

> **For agentic workers:** Implement drills + p50 gate; keep production paths unchanged.

**Goal:** Lab assurance drills for Byzantine/partition/key-theft plus published release p50 vs v1 single-digit-ms criterion.

**Architecture:** Integration tests on in-process 4-node shard cluster + `assurance_sim` harness that measures PAY authorize latency and prints a checklist.

**Tech Stack:** Tokio tests, existing `cluster4_with_issuer_bytes`, clap binary in `blockai-tools`.

## Global Constraints

- p50 gate default: **10_000 µs** (single-digit ms) under low lab load; enforced in **release** builds only  

- Kill-two quorum failure must not mint / double-spend  
- Key theft loss ≤ capability `maximum_total` remaining  
- No changes to BFT vote crypto (classical)

---

### Task 1: Engine remaining accessor + drills

**Files:**
- Modify: `crates/shard/src/engine.rs`
- Create: `crates/shard/tests/assurance_drills.rs`

- [ ] `ShardEngine::remaining`
- [ ] Drills listed in design §2
- [ ] `cargo test -p blockai-shard --test assurance_drills`

### Task 2: assurance_sim + docs

**Files:**
- Create: `crates/tools/src/bin/assurance_sim.rs`
- Modify: `crates/tools/Cargo.toml`, `README.md`

- [ ] Measure p50; exit 1 if ≥ max
- [ ] Print drill checklist PASS lines
- [ ] Commit + PR
