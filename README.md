# BlockAI

Secure Economic Execution Fabric (SEEF) — from-scratch Rust L1 + agent micropayment authorization.

## Spec

- Design: `docs/superpowers/specs/2026-08-09-blockai-seef-design.md`
- Plan 1: `docs/superpowers/plans/2026-08-09-seef-authorization-core.md`
- Plan 2: `docs/superpowers/plans/2026-08-09-seef-checkpoints-witnesses.md`
- Plan 3: `docs/superpowers/plans/2026-08-09-seef-global-l1.md`
- Plan 4: `docs/superpowers/plans/2026-08-09-seef-wasm-registry.md`
- Plan 5: `docs/superpowers/plans/2026-08-09-seef-quic-attest-pq.md`
- Plan 6: `docs/superpowers/plans/2026-08-12-seef-tokenized-assets.md` (tokenized assets + spot trade)
- Plan 7: `docs/superpowers/plans/2026-08-12-seef-order-book.md` (limit order book)
- Plan 8: `docs/superpowers/plans/2026-08-12-seef-asset-compliance.md` (freeze + allowlist)
- Plan 9: `docs/superpowers/plans/2026-08-12-seef-prod-dataplane.md` (AF_XDP/DPDK interfaces, multipath, HSM)
- Assets design: `docs/superpowers/specs/2026-08-12-blockai-tokenized-assets-design.md`
- Order book design: `docs/superpowers/specs/2026-08-12-blockai-order-book-design.md`
- Compliance design: `docs/superpowers/specs/2026-08-12-blockai-asset-compliance-design.md`
- Prod dataplane design: `docs/superpowers/specs/2026-08-12-blockai-prod-dataplane-design.md`

## Develop

```bash
cargo test
cargo run -p blockai-tools --bin pay_sim -- --pays 50
cargo run -p blockai-tools --bin checkpoint_sim -- --pays 2
cargo run -p blockai-tools --bin l1_sim -- --exposure 10
cargo run -p blockai-tools --bin wasm_sim -- --a 2 --b 40
cargo run -p blockai-tools --bin quic_sim -- --amount 1
cargo run -p blockai-tools --bin seef_sim -- --amount 10
cargo run -p blockai-tools --bin trade_sim -- --symbol ACME --mint 100 --units 10 --price 2500
cargo run -p blockai-tools --bin dataplane_sim
cargo run -p blockai-tools --bin dataplane_sim -- --measured-hw
cargo bench -p blockai-shard --bench pay_authorize
```
