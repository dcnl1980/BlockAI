use blockai_crypto::{
    seal_pay_hybrid, verify_pay, verify_pay_hybrid, AlgorithmId, Keypair, PqKeypair,
};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};

fn sample_pay(agent: &Keypair) -> Pay {
    Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId(agent.verifying_key_bytes()),
        service_id: "inference/x".into(),
        amount: AmountMicros(100),
        currency: "EURC".into(),
        request_hash: [2u8; 32],
        price_quote_hash: [3u8; 32],
        max_amount: AmountMicros(100),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
        ..Default::default()
    }
}

#[test]
fn hybrid_pay_roundtrip() {
    let classical = Keypair::generate();
    let pq = PqKeypair::generate();
    let mut pay = sample_pay(&classical);
    seal_pay_hybrid(&classical, &pq, &mut pay).unwrap();
    assert_eq!(
        AlgorithmId::from_u16(pay.agent_alg),
        Some(AlgorithmId::HybridEd25519MlDsa65)
    );
    assert!(verify_pay_hybrid(&pay).is_ok());
    assert!(verify_pay(&classical.verifying_key(), &pay).is_ok());
}

#[test]
fn hybrid_pay_rejects_tamper_and_missing_pq() {
    let classical = Keypair::generate();
    let pq = PqKeypair::generate();
    let mut pay = sample_pay(&classical);
    seal_pay_hybrid(&classical, &pq, &mut pay).unwrap();
    pay.amount = AmountMicros(101);
    assert!(verify_pay_hybrid(&pay).is_err());

    let mut pay = sample_pay(&classical);
    seal_pay_hybrid(&classical, &pq, &mut pay).unwrap();
    pay.agent_pq_signature.clear();
    assert!(verify_pay_hybrid(&pay).is_err());
}
