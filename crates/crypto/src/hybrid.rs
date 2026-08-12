use crate::alg::AlgorithmId;
use crate::keys::Keypair;
use crate::pq::{verify_pq, PqKeypair};
use crate::receipt_sign::{
    sign_edge_acceptance, sign_service_receipt, verify_edge_acceptance, verify_service_receipt,
};
use crate::sign::{sign_pay, verify_pay, verifying_key_from_bytes, CryptoError};
use blockai_types::{
    encode_cbor, CheckpointHeader, EdgeAcceptance, Pay, ServiceReceipt, SignedCheckpoint,
    SpendCapability, WitnessSig,
};
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

fn hybrid_cap_body<'a>(cap: &'a SpendCapability) -> HybridCapBody<'a> {
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
    cap.issuer_signature = crate::sign::sign_capability(classical, cap);
    let body = hybrid_cap_body(cap);
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    cap.issuer_pq_signature = pq.sign(&bytes);
    let _ = classical.signing_key().sign(&bytes);
    Ok(())
}

pub fn verify_capability_hybrid(cap: &SpendCapability) -> Result<(), CryptoError> {
    if AlgorithmId::from_u16(cap.issuer_alg) != Some(AlgorithmId::HybridEd25519MlDsa65) {
        return Err(CryptoError::InvalidSignature);
    }
    let vk = verifying_key_from_bytes(&cap.issuer_pubkey)?;
    crate::sign::verify_capability(&vk, cap)?;
    if cap.issuer_pq_pubkey.is_empty() || cap.issuer_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = hybrid_cap_body(cap);
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(&cap.issuer_pq_pubkey, &bytes, &cap.issuer_pq_signature)
}

#[derive(Serialize)]
struct HybridPayBody<'a> {
    domain: &'static str,
    alg: u16,
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
    agent_pq_pubkey: &'a [u8],
}

fn hybrid_pay_body<'a>(pay: &'a Pay) -> HybridPayBody<'a> {
    HybridPayBody {
        domain: "PAY_HYBRID",
        alg: AlgorithmId::HybridEd25519MlDsa65.as_u16(),
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
        agent_pq_pubkey: &pay.agent_pq_pubkey,
    }
}

pub fn seal_pay_hybrid(classical: &Keypair, pq: &PqKeypair, pay: &mut Pay) -> Result<(), CryptoError> {
    pay.agent_alg = AlgorithmId::HybridEd25519MlDsa65.as_u16();
    pay.agent_id = blockai_types::AgentId(classical.verifying_key_bytes());
    pay.agent_pq_pubkey = pq.verifying_key_bytes();
    pay.agent_signature = sign_pay(classical, pay);
    let bytes = encode_cbor(&hybrid_pay_body(pay)).map_err(|_| CryptoError::CborEncode)?;
    pay.agent_pq_signature = pq.sign(&bytes);
    Ok(())
}

pub fn verify_pay_hybrid(pay: &Pay) -> Result<(), CryptoError> {
    if AlgorithmId::from_u16(pay.agent_alg) != Some(AlgorithmId::HybridEd25519MlDsa65) {
        return Err(CryptoError::InvalidSignature);
    }
    let vk = verifying_key_from_bytes(&pay.agent_id.0)?;
    verify_pay(&vk, pay)?;
    if pay.agent_pq_pubkey.is_empty() || pay.agent_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let bytes = encode_cbor(&hybrid_pay_body(pay)).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(&pay.agent_pq_pubkey, &bytes, &pay.agent_pq_signature)
}

#[derive(Serialize)]
struct HybridCheckpointBody<'a> {
    domain: &'static str,
    shard_id: &'a str,
    epoch: u64,
    root: [u8; 32],
    height: u64,
    tx_count: u64,
    exposure: u128,
    sealed_at_unix_ms: u64,
    shard_signer_pubkey: [u8; 32],
    shard_pq_pubkey: &'a [u8],
}

fn hybrid_checkpoint_body<'a>(
    header: &'a CheckpointHeader,
    classical_pk: &[u8; 32],
    pq_pk: &'a [u8],
) -> HybridCheckpointBody<'a> {
    HybridCheckpointBody {
        domain: "CHECKPOINT_HYBRID",
        shard_id: header.shard_id.as_str(),
        epoch: header.epoch.0,
        root: header.root,
        height: header.height,
        tx_count: header.tx_count,
        exposure: header.exposure.0,
        sealed_at_unix_ms: header.sealed_at_unix_ms,
        shard_signer_pubkey: *classical_pk,
        shard_pq_pubkey: pq_pk,
    }
}

pub fn seal_checkpoint_pq(
    classical_pk: [u8; 32],
    pq: &PqKeypair,
    header: &CheckpointHeader,
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let pq_pk = pq.verifying_key_bytes();
    let bytes = encode_cbor(&hybrid_checkpoint_body(header, &classical_pk, &pq_pk))
        .map_err(|_| CryptoError::CborEncode)?;
    Ok((pq_pk, pq.sign(&bytes)))
}

pub fn verify_checkpoint_pq(checkpoint: &SignedCheckpoint) -> Result<(), CryptoError> {
    if checkpoint.shard_pq_pubkey.is_empty() || checkpoint.shard_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let bytes = encode_cbor(&hybrid_checkpoint_body(
        &checkpoint.header,
        &checkpoint.shard_signer_pubkey,
        &checkpoint.shard_pq_pubkey,
    ))
    .map_err(|_| CryptoError::CborEncode)?;
    verify_pq(
        &checkpoint.shard_pq_pubkey,
        &bytes,
        &checkpoint.shard_pq_signature,
    )
}

#[derive(Serialize)]
struct HybridWitnessBody<'a> {
    domain: &'static str,
    shard_id: &'a str,
    epoch: u64,
    root: [u8; 32],
    height: u64,
    shard_signature: &'a [u8],
    witness_pubkey: [u8; 32],
    witness_pq_pubkey: &'a [u8],
}

pub fn seal_witness_pq(
    classical: &Keypair,
    pq: &PqKeypair,
    checkpoint: &SignedCheckpoint,
    classical_sig: Vec<u8>,
) -> Result<WitnessSig, CryptoError> {
    let witness_pq_pubkey = pq.verifying_key_bytes();
    let body = HybridWitnessBody {
        domain: "WITNESS_CHECKPOINT_HYBRID",
        shard_id: checkpoint.header.shard_id.as_str(),
        epoch: checkpoint.header.epoch.0,
        root: checkpoint.header.root,
        height: checkpoint.header.height,
        shard_signature: &checkpoint.shard_signature,
        witness_pubkey: classical.verifying_key_bytes(),
        witness_pq_pubkey: &witness_pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    Ok(WitnessSig {
        witness_pubkey: classical.verifying_key_bytes(),
        signature: classical_sig,
        witness_pq_pubkey,
        witness_pq_signature: pq.sign(&bytes),
    })
}

pub fn verify_witness_hybrid(
    checkpoint: &SignedCheckpoint,
    sig: &WitnessSig,
) -> Result<(), CryptoError> {
    if sig.witness_pq_pubkey.is_empty() || sig.witness_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = HybridWitnessBody {
        domain: "WITNESS_CHECKPOINT_HYBRID",
        shard_id: checkpoint.header.shard_id.as_str(),
        epoch: checkpoint.header.epoch.0,
        root: checkpoint.header.root,
        height: checkpoint.header.height,
        shard_signature: &checkpoint.shard_signature,
        witness_pubkey: sig.witness_pubkey,
        witness_pq_pubkey: &sig.witness_pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(&sig.witness_pq_pubkey, &bytes, &sig.witness_pq_signature)
}

#[derive(Serialize)]
struct HybridEdgeBody<'a> {
    domain: &'static str,
    agent_auth_hash: [u8; 32],
    commit_index: u64,
    tx_id: [u8; 32],
    edge_pubkey: [u8; 32],
    edge_pq_pubkey: &'a [u8],
}

#[derive(Serialize)]
struct HybridServiceBody<'a> {
    domain: &'static str,
    edge_accept_hash: [u8; 32],
    execution_hash: [u8; 32],
    actual_amount: u128,
    service_pubkey: [u8; 32],
    service_pq_pubkey: &'a [u8],
}

pub fn seal_edge_hybrid(
    classical: &Keypair,
    pq: &PqKeypair,
    acceptance: &mut EdgeAcceptance,
) -> Result<(), CryptoError> {
    acceptance.edge_pubkey = classical.verifying_key_bytes();
    acceptance.edge_pq_pubkey = pq.verifying_key_bytes();
    acceptance.edge_signature = sign_edge_acceptance(classical, acceptance);
    let body = HybridEdgeBody {
        domain: "EDGE_ACCEPT_HYBRID",
        agent_auth_hash: acceptance.agent_auth_hash,
        commit_index: acceptance.commit_index,
        tx_id: acceptance.tx_id,
        edge_pubkey: acceptance.edge_pubkey,
        edge_pq_pubkey: &acceptance.edge_pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    acceptance.edge_pq_signature = pq.sign(&bytes);
    Ok(())
}

pub fn verify_edge_hybrid(acceptance: &EdgeAcceptance) -> Result<(), CryptoError> {
    let vk = verifying_key_from_bytes(&acceptance.edge_pubkey)?;
    verify_edge_acceptance(&vk, acceptance)?;
    if acceptance.edge_pq_pubkey.is_empty() || acceptance.edge_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = HybridEdgeBody {
        domain: "EDGE_ACCEPT_HYBRID",
        agent_auth_hash: acceptance.agent_auth_hash,
        commit_index: acceptance.commit_index,
        tx_id: acceptance.tx_id,
        edge_pubkey: acceptance.edge_pubkey,
        edge_pq_pubkey: &acceptance.edge_pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(
        &acceptance.edge_pq_pubkey,
        &bytes,
        &acceptance.edge_pq_signature,
    )
}

pub fn seal_service_hybrid(
    classical: &Keypair,
    pq: &PqKeypair,
    receipt: &mut ServiceReceipt,
) -> Result<(), CryptoError> {
    receipt.service_pubkey = classical.verifying_key_bytes();
    receipt.service_pq_pubkey = pq.verifying_key_bytes();
    receipt.service_signature = sign_service_receipt(classical, receipt);
    let body = HybridServiceBody {
        domain: "SERVICE_RECEIPT_HYBRID",
        edge_accept_hash: receipt.edge_accept_hash,
        execution_hash: receipt.execution_hash,
        actual_amount: receipt.actual_amount.0,
        service_pubkey: receipt.service_pubkey,
        service_pq_pubkey: &receipt.service_pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    receipt.service_pq_signature = pq.sign(&bytes);
    Ok(())
}

pub fn verify_service_hybrid(receipt: &ServiceReceipt) -> Result<(), CryptoError> {
    let vk = verifying_key_from_bytes(&receipt.service_pubkey)?;
    verify_service_receipt(&vk, receipt)?;
    if receipt.service_pq_pubkey.is_empty() || receipt.service_pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = HybridServiceBody {
        domain: "SERVICE_RECEIPT_HYBRID",
        edge_accept_hash: receipt.edge_accept_hash,
        execution_hash: receipt.execution_hash,
        actual_amount: receipt.actual_amount.0,
        service_pubkey: receipt.service_pubkey,
        service_pq_pubkey: &receipt.service_pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(
        &receipt.service_pq_pubkey,
        &bytes,
        &receipt.service_pq_signature,
    )
}

#[derive(Serialize)]
pub struct HybridRootOpBody<'a, T: Serialize> {
    pub domain: &'static str,
    pub op: &'a T,
    pub share_pubkey: [u8; 32],
    pub share_pq_pubkey: &'a [u8],
}

pub fn dual_sign_root_op<T: Serialize>(
    classical: &Keypair,
    pq: &PqKeypair,
    classical_msg: &[u8],
    op: &T,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), CryptoError> {
    let classical_sig = classical.signing_key().sign(classical_msg).to_bytes().to_vec();
    let pq_pk = pq.verifying_key_bytes();
    let body = HybridRootOpBody {
        domain: "HSM_ROOT_OP_HYBRID",
        op,
        share_pubkey: classical.verifying_key_bytes(),
        share_pq_pubkey: &pq_pk,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    Ok((classical_sig, pq_pk, pq.sign(&bytes)))
}

pub fn verify_root_op_pq<T: Serialize>(
    share_pubkey: &[u8; 32],
    pq_pubkey: &[u8],
    pq_signature: &[u8],
    op: &T,
) -> Result<(), CryptoError> {
    if pq_pubkey.is_empty() || pq_signature.is_empty() {
        return Err(CryptoError::EmptySignature);
    }
    let body = HybridRootOpBody {
        domain: "HSM_ROOT_OP_HYBRID",
        op,
        share_pubkey: *share_pubkey,
        share_pq_pubkey: pq_pubkey,
    };
    let bytes = encode_cbor(&body).map_err(|_| CryptoError::CborEncode)?;
    verify_pq(pq_pubkey, &bytes, pq_signature)
}
