use serde::{Deserialize, Serialize};

/// Envelope algorithm identifiers (crypto-agility).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum AlgorithmId {
    Ed25519 = 1,
    MlDsa65 = 2,
    HybridEd25519MlDsa65 = 3,
}

impl AlgorithmId {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            1 => Some(Self::Ed25519),
            2 => Some(Self::MlDsa65),
            3 => Some(Self::HybridEd25519MlDsa65),
            _ => None,
        }
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}
