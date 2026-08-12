use blockai_crypto::Keypair;
use blockai_shard::{
    complete_payment_proof, merkle_root, verify_signed_checkpoint, CheckpointSealer, EdgeAccept,
    ReceiptLog,
};
use blockai_types::{
    AmountMicros, Epoch, Pay, Sequence, ShardId, CapabilityId, AgentId,
};

#[test]
fn seals_after_two_payments_with_correct_counts() {
    let mut log = ReceiptLog::new();
    let mut sealer = CheckpointSealer::new(2, AmountMicros(1_000_000));
    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();
    let shard = ShardId::new("FRA-004").unwrap();

    for i in 1..=2u64 {
        let pay = Pay {
            capability_id: CapabilityId([1u8; 32]),
            epoch: Epoch(1),
            sequence: Sequence(i),
            agent_id: AgentId([2u8; 32]),
            service_id: "inference/x".into(),
            amount: AmountMicros(5),
            currency: "EURC".into(),
            request_hash: [i as u8; 32],
            price_quote_hash: [3u8; 32],
            max_amount: AmountMicros(10),
            pricing_schedule_version: 1,
            expiry_unix_ms: 9_999_999_999,
            agent_signature: vec![4u8; 64],
        ..Default::default()
        };
        let edge = EdgeAccept {
            commit_index: i,
            tx_id: [i as u8; 32],
            edge_signature: vec![5u8; 64],
        };
        complete_payment_proof(
            &pay,
            &edge,
            &edge_kp,
            [9u8; 32],
            AmountMicros(5),
            &service_kp,
            &mut log,
        )
        .unwrap();
    }

    let leaves = log.leaves().to_vec();
    let expected_root = merkle_root(&leaves);
    let sealed = sealer
        .maybe_seal(
            &mut log,
            &edge_kp,
            shard,
            Epoch(1),
            1_000,
        )
        .unwrap()
        .expect("sealed");
    assert_eq!(sealed.header.tx_count, 2);
    assert_eq!(sealed.header.exposure, AmountMicros(10));
    assert_eq!(sealed.header.root, expected_root);
    assert!(verify_signed_checkpoint(&sealed).is_ok());
    assert!(log.is_empty());
}
