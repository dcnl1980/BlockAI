use blockai_execute::GlobalState;
use blockai_types::{AccountId, AmountMicros, L1Tx, ShardId};

#[test]
fn reallocate_conserves_supply() {
    let mut state = GlobalState::new(2);
    let account = AccountId([1u8; 32]);
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(100),
        })
        .unwrap();
    state
        .apply(&L1Tx::AllocateShardAllowance {
            account,
            shard_id: fra.clone(),
            amount: AmountMicros(40),
        })
        .unwrap();
    state
        .apply(&L1Tx::ReallocateShardOutstanding {
            account,
            from_shard: fra.clone(),
            to_shard: ams.clone(),
            amount: AmountMicros(15),
        })
        .unwrap();
    state.check_conservation().unwrap();
    assert_eq!(
        state
            .shard_outstanding
            .get(&(fra.as_str().to_string(), account))
            .unwrap()
            .0,
        25
    );
    assert_eq!(
        state
            .shard_outstanding
            .get(&(ams.as_str().to_string(), account))
            .unwrap()
            .0,
        15
    );
}
