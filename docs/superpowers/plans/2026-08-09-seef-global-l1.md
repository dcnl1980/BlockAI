# SEEF Global L1 (DAG+BFT) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a from-scratch global settlement L1 that ingests `WitnessedCheckpoint`s via a DAG mempool + BFT commit path, maintains accounts/staking skeletons, and enforces supply conservation.

**Architecture:** New crates `blockai-execute` (state machine) and `blockai-consensus` (DAG + small-committee BFT). Plan 2 witnesses produce checkpoints; L1 verifies and applies them. No WASM yet (Plan 4).

**Tech Stack:** Rust workspace, blake3, ed25519, tokio in-process validators for tests.

**Spec:** `docs/superpowers/specs/2026-08-09-blockai-seef-design.md` §6

## Global Constraints

- No chain forks; original protocol code.
- Conservation: `global_available + Σ shard_outstanding_allowances + Σ locked = total_supply`.
- Conflicting finalized roots for same `(shard, epoch, height)` → reject + slash path stub.
- Checkpoint apply requires valid shard sig + ≥K witness sigs.

## File map

```text
crates/types/src/account.rs
crates/types/src/l1_tx.rs
crates/execute/…   # GlobalState, apply_tx
crates/consensus/… # DagMempool, BftCommitter
crates/node/…      # blockai-node binary / lib used by l1_sim
crates/tools/src/bin/l1_sim.rs
```

### Task 1: Account + L1 tx types
### Task 2: GlobalState apply + conservation invariants
### Task 3: DAG mempool + BFT commit (in-process 4 validators)
### Task 4: CheckpointFinalized ingest + conflict/slash stub
### Task 5: l1_sim e2e: Plan2 checkpoint → L1 commit → balances

Deferred to Plan 4: WASM, registry/reputation modules beyond stubs.
