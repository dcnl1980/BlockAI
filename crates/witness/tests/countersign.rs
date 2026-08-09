use blockai_crypto::Keypair;
use blockai_shard::{complete_payment_proof, CheckpointSealer, EdgeAccept, ReceiptLog};
use blockai_types::{
    AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence, ShardId, SignedCheckpoint,
    WitnessedCheckpoint,
};
use blockai_witness::{verify_witnessed, Witness, WitnessError, WitnessSet};

fn seal_one(height: u64, execution_hash: [u8; 32]) -> (SignedCheckpoint, Keypair) {
    let mut log = ReceiptLog::new();
    let mut sealer = CheckpointSealer {
        max_txs: 1,
        max_exposure: AmountMicros(1_000_000),
        next_height: height,
    };
    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();
    let pay = Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(height),
        agent_id: AgentId([2u8; 32]),
        service_id: "inference/x".into(),
        amount: AmountMicros(3),
        currency: "EURC".into(),
        request_hash: [height as u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(10),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![6u8; 64],
    };
    let edge = EdgeAccept {
        commit_index: height,
        tx_id: [height as u8; 32],
        edge_signature: vec![8u8; 64],
    };
    complete_payment_proof(
        &pay,
        &edge,
        &edge_kp,
        execution_hash,
        AmountMicros(3),
        &service_kp,
        &mut log,
    )
    .unwrap();
    let cp = sealer
        .force_seal(
            &mut log,
            &edge_kp,
            ShardId::new("FRA-004").unwrap(),
            Epoch(1),
            1_000,
        )
        .unwrap();
    (cp, edge_kp)
}

#[test]
fn two_of_three_witnesses_verify() {
    let (checkpoint, _) = seal_one(1, [9u8; 32]);
    let w1 = Witness::generate();
    let w2 = Witness::generate();
    let witnessed = WitnessedCheckpoint {
        checkpoint: checkpoint.clone(),
        witness_sigs: vec![
            w1.countersign(&checkpoint).unwrap(),
            w2.countersign(&checkpoint).unwrap(),
        ],
    };
    assert!(verify_witnessed(&witnessed, 2).is_ok());
    assert!(verify_witnessed(&witnessed, 3).is_err());
}

#[test]
fn conflicting_roots_rejected() {
    let (first, _) = seal_one(1, [9u8; 32]);
    let (second, _) = seal_one(1, [10u8; 32]);
    assert_eq!(first.header.height, second.header.height);
    assert_ne!(first.header.root, second.header.root);

    let w1 = Witness::generate();
    let w2 = Witness::generate();
    let mut set = WitnessSet::new();
    let first_w = WitnessedCheckpoint {
        checkpoint: first.clone(),
        witness_sigs: vec![
            w1.countersign(&first).unwrap(),
            w2.countersign(&first).unwrap(),
        ],
    };
    set.accept(&first_w, 2).unwrap();

    let second_w = WitnessedCheckpoint {
        checkpoint: second.clone(),
        witness_sigs: vec![
            w1.countersign(&second).unwrap(),
            w2.countersign(&second).unwrap(),
        ],
    };
    assert!(matches!(
        set.accept(&second_w, 2),
        Err(WitnessError::ConflictingCheckpoint)
    ));
}
