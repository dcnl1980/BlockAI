//! Plan 11 assurance drills — predictable outcomes for SEEF §9 / §10.3.

use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_shard::{ShardError, Wal, WalRecord};
use blockai_types::{
    AccountId, AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence, ShardId, SpendCapability,
};
use tempfile::tempdir;

fn fund_issue(
    auth: &mut Authority,
    account: AccountId,
    agent: AgentId,
    shard: ShardId,
    total: AmountMicros,
    per_call: AmountMicros,
    ttl_ms: u64,
    now_unix_ms: u64,
) -> SpendCapability {
    auth.fund(account, AmountMicros(10_000)).unwrap();
    auth.allocate(account, shard.clone(), total).unwrap();
    let evidence = auth.passing_attestation();
    auth.issue_capability(
        IssueRequest {
            account_id: account,
            agent_id: agent,
            shard_id: shard,
            epoch: Epoch(1),
            maximum_total: total,
            maximum_per_call: per_call,
            service_scope: vec!["inference/*".into()],
            policy_hash: [9u8; 32],
            sequence_start: Sequence(1),
            sequence_end: Sequence(1_000),
            ttl_ms,
            region: "EU".into(),
            now_unix_ms,
        },
        &evidence,
    )
    .unwrap()
}

fn signed_pay(agent_kp: &Keypair, cap: &SpendCapability, seq: u64, amount: AmountMicros) -> Pay {
    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(seq),
        agent_id: AgentId(agent_kp.verifying_key_bytes()),
        service_id: "inference/x".into(),
        amount,
        currency: "EURC".into(),
        request_hash: {
            let mut h = [0u8; 32];
            h[0..8].copy_from_slice(&seq.to_le_bytes());
            h
        },
        price_quote_hash: [5u8; 32],
        max_amount: amount,
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
        ..Default::default()
    };
    pay.agent_signature = sign_pay(agent_kp, &pay);
    pay
}

#[tokio::test]
async fn kill_two_validators_fails_quorum_no_mint() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(
        &mut auth,
        account,
        agent,
        fra.clone(),
        AmountMicros(20),
        AmountMicros(5),
        60_000,
        1_000,
    );
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    // Compromise 2 followers (Byzantine / outage beyond fault threshold).
    cluster.kill(2).await;
    cluster.kill(3).await;
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    let err = cluster.leader().handle_pay(pay, 1_100).await.unwrap_err();
    assert_eq!(err, ShardError::BftQuorumFailed);
    // No spend recorded.
    assert_eq!(
        cluster.leader().remaining(&cap.capability_id).await.unwrap(),
        AmountMicros(20)
    );
}

#[tokio::test]
async fn agent_key_theft_loss_bounded_by_lease() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let lease = AmountMicros(10);
    let cap = fund_issue(
        &mut auth,
        account,
        agent,
        fra.clone(),
        lease,
        AmountMicros(5),
        60_000,
        1_000,
    );
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }

    // Attacker steals agent_kp (same signing material).
    let stolen = &agent_kp;
    cluster
        .leader()
        .handle_pay(signed_pay(stolen, &cap, 1, AmountMicros(5)), 1_100)
        .await
        .unwrap();
    cluster
        .leader()
        .handle_pay(signed_pay(stolen, &cap, 2, AmountMicros(5)), 1_101)
        .await
        .unwrap();
    let remaining = cluster.leader().remaining(&cap.capability_id).await.unwrap();
    assert_eq!(remaining, AmountMicros(0));
    let err = cluster
        .leader()
        .handle_pay(signed_pay(stolen, &cap, 3, AmountMicros(1)), 1_102)
        .await
        .unwrap_err();
    assert!(matches!(err, ShardError::InsufficientRemaining { .. }));
    // Loss equals lease, never more.
    assert_eq!(lease.0 - remaining.0, lease.0);
}

#[tokio::test]
async fn expired_capability_and_pay_rejected() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(
        &mut auth,
        account,
        agent,
        fra.clone(),
        AmountMicros(20),
        AmountMicros(5),
        1_000, // ttl
        1_000, // issued at
    );
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    // now > valid_until (1000+1000)
    let err = cluster
        .leader()
        .handle_pay(signed_pay(&agent_kp, &cap, 1, AmountMicros(1)), 3_000)
        .await
        .unwrap_err();
    assert_eq!(err, ShardError::CapabilityExpired);

    let mut pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    pay.expiry_unix_ms = 500;
    pay.agent_signature = sign_pay(&agent_kp, &pay);
    // Fresh cluster time still inside cap window for pay-expiry path:
    let fra2 = ShardId::new("AMS-001").unwrap();
    let mut auth2 = Authority::new_for_tests();
    let account2 = AccountId([2u8; 32]);
    let agent2_kp = Keypair::generate();
    let agent2 = AgentId(agent2_kp.verifying_key_bytes());
    let cap2 = fund_issue(
        &mut auth2,
        account2,
        agent2,
        fra2.clone(),
        AmountMicros(20),
        AmountMicros(5),
        60_000,
        1_000,
    );
    let cluster2 = cluster4_with_issuer_bytes(fra2, auth2.issuer_signing_bytes_for_tests()).await;
    for eng in cluster2.engines.iter() {
        eng.activate_capability(cap2.clone()).await.unwrap();
    }
    let mut pay2 = signed_pay(&agent2_kp, &cap2, 1, AmountMicros(1));
    pay2.expiry_unix_ms = 1_050;
    pay2.agent_signature = sign_pay(&agent2_kp, &pay2);
    let err2 = cluster2
        .leader()
        .handle_pay(pay2, 1_100)
        .await
        .unwrap_err();
    assert_eq!(err2, ShardError::PayExpired);
}

#[tokio::test]
async fn partition_allocation_cannot_exceed_funded_total() {
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    auth.fund(account, AmountMicros(50)).unwrap();
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    auth.allocate(account, fra, AmountMicros(30)).unwrap();
    assert!(auth.allocate(account, ams, AmountMicros(25)).is_err());
}

#[tokio::test]
async fn duplicate_pay_after_commit_is_replay() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(
        &mut auth,
        account,
        agent,
        fra.clone(),
        AmountMicros(20),
        AmountMicros(5),
        60_000,
        1_000,
    );
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let pay = signed_pay(&agent_kp, &cap, 1, AmountMicros(1));
    cluster
        .leader()
        .handle_pay(pay.clone(), 1_100)
        .await
        .unwrap();
    assert!(
        cluster
            .leader()
            .is_consumed(cap.capability_id, Epoch(1), Sequence(1))
            .await
    );
    let err = cluster.leader().handle_pay(pay, 1_101).await.unwrap_err();
    assert!(matches!(err, ShardError::Replay { .. }));
}

#[tokio::test]
async fn fenced_epoch_rejects_new_pays() {
    let fra = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let cap = fund_issue(
        &mut auth,
        account,
        agent,
        fra.clone(),
        AmountMicros(20),
        AmountMicros(5),
        60_000,
        1_000,
    );
    let cluster = cluster4_with_issuer_bytes(fra, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    cluster.leader().fence_epoch(Epoch(1)).await.unwrap();
    let err = cluster
        .leader()
        .handle_pay(signed_pay(&agent_kp, &cap, 1, AmountMicros(1)), 1_100)
        .await
        .unwrap_err();
    assert!(matches!(err, ShardError::EpochFenced { .. }));
}

#[test]
fn wal_reboot_preserves_consumed_sequences() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("assurance.wal");
    let mut wal = Wal::open(&path).unwrap();
    let cap = CapabilityId([7u8; 32]);
    wal.append(&WalRecord::ActivateCapability {
        capability_id: cap,
        epoch: Epoch(1),
        remaining: AmountMicros(40),
        sequence_start: Sequence(1),
        sequence_end: Sequence(100),
    })
    .unwrap();
    wal.append(&WalRecord::ConsumePay {
        tx_id: [1u8; 32],
        capability_id: cap,
        epoch: Epoch(1),
        sequence: Sequence(1),
        amount: AmountMicros(7),
    })
    .unwrap();
    drop(wal);

    // Reboot: reopen WAL and replay.
    let wal2 = Wal::open(&path).unwrap();
    let state = wal2.replay().unwrap();
    assert_eq!(state.remaining(&cap).unwrap(), AmountMicros(33));
    assert!(state.is_consumed(cap, Epoch(1), Sequence(1)));
    assert!(!state.is_consumed(cap, Epoch(1), Sequence(2)));
}
