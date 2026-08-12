use blockai_types::{
    receipt_leaf_hash, AgentAuthorization, AmountMicros, EdgeAcceptance, PaymentProof,
    ServiceReceipt,
};

fn sample_proof(execution_hash: [u8; 32]) -> PaymentProof {
    PaymentProof {
        agent: AgentAuthorization {
            pay_cbor_hash: [1u8; 32],
            agent_signature: vec![2u8; 64],
        },
        edge: EdgeAcceptance {
            agent_auth_hash: [3u8; 32],
            commit_index: 1,
            tx_id: [4u8; 32],
            edge_pubkey: [5u8; 32],
            edge_signature: vec![6u8; 64],
            edge_pq_pubkey: vec![],
            edge_pq_signature: vec![],
        },
        service: ServiceReceipt {
            edge_accept_hash: [7u8; 32],
            execution_hash,
            actual_amount: AmountMicros(100),
            service_pubkey: [8u8; 32],
            service_signature: vec![9u8; 64],
            service_pq_pubkey: vec![],
            service_pq_signature: vec![],
        },
    }
}

#[test]
fn receipt_leaf_hash_changes_when_execution_hash_changes() {
    let a = receipt_leaf_hash(&sample_proof([10u8; 32])).unwrap();
    let b = receipt_leaf_hash(&sample_proof([11u8; 32])).unwrap();
    assert_ne!(a, b);
}
