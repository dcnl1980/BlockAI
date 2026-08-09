use blockai_crypto::{sign_pay, verify_pay, Keypair};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};

#[test]
fn pay_sign_and_verify_roundtrip() {
    let kp = Keypair::generate();
    let mut pay = Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId(kp.verifying_key_bytes()),
        service_id: "inference/x".into(),
        amount: AmountMicros(100),
        currency: "EURC".into(),
        request_hash: [2u8; 32],
        price_quote_hash: [3u8; 32],
        max_amount: AmountMicros(100),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = sign_pay(&kp, &pay);
    assert!(verify_pay(&kp.verifying_key(), &pay).is_ok());
    pay.amount = AmountMicros(101);
    assert!(verify_pay(&kp.verifying_key(), &pay).is_err());
}
