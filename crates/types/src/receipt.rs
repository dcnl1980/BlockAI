use crate::{encode_cbor, AmountMicros, TypesError};
use serde::{Deserialize, Serialize};

/// Agent authorization proof (A): hash of signed PAY body + agent signature bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAuthorization {
    pub pay_cbor_hash: [u8; 32],
    pub agent_signature: Vec<u8>,
}

/// Edge acceptance (E) binding agent auth to a local commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeAcceptance {
    pub agent_auth_hash: [u8; 32],
    pub commit_index: u64,
    pub tx_id: [u8; 32],
    pub edge_pubkey: [u8; 32],
    pub edge_signature: Vec<u8>,
    #[serde(default)]
    pub edge_pq_pubkey: Vec<u8>,
    #[serde(default)]
    pub edge_pq_signature: Vec<u8>,
}

/// Service execution receipt (S).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceReceipt {
    pub edge_accept_hash: [u8; 32],
    pub execution_hash: [u8; 32],
    pub actual_amount: AmountMicros,
    pub service_pubkey: [u8; 32],
    pub service_signature: Vec<u8>,
    #[serde(default)]
    pub service_pq_pubkey: Vec<u8>,
    #[serde(default)]
    pub service_pq_signature: Vec<u8>,
}

/// Full three-party payment proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentProof {
    pub agent: AgentAuthorization,
    pub edge: EdgeAcceptance,
    pub service: ServiceReceipt,
}

pub fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

pub fn hash_cbor<T: Serialize>(value: &T) -> Result<[u8; 32], TypesError> {
    let bytes = encode_cbor(value)?;
    Ok(hash_bytes(&bytes))
}

/// Leaf hash for Merkle inclusion of a completed payment.
pub fn receipt_leaf_hash(proof: &PaymentProof) -> Result<[u8; 32], TypesError> {
    hash_cbor(proof)
}
