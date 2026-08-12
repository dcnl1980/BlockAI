use blockai_consensus::cluster4;
use blockai_crypto::Keypair;
use blockai_shard::{complete_payment_proof, CheckpointSealer, EdgeAccept, ReceiptLog};
use blockai_types::{
    AccountId, AgentId, AmountMicros, CapabilityId, Epoch, L1Tx, Pay, Sequence, ShardId,
    WitnessedCheckpoint,
};
use blockai_witness::Witness;

fn make_witnessed(exposure: AmountMicros) -> WitnessedCheckpoint {
    let mut log = ReceiptLog::new();
    let mut sealer = CheckpointSealer::new(1, AmountMicros(u128::MAX));
    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();
    let pay = Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId([2u8; 32]),
        service_id: "inference/x".into(),
        amount: exposure,
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: exposure,
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![6u8; 64],
    ..Default::default()
    };
    let edge = EdgeAccept {
        commit_index: 1,
        tx_id: [7u8; 32],
        edge_signature: vec![8u8; 64],
    };
    complete_payment_proof(
        &pay,
        &edge,
        &edge_kp,
        [9u8; 32],
        exposure,
        &service_kp,
        &mut log,
    )
    .unwrap();
    let sealed = sealer
        .force_seal(
            &mut log,
            &edge_kp,
            ShardId::new("FRA-004").unwrap(),
            Epoch(1),
            1_000,
        )
        .unwrap();
    let w1 = Witness::generate();
    let w2 = Witness::generate();
    WitnessedCheckpoint {
        checkpoint: sealed.clone(),
        witness_sigs: vec![
            w1.countersign(&sealed).unwrap(),
            w2.countersign(&sealed).unwrap(),
        ],
    }
}

#[tokio::test]
async fn commit_checkpoint_updates_global_state() {
    let cluster = cluster4(2).await;
    let account = AccountId([1u8; 32]);
    let shard = ShardId::new("FRA-004").unwrap();
    let exposure = AmountMicros(10);

    cluster
        .leader()
        .submit_and_commit(vec![
            L1Tx::GenesisFund {
                account,
                amount: AmountMicros(100),
            },
            L1Tx::Stake {
                account,
                amount: AmountMicros(20),
            },
            L1Tx::AllocateShardAllowance {
                account,
                shard_id: shard,
                amount: AmountMicros(30),
            },
        ])
        .await
        .unwrap();

    let witnessed = make_witnessed(exposure);
    cluster
        .leader()
        .submit_and_commit(vec![L1Tx::CheckpointFinalized {
            checkpoint: witnessed,
            funding_account: account,
        }])
        .await
        .unwrap();

    let state = cluster.leader().state_snapshot().await;
    state.check_conservation().unwrap();
    assert_eq!(state.shard_outstanding_sum(), AmountMicros(20));
    assert_eq!(state.accounts[&account].balance_locked, AmountMicros(10));
    assert_eq!(state.total_supply, AmountMicros(100));
    assert!(state
        .events
        .iter()
        .any(|e| e.starts_with("CheckpointFinalized")));
}
