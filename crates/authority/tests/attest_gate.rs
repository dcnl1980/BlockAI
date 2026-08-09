use blockai_attest::TestPlatform;
use blockai_authority::{Authority, AuthorityError, IssueRequest};
use blockai_crypto::{verify_capability_hybrid, AlgorithmId};
use blockai_types::{AccountId, AgentId, AmountMicros, Epoch, Sequence, ShardId};

#[test]
fn issue_requires_valid_attestation() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent = AgentId([2u8; 32]);
    auth.fund(account, AmountMicros(100)).unwrap();
    let fra = ShardId::new("FRA-004").unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(20))
        .unwrap();

    let req = IssueRequest {
        account_id: account,
        agent_id: agent,
        shard_id: fra,
        epoch: Epoch(1),
        maximum_total: AmountMicros(20),
        maximum_per_call: AmountMicros(1),
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(1),
        sequence_end: Sequence(10),
        ttl_ms: 60_000,
        region: "EU".into(),
        now_unix_ms: 1_000,
    };

    let foreign = TestPlatform::new().evidence();
    assert_eq!(
        auth.issue_capability(req.clone(), &foreign).unwrap_err(),
        AuthorityError::AttestationFailed
    );

    let ok = auth.passing_attestation();
    let cap = auth.issue_capability(req, &ok).unwrap();
    assert_eq!(
        AlgorithmId::from_u16(cap.issuer_alg),
        Some(AlgorithmId::HybridEd25519MlDsa65)
    );
    verify_capability_hybrid(&cap).unwrap();
}
