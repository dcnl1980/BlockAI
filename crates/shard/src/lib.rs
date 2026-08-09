pub mod bft;
pub mod checkpoint;
pub mod engine;
pub mod merkle;
pub mod payment;
pub mod receipt_log;
pub mod state;
pub mod testkit;
pub mod wal;

use blockai_types::{AmountMicros, CapabilityId, Epoch, Sequence};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShardError {
    #[error("empty receipt log")]
    EmptyReceiptLog,
    #[error("unknown capability")]
    UnknownCapability,
    #[error("REPLAY capability={capability_id:?} epoch={epoch:?} sequence={sequence:?}")]
    Replay {
        capability_id: CapabilityId,
        epoch: Epoch,
        sequence: Sequence,
    },
    #[error("epoch fenced: {epoch:?}")]
    EpochFenced { epoch: Epoch },
    #[error("epoch expired: {epoch:?}")]
    EpochExpired { epoch: Epoch },
    #[error("epoch mismatch")]
    EpochMismatch,
    #[error("sequence out of range: {sequence:?}")]
    SequenceOutOfRange { sequence: Sequence },
    #[error("insufficient remaining: have {remaining:?} need {requested:?}")]
    InsufficientRemaining {
        remaining: AmountMicros,
        requested: AmountMicros,
    },
    #[error("wrong shard: capability={capability_shard} engine={engine_shard}")]
    WrongShard {
        capability_shard: String,
        engine_shard: String,
    },
    #[error("exceeds per-call max")]
    ExceedsPerCall {
        amount: AmountMicros,
        maximum_per_call: AmountMicros,
    },
    #[error("amount exceeds max_amount")]
    ExceedsMaxAmount,
    #[error("bad signature")]
    BadSignature,
    #[error("service out of scope")]
    ServiceOutOfScope,
    #[error("capability not yet valid")]
    NotYetValid,
    #[error("capability expired")]
    CapabilityExpired,
    #[error("pay expired")]
    PayExpired,
    #[error("agent mismatch")]
    AgentMismatch,
    #[error("currency mismatch")]
    CurrencyMismatch,
    #[error("bft quorum failed")]
    BftQuorumFailed,
    #[error("validator killed")]
    ValidatorKilled,
    #[error("io error: {0}")]
    Io(String),
    #[error("cbor error")]
    Cbor,
}

pub use checkpoint::{verify_signed_checkpoint, CheckpointSealer};
pub use engine::{EdgeAccept, ShardEngine};
pub use merkle::{merkle_proof, merkle_root, verify_merkle_proof, MerkleProof};
pub use payment::complete_payment_proof;
pub use receipt_log::ReceiptLog;
pub use state::ShardState;
pub use wal::{Wal, WalRecord};
