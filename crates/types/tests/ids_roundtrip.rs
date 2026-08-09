use blockai_types::{AmountMicros, Epoch, Sequence, ShardId};

#[test]
fn amount_micros_display_and_eq() {
    let a = AmountMicros(1_000_000);
    assert_eq!(a.0, 1_000_000);
    assert_eq!(a, AmountMicros(1_000_000));
}

#[test]
fn shard_id_rejects_empty() {
    assert!(ShardId::new("FRA-004").is_ok());
    assert!(ShardId::new("").is_err());
}

#[test]
fn epoch_and_sequence_order() {
    assert!(Epoch(1) < Epoch(2));
    assert!(Sequence(10) < Sequence(11));
}
