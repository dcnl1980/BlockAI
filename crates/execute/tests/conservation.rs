use blockai_execute::GlobalState;
use blockai_types::{AccountId, AmountMicros, L1Tx, ShardId};

#[test]
fn genesis_allocate_conserves_supply() {
    let mut state = GlobalState::new(2);
    let account = AccountId([1u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(100),
        })
        .unwrap();
    state
        .apply(&L1Tx::AllocateShardAllowance {
            account,
            shard_id: ShardId::new("FRA-004").unwrap(),
            amount: AmountMicros(40),
        })
        .unwrap();
    assert_eq!(state.total_supply, AmountMicros(100));
    assert_eq!(state.available_sum(), AmountMicros(60));
    assert_eq!(state.shard_outstanding_sum(), AmountMicros(40));
    state.check_conservation().unwrap();
}

#[test]
fn stake_unstake_conserves() {
    let mut state = GlobalState::new(2);
    let account = AccountId([1u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(50),
        })
        .unwrap();
    state
        .apply(&L1Tx::Stake {
            account,
            amount: AmountMicros(20),
        })
        .unwrap();
    assert_eq!(state.available_sum(), AmountMicros(30));
    state
        .apply(&L1Tx::Unstake {
            account,
            amount: AmountMicros(5),
        })
        .unwrap();
    assert_eq!(state.available_sum(), AmountMicros(35));
    state.check_conservation().unwrap();
}
