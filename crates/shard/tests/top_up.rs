use blockai_shard::{ShardState, Wal, WalRecord};
use blockai_types::{AmountMicros, CapabilityId, Epoch, Sequence};
use tempfile::tempdir;

#[test]
fn top_up_increases_remaining_and_wal_replays() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("topup.wal");
    let mut wal = Wal::open(&path).unwrap();
    let cap = CapabilityId([3u8; 32]);
    wal.append(&WalRecord::ActivateCapability {
        capability_id: cap,
        epoch: Epoch(1),
        remaining: AmountMicros(5),
        sequence_start: Sequence(1),
        sequence_end: Sequence(100),
    })
    .unwrap();
    wal.append(&WalRecord::TopUpCapability {
        capability_id: cap,
        amount: AmountMicros(7),
    })
    .unwrap();
    drop(wal);

    let state = Wal::open(&path).unwrap().replay().unwrap();
    assert_eq!(state.remaining(&cap).unwrap(), AmountMicros(12));
}

#[test]
fn top_up_unknown_capability_fails() {
    let mut state = ShardState::new();
    assert!(state
        .top_up(CapabilityId([1u8; 32]), AmountMicros(1))
        .is_err());
}
