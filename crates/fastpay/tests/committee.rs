use blockai_fastpay::{
    RegionalCommittee, RegionalOp, FastPayError, REGIONAL_QUORUM,
};
use blockai_types::{AccountId, AmountMicros, CapabilityId, ShardId};

#[test]
fn three_of_four_seals_and_verifies_reallocate() {
    let mut committee = RegionalCommittee::generate();
    let op = RegionalOp::Reallocate {
        account: AccountId([1u8; 32]),
        from_shard: ShardId::new("FRA-004").unwrap(),
        to_shard: ShardId::new("AMS-001").unwrap(),
        amount: AmountMicros(10),
        nonce: 1,
    };
    let cert = committee.sign_with(&op, &[0, 1, 2]).unwrap();
    committee.consume(&cert, REGIONAL_QUORUM).unwrap();
    // nonce single-use
    assert_eq!(
        committee.verify(&cert, REGIONAL_QUORUM).unwrap_err(),
        FastPayError::NonceConsumed
    );
}

#[test]
fn two_shares_fail_quorum() {
    let committee = RegionalCommittee::generate();
    let op = RegionalOp::TopUpCapability {
        account: AccountId([1u8; 32]),
        shard_id: ShardId::new("FRA-004").unwrap(),
        capability_id: CapabilityId([9u8; 32]),
        amount: AmountMicros(5),
        nonce: 2,
    };
    let cert = committee.sign_with(&op, &[0, 1]).unwrap();
    assert_eq!(
        committee.verify(&cert, REGIONAL_QUORUM).unwrap_err(),
        FastPayError::InsufficientShares {
            have: 2,
            need: REGIONAL_QUORUM
        }
    );
}

#[test]
fn same_shard_reallocate_rejected() {
    let committee = RegionalCommittee::generate();
    let shard = ShardId::new("FRA-004").unwrap();
    let op = RegionalOp::Reallocate {
        account: AccountId([1u8; 32]),
        from_shard: shard.clone(),
        to_shard: shard,
        amount: AmountMicros(1),
        nonce: 3,
    };
    assert_eq!(
        committee.sign_with(&op, &[0, 1, 2]).unwrap_err(),
        FastPayError::SameShard
    );
}
