# BlockAI / SEEF — Hardware BOM & Host Role Checklist

**Date:** 2026-08-12  
**Status:** Ops guide for leaving the software lab  
**Related:** SEEF design §5 / §7; Plans 9–13 production seams

This document answers: **what to buy**, **which process runs where**, and **which keys must never leave hardware**.

---

## 1. Bill of materials (minimum production shard)

Assumes **one local economic shard** (4 validators) + offline root + light ops. Scale witnesses / authority / L1 separately.

| Qty | Item | Spec notes | Role |
|---|---|---|---|
| 5 | PKCS#11 HSM (or 5 isolated slots) | FIPS-oriented; Ed25519 (or ECDSA P-256 + wrap) today; PQ path optional/dual | Offline root 3-of-5 |
| 2 | Ceremony laptops | Air-gapped capable; no shard NICs; USB-C / smartcard readers for HSMs | Root ceremony only |
| 4 | Shard validator servers | x86_64, ≥16 GB RAM, NVMe, **TPM 2.0**, UEFI Secure Boot | Local BFT `V1–V4` |
| 4 | Dataplane NICs | AF_XDP **or** DPDK PMD support (Intel E810 / Mellanox ConnectX-5+ class) | PAY ingress path |
| 1–2 | Capability authority hosts | TPM 2.0; no root HSM material; issuance keys only | Issue / refresh leases |
| 2–3 | Witness hosts | Separate operators preferred; TPM optional but recommended | Checkpoint countersign |
| 4 | L1 / global validators (can colocate later) | Same class as shard hosts; WAN-capable NICs | Settlement BFT |
| 1 | Ops jump / bastion | MFA; no payment keys | Deploy, metrics, `econ_gov_sim` |

**Networking:** metro switch fabric (or DC interconnect) sized for **sub‑5 ms** validator RTT. Prefer validators in **separate rack / power / NIC / host**; ideally span **≥2 metro sites** when RTT still fits.

**Consumables:** HSM backup / recovery cards, sealed evidence bags for ceremony transcripts, hardware security keys for ops MFA.

---

## 2. Host roles (what runs where)

```text
[Ceremony air-gap]     Soft ceremony UI / vendor tools → HSM shares only
        │ export CeremonyTranscript (pubkeys + commitment)
        ▼
[Authority]            issuance keys + attest verify → SpendCapability
        │
[Agent]  --QUIC 1-RTT-->  [Shard VIP / anycast]
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
                 V1 NIC    V2 NIC    V3/V4 NIC
                 AF_XDP/   AF_XDP/   AF_XDP/
                 DPDK      DPDK      DPDK
                    │         │         │
                    └──── local BFT ────┘
                              │ EdgeAccept
                              ▼
                         [Service edge]
                              │ checkpoints
                              ▼
                         [Witnesses] → [L1 validators]
```

| Host role | Binaries / services (lab names) | Must have | Must not have |
|---|---|---|---|
| **Ceremony** | Vendor PKCS#11 tools; export `CeremonyTranscript` | Offline root HSMs | Shard WAL, PAY paths, internet |
| **Authority** | Issuance service (today: `Authority` crate path) | Issuance key (HSM or sealed disk+TPM); attest policy | Root share private keys |
| **Shard validator** | Shard engine + dataplane (`pay` hot path; `dataplane_sim` → prod daemon) | TPM quote; XDP/DPDK NIC; WAL disk | Root HSM; foreign-shard authority |
| **Service edge** | Gateway signing EdgeAccept / receipts | Edge key (TPM-sealed or HSM) | Issuance / root |
| **Witness** | Countersign checkpoints | Witness key | PAY authorize |
| **L1 validator** | Global BFT + `GlobalState` apply | Settlement keys | Capability issuance |
| **Ops bastion** | `assurance_sim`, `econ_gov_sim`, metrics | Deploy creds | Any payment private keys |

Lab binaries that map to these roles: `pay_sim` / shard tests → shard; `l1_sim` → L1; `dataplane_sim` → NIC path; `pq_sim` / SoftHsm → replace with real HSM; `econ_gov_sim` → ops only.

---

## 3. Key custody checklist (never leave hardware)

| Key domain | Storage | May do | Must never |
|---|---|---|---|
| **Root shares (3-of-5)** | Offline HSM only | Authorize issuers, rotate root | Touch ordinary servers or CI |
| **Issuance** | Authority HSM or TPM-sealed | Mint/refresh `SpendCapability` | Settle L1; sign PAY |
| **Shard / edge** | TPM-sealed or local HSM | `EdgeAccept`, checkpoint seal | Issue capabilities; hold root |
| **Agent session** | Agent device TEE/OS keystore | Sign PAY | Issue / settle |
| **Service receipt** | Service HSM/TPM | Sign `ServiceReceipt` | Move balances alone |
| **Witness** | Witness HSM/TPM | Countersign roots | Authorize PAY |
| **Settlement / L1** | L1 validator HSM/TPM | Commit global txs | Issue spend leases |
| **QUIC/TLS** | Ephemeral / transport only | Authenticate sessions | Double as payment keys |

**Hard rules**

1. Root private material **never** on shard, authority, or CI hosts.  
2. Ceremony produces only **public** `CeremonyTranscript` (+ sealed recovery).  
3. PAY path uses **agent + edge** keys only; TLS keys are unrelated.  
4. Failed / missing TPM quote → **no new capability issuance** (fail closed).  
5. Set `BLOCKAI_AF_XDP` / `BLOCKAI_DPDK` only on hosts that actually bind those drivers.

---

## 4. Suggested phased buy

### Phase A — single-metro lab-prod (prove hardware seams)
- 4× TPM servers + XDP NICs (one shard)  
- 1× authority host with TPM  
- SoftHsm **or** 1 cheap HSM for issuance practice  
- Keep root as SoftHsm until ceremony rehearsed  

### Phase B — real root
- 5× PKCS#11 HSMs + 2 ceremony laptops  
- Rehearse 3-of-5 offline; archive `CeremonyTranscript`  
- Authority issuance key authorized by root op  

### Phase C — multi-metro + witnesses + L1
- Second site validators / witnesses  
- Dedicated L1 committee  
- Ops bastion + monitoring  

---

## 5. Acceptance checks (hardware-backed)

- [ ] TPM quote verifies against measured PCR policy on authority  
- [ ] Unmeasured host cannot obtain a capability  
- [ ] AF_XDP or DPDK path selected (`select_backend` ≠ Userspace) under load  
- [ ] Root op requires ≥3 distinct HSM share signatures  
- [ ] Ceremony transcript commitment matches live share pubkeys  
- [ ] Kill-one validator + WAL reboot still no double-spend  
- [ ] Release `assurance_sim --release` p50 still single-digit ms on metro NIC  

---

## 6. Explicit non-hardware (still software / process)

- Web governance UI / wallets  
- Regulated exchange custody certifications  
- IETF multipath-QUIC wire (app-level race is enough until then)  
- Vendor-specific PKCS#11 glue (integrate per chosen HSM)
