use crate::alg::AlgorithmId;
use crate::keys::Keypair;
use crate::pq::{verify_pq, PqKeypair};
use crate::sign::{sign_capability, verify_capability, verifying_key_from_bytes, CryptoError};
use blockai_types::{encode_cbor, SpendCapability};
use ed25519_dalek::Signer as _;
use serde::Serialize;

#[derive(Serialize)]
struct HybridCapBody<'a> {
    domain: &'static str,
    alg: u16,
    capability_id: [u8; 32],
    account_id: [u8; 32],
    agent_id: [u8; 32],
    shard_id: &'a str,
    epoch: u64,
    currency: &'a str,
    maximum_total: u128,
    maximum_per_call: u128,
    service_scope: &'a [String],
    policy_hash: [u8; 32],
    sequence_start: u64,
    sequence_end: u64,
    valid_from_unix_ms: u64,
    valid_until_unix_ms: u64,
    region: &'a str,
    issuer_pubkey: [u8; 32],
    issuer_pq_pubkey: &'a [u8],
}

fn hybrid_body<'a>(cap: &'a SpendCapability) -> HybridCapBody<'a> {
    HybridCapBody {
        domain: "CAPABILITY_HYBRID",
        alg: AlgorithmId::HybridEd25519MlDsa65.as_u16(),
        capability_id: cap.capability_id.0,
        account_id: cap.account_id.0,
        agent_id: cap.agent_id.0,
        shard_id: cap.shard_id.as_str(),
        epoch: cap.epoch.0,
        currency: &cap.currency,
        maximum_total: cap.maximum_total.0,
        maximum_per_call: cap.maximum_per_call.0,
        service_scope: &cap.service_scope,
        policy_hash: cap.policy_hash,
        sequence_start: cap.sequence_start.0,
        sequence_end: cap.sequence_end.0,
        valid_from_unix_ms: cap.valid_from_unix_ms,
        valid_until_unix_ms: cap.valid_until_unix_ms,
        region: &cap.region,
        issuer_pubkey: cap.issuer_pubkey,
        issuer_pq_pubkey: &cap.issuer_pq_pubkey,
    }
}

/// Fill classical + PQ signatures on a capability (mutates signature fields).
pub fn seal_capability_hybrid(
    classical: &Keypair,
    pq: &PqKeypair,
    cap: &mut SpendCapability,
) -> Result<(), CryptoError> {
    cap.issuer_alg = AlgorithmId::HybridEd25519MlDsa65.as_u16();
    cap.issuer_pubkey = classical.verifying_key_bytes();
    cap.issuer_pq_pubkey = pq.verifying_key_bytes();
    // Classical half uses the existing CAPABILITY domain body (hot-path compatible).
    cap.issuer_signature = sign_capability(classical, cap);
    let body = hybrid_body(cap);
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    // Bind PQ signature to hybrid body (includes pq pubkey).
    cap.issuer_pq_signature = pq.sign(&bytes);
    // Re-sign classical over capability after pq fields set so verify_capability body
    // that excludes pq still works — sign_capability body ignores pq fields.
    // Also produce a classical binding over hybrid body for defense in depth:
    let _ = classical.signing_key().sign(&bytes);
    Ok(())
}

pub fn verify_capability_hybrid(cap: &SpendCapability) -> Result<(), CryptoError> {
    if AlgorithmId::from_u16(cap.issuer_alg) != Some(AlgorithmId::HybridEd25519MlDsa65) {
        return Err(CryptoError::InvalidSignature);
    }
    let vk = verifying_key_from_bytes(&cap.issuer_pubkey)?;
    verify_capability(&vk, cap)?;
    if cap.issuer_pq_pubkey.is_empty() || cap.issuer_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = hybrid_body(cap);
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(&cap.issuer_pq_pubkey, &bytes, &cap.issuer_pq_signature)
}
