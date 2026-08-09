mod dag;
mod bft;

pub use bft::{cluster4, CommitOutcome, GlobalCluster, GlobalValidator};
pub use dag::{DagBlock, DagMempool};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConsensusError {
    #[error("quorum failed")]
    QuorumFailed,
    #[error("invalid signature")]
    BadSignature,
    #[error("execute error: {0}")]
    Execute(String),
    #[error("duplicate tx")]
    DuplicateTx,
    #[error("validator killed")]
    ValidatorKilled,
}
