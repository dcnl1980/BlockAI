# SEEF WASM + Registry/Reputation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a metered WASM contract runtime and system-module stubs for agent registry, reputation, and disputes on the global L1 state machine.

**Architecture:** New `blockai-wasm` crate (wasmtime + fuel). Extend `blockai-execute` / `L1Tx` with `DeployContract`, `CallContract`, `RegisterAgent`, `UpdateReputation`, `OpenDispute`, `ResolveDispute`. No on-chain LLM inference.

**Tech Stack:** `wasmtime`, existing execute/consensus crates, tokio tests.

**Spec:** `docs/superpowers/specs/2026-08-09-blockai-seef-design.md` §§6.5–6.6

## Global Constraints

- Metered WASM only; reject unbounded execution via fuel.
- System modules are native stubs (not user WASM) for registry/reputation/dispute.
- Conservation invariants from Plan 3 must still hold.
- Plan 5 (QUIC/attestation/PQ) remains deferred.

### Tasks
1. WASM runtime deploy/call with fuel
2. Agent registry on GlobalState
3. Reputation + dispute stubs
4. L1 wiring, tests, `wasm_sim`
