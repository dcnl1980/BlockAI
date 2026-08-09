use blockai_authority::{Authority, IssueRequest};
use blockai_consensus::cluster4 as l1_cluster4;
use blockai_crypto::{sign_pay, verify_capability_hybrid, AlgorithmId, Keypair};
use blockai_net::{admit_frame, AppFrame};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_shard::{complete_payment_proof, CheckpointSealer, ReceiptLog};
use blockai_types::{
    AccountId, AgentId, AmountMicros, Epoch, L1Tx, Pay, Sequence, ShardId, WitnessedCheckpoint,
};
use blockai_witness::Witness;

#[tokio::test]
async fn attest_quic_policy_shard_receipt_l1_conservation() {
    let shard = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    auth.fund(account, AmountMicros(1_000)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(100))
        .unwrap();
    let evidence = auth.passing_attestation();
    let foreign = blockai_attest::TestPlatform::new().evidence();
    let req = IssueRequest {
        account_id: account,
        agent_id: agent,
        shard_id: shard.clone(),
        epoch: Epoch(1),
        maximum_total: AmountMicros(50),
        maximum_per_call: AmountMicros(10),
        service_scope: vec!["inference/*".into()],
        policy_hash: [9u8; 32],
        sequence_start: Sequence(1),
        sequence_end: Sequence(10),
        ttl_ms: 60_000,
        region: "EU".into(),
        now_unix_ms: 1_000,
    };
    assert!(auth.issue_capability(req.clone(), &foreign).is_err());
    let cap = auth.issue_capability(req, &evidence).unwrap();
    assert_eq!(
        AlgorithmId::from_u16(cap.issuer_alg),
        Some(AlgorithmId::HybridEd25519MlDsa65)
    );
    verify_capability_hybrid(&cap).unwrap();

    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(1),
        agent_id: agent,
        service_id: "inference/x".into(),
        amount: AmountMicros(10),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(10),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    };
    pay.agent_signature = sign_pay(&agent_kp, &pay);

    // Transport policy: 0-RTT PAY forbidden; 1-RTT OK.
    assert!(admit_frame(true, &AppFrame::Pay { pay: pay.clone() }).is_err());
    admit_frame(false, &AppFrame::Pay { pay: pay.clone() }).unwrap();

    let shard_cluster =
        cluster4_with_issuer_bytes(shard.clone(), auth.issuer_signing_bytes_for_tests()).await;
    for eng in shard_cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let edge = shard_cluster
        .leader()
        .handle_pay(pay.clone(), 1_100)
        .await
        .unwrap();

    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();
    let mut log = ReceiptLog::new();
    let mut sealer = CheckpointSealer::new(1, AmountMicros(u128::MAX));
    complete_payment_proof(
        &pay,
        &edge,
        &edge_kp,
        [9u8; 32],
        AmountMicros(10),
        &service_kp,
        &mut log,
    )
    .unwrap();
    let sealed = sealer
        .force_seal(&mut log, &edge_kp, shard.clone(), Epoch(1), 2_000)
        .unwrap();
    let w1 = Witness::generate();
    let w2 = Witness::generate();
    let witnessed = WitnessedCheckpoint {
        checkpoint: sealed.clone(),
        witness_sigs: vec![
            w1.countersign(&sealed).unwrap(),
            w2.countersign(&sealed).unwrap(),
        ],
    };

    let l1 = l1_cluster4(2).await;
    l1.leader()
        .submit_and_commit(vec![
            L1Tx::GenesisFund {
                account,
                amount: AmountMicros(1_000),
            },
            L1Tx::AllocateShardAllowance {
                account,
                shard_id: shard,
                amount: AmountMicros(100),
            },
        ])
        .await
        .unwrap();
    l1.leader()
        .submit_and_commit(vec![L1Tx::CheckpointFinalized {
            checkpoint: witnessed,
            funding_account: account,
        }])
        .await
        .unwrap();
    let state = l1.leader().state_snapshot().await;
    state.check_conservation().unwrap();
    assert_eq!(state.accounts[&account].balance_locked.0, 10);
    assert_eq!(state.shard_outstanding_sum().0, 90);
}
