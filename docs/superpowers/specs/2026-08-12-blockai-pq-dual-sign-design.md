# BlockAI Full PQ Dual-Sign (Plan 10) Design

**Date:** 2026-08-12  
**Status:** Implementation slice for SEEF crypto-agility (hybrid Ed25519 + ML-DSA-65)

## 1. Goal

Expand hybrid dual-sign beyond spend capabilities to the long-lived / externally verifiable artifacts:

1. **PAY** agent authorizations  
2. **Shard checkpoints**  
3. **Witness countersignatures**  
4. **Edge acceptance + service receipts**  
5. **HSM root share signatures**

Local BFT vote digests remain **classical Ed25519 only** (latency hot path).

## 2. Non-goals

- PQ-only (ML-DSA without Ed25519) identity for agents or shards  
- Dual-signing every local BFT Prepare/Commit vote  
- Hardware PQ modules / vendor HSM PKCS#11  
- Changing economics, order book, or dataplane filters

## 3. Algorithm policy

| Artifact | Classical | PQ (when hybrid) | Default |
|---|---|---|---|
| SpendCapability | Ed25519 | ML-DSA-65 | hybrid (authority) |
| PAY | Ed25519 | ML-DSA-65 | classical (opt-in hybrid) |
| SignedCheckpoint | Ed25519 | ML-DSA-65 | classical (opt-in) |
| WitnessSig | Ed25519 | ML-DSA-65 | classical (opt-in) |
| EdgeAcceptance / ServiceReceipt | Ed25519 | ML-DSA-65 | classical (opt-in) |
| HSM ShareSig | Ed25519 | ML-DSA-65 | classical (opt-in) |
| Local BFT votes | Ed25519 | — | classical only |

`AlgorithmId::HybridEd25519MlDsa65 = 3`. Missing PQ material when alg is hybrid → reject (fail closed).

## 4. Domain separation

Classical bodies keep existing domains (`PAY`, `CHECKPOINT`, `WITNESS_CHECKPOINT`, `EDGE_ACCEPT`, `SERVICE_RECEIPT`, `HSM_ROOT_OP`) so classical verify stays hot-path compatible.

PQ signatures bind a `*_HYBRID` body that includes the classical pubkey + PQ pubkey + the same semantic fields (CBOR), matching capability hybrid.

## 5. Wire fields

Optional, `#[serde(default)]`:

- `Pay`: `agent_alg`, `agent_pq_pubkey`, `agent_pq_signature`
- `SignedCheckpoint`: `shard_pq_pubkey`, `shard_pq_signature`
- `WitnessSig`: `witness_pq_pubkey`, `witness_pq_signature`
- `EdgeAcceptance` / `ServiceReceipt`: `*_pq_pubkey`, `*_pq_signature`
- `ShareSig`: `pq_pubkey`, `pq_signature`

## 6. Success criteria

- Hybrid PAY rejects if either half fails  
- Classical PAY/checkpoint paths unchanged when PQ fields empty  
- Witness / receipt / HSM hybrid seal+verify roundtrips  
- `pq_sim` prints a single OK line  
- Local BFT tests still pass without PQ on votes
