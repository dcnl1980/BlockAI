use blockai_shard::{ShardState, Wal, WalRecord};
use blockai_types::{AmountMicros, CapabilityId, Epoch, Sequence};
use tempfile::tempdir;

#[test]
fn wal_replay_restores_consumed_sequences() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shard.wal");
    let mut wal = Wal::open(&path).unwrap();
    let cap = CapabilityId([1u8; 32]);
    wal.append(&WalRecord::ActivateCapability {
        capability_id: cap,
        epoch: Epoch(1),
        remaining: AmountMicros(100),
        sequence_start: Sequence(1),
        sequence_end: Sequence(100),
    })
    .unwrap();
    wal.append(&WalRecord::ConsumePay {
        tx_id: [9u8; 32],
        capability_id: cap,
        epoch: Epoch(1),
        sequence: Sequence(1),
        amount: AmountMicros(5),
    })
    .unwrap();

    let mut state = wal.replay().unwrap();
    assert_eq!(state.remaining(&cap).unwrap(), AmountMicros(95));
    assert!(state.is_consumed(cap, Epoch(1), Sequence(1)));
    assert!(state.try_mark_consumed(cap, Epoch(1), Sequence(1)).is_err());
}

#[test]
fn shard_state_consume_increments_commit_index() {
    let mut state = ShardState::new();
    let cap = CapabilityId([2u8; 32]);
    state.activate_capability(cap, Epoch(1), AmountMicros(50), Sequence(1), Sequence(10));
    let idx = state
        .consume_pay(cap, Epoch(1), Sequence(1), AmountMicros(7))
        .unwrap();
    assert_eq!(idx, 1);
}
