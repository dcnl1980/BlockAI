# BlockAI

Secure Economic Execution Fabric (SEEF) — from-scratch Rust L1 + agent micropayment authorization.

## Spec

- Design: `docs/superpowers/specs/2026-08-09-blockai-seef-design.md`
- Plan 1: `docs/superpowers/plans/2026-08-09-seef-authorization-core.md`
- Plan 2: `docs/superpowers/plans/2026-08-09-seef-checkpoints-witnesses.md`

## Develop

```bash
cargo test
cargo run -p blockai-tools --bin pay_sim -- --pays 50
cargo run -p blockai-tools --bin checkpoint_sim -- --pays 2
```
