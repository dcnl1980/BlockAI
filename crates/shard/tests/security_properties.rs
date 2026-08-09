use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_shard::ShardError;
use blockai_types::{
    tx_id, AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId, SpendCapability,
};

fn fund_issue(
    auth: &mut Authority,
    account: AccountId,
    agent: AgentId,
    shard: ShardId,
    per_call: AmountMicros,
) -> SpendCapability {
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(20))
        .unwrap();
    let __evidence = auth.passing_attestation();
    auth.issue_capability(IssueRequest {
        account_id: account,
        agent_id: agent,
        shard_id: shard,
        epoch: Epoch(1),
        maximum_total: AmountMicros(20),
        maximum_per_call: per_call,
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(1),
        sequence_end: Sequence(100),
        ttl_ms: 60_000,
        region: "EU".into(),
        now_unix_ms: 1_000,
    }, &__evidence)
    .unwrap()
}

fn signed_pay(agent_kp: &Keypair, cap: &SpendCapability, seq: u64, amount: AmountMicros) -> Pay {
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(seq),
        agent_id: AgentId(agent_kp.verifying_key_bytes()),
        service_id: "inference/x".into(),
        amount,
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = sign_pay(agent_kp, &pay);
    pay
}

#[tokio::test]
async fn foreign_shard_capability_rejected() {
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, ams, AmountMicros(5));
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        let _ = eng.activate_capability(cap.clone()).await;
    }
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert!(matches!(err, ShardError::WrongShard { .. }));
}

#[tokio::test]
async fn fenced_epoch_rejects_new_pays() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, fra.clone(), AmountMicros(5));
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    cluster.leader().fence_epoch(Epoch(1)).await.unwrap();
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert!(matches!(err, ShardError::EpochFenced { .. }));
}

#[tokio::test]
async fn over_per_call_rejected() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, fra.clone(), AmountMicros(2));
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(3));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert!(matches!(err, ShardError::ExceedsPerCall { .. }));
}

#[tokio::test]
async fn kill_one_validator_still_safe_no_double_spend() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(&mut auth, account, agent, fra.clone(), AmountMicros(5));
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let pay1 = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let accept = cluster.leader().handle_pay(pay1.clone(), 1_100).await.unwrap();
    assert_eq!(accept.tx_id, tx_id(&pay1));
    cluster.kill(2).await;
    let err = cluster.leader().handle_pay(pay1, 1_101).await.unwrap_err();
    assert!(matches!(err, ShardError::Replay { .. }));
    let pay2 = signed_pay(&agent_kp, &cap, 2, AmountMicros(1));
    let accept2 = cluster.leader().handle_pay(pay2, 1_102).await.unwrap();
    assert!(accept2.commit_index > accept.commit_index);
}
