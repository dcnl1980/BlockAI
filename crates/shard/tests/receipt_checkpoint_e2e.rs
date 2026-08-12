use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_shard::{
    complete_payment_proof, merkle_proof, verify_merkle_proof, CheckpointSealer, ReceiptLog,
};
use blockai_types::{
    receipt_leaf_hash, AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId,
    WitnessedCheckpoint,
};
use blockai_witness::{verify_witnessed, Witness};

#[tokio::test]
async fn pay_receipt_checkpoint_witness_merkle_path() {
    let shard = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(50))
        .unwrap();
    let __evidence = auth.passing_attestation();
    let cap = auth
        .issue_capability(IssueRequest {
            account_id: account,
            agent_id: agent,
            shard_id: shard.clone(),
            epoch: Epoch(1),
            maximum_total: AmountMicros(50),
            maximum_per_call: AmountMicros(5),
            service_scope: vec!["inference/*".into()],
            policy_hash: [9u8; 32],
            sequence_start: Sequence(1),
            sequence_end: Sequence(100),
            ttl_ms: 60_000,
            region: "EU".into(),
            now_unix_ms: 1_000,
        }, &__evidence)
        .unwrap();

    let cluster = cluster4_with_issuer_bytes(shard.clone(), auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }

    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();
    let mut log = ReceiptLog::new();
    let mut sealer = CheckpointSealer::new(2, AmountMicros(1_000_000));
    let mut first_leaf = None;

    for i in 1..=2u64 {
        let mut pay = Pay {
            capability_id: cap.capability_id,
            epoch: cap.epoch,
            sequence: Sequence(i),
            agent_id: agent,
            service_id: "inference/x".into(),
            amount: AmountMicros(2),
            currency: "EURC".into(),
            request_hash: [i as u8; 32],
            price_quote_hash: [5u8; 32],
            max_amount: AmountMicros(5),
            pricing_schedule_version: 1,
            expiry_unix_ms: 9_999_999_999,
            agent_signature: vec![],
        ..Default::default()
        };
        pay.agent_signature = sign_pay(&agent_kp, &pay);
        let accept = cluster.leader().handle_pay(pay.clone(), 1_100).await.unwrap();
        let proof = complete_payment_proof(
            &pay,
            &accept,
            &edge_kp,
            [20 + i as u8; 32],
            AmountMicros(2),
            &service_kp,
            &mut log,
        )
        .unwrap();
        if i == 1 {
            first_leaf = Some(receipt_leaf_hash(&proof).unwrap());
        }
    }

    let leaves = log.leaves().to_vec();
    let sealed = sealer
        .force_seal(&mut log, &edge_kp, shard, Epoch(1), 2_000)
        .unwrap();
    assert_eq!(sealed.header.tx_count, 2);

    let w1 = Witness::generate();
    let w2 = Witness::generate();
    let w3 = Witness::generate();
    let witnessed = WitnessedCheckpoint {
        checkpoint: sealed.clone(),
        witness_sigs: vec![
            w1.countersign(&sealed).unwrap(),
            w2.countersign(&sealed).unwrap(),
            w3.countersign(&sealed).unwrap(),
        ],
    };
    verify_witnessed(&witnessed, 3).unwrap();

    let leaf = first_leaf.unwrap();
    let proof = merkle_proof(&leaves, 0).unwrap();
    assert!(verify_merkle_proof(leaf, &proof, sealed.header.root));
}
