# BlockAI SEEF Design

**Date:** 2026-08-09  
**Status:** Draft for review  
**Codename:** Secure Economic Execution Fabric (SEEF)  
**Repo:** BlockAI (from-scratch Rust L1; no chain forks)

## 1. Purpose

Build a next-generation, AI-agent-native blockchain and payment fabric where:

- AI agents are first-class accounts.
- Micropayments authorize in local milliseconds.
- Smart contracts, staking, registry, and reputation settle on a global DAG+BFT L1.
- Compromise of agents, edges, networks, databases, operators, or regions cannot create *unlimited* loss.

**Core thesis:** Agents never wait on WAN BFT for pay-per-call. Authorization is local and cryptographically bounded; global consensus finalizes aggregates asynchronously.

**Security thesis:** Do not merely “secure the fast path.” Make damage **mathematically bounded** even when parts of the system are fully compromised.

Primary invariant:

> Accepted Spend ≤ Cryptographically Issued Spend Authority

## 2. Goals and non-goals

### 2.1 Goals

- From-scratch Rust implementation (no Bitcoin/Ethereum/Substrate/Linera/Sui forks; standard crates allowed).
- Full product vision path: agent accounts, native payments/staking, WASM contracts, micropayment authorization, agent registry, reputation.
- Local authorization targets (engineering targets, not marketing WAN guarantees):
  - Tier 0 same machine: ~50–500 µs
  - Tier 1 same DC/metro: &lt;1–5 ms
  - Tier 2 regional: ~5–20 ms
  - Tier 3 global settlement: async (~0.3–1 s aggregates)
- Partitioned spend authority, short-lived capabilities, local BFT shards, witnessed checkpoints, crypto-agility (classical + PQ where appropriate).

### 2.2 Non-goals

- Per-inference global L1 transaction.
- Single ultra-fast edge server as sole authorization authority.
- QUIC 0-RTT for irreversible payments.
- Replicating full account balances to every region.
- Claiming absolute “10/10” security for a live distributed financial system.
- Running LLM inference inside the L1 WASM VM.

## 3. Architecture overview

```text
OFFLINE ROOT (HSM 3-of-5, classical+PQ)
        │ threshold issue
        ▼
CAPABILITY AUTHORITY  →  shard-bound, short-lived, partitioned allowances
        │
   FRA €20   AMS €20   PAR €10   Reserve €50
        │
        ▼
LOCAL ECONOMIC SHARD (4 validators, metro fault domains)
  verify → LOCAL QUORUM COMMIT (consume seq) → authorize → service
        │ ~1–5 ms engineering target
        ▼
3-party receipts (Agent + Edge + Service)
        │ Merkle checkpoint (time / count / exposure)
        ▼
Independent witnesses → Global DAG+BFT settlement
        │
        ▼
WASM contracts · agent registry · reputation · staking · disputes
```

### 3.1 Planes

| Plane | Job | Latency class |
|---|---|---|
| Authorization | PAY against shard-bound capabilities | Local ms |
| Regional (optional later) | FastPay-class top-ups / cross-shard transfers | Sub-second |
| Global settlement L1 | Checkpoints, contracts, registry, reputation, stake | WAN sub-second to ~1s |

### 3.2 Why not DAG-only for micropayments

Research comparison (Mysticeti/Autobahn/FastPay/channels/Linera/MegaETH):

- Geo-distributed DAG+BFT commit is typically hundreds of ms WAN; physics bounds multi-message BFT.
- True millisecond pay-per-call needs **removing global round trips**, not faster fiber alone.
- Best pattern: pre-authorized local spend + async global settlement.
- SEEF keeps BFT, but makes hot-path BFT **local**.

## 4. Capability and PAY protocol

### 4.1 SpendCapability

Issued by Capability Authority; classical + PQ signatures on issuance artifacts.

Fields:

- `capability_id`, `account_id`, `agent_id`
- `shard_id` — exactly one local shard may accept
- `epoch` — fencing generation
- `currency`, `maximum_total`, `maximum_per_call`
- `service_scope[]`, `policy_hash`
- `sequence_start` .. `sequence_end`
- `valid_from`, `valid_until`
- `region`, `delegation=false`
- `issuer_signature(s)`

### 4.2 PAY

Agent → home local shard; CBOR + COSE; classical signature on hot path.

Fields:

- `capability_id`, `epoch`, `sequence`
- `agent_id`, `service_id`
- `amount`, `currency`
- `request_hash` — binds model/request/destination/pricing_version/etc.
- `price_quote_hash`, `max_amount`, `pricing_schedule_version`
- `expiry`
- `agent_signature`

**TX_ID** = `(capability_id, epoch, sequence, request_hash)` — one-time consumable.

### 4.3 Partitioned authority

Never replicate full account balance to all regions.

Example: €100 total → FRA €20, AMS €20, PAR €10, Reserve €50.  
Under full partition, max accepted spend from issued shards = €50, not €300.

### 4.4 Epoch fencing

On failover, successor shard receives `epoch+1`; prior epoch becomes `FENCED`.

| Epoch state | Accept PAY? | Enter future checkpoints? |
|---|---|---|
| ACTIVE | yes | yes |
| FENCED | no | no (except controlled drain policy for already-committed local log) |
| EXPIRED | no | no |

Residual risk after fence = leftover allowance on the fenced epoch only.

### 4.5 Hot-path state machine

```text
recv PAY
 → cheap filters (version / shard / rate-limit)
 → verify agent sig + capability
   (shard match, epoch ACTIVE, time, scope, amount ≤ per_call & remaining)
 → LOCAL BFT COMMIT: mark (cap, epoch, seq) CONSUMED + decrement remaining
 → emit EdgeAccept(A, commit_index)          # only after durable quorum
 → service executes under max_amount
 → ServiceReceipt(E, execution_hash, actual_amount ≤ max)
 → append to shard log → Merkle checkpoint → witnesses → global settle
```

**Hard rule:** never execute before local quorum consume.

### 4.6 Three-party receipts

1. **A** = SignAgent(PAY)
2. **E** = SignEdge(A ‖ commit_index)
3. **S** = SignService(E ‖ execution_hash ‖ actual_amount)

Final TX proof = `{A, E, S}` plus Merkle path at checkpoint.

### 4.7 Transport

- Persistent QUIC to home shard (anycast → metro).
- PAY on authenticated session / reliable stream semantics; DATAGRAM only with explicit app ack strategy.
- **No QUIC 0-RTT for PAY / TRANSFER / PURCHASE / WITHDRAW.**
- 0-RTT allowed only for idempotent reads (price, capability status, metadata).

### 4.8 Protocol invariants

1. Σ accepted spend ≤ issued allowance for `(account, shard, epoch)`
2. Each TX_ID accepted ≤ once
3. Spend matches agent, service, amount, region, epoch, expiry, `policy_hash`
4. Capability usable only on `shard_id`
5. Commit-before-exec

## 5. Local economic shard

### 5.1 Topology

- 4 validators per shard (`V1–V4`), quorum 3-of-4.
- Separate rack / power / NIC / host; prefer ≥2 metro DCs when RTT budget allows.
- Shard identity e.g. `FRA-004`; anycast VIP fronts the shard.

Tolerance note: 3-of-4 is an engineering choice for small committees; safety/liveness assumptions must be documented and tested. Future sizing may move to classic `3f+1` if required.

### 5.2 Consensus scope

Local BFT orders only:

- PAY consume / allowance decrement
- capability activate / fence ack
- receipt commit index assignment
- checkpoint seal

Not in local consensus: WASM contracts, global registry, cross-shard transfers, root operations.

v1 algorithm direction: single-leader partially synchronous BFT optimized for LAN/metro (HotStuff-class). Benchmark required.

### 5.3 Persistence

Each validator maintains WAL / replicated log for:

- `(cap_id, epoch, seq) CONSUMED`
- `remaining_allowance`
- `commit_index`

Quorum commit means durable on ≥ quorum members **before** `EdgeAccept`.  
Restart replays WAL; consumed sequences never reopen.  
Client retry of same TX_ID is idempotent (`ALREADY_COMMITTED` or `REPLAY_REJECT` with prior receipt if available).

### 5.4 Dataplane

```text
NIC → (optional AF_XDP/DPDK) → cheap filters → rate limit
    → capability cache → sig verify → propose to local BFT
    → on commit: EdgeAccept → service gateway
```

- Encoding: CBOR + COSE.
- Expensive crypto after cheap rejects.
- Caches: active capabilities, consumed sequence bitmaps, policies by `policy_hash`.

### 5.5 Attestation and keys

- Secure Boot → measured runtime → RATS-style attestation → issuance/refresh only if evidence matches approved binary/config/version/hardware.
- Edge identity keys hardware-bound with short rotation.
- No / failed attestation → no new capabilities (fail closed for issuance).

### 5.6 Checkpointing and witnesses

Seal when first of: wall-clock window (e.g. 100 ms), N transactions, or exposure cap.

```text
shard log → Merkle root R → SignShard(R)
         → Witnesses W1..Wn countersign
         → async Global Settlement
```

History rewrite requires compromising shard quorum **and** witnesses, and still cannot exceed issued ceilings.

### 5.7 Failure modes

| Event | Behavior |
|---|---|
| 1 validator down | continue with quorum |
| Leader fail | view-change; brief latency bump |
| Shard majority lost | stop authorizing; fence epoch; Authority may issue successor epoch elsewhere |
| Partition from global | serve remaining local allowance only; no silent top-up |
| Attestation break | drain/expire; no refresh |

## 6. Global settlement L1

### 6.1 Responsibilities

- Stake / slash global validators
- Ingest witnessed Merkle checkpoints
- Finalize aggregate settlements
- WASM smart contracts
- Agent registry and reputation
- Dispute resolution
- Risk-tiered high-value operations

### 6.2 Consensus and data path

```text
Shard checkpoints + settlement txs
  → DAG dissemination
  → BFT commit over DAG anchors
  → deterministic state transition
```

Inspired by Mysticeti/Autobahn-class designs; implemented from scratch in Rust.

### 6.3 State model

- Accounts: `Human | Agent | Contract`
- Agent fields: keys/DID, stake, reputation, status, capability metadata
- Canonical global balances; shard allowances are escrowed projections
- Checkpoint objects: `{shard_id, epoch, root, shard_sig, witness_sigs[], tx_count, exposure}`

Conservation:

`global_available + Σ shard_outstanding_allowances + Σ locked = total_supply`

### 6.4 Settlement pipeline

1. Shard seals root `R` (+ witnesses)
2. Post `CheckpointFinalized`
3. L1 verifies signatures, fencing, no conflicting finalized roots
4. Apply net deltas from proven aggregates
5. Emit audit/indexer events

Disputes on conflicting roots → slashing path; capability ceilings still bound loss.

### 6.5 WASM

- Metered WASM runtime (`wasmtime`/`wasmi`)
- No on-chain LLM inference
- System modules: registry, reputation, staking, dispute, capability-authority governance hooks
- User contracts act on settled balances and registry identity

### 6.6 Registry and reputation

- Registry maps agent id → keys, controllers, metadata, suspension
- Reputation from finalized receipt outcomes, stake, slashing
- Local PAY may use signed reputation snapshots; not a global read per call
- Suspension stops new issuance; residual = unexpired local allowance

### 6.7 Optional regional middle tier

FastPay-style consistent broadcast for cross-shard transfers and capability top-ups after shard+L1 exist. Not the per-call path.

## 7. Identity, keys, and risk tiers

### 7.1 Identity layers

```text
ROOT IDENTITY (hardware-backed, non-extractable)
   → AGENT SESSION KEY (minutes, scoped)
      → SpendCapability lease (seconds–minutes, shard-bound)
```

### 7.2 Key domains

| Domain | May do | Must not do |
|---|---|---|
| Root (offline HSM 3-of-5 + PQ) | Rotate roots, authorize issuers | Touch ordinary servers |
| Issuance | Mint/refresh shard capabilities | Settle / run edges |
| Edge / shard | Sign EdgeAccept, checkpoints | Issue capabilities |
| Agent session | Sign PAY | Issue caps / settle |
| Service receipt | Sign execution receipts | Move balances alone |
| Settlement | Commit L1 / apply checkpoints | Issue spend leases |
| Audit / witness | Countersign roots | Authorize PAY |

QUIC/TLS transport keys never double as payment keys.

### 7.3 Risk-adaptive authorization

Illustrative parameters (tunable):

| Exposure | Path |
|---|---|
| €0.000001–€0.01 | Local shard |
| €0.01–€10 | Local shard + signed policy engine |
| €10–€1,000 | Regional confirmation |
| €1k–€50k | Multi-region quorum |
| €50k+ | Strong global authorization |
| Major treasury | Human multi-party + HSM threshold + delay |

Adaptive issuance shrinks leases under anomaly; cryptographic ceilings remain the primary boundary.

### 7.4 Crypto-agility

- Envelope algorithm IDs
- Hot path: fast classical signatures
- Capabilities, checkpoints, root: classical **and** PQ (ML-DSA / SLH-DSA class; exact suite chosen in implementation plan)
- Algorithms replaceable without rewriting the economic state machine

### 7.5 Zero trust

Assume agent, network, edge, service, database, operator, and cloud may be malicious. Every economic action requires the correct proof from the correct key domain under the correct `policy_hash`.

## 8. Formal invariants

1. **Conservation** — Σ accepted spend ≤ issued allowance
2. **Uniqueness** — TX_ID accepted ≤ once
3. **Authority** — valid agent/capability proof required
4. **Scope** — agent/service/amount/region/purpose/epoch/expiry/policy match
5. **Isolation** — no cross-shard capability spend
6. **Bounded compromise** — loss ≤ outstanding compromised authority
7. **Non-retroactivity** — witnessed checkpoints not silently rewritten
8. **Key separation** — no single ops key mints unbounded authority
9. **Fail-safe** — high-risk ambiguity fail-closed; partition fail-bounded
10. **Verifiability** — settled payment has proof path Agent→Edge→Service→Merkle→L1

## 9. Testing and assurance

Pre-production program:

Threat model → property tests → fuzzing → fault injection → Byzantine simulation → clock/replay/partition/rollback → key-compromise drills → red team → independent crypto audit.

Mandatory scenarios:

- Kill validator mid-commit
- Reboot entire shard
- Duplicate / reorder / delay packets
- Forge timestamps
- Clone agent state
- Steal agent key / one edge key / DB credentials
- Partition region from world
- Compromise 1–2 validators
- Roll state backwards
- Issue conflicting epochs
- Reuse expired capability

Every scenario must have a mathematically predictable outcome.

## 10. v1 scope

### 10.1 In scope

- CBOR+COSE PAY + SpendCapability types
- Local 4-node shard BFT (LAN) + WAL commit-before-exec
- Capability authority (software HSM mode; real HSM interface stubs)
- Shard-bound short leases + epoch fencing
- Three-party receipts + Merkle checkpoints
- Witness co-signing
- Global L1: DAG mempool + BFT commit, accounts, staking skeleton
- Checkpoint verification + aggregate apply
- WASM loader + system stubs (registry, reputation, dispute hooks)
- Persistent QUIC dataplane for PAY (no 0-RTT PAY)
- Invariant property tests + partition/replay suites

### 10.2 Deferred (designed-in)

- AF_XDP/DPDK production dataplane
- Multipath QUIC / packet racing
- Full PQ dual-sign everywhere (land agility + at least one PQ path first)
- Production HSM 3-of-5 operations
- Enforced hardware attestation (stub → real)
- FastPay regional middle tier
- Mainnet economics / governance UI

### 10.3 v1 success criteria

- Same-DC PAY authorize p50 in single-digit ms under low lab load (benchmarked)
- Double-spend / replay / cross-shard tests fail closed
- Kill-one-validator: no duplicate execution; no unbounded mint
- Checkpoint → L1 apply conserves supply
- Agent key theft simulation: loss ≤ remaining lease

## 11. Planned repository layout

```text
blockai/
  crates/
    types/
    proto/
    crypto/
    shard/
    authority/
    witness/
    consensus/
    execute/
    node/
    tools/
  docs/superpowers/specs/
```

## 12. Decisions log

| Decision | Choice | Rationale |
|---|---|---|
| Implementation language | Rust | Safety + performance for L1 and dataplane |
| Chain lineage | From scratch, no forks | User requirement |
| Hot path | Local 4-node BFT shard | ms auth without WAN BFT |
| Spend model | Partitioned shard-bound capabilities | Bound double-spend under partition |
| Micropay transport | Persistent QUIC; no 0-RTT PAY | Avoid 0-RTT replay class |
| Global consensus | DAG + BFT | Throughput for aggregates/shared state |
| Contracts | WASM | Portable metered contracts without AI-in-VM |
| Security model | Bounded compromise / zero trust | Assume components fail |

## 13. Open parameters (implementation plan)

These are intentional tunables, not unresolved design blockers:

- Exact local BFT algorithm and timeout profile
- Checkpoint triggers (ms / N / exposure)
- Risk-tier currency thresholds
- PQ suite selection and hybrid mode
- Quorum size evolution beyond 3-of-4
- Witness set size and incentive model
- Genesis validator and authority bootstrap ceremony

## 14. Next step

After human review/approval of this spec, create an implementation plan via the writing-plans workflow. No production implementation begins before that plan exists.
