# BlockAI

Secure Economic Execution Fabric (SEEF) — from-scratch Rust L1 + agent micropayment authorization.

## Spec

- Design: `docs/superpowers/specs/2026-08-09-blockai-seef-design.md`
- Plan 1: `docs/superpowers/plans/2026-08-09-seef-authorization-core.md`
- Plan 2: `docs/superpowers/plans/2026-08-09-seef-checkpoints-witnesses.md`
- Plan 3: `docs/superpowers/plans/2026-08-09-seef-global-l1.md`

## Develop

```bash
cargo test
cargo run -p blockai-tools --bin pay_sim -- --pays 50
cargo run -p blockai-tools --bin checkpoint_sim -- --pays 2
cargo run -p blockai-tools --bin l1_sim -- --exposure 10
```
