# SEEF Plan 7 — Limit Order Book

> **For agentic workers:** Use executing-plans / subagent-driven-development task-by-task.

**Goal:** Per-asset EURC limit book with place/cancel, partial fills, fill history.

**Architecture:** Types for Order/Fill; `blockai-execute` market matcher; escrow in GlobalState; `trade_sim` book demo mode.

**Spec:** `docs/superpowers/specs/2026-08-12-blockai-order-book-design.md`

## Tasks
1. Types + L1 txs
2. Escrow + match + cancel in execute
3. Tests + trade_sim book path + README
