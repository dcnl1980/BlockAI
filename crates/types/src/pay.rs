use crate::{AgentId, AmountMicros, CapabilityId, Epoch, Sequence, TypesError};
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

pub fn decode_cbor<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, TypesError> {
    ciborium::from_reader(bytes).map_err(|_| TypesError::CborDecode)
}
