# SEEF Plan 9 — Production Dataplane Foundations

**Goal:** AF_XDP/DPDK interfaces + userspace pipeline, multipath QUIC race, hardware attest trait, HSM 3-of-5 soft root.

**Spec:** `docs/superpowers/specs/2026-08-12-blockai-prod-dataplane-design.md`

## Tasks
1. `blockai-dataplane` crate
2. Multipath race in `blockai-net`
3. Attestor trait + hardware stub
4. `blockai-hsm` SoftHsm3of5
5. Tests + `dataplane_sim` + README
