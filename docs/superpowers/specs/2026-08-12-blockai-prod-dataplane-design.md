# BlockAI Production Dataplane Foundations (Plan 9) Design

**Date:** 2026-08-12  
**Status:** Implementation slice for deferred SEEF §10.2 items (interfaces + lab backends)

## 1. Goal

Land **production-shaped interfaces** for:

1. NIC dataplane path (AF_XDP / DPDK) with a **userspace fallback** that exercises the same filter pipeline  
2. **Multipath QUIC racing** (first successful 1-RTT path wins; still no 0-RTT PAY)  
3. **Hardware attestation** trait (TPM/RATS-shaped) with fail-closed default when evidence is missing  
4. **HSM 3-of-5** threshold root signing (software multi-key quorum simulating offline root)

## 2. Non-goals (this plan)

- Binding real AF_XDP sockets or DPDK PMD in CI (no privileged NIC)  
- Full IETF multipath-QUIC wire format (application-level dual-path race is enough for v1 lab)  
- Vendor HSM PKCS#11 drivers  
- Changing local BFT or L1 economics

## 3. Architecture

```text
NIC / UserspaceIngress
    → cheap filters (size, rate limit)
    → capability cache hit/miss
    → (optional) hand off to shard authorize
QUIC client: race path A || path B → first 1-RTT ready wins
Attestor::collect() → verify_evidence(policy) fail-closed
SoftHsm3of5: 3-of-5 Keypair shares countersign RootOp digests
```

## 4. Crates

| Crate | Role |
|---|---|
| `blockai-dataplane` | Ingress traits, pipeline, userspace + AF_XDP/DPDK stubs |
| `blockai-net` | `multipath` race helper |
| `blockai-attest` | `Attestor` trait + `HardwareAttestor` stub |
| `blockai-hsm` | Soft 3-of-5 threshold root signer |

## 5. Success criteria

- Pipeline rejects oversized / rate-limited frames before “crypto stage”  
- Multipath race returns a connected path; PAY still rejected on early data  
- Hardware attestor without measured boot evidence fails closed  
- Root op requires ≥3 of 5 HSM share signatures  
- `dataplane_sim` prints a single OK line
