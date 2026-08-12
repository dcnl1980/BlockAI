use crate::AmountMicros;
use serde::{Deserialize, Serialize};

/// On-chain parameter / note actions (mainnet economics governance).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceAction {
    SetMinStake { value: AmountMicros },
    SetBaseFee { value: AmountMicros },
    SetVoteQuorumBps { value: u16 },
    TextNote { note_hash: [u8; 32] },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Open,
    Passed,
    Rejected,
    Executed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicParams {
    pub min_stake: AmountMicros,
    pub base_fee: AmountMicros,
    pub proposal_bond: AmountMicros,
    /// Yes-stake must be ≥ this fraction of total stake (basis points, 10_000 = 100%).
    pub vote_quorum_bps: u16,
}

impl Default for EconomicParams {
    fn default() -> Self {
        Self {
            min_stake: AmountMicros(1),
            base_fee: AmountMicros(1),
            proposal_bond: AmountMicros(10),
            vote_quorum_bps: 5_000,
        }
    }
}
