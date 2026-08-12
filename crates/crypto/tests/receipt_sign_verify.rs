use blockai_crypto::{
    sign_edge_acceptance, sign_service_receipt, verify_edge_acceptance, verify_service_receipt,
    Keypair,
};
use blockai_types::{AmountMicros, EdgeAcceptance, ServiceReceipt};

#[test]
fn edge_and_service_sign_verify_and_tamper() {
    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();

    let mut edge = EdgeAcceptance {
        agent_auth_hash: [1u8; 32],
        commit_index: 7,
        tx_id: [2u8; 32],
        edge_pubkey: edge_kp.verifying_key_bytes(),
        edge_signature: vec![],
        edge_pq_pubkey: vec![],
        edge_pq_signature: vec![],
    };
    edge.edge_signature = sign_edge_acceptance(&edge_kp, &edge);
    assert!(verify_edge_acceptance(&edge_kp.verifying_key(), &edge).is_ok());
    edge.commit_index = 8;
    assert!(verify_edge_acceptance(&edge_kp.verifying_key(), &edge).is_err());

    let mut service = ServiceReceipt {
        edge_accept_hash: [3u8; 32],
        execution_hash: [4u8; 32],
        actual_amount: AmountMicros(42),
        service_pubkey: service_kp.verifying_key_bytes(),
        service_signature: vec![],
        service_pq_pubkey: vec![],
        service_pq_signature: vec![],
    };
    service.service_signature = sign_service_receipt(&service_kp, &service);
    assert!(verify_service_receipt(&service_kp.verifying_key(), &service).is_ok());
    service.actual_amount = AmountMicros(43);
    assert!(verify_service_receipt(&service_kp.verifying_key(), &service).is_err());
}
