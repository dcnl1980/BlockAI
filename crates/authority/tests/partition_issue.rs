use blockai_authority::{Authority, IssueRequest};
use blockai_types::{AccountId, AgentId, AmountMicros, Epoch, Sequence, ShardId};

#[test]
fn cannot_over_allocate_across_shards() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    auth.fund(account, AmountMicros(100)).unwrap();
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(60)).unwrap();
    assert!(auth.allocate(account, ams, AmountMicros(50)).is_err());
}

#[test]
fn issued_capability_is_shard_bound_and_signed() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent = AgentId([2u8; 32]);
    auth.fund(account, AmountMicros(100)).unwrap();
    let fra = ShardId::new("FRA-004").unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(20)).unwrap();
    let cap = auth
        .issue_capability(IssueRequest {
            account_id: account,
            agent_id: agent,
            shard_id: fra.clone(),
            epoch: Epoch(1),
            maximum_total: AmountMicros(20),
            maximum_per_call: AmountMicros(1),
            service_scope: vec!["inference/*".into()],
            policy_hash: [9u8; 32],
            sequence_start: Sequence(1),
            sequence_end: Sequence(10_000),
            ttl_ms: 60_000,
            region: "EU".into(),
            now_unix_ms: 1_000,
        })
        .unwrap();
    assert_eq!(cap.shard_id, fra);
    assert_eq!(cap.maximum_total, AmountMicros(20));
    assert!(!cap.issuer_signature.is_empty());
}
