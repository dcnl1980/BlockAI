use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_types::{
    tx_id, AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId,
};

#[tokio::test]
async fn three_of_four_commits_pay_before_accept() {
    let shard = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(20))
        .unwrap();
    let cap = auth
        .issue_capability(IssueRequest {
            account_id: account,
            agent_id: agent,
            shard_id: shard.clone(),
            epoch: Epoch(1),
            maximum_total: AmountMicros(20),
            maximum_per_call: AmountMicros(5),
            service_scope: vec!["inference/*".into()],
            policy_hash: [9u8; 32],
            sequence_start: Sequence(1),
            sequence_end: Sequence(100),
            ttl_ms: 60_000,
            region: "EU".into(),
            now_unix_ms: 1_000,
        })
        .unwrap();

    let cluster = cluster4_with_issuer_bytes(shard, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }

    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(1),
        agent_id: agent,
        service_id: "inference/x".into(),
        amount: AmountMicros(3),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = sign_pay(&agent_kp, &pay);

    let accept = cluster.leader().handle_pay(pay.clone(), 1_100).await.unwrap();
    assert_eq!(accept.tx_id, tx_id(&pay));
    assert!(accept.commit_index >= 1);

    let err = cluster.leader().handle_pay(pay, 1_101).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("REPLAY") || msg.contains("consumed") || msg.contains("Replay"));
}
