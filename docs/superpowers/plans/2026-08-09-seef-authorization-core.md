# SEEF Authorization Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a working Rust vertical slice where an agent PAY is authorized by a shard-bound capability through local 3-of-4 commit-before-exec, with partitioned allowances and fail-closed replay/cross-shard checks.

**Architecture:** Cargo workspace implementing SEEF Plan 1 only: `types` + `crypto` + `authority` + `shard` + `tools` harness. No global L1, no QUIC production dataplane, no WASM yet. In-process 4-validator local BFT over a narrow `PayCommit` log; durable WAL before `EdgeAccept`.

**Tech Stack:** Rust 2021, Cargo workspace, `ed25519-dalek`, `blake3`, `serde`/`ciborium` (CBOR), `tokio` (async test harness), `proptest`/`tempfile`, `anyhow`/`thiserror`.

**Spec:** `docs/superpowers/specs/2026-08-09-blockai-seef-design.md` §§4–5, §8, §10.1 (authorization subset)

## Companion plans (not this document)

| Plan | Scope |
|---|---|
| **Plan 1 (this)** | Types, crypto, capability authority, local shard BFT, PAY authorize |
| Plan 2 | Merkle checkpoints, witnesses, three-party receipt persistence |
| Plan 3 | Global DAG+BFT node, accounts, staking skeleton, checkpoint apply |
| Plan 4 | WASM loader, registry/reputation/dispute stubs |
| Plan 5 | QUIC dataplane, attestation stubs, PQ agility hooks, benches |

## Global Constraints

- From-scratch BlockAI protocol code; no chain forks (Bitcoin/Ethereum/Substrate/Linera/Sui).
- Standard crates allowed.
- No QUIC 0-RTT for PAY.
- Commit-before-exec is mandatory.
- Capabilities are shard-bound; never replicate full account balance to all shards.
- TDD: failing test → implement → pass → commit each task.
- Amounts in Plan 1 use integer **micros** (`u128`) of a single currency code `"EURC"` (no floats).

## File map

```text
Cargo.toml                          # workspace
crates/types/Cargo.toml
crates/types/src/lib.rs
crates/types/src/ids.rs
crates/types/src/capability.rs
crates/types/src/pay.rs
crates/types/src/errors.rs
crates/crypto/Cargo.toml
crates/crypto/src/lib.rs
crates/crypto/src/keys.rs
crates/crypto/src/sign.rs
crates/authority/Cargo.toml
crates/authority/src/lib.rs
crates/authority/src/issuer.rs
crates/shard/Cargo.toml
crates/shard/src/lib.rs
crates/shard/src/state.rs
crates/shard/src/wal.rs
crates/shard/src/bft.rs
crates/shard/src/engine.rs
crates/tools/Cargo.toml
crates/tools/src/bin/pay_sim.rs
```

---

### Task 1: Workspace and core ID/amount types

**Files:**
- Create: `Cargo.toml`
- Create: `crates/types/Cargo.toml`
- Create: `crates/types/src/lib.rs`
- Create: `crates/types/src/ids.rs`
- Create: `crates/types/src/errors.rs`
- Create: `README.md` (minimal workspace pointer only if needed for `cargo test` docs — prefer updating existing `# BlockAI` README with build commands)
- Test: `crates/types/tests/ids_roundtrip.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `ShardId(pub String)`
  - `AccountId([u8; 32])`
  - `AgentId([u8; 32])`
  - `CapabilityId([u8; 32])`
  - `Epoch(pub u64)`
  - `Sequence(pub u64)`
  - `AmountMicros(pub u128)`
  - `CurrencyCode` newtype `&'static str` validation helper `fn parse_currency(s: &str) -> Result<String, TypesError>` accepting `"EURC"` only in v1
  - `TypesError` enum

- [ ] **Step 1: Write the failing test**

Create `crates/types/tests/ids_roundtrip.rs`:

```rust
use blockai_types::{AmountMicros, Epoch, Sequence, ShardId};

#[test]
fn amount_micros_display_and_eq() {
    let a = AmountMicros(1_000_000);
    assert_eq!(a.0, 1_000_000);
    assert_eq!(a, AmountMicros(1_000_000));
}

#[test]
fn shard_id_rejects_empty() {
    assert!(ShardId::new("FRA-004").is_ok());
    assert!(ShardId::new("").is_err());
}

#[test]
fn epoch_and_sequence_order() {
    assert!(Epoch(1) < Epoch(2));
    assert!(Sequence(10) < Sequence(11));
}
```

- [ ] **Step 2: Create workspace manifests (types crate empty lib)**

Root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/types",
]

[workspace.package]
edition = "2021"
license = "Apache-2.0"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

`crates/types/Cargo.toml`:

```toml
[package]
name = "blockai-types"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
```

`crates/types/src/lib.rs`:

```rust
pub mod errors;
pub mod ids;

pub use errors::TypesError;
pub use ids::*;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p blockai-types --test ids_roundtrip`

Expected: FAIL (missing types / compile errors)

- [ ] **Step 4: Implement IDs**

`crates/types/src/errors.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TypesError {
    #[error("empty shard id")]
    EmptyShardId,
    #[error("unsupported currency: {0}")]
    UnsupportedCurrency(String),
}
```

`crates/types/src/ids.rs`:

```rust
use crate::TypesError;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ShardId(String);

impl ShardId {
    pub fn new(s: impl Into<String>) -> Result<Self, TypesError> {
        let s = s.into();
        if s.is_empty() {
            return Err(TypesError::EmptyShardId);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Epoch(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Sequence(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AmountMicros(pub u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub [u8; 32]);

pub fn parse_currency(s: &str) -> Result<String, TypesError> {
    if s == "EURC" {
        Ok(s.to_string())
    } else {
        Err(TypesError::UnsupportedCurrency(s.to_string()))
    }
}

impl fmt::Display for AmountMicros {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}µ", self.0)
    }
}
```

Update root `README.md` to:

```markdown
# BlockAI

Secure Economic Execution Fabric (SEEF) — from-scratch Rust L1 + agent micropayment authorization.

## Spec

- Design: `docs/superpowers/specs/2026-08-09-blockai-seef-design.md`
- Plan 1: `docs/superpowers/plans/2026-08-09-seef-authorization-core.md`

## Develop

```bash
cargo test
```
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p blockai-types`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/types README.md
git commit -m "feat(types): add workspace and core SEEF id types"
```

---

### Task 2: SpendCapability, PAY, and TX_ID

**Files:**
- Create: `crates/types/src/capability.rs`
- Create: `crates/types/src/pay.rs`
- Modify: `crates/types/src/lib.rs`
- Modify: `crates/types/Cargo.toml` (add `ciborium`, `blake3`)
- Test: `crates/types/tests/pay_txid.rs`

**Interfaces:**
- Consumes: ID types from Task 1
- Produces:
  - `SpendCapability { .. }`
  - `Pay { .. }`
  - `fn tx_id(pay: &Pay) -> [u8; 32]`
  - `fn encode_cbor<T: Serialize>(v: &T) -> Result<Vec<u8>, TypesError>`
  - `EpochState { Active, Fenced, Expired }`

- [ ] **Step 1: Write the failing test**

```rust
use blockai_types::{
    AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence, ShardId,
    SpendCapability, tx_id,
};

fn sample_cap() -> SpendCapability {
    SpendCapability {
        capability_id: CapabilityId([1u8; 32]),
        account_id: AccountId([2u8; 32]),
        agent_id: AgentId([3u8; 32]),
        shard_id: ShardId::new("FRA-004").unwrap(),
        epoch: Epoch(1),
        currency: "EURC".into(),
        maximum_total: AmountMicros(20_000_000),
        maximum_per_call: AmountMicros(10_000),
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(100),
        sequence_end: Sequence(200),
        valid_from_unix_ms: 0,
        valid_until_unix_ms: 9_999_999_999,
        region: "EU".into(),
        issuer_pubkey: [7u8; 32],
        issuer_signature: vec![0u8; 64],
    }
}

#[test]
fn tx_id_changes_when_sequence_changes() {
    let cap = sample_cap();
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(100),
        agent_id: cap.agent_id,
        service_id: "inference/supernova".into(),
        amount: AmountMicros(1000),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(4000),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![1u8; 64],
    };
    let a = tx_id(&pay);
    pay.sequence = Sequence(101);
    let b = tx_id(&pay);
    assert_ne!(a, b);
}

#[test]
fn tx_id_includes_request_hash() {
    let cap = sample_cap();
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(100),
        agent_id: cap.agent_id,
        service_id: "inference/supernova".into(),
        amount: AmountMicros(1000),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(4000),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![1u8; 64],
    };
    let a = tx_id(&pay);
    pay.request_hash = [8u8; 32];
    assert_ne!(a, tx_id(&pay));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockai-types --test pay_txid`

Expected: FAIL (missing `Pay` / `tx_id`)

- [ ] **Step 3: Implement capability and pay modules**

Add to `crates/types/Cargo.toml`:

```toml
blake3 = "1"
ciborium = "0.2"
```

`crates/types/src/capability.rs`:

```rust
use crate::{AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Sequence, ShardId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendCapability {
    pub capability_id: CapabilityId,
    pub account_id: AccountId,
    pub agent_id: AgentId,
    pub shard_id: ShardId,
    pub epoch: Epoch,
    pub currency: String,
    pub maximum_total: AmountMicros,
    pub maximum_per_call: AmountMicros,
    pub service_scope: Vec<String>,
    pub policy_hash: [u8; 32],
    pub sequence_start: Sequence,
    pub sequence_end: Sequence,
    pub valid_from_unix_ms: u64,
    pub valid_until_unix_ms: u64,
    pub region: String,
    pub issuer_pubkey: [u8; 32],
    pub issuer_signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpochState {
    Active,
    Fenced,
    Expired,
}
```

`crates/types/src/pay.rs`:

```rust
use crate::{
    AgentId, AmountMicros, CapabilityId, Epoch, Sequence, TypesError,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pay {
    pub capability_id: CapabilityId,
    pub epoch: Epoch,
    pub sequence: Sequence,
    pub agent_id: AgentId,
    pub service_id: String,
    pub amount: AmountMicros,
    pub currency: String,
    pub request_hash: [u8; 32],
    pub price_quote_hash: [u8; 32],
    pub max_amount: AmountMicros,
    pub pricing_schedule_version: u64,
    pub expiry_unix_ms: u64,
    pub agent_signature: Vec<u8>,
}

/// Canonical TX_ID = BLAKE3(capability_id || epoch || sequence || request_hash)
pub fn tx_id(pay: &Pay) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(&pay.capability_id.0);
    h.update(&pay.epoch.0.to_le_bytes());
    h.update(&pay.sequence.0.to_le_bytes());
    h.update(&pay.request_hash);
    *h.finalize().as_bytes()
}

pub fn encode_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, TypesError> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).map_err(|_| TypesError::CborEncode)?;
    Ok(buf)
}
```

Extend `TypesError` with `CborEncode` / `CborDecode`. Export new modules from `lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockai-types`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/types
git commit -m "feat(types): add SpendCapability, Pay, and TX_ID"
```

---

### Task 3: Crypto key domains and signatures

**Files:**
- Create: `crates/crypto/Cargo.toml`
- Create: `crates/crypto/src/lib.rs`
- Create: `crates/crypto/src/keys.rs`
- Create: `crates/crypto/src/sign.rs`
- Modify: root `Cargo.toml` members
- Test: `crates/crypto/tests/sign_verify.rs`

**Interfaces:**
- Consumes: `blockai-types::{Pay, SpendCapability, encode helpers as needed}`
- Produces:
  - `KeyDomain` enum: `Root, Issuance, Edge, AgentSession, ServiceReceipt, Settlement, Audit`
  - `SigningKey` / `VerifyingKey` wrappers (ed25519)
  - `fn sign_pay(agent: &SigningKey, pay_without_sig: &Pay) -> Vec<u8>`
  - `fn verify_pay(agent_vk: &VerifyingKey, pay: &Pay) -> Result<(), CryptoError>`
  - `fn sign_capability(issuer: &SigningKey, cap_without_sig: &SpendCapability) -> Vec<u8>`
  - `fn verify_capability(issuer_vk: &VerifyingKey, cap: &SpendCapability) -> Result<(), CryptoError>`
  - Signing bytes = CBOR of struct with signature field empty / omitted via `PaySignBody` / `CapabilitySignBody`

- [ ] **Step 1: Write the failing test**

```rust
use blockai_crypto::{Keypair, sign_pay, verify_pay};
use blockai_types::{
    AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence,
};

#[test]
fn pay_sign_and_verify_roundtrip() {
    let kp = Keypair::generate();
    let mut pay = Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId(kp.verifying_key_bytes()),
        service_id: "inference/x".into(),
        amount: AmountMicros(100),
        currency: "EURC".into(),
        request_hash: [2u8; 32],
        price_quote_hash: [3u8; 32],
        max_amount: AmountMicros(100),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = sign_pay(&kp, &pay);
    assert!(verify_pay(&kp.verifying_key(), &pay).is_ok());
    pay.amount = AmountMicros(101);
    assert!(verify_pay(&kp.verifying_key(), &pay).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockai-crypto --test sign_verify`

Expected: FAIL (crate missing)

- [ ] **Step 3: Implement crypto crate**

`crates/crypto/Cargo.toml`:

```toml
[package]
name = "blockai-crypto"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
blockai-types = { path = "../types" }
ed25519-dalek = { version = "2", features = ["rand_core"] }
rand = "0.8"
serde = { workspace = true }
thiserror = { workspace = true }
blake3 = "1"
ciborium = "0.2"
```

Implement `Keypair`, domain tagging in signed body (`domain: "PAY"` / `"CAPABILITY"`), CBOR body without signature bytes, ed25519 sign/verify. Reject empty signatures.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockai-crypto`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/crypto
git commit -m "feat(crypto): add ed25519 PAY and capability signatures"
```

---

### Task 4: Capability Authority (partitioned issuance)

**Files:**
- Create: `crates/authority/Cargo.toml`
- Create: `crates/authority/src/lib.rs`
- Create: `crates/authority/src/issuer.rs`
- Modify: root `Cargo.toml` members
- Test: `crates/authority/tests/partition_issue.rs`

**Interfaces:**
- Consumes: `blockai-crypto`, `blockai-types`
- Produces:
  - `struct Authority { issuer: Keypair, accounts: HashMap<AccountId, AccountFloat> }`
  - `struct AccountFloat { total: AmountMicros, reserve: AmountMicros, shard_allowances: HashMap<ShardId, AmountMicros> }`
  - `fn Authority::fund(account, total)`
  - `fn Authority::allocate(account, shard, amount) -> Result<()>` — moves from reserve to shard bucket; fails if insufficient reserve
  - `fn Authority::issue_capability(IssueRequest) -> Result<SpendCapability>` — amount ≤ shard bucket outstanding capacity rules for Plan 1: deduct from shard bucket into `outstanding` map keyed by capability id; signs capability; enforces short TTL (`valid_until = now + ttl_ms`)
  - `fn Authority::fence_epoch(shard, epoch)` — marks epoch fenced in authority bookkeeping

- [ ] **Step 1: Write the failing test**

```rust
use blockai_authority::{Authority, IssueRequest};
use blockai_types::{AccountId, AgentId, AmountMicros, Epoch, Sequence, ShardId};

#[test]
fn cannot_over_allocate_across_shards() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    auth.fund(account, AmountMicros(100)).unwrap();
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(60)).unwrap();
    assert!(auth.allocate(account, ams, AmountMicros(50)).is_err());
}

#[test]
fn issued_capability_is_shard_bound_and_signed() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent = AgentId([2u8; 32]);
    auth.fund(account, AmountMicros(100)).unwrap();
    let fra = ShardId::new("FRA-004").unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(20)).unwrap();
    let cap = auth
        .issue_capability(IssueRequest {
            account_id: account,
            agent_id: agent,
            shard_id: fra.clone(),
            epoch: Epoch(1),
            maximum_total: AmountMicros(20),
            maximum_per_call: AmountMicros(1),
            service_scope: vec!["inference/*".into()],
            policy_hash: [9u8; 32],
            sequence_start: Sequence(1),
            sequence_end: Sequence(10_000),
            ttl_ms: 60_000,
            region: "EU".into(),
            now_unix_ms: 1_000,
        })
        .unwrap();
    assert_eq!(cap.shard_id, fra);
    assert_eq!(cap.maximum_total, AmountMicros(20));
    assert!(!cap.issuer_signature.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockai-authority --test partition_issue`

Expected: FAIL

- [ ] **Step 3: Implement authority issuer**

Keep logic explicit:

- `fund` sets `total` and `reserve = total`
- `allocate` moves `reserve -> shard_allowances[shard]`
- `issue_capability` requires `maximum_total <= shard_allowances[shard]`, then `shard_allowances[shard] -= maximum_total`, track `outstanding[capability_id] = maximum_total`, sign with issuance key

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockai-authority`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/authority
git commit -m "feat(authority): partitioned shard capability issuance"
```

---

### Task 5: Shard WAL and durable sequence consume

**Files:**
- Create: `crates/shard/Cargo.toml`
- Create: `crates/shard/src/lib.rs`
- Create: `crates/shard/src/wal.rs`
- Create: `crates/shard/src/state.rs`
- Modify: root `Cargo.toml` members
- Test: `crates/shard/tests/wal_consume.rs`

**Interfaces:**
- Consumes: types
- Produces:
  - `WalRecord::{ActivateCapability, ConsumePay { tx_id, capability_id, epoch, sequence, amount }, FenceEpoch}`
  - `struct ShardState` with remaining per capability, consumed `(cap, epoch, seq)` set, epoch states
  - `struct Wal { path }` with `append(record)`, `replay() -> ShardState`
  - `fn ShardState::consume_pay(...) -> Result<CommitIndex, ShardError>` pure in-memory transition used by WAL apply

- [ ] **Step 1: Write the failing test**

```rust
use blockai_shard::{ShardState, Wal, WalRecord};
use blockai_types::{AmountMicros, CapabilityId, Epoch, Sequence};
use tempfile::tempdir;

#[test]
fn wal_replay_restores_consumed_sequences() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shard.wal");
    let mut wal = Wal::open(&path).unwrap();
    let cap = CapabilityId([1u8; 32]);
    wal.append(&WalRecord::ActivateCapability {
        capability_id: cap,
        epoch: Epoch(1),
        remaining: AmountMicros(100),
        sequence_start: Sequence(1),
        sequence_end: Sequence(100),
    })
    .unwrap();
    wal.append(&WalRecord::ConsumePay {
        tx_id: [9u8; 32],
        capability_id: cap,
        epoch: Epoch(1),
        sequence: Sequence(1),
        amount: AmountMicros(5),
    })
    .unwrap();

    let state = wal.replay().unwrap();
    assert_eq!(state.remaining(&cap).unwrap(), AmountMicros(95));
    assert!(state.is_consumed(cap, Epoch(1), Sequence(1)));
    assert!(state.try_mark_consumed(cap, Epoch(1), Sequence(1)).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockai-shard --test wal_consume`

Expected: FAIL

- [ ] **Step 3: Implement WAL + state**

Use length-prefixed CBOR records on disk; `append` must `fsync` file before return. `replay` folds records into `ShardState`.

Reject:

- unknown capability
- amount &gt; remaining
- sequence outside range
- duplicate `(cap, epoch, seq)`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockai-shard --test wal_consume`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/shard
git commit -m "feat(shard): durable WAL and sequence consume state"
```

---

### Task 6: Minimal local 3-of-4 BFT for PayCommit

**Files:**
- Create: `crates/shard/src/bft.rs`
- Create: `crates/shard/src/engine.rs`
- Modify: `crates/shard/src/lib.rs`
- Test: `crates/shard/tests/local_bft_pay.rs`

**Interfaces:**
- Consumes: WAL/state, crypto, types, authority-issued capabilities
- Produces:
  - `struct ValidatorConfig { id: u8, shard_id: ShardId, key: Keypair, peers: Vec<VerifyingKey> }`
  - `struct InProcessNetwork` for tests (mpsc channels)
  - `struct ShardEngine` with `activate_capability(cap)`, `handle_pay(pay, now_ms) -> Result<EdgeAccept>`
  - `struct EdgeAccept { commit_index: u64, tx_id: [u8; 32], edge_signature: Vec<u8> }`
  - BFT messages: `Propose(PayCommit)`, `Vote { digest, voter }`, `CommitEntry`
  - Quorum = 3 distinct votes for same digest among 4 validators before WAL append + EdgeAccept

**Algorithm (Plan 1 minimal, LAN-only):**

1. Leader proposes `PayCommit` digest = BLAKE3(CBOR(PayCommitBody))
2. Each validator re-validates PAY against local capability cache + state
3. On valid: sign vote
4. Leader collects ≥3 votes (including self), broadcasts `Commit`
5. Every validator appends WAL then acknowledges
6. Leader returns `EdgeAccept` only after ≥3 durable acks

This is intentionally narrower than full HotStuff; replaceable in a later plan without changing PAY types.

- [ ] **Step 1: Write the failing test**

```rust
use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::Keypair;
use blockai_shard::testkit::cluster4;
use blockai_types::{
    AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId, tx_id,
};

#[tokio::test]
async fn three_of_four_commits_pay_before_accept() {
    let shard = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(20)).unwrap();
    let cap = auth
        .issue_capability(IssueRequest {
            account_id: account,
            agent_id: agent,
            shard_id: shard.clone(),
            epoch: Epoch(1),
            maximum_total: AmountMicros(20),
            maximum_per_call: AmountMicros(5),
            service_scope: vec!["inference/*".into()],
            policy_hash: [9u8; 32],
            sequence_start: Sequence(1),
            sequence_end: Sequence(100),
            ttl_ms: 60_000,
            region: "EU".into(),
            now_unix_ms: 1_000,
        })
        .unwrap();

    let cluster = cluster4(shard.clone()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }

    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(1),
        agent_id: agent,
        service_id: "inference/x".into(),
        amount: AmountMicros(3),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = blockai_crypto::sign_pay(&agent_kp, &pay);

    let accept = cluster.leader().handle_pay(pay.clone(), 1_100).await.unwrap();
    assert_eq!(accept.tx_id, tx_id(&pay));
    assert!(accept.commit_index >= 1);

    // replay rejected
    let err = cluster.leader().handle_pay(pay, 1_101).await.unwrap_err();
    assert!(format!("{err}").contains("REPLAY") || format!("{err}").contains("consumed"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockai-shard --test local_bft_pay -- --nocapture`

Expected: FAIL

- [ ] **Step 3: Implement bft + engine + testkit**

Validation checklist inside each validator before vote:

- capability present and signature valid (issuer vk configured on engine)
- `pay.shard` implied by engine.shard_id == capability.shard_id
- epoch Active
- now within capability validity and pay.expiry
- service_id matches scope (`inference/*` prefix/glob: Plan 1 supports exact match or trailing `/*` prefix)
- amount ≤ per_call, ≤ remaining, ≤ max_amount
- sequence in range and not consumed
- agent signature valid and agent_id matches capability

Kill path for tests: `cluster.kill(validator_id)` still allows commit with 3 remaining.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockai-shard`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/shard
git commit -m "feat(shard): minimal 3-of-4 local BFT PAY commit-before-exec"
```

---

### Task 7: Security property tests (fail-closed)

**Files:**
- Create: `crates/shard/tests/security_properties.rs`
- Modify: `crates/shard/src/engine.rs` only if needed for error codes

**Interfaces:**
- Consumes: Task 6 cluster APIs
- Produces: deterministic fail-closed errors:
  - `ShardError::WrongShard`
  - `ShardError::EpochFenced`
  - `ShardError::Replay`
  - `ShardError::ExceedsPerCall`
  - `ShardError::InsufficientRemaining`
  - `ShardError::BadSignature`

- [ ] **Step 1: Write the failing tests**

Create `crates/shard/tests/security_properties.rs` with full bodies:

```rust
use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::{testkit::cluster4, ShardError};
use blockai_types::{
    tx_id, AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId,
};

fn fund_issue(
    auth: &mut Authority,
    account: AccountId,
    agent: AgentId,
    shard: ShardId,
    per_call: AmountMicros,
) -> blockai_types::SpendCapability {
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(20)).unwrap();
    auth.issue_capability(IssueRequest {
        account_id: account,
        agent_id: agent,
        shard_id: shard,
        epoch: Epoch(1),
        maximum_total: AmountMicros(20),
        maximum_per_call: per_call,
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(1),
        sequence_end: Sequence(100),
        ttl_ms: 60_000,
        region: "EU".into(),
        now_unix_ms: 1_000,
    })
    .unwrap()
}

fn signed_pay(
    agent_kp: &Keypair,
    cap: &blockai_types::SpendCapability,
    seq: u64,
    amount: AmountMicros,
) -> Pay {
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(seq),
        agent_id: AgentId(agent_kp.verifying_key_bytes()),
        service_id: "inference/x".into(),
        amount,
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = sign_pay(agent_kp, &pay);
    pay
}

#[tokio::test]
async fn foreign_shard_capability_rejected() {
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, ams, AmountMicros(5));
    let cluster = cluster4(fra).await;
    for eng in cluster.engines.iter() {
        // activation may succeed locally for testing, but PAY must still fail WrongShard
        let _ = eng.activate_capability(cap.clone()).await;
    }
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert!(matches!(err, ShardError::WrongShard { .. }));
}

#[tokio::test]
async fn fenced_epoch_rejects_new_pays() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, fra.clone(), AmountMicros(5));
    let cluster = cluster4(fra).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    cluster.leader().fence_epoch(Epoch(1)).await.unwrap();
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert!(matches!(err, ShardError::EpochFenced { .. }));
}

#[tokio::test]
async fn over_per_call_rejected() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, fra.clone(), AmountMicros(2));
    let cluster = cluster4(fra).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(3));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert!(matches!(err, ShardError::ExceedsPerCall { .. }));
}

#[tokio::test]
async fn kill_one_validator_still_safe_no_double_spend() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, fra.clone(), AmountMicros(5));
    let cluster = cluster4(fra).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let pay1 = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let accept = cluster.leader().handle_pay(pay1.clone(), 1_100).await.unwrap();
    assert_eq!(accept.tx_id, tx_id(&pay1));
    cluster.kill(2).await;
    let err = cluster.leader().handle_pay(pay1, 1_101).await.unwrap_err();
    assert!(matches!(err, ShardError::Replay { .. }));
    let pay2 = signed_pay(&agent_kp, &cap, 2, AmountMicros(1));
    let accept2 = cluster.leader().handle_pay(pay2, 1_102).await.unwrap();
    assert!(accept2.commit_index > accept.commit_index);
}
```

- [ ] **Step 2: Run tests to verify they fail or compile-fail on missing error variants**

Run: `cargo test -p blockai-shard --test security_properties`

Expected: FAIL until error variants / fence API exist

- [ ] **Step 3: Implement missing error variants and `engine.fence_epoch(Epoch)`**

Broadcast fence via BFT as `FenceEpoch` entry; WAL + state; then reject PAY.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockai-shard --test security_properties`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/shard
git commit -m "test(shard): fail-closed replay, fence, and cross-shard properties"
```

---

### Task 8: `pay_sim` CLI smoke tool

**Files:**
- Create: `crates/tools/Cargo.toml`
- Create: `crates/tools/src/bin/pay_sim.rs`
- Modify: root `Cargo.toml` members

**Interfaces:**
- Consumes: authority + shard testkit
- Produces: CLI that funds, allocates, issues, runs 4-node cluster, submits N pays, prints accept latency micros

- [ ] **Step 1: Write a smoke test invoking library path used by CLI**

Add `crates/shard/tests/pay_burst.rs` that submits 50 sequential pays and asserts all commit_index increasing and remaining correct.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockai-shard --test pay_burst`

Expected: FAIL until written/supported

- [ ] **Step 3: Implement burst test + `pay_sim` binary**

`pay_sim` args: `--pays 50` (default). Print:

```text
ok pays=50 p50_us=... remaining=...
```

- [ ] **Step 4: Run tests and CLI**

Run:

```bash
cargo test -p blockai-shard --test pay_burst
cargo run -p blockai-tools --bin pay_sim -- --pays 50
```

Expected: tests PASS; CLI prints `ok pays=50 ...`

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/tools crates/shard/tests/pay_burst.rs
git commit -m "feat(tools): add pay_sim smoke binary for local SEEF auth path"
```

---

## Plan self-review

**Spec coverage (Plan 1 subset):**

| Spec item | Task |
|---|---|
| SpendCapability / PAY / TX_ID | Task 2 |
| Signatures / key separation (agent + issuance) | Task 3 |
| Partitioned allocation | Task 4 |
| Commit-before-exec WAL | Task 5 |
| Local 3-of-4 BFT shard | Task 6 |
| Replay / fence / wrong-shard fail-closed | Task 7 |
| Lab smoke / latency print | Task 8 |
| No 0-RTT PAY / no global L1 / no WASM | Explicitly out of this plan |

**Deferred to later plans:** witnesses, Merkle checkpoints, three-party service receipts beyond EdgeAccept, global DAG+BFT, WASM/registry/reputation, QUIC, AF_XDP, PQ dual-sign, HSM, attestation enforcement.

**Placeholder scan:** clean after expanding Task 7 test bodies.

**Type consistency:** `AmountMicros(u128)`, `Epoch(u64)`, `Sequence(u64)`, `tx_id(&[Pay]) -> [u8;32]`, `EdgeAccept { commit_index, tx_id, edge_signature }` used uniformly.
