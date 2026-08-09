use crate::receipt_log::ReceiptLog;
use crate::{EdgeAccept, ShardError};
use blockai_crypto::{
    sign_edge_acceptance, sign_service_receipt, Keypair,
};
use blockai_types::{
    encode_cbor, hash_cbor, AgentAuthorization, AmountMicros, EdgeAcceptance, Pay, PaymentProof,
    ServiceReceipt,
};

/// Build A/E/S payment proof from an authorized PAY + edge accept + service execution, append to log.
pub fn complete_payment_proof(
    pay: &Pay,
    edge_accept: &EdgeAccept,
    edge_kp: &Keypair,
    execution_hash: [u8; 32],
    actual_amount: AmountMicros,
    service_kp: &Keypair,
    log: &mut ReceiptLog,
) -> Result<PaymentProof, ShardError> {
    if actual_amount.0 > pay.max_amount.0 {
        return Err(ShardError::ExceedsMaxAmount);
    }

    let mut pay_for_hash = pay.clone();
    // Leaf identity uses pay fields excluding relying solely on signature bytes variability —
    // hash full CBOR of pay as authorized.
    let pay_cbor_hash = {
        let bytes = encode_cbor(&pay_for_hash).map_err(|_| ShardError::Cbor)?;
        *blake3::hash(&bytes).as_bytes()
    };
    let _ = &mut pay_for_hash;

    let agent = AgentAuthorization {
        pay_cbor_hash,
        agent_signature: pay.agent_signature.clone(),
    };
    let agent_auth_hash = hash_cbor(&agent).map_err(|_| ShardError::Cbor)?;

    let mut edge = EdgeAcceptance {
        agent_auth_hash,
        commit_index: edge_accept.commit_index,
        tx_id: edge_accept.tx_id,
        edge_pubkey: edge_kp.verifying_key_bytes(),
        edge_signature: vec![],
    };
    edge.edge_signature = sign_edge_acceptance(edge_kp, &edge);
    let edge_accept_hash = hash_cbor(&edge).map_err(|_| ShardError::Cbor)?;

    let mut service = ServiceReceipt {
        edge_accept_hash,
        execution_hash,
        actual_amount,
        service_pubkey: service_kp.verifying_key_bytes(),
        service_signature: vec![],
    };
    service.service_signature = sign_service_receipt(service_kp, &service);

    let proof = PaymentProof {
        agent,
        edge,
        service,
    };
    log.append(proof.clone())?;
    Ok(proof)
}
