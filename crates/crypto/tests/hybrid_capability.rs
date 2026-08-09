use blockai_crypto::{
    seal_capability_hybrid, verify_capability_hybrid, AlgorithmId, Keypair, PqKeypair,
};
use blockai_types::{
    AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Sequence, ShardId, SpendCapability,
};

fn sample_cap() -> SpendCapability {
    SpendCapability {
        capability_id: CapabilityId([1u8; 32]),
        account_id: AccountId([2u8; 32]),
        agent_id: AgentId([3u8; 32]),
        shard_id: ShardId::new("FRA-004").unwrap(),
        epoch: Epoch(1),
        currency: "EURC".into(),
        maximum_total: AmountMicros(20),
        maximum_per_call: AmountMicros(1),
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(1),
        sequence_end: Sequence(10),
        valid_from_unix_ms: 0,
        valid_until_unix_ms: 9_999,
        region: "EU".into(),
        issuer_alg: AlgorithmId::Ed25519.as_u16(),
        issuer_pubkey: [0u8; 32],
        issuer_signature: vec![],
        issuer_pq_pubkey: vec![],
        issuer_pq_signature: vec![],
    }
}

#[test]
fn hybrid_sign_verify_roundtrip() {
    let classical = Keypair::generate();
    let pq = PqKeypair::generate();
    let mut cap = sample_cap();
    seal_capability_hybrid(&classical, &pq, &mut cap).unwrap();
    assert_eq!(
        AlgorithmId::from_u16(cap.issuer_alg),
        Some(AlgorithmId::HybridEd25519MlDsa65)
    );
    verify_capability_hybrid(&cap).unwrap();
}

#[test]
fn hybrid_rejects_tampered_pq_half() {
    let classical = Keypair::generate();
    let pq = PqKeypair::generate();
    let mut cap = sample_cap();
    seal_capability_hybrid(&classical, &pq, &mut cap).unwrap();
    cap.issuer_pq_signature[0] ^= 0xff;
    assert!(verify_capability_hybrid(&cap).is_err());
}
