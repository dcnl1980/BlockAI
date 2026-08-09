use crate::keys::Keypair;
use crate::sign::CryptoError;
use blockai_types::{encode_cbor, EdgeAcceptance, ServiceReceipt};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::Serialize;

#[derive(Serialize)]
struct EdgeSignBody {
    domain: &'static str,
    agent_auth_hash: [u8; 32],
    commit_index: u64,
    tx_id: [u8; 32],
    edge_pubkey: [u8; 32],
}

#[derive(Serialize)]
struct ServiceSignBody {
    domain: &'static str,
    edge_accept_hash: [u8; 32],
    execution_hash: [u8; 32],
    actual_amount: u128,
    service_pubkey: [u8; 32],
}

fn edge_body(e: &EdgeAcceptance) -> EdgeSignBody {
    EdgeSignBody {
        domain: "EDGE_ACCEPT",
        agent_auth_hash: e.agent_auth_hash,
        commit_index: e.commit_index,
        tx_id: e.tx_id,
        edge_pubkey: e.edge_pubkey,
    }
}

fn service_body(s: &ServiceReceipt) -> ServiceSignBody {
    ServiceSignBody {
        domain: "SERVICE_RECEIPT",
        edge_accept_hash: s.edge_accept_hash,
        execution_hash: s.execution_hash,
        actual_amount: s.actual_amount.0,
        service_pubkey: s.service_pubkey,
    }
}

pub fn sign_edge_acceptance(edge: &Keypair, acceptance: &EdgeAcceptance) -> Vec<u8> {
    let bytes = encode_cbor(&edge_body(acceptance)).expect("encode");
    edge.signing_key().sign(&bytes).to_bytes().to_vec()
}

pub fn verify_edge_acceptance(
    vk: &VerifyingKey,
    acceptance: &EdgeAcceptance,
) -> Result<(), CryptoError> {
    if acceptance.edge_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let bytes = encode_cbor(&edge_body(acceptance)).map_err(|_| CryptoError::CborEncode)?;
    let sig_bytes: [u8; 64] = acceptance
        .edge_signature
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidSignature)?;
    vk.verify(&bytes, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| CryptoError::InvalidSignature)
}

pub fn sign_service_receipt(service: &Keypair, receipt: &ServiceReceipt) -> Vec<u8> {
    let bytes = encode_cbor(&service_body(receipt)).expect("encode");
    service.signing_key().sign(&bytes).to_bytes().to_vec()
}

pub fn verify_service_receipt(
    vk: &VerifyingKey,
    receipt: &ServiceReceipt,
) -> Result<(), CryptoError> {
    if receipt.service_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let bytes = encode_cbor(&service_body(receipt)).map_err(|_| CryptoError::CborEncode)?;
    let sig_bytes: [u8; 64] = receipt
        .service_signature
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidSignature)?;
    vk.verify(&bytes, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| CryptoError::InvalidSignature)
}
