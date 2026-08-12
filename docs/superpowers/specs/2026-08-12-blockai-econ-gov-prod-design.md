# BlockAI Economics, Governance & Production Seams (Plan 13) Design

**Date:** 2026-08-12  
**Status:** Closes remaining SEEF §10.2 deferred items that can ship in-lab

## 1. Goal

1. **Mainnet economics skeleton** — fees, treasury, epoch rewards, min stake  
2. **On-chain governance** — stake-weighted propose / vote / finalize / execute (CLI ops surface)  
3. **Production seams** — PCR-shaped hardware attestation, HSM ceremony transcript, dataplane backend selection

## 2. Non-goals

- Real TPM/PKCS#11/AF_XDP privileged drivers in CI  
- Full governance web UI / wallet dapp  
- Complex inflation schedules / validator set rotation productization  

## 3. Economics

| Field | Role |
|---|---|
| `min_stake` | Stake txs below this fail |
| `base_fee` | Deducted to `fee_treasury` on `ChargeBaseFee` |
| `proposal_bond` | Locked while proposal is open |
| `vote_quorum_bps` | Yes-stake / total-stake threshold (basis points) |
| `fee_treasury` | Fee sink (counted in locked sum for conservation) |

Rewards: `DistributeRewards` pays from `fee_treasury` → validator available (no silent mint).

## 4. Governance

`GovernanceAction`: `SetMinStake`, `SetBaseFee`, `SetVoteQuorumBps`, `TextNote`  
Flow: `Propose` → `Vote` (stake weight, one vote per account) → `FinalizeProposal` → apply params if passed.

## 5. Production seams

- Attestation: optional PCR digest list + quote nonce; hardware attestor includes them when measured  
- HSM: export `CeremonyTranscript` (share pubkeys + root commitment)  
- Dataplane: `select_backend` prefers AF_XDP/DPDK but falls back to userspace when probes fail  

## 6. Success criteria

- Fee charge + reward distribute conserve supply  
- Governance changes params only after quorum finalize  
- Unmeasured hardware attest still fails closed; measured includes PCRs  
- `econ_gov_sim` prints OK  
