use blockai_authority::Authority;
use blockai_types::{AccountId, AmountMicros, ShardId};

#[test]
fn reallocate_moves_shard_allowance() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(40)).unwrap();
    auth.reallocate(account, fra.clone(), ams.clone(), AmountMicros(15))
        .unwrap();
    assert_eq!(
        auth.shard_allowance(account, &fra).unwrap(),
        AmountMicros(25)
    );
    assert_eq!(
        auth.shard_allowance(account, &ams).unwrap(),
        AmountMicros(15)
    );
    assert_eq!(auth.reserve(account).unwrap(), AmountMicros(60));
}

#[test]
fn reallocate_cannot_overdraw() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(10)).unwrap();
    assert!(auth
        .reallocate(account, fra, ams, AmountMicros(11))
        .is_err());
}
