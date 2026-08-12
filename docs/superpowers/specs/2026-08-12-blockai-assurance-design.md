# BlockAI Assurance Suite (Plan 11) Design

**Date:** 2026-08-12  
**Status:** Implementation slice for SEEF §9 / §10.3 v1 success criteria

## 1. Goal

Land a **lab assurance suite** that exercises mandatory fault scenarios and publishes a **release p50** check against v1 success criteria:

| Criterion | Lab check |
|---|---|
| Same-DC PAY authorize p50 in single-digit ms | `assurance_sim --release` measures p50; fail if ≥ 10_000 µs |
| Double-spend / replay / cross-shard fail closed | existing + drill tests |
| Kill-one-validator: no duplicate execution | existing `kill_one_validator_*` + drill |
| Checkpoint → L1 apply conserves supply | covered by L1 e2e; referenced in report |
| Agent key theft: loss ≤ remaining lease | new drill |

## 2. Mandatory drills (this plan)

1. **Byzantine / kill-two** — compromise 2 of 4 validators → new PAY fails quorum (no unbounded mint)  
2. **Kill-one** — still commits; replay closed (already present; reasserted in suite)  
3. **Agent key theft** — attacker with stolen agent key can spend only remaining lease  
4. **Expired capability** — TTL / clock past `valid_until` rejects  
5. **Partition-bounded spend** — authority cannot allocate more than funded total across shards  
6. **WAL reboot** — consumed sequences survive reopen  
7. **Clone / duplicate PAY** — identical PAY after commit → `Replay`  
8. **Fenced epoch** — fenced epoch rejects new PAY  

## 3. Non-goals

- Full red-team / independent crypto audit  
- Production load soak / multi-DC WAN benches  
- Packet-level network emulator (in-process BFT kill is enough for v1 lab)  
- Changing consensus economics

## 4. Artifacts

| Artifact | Role |
|---|---|
| `crates/shard/tests/assurance_drills.rs` | Automated drills with predictable outcomes |
| `assurance_sim` | Runs latency sample + prints PASS checklist; exits non-zero if p50 gate fails |
| Design / plan docs | Traceability to SEEF §9 / §10.3 |

## 5. Success criteria

- All drills pass under `cargo test -p blockai-shard --test assurance_drills`  
- `assurance_sim --release --pays 50` prints `assurance_sim OK` with `p50_us=<10000`  

- Classical PAY path unchanged
