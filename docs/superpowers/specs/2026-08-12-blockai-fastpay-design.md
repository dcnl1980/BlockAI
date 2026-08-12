# BlockAI FastPay Regional Middle Tier (Plan 12) Design

**Date:** 2026-08-12  
**Status:** Implementation slice for SEEF §6.7 / §10.2 FastPay middle tier

## 1. Goal

Add a **regional FastPay-class plane** for:

1. **Cross-shard allowance reallocation** (unused authority float FRA → AMS)  
2. **Capability top-ups** (increase live shard `remaining` from unused shard allowance)

This is **not** the per-call PAY path. Latency class: sub-second lab / consistent broadcast.

## 2. Non-goals

- Replacing local shard BFT for PAY  
- Silent top-ups during partition (still fail-closed without certificate)  
- Full asynchronous WAN committee with timeouts/retries productization  
- 0-RTT transfer frames (certs ride authenticated paths only)

## 3. Architecture

```text
Client proposes RegionalOp
    → RegionalCommittee (4 authorities, quorum 3) signs digest
    → RegionalCertificate
         ├─ Reallocate → Authority.reallocate + optional L1 ReallocateShardOutstanding
         └─ TopUpCapability → Authority.debit shard allowance + Shard.top_up (+ WAL)
```

Domain-separated CBOR body: `REGIONAL_OP`.

## 4. Invariants

- Reallocate conserves authority float (`from + to` unchanged total for account)  
- L1 reallocate conserves `shard_outstanding` sum / total supply  
- Top-up cannot exceed authority shard allowance  
- PAY still shard-bound; foreign shard remains `WrongShard`  
- Duplicate nonce on committee reject (lab: track consumed nonces)

## 5. Success criteria

- 3-of-4 certificate required; 2 shares fail  
- Reallocate enables issuance on destination shard  
- Top-up increases `ShardEngine::remaining` and survives WAL replay  
- `fastpay_sim` prints a single OK line  
- Conservation holds after L1 reallocate
