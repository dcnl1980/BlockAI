use blockai_execute::{ExecuteError, GlobalState};
use blockai_types::{
    AccountId, AmountMicros, GovernanceAction, L1Tx, ProposalStatus,
};

#[test]
fn fees_and_rewards_conserve() {
    let mut state = GlobalState::new(2);
    let a = AccountId([1u8; 32]);
    let v1 = AccountId([2u8; 32]);
    let v2 = AccountId([3u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account: a,
            amount: AmountMicros(100),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: v1,
            amount: AmountMicros(0),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: v2,
            amount: AmountMicros(0),
        })
        .unwrap();
    for _ in 0..10 {
        state.apply(&L1Tx::ChargeBaseFee { payer: a }).unwrap();
    }
    assert_eq!(state.fee_treasury.0, 10);
    state
        .apply(&L1Tx::DistributeRewards {
            recipients: vec![v1, v2],
            total: AmountMicros(10),
        })
        .unwrap();
    assert_eq!(state.fee_treasury.0, 0);
    state.check_conservation().unwrap();
}

#[test]
fn governance_sets_min_stake_after_quorum() {
    let mut state = GlobalState::new(2);
    let proposer = AccountId([1u8; 32]);
    let voter = AccountId([2u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account: proposer,
            amount: AmountMicros(1_000),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: voter,
            amount: AmountMicros(1_000),
        })
        .unwrap();
    state
        .apply(&L1Tx::Stake {
            account: voter,
            amount: AmountMicros(100),
        })
        .unwrap();
    let id = [9u8; 32];
    state
        .apply(&L1Tx::ProposeGovernance {
            id,
            proposer,
            action: GovernanceAction::SetMinStake {
                value: AmountMicros(50),
            },
        })
        .unwrap();
    state
        .apply(&L1Tx::VoteGovernance {
            id,
            voter,
            approve: true,
        })
        .unwrap();
    state.apply(&L1Tx::FinalizeGovernance { id }).unwrap();
    assert_eq!(state.economics.min_stake, AmountMicros(50));
    assert_eq!(
        state.proposals[&id].status,
        ProposalStatus::Executed
    );
    // Stake below new minimum fails.
    assert_eq!(
        state
            .apply(&L1Tx::Stake {
                account: proposer,
                amount: AmountMicros(10),
            })
            .unwrap_err(),
        ExecuteError::StakeBelowMinimum
    );
    state.check_conservation().unwrap();
}
