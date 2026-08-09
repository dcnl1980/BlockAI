use crate::keys::Keypair;
use blockai_types::{encode_cbor, Pay, SpendCapability};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("empty signature")]
    EmptySignature,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid verifying key")]
    InvalidVerifyingKey,
    #[error("cbor encode failed")]
    CborEncode,
}

#[derive(Serialize)]
struct PaySignBody<'a> {
    domain: &'static str,
    capability_id: [u8; 32],
    epoch: u64,
    sequence: u64,
    agent_id: [u8; 32],
    service_id: &'a str,
    amount: u128,
    currency: &'a str,
    request_hash: [u8; 32],
    price_quote_hash: [u8; 32],
    max_amount: u128,
    pricing_schedule_version: u64,
    expiry_unix_ms: u64,
}

#[derive(Serialize)]
struct CapabilitySignBody<'a> {
    domain: &'static str,
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
}

fn pay_body(pay: &Pay) -> PaySignBody<'_> {
    PaySignBody {
        domain: "PAY",
        capability_id: pay.capability_id.0,
        epoch: pay.epoch.0,
        sequence: pay.sequence.0,
        agent_id: pay.agent_id.0,
        service_id: &pay.service_id,
        amount: pay.amount.0,
        currency: &pay.currency,
        request_hash: pay.request_hash,
        price_quote_hash: pay.price_quote_hash,
        max_amount: pay.max_amount.0,
        pricing_schedule_version: pay.pricing_schedule_version,
        expiry_unix_ms: pay.expiry_unix_ms,
    }
}

fn capability_body(cap: &SpendCapability) -> CapabilitySignBody<'_> {
    CapabilitySignBody {
        domain: "CAPABILITY",
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
    }
}

pub fn sign_pay(agent: &Keypair, pay: &Pay) -> Vec<u8> {
    let body = pay_body(pay);
    let bytes = encode_cbor(&body).expect("pay body encodes");
    agent.signing_key().sign(&bytes).to_bytes().to_vec()
}

pub fn verify_pay(agent_vk: &VerifyingKey, pay: &Pay) -> Result<(), CryptoError> {
    if pay.agent_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = pay_body(pay);
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    let sig_bytes: [u8; 64] = pay
        .agent_signature
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);
    agent_vk
        .verify(&bytes, &sig)
        .map_err(|_| CryptoError::InvalidSignature)
}

pub fn sign_capability(issuer: &Keypair, cap: &SpendCapability) -> Vec<u8> {
    let body = capability_body(cap);
    let bytes = encode_cbor(&body).expect("capability body encodes");
    issuer.signing_key().sign(&bytes).to_bytes().to_vec()
}

pub fn verify_capability(issuer_vk: &VerifyingKey, cap: &SpendCapability) -> Result<(), CryptoError> {
    if cap.issuer_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = capability_body(cap);
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    let sig_bytes: [u8; 64] = cap
        .issuer_signature
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);
    issuer_vk
        .verify(&bytes, &sig)
        .map_err(|_| CryptoError::InvalidSignature)
}

pub fn verifying_key_from_bytes(bytes: &[u8; 32]) -> Result<VerifyingKey, CryptoError> {
    VerifyingKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidVerifyingKey)
}
