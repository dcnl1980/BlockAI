use blockai_types::{
    tx_id, AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence, ShardId,
    SpendCapability,
};

fn sample_cap() -> SpendCapability {
    SpendCapability {
        capability_id: CapabilityId([1u8; 32]),
        account_id: AccountId([2u8; 32]),
        agent_id: AgentId([3u8; 32]),
        shard_id: ShardId::new("FRA-004").unwrap(),
        epoch: Epoch(1),
        currency: "EURC".into(),
        maximum_total: AmountMicros(20_000_000),
        maximum_per_call: AmountMicros(10_000),
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(100),
        sequence_end: Sequence(200),
        valid_from_unix_ms: 0,
        valid_until_unix_ms: 9_999_999_999,
        region: "EU".into(),
        issuer_alg: 1,
        issuer_pubkey: [7u8; 32],
        issuer_signature: vec![0u8; 64],
        issuer_pq_pubkey: vec![],
        issuer_pq_signature: vec![],
    }
}

#[test]
fn tx_id_changes_when_sequence_changes() {
    let cap = sample_cap();
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(100),
        agent_id: cap.agent_id,
        service_id: "inference/supernova".into(),
        amount: AmountMicros(1000),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(4000),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![1u8; 64],
    ..Default::default()
    };
    let a = tx_id(&pay);
    pay.sequence = Sequence(101);
    let b = tx_id(&pay);
    assert_ne!(a, b);
}

#[test]
fn tx_id_includes_request_hash() {
    let cap = sample_cap();
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(100),
        agent_id: cap.agent_id,
        service_id: "inference/supernova".into(),
        amount: AmountMicros(1000),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(4000),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![1u8; 64],
    ..Default::default()
    };
    let a = tx_id(&pay);
    pay.request_hash = [8u8; 32];
    assert_ne!(a, tx_id(&pay));
}
