//! Plan 12: FastPay regional reallocate + capability top-up demo.

use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_execute::GlobalState;
use blockai_fastpay::{RegionalCommittee, RegionalOp, REGIONAL_QUORUM};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_types::{
    AccountId, AgentId, AmountMicros, Epoch, L1Tx, Pay, Sequence, ShardId,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "fastpay_sim")]
struct Args {
    #[arg(long, default_value_t = 15)]
    reallocate: u128,
    #[arg(long, default_value_t = 5)]
    topup: u128,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let fra = ShardId::new("FRA-004").unwrap();
    let ams = ShardId::new("AMS-001").unwrap();
    let account = AccountId([1u8; 32]);

    // --- Authority float + FastPay reallocate FRA → AMS ---
    let mut auth = Authority::new_for_tests();
    auth.fund(account, AmountMicros(100)).unwrap();
    auth.allocate(account, fra.clone(), AmountMicros(40)).unwrap();
    let mut committee = RegionalCommittee::generate();
    let realloc_op = RegionalOp::Reallocate {
        account,
        from_shard: fra.clone(),
        to_shard: ams.clone(),
        amount: AmountMicros(args.reallocate),
        nonce: 1,
    };
    let realloc_cert = committee.sign_with(&realloc_op, &[0, 1, 2]).unwrap();
    committee.consume(&realloc_cert, REGIONAL_QUORUM).unwrap();
    match &realloc_cert.op {
        RegionalOp::Reallocate {
            account,
            from_shard,
            to_shard,
            amount,
            ..
        } => auth
            .reallocate(*account, from_shard.clone(), to_shard.clone(), *amount)
            .unwrap(),
        other => panic!("unexpected op {other:?}"),
    }
    assert_eq!(
        auth.shard_allowance(account, &ams).unwrap().0,
        args.reallocate
    );

    // --- L1 outstanding mirrors regional settle ---
    let mut l1 = GlobalState::new(2);
    l1.apply(&L1Tx::GenesisFund {
        account,
        amount: AmountMicros(100),
    })
    .unwrap();
    l1.apply(&L1Tx::AllocateShardAllowance {
        account,
        shard_id: fra.clone(),
        amount: AmountMicros(40),
    })
    .unwrap();
    l1.apply(&L1Tx::ReallocateShardOutstanding {
        account,
        from_shard: fra.clone(),
        to_shard: ams.clone(),
        amount: AmountMicros(args.reallocate),
    })
    .unwrap();
    l1.check_conservation().unwrap();

    // --- Issue on AMS after reallocate; top-up live FRA capability ---
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    let evidence = auth.passing_attestation();
    // Leave some FRA allowance for top-up after issuing a small FRA cap.
    let fra_cap = auth
        .issue_capability(
            IssueRequest {
                account_id: account,
                agent_id: agent,
                shard_id: fra.clone(),
                epoch: Epoch(1),
                maximum_total: AmountMicros(10),
                maximum_per_call: AmountMicros(5),
                service_scope: vec!["inference/*".into()],
                policy_hash: [9u8; 32],
                sequence_start: Sequence(1),
                sequence_end: Sequence(100),
                ttl_ms: 60_000,
                region: "EU".into(),
                now_unix_ms: 1_000,
            },
            &evidence,
        )
        .unwrap();
    let ams_cap = auth
        .issue_capability(
            IssueRequest {
                account_id: account,
                agent_id: agent,
                shard_id: ams.clone(),
                epoch: Epoch(1),
                maximum_total: AmountMicros(args.reallocate.min(10)),
                maximum_per_call: AmountMicros(5),
                service_scope: vec!["inference/*".into()],
                policy_hash: [9u8; 32],
                sequence_start: Sequence(1),
                sequence_end: Sequence(100),
                ttl_ms: 60_000,
                region: "EU".into(),
                now_unix_ms: 1_000,
            },
            &evidence,
        )
        .unwrap();
    assert_eq!(ams_cap.shard_id, ams);

    let cluster = cluster4_with_issuer_bytes(fra.clone(), auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(fra_cap.clone()).await.unwrap();
    }
    // Spend 1 so remaining is 9, then top-up.
    let mut pay = Pay {
        capability_id: fra_cap.capability_id,
        epoch: fra_cap.epoch,
        sequence: Sequence(1),
        agent_id: agent,
        service_id: "inference/x".into(),
        amount: AmountMicros(1),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
        ..Default::default()
    };
    pay.agent_signature = sign_pay(&agent_kp, &pay);
    cluster.leader().handle_pay(pay, 1_100).await.unwrap();
    assert_eq!(
        cluster.leader().remaining(&fra_cap.capability_id).await.unwrap(),
        AmountMicros(9)
    );

    let topup_op = RegionalOp::TopUpCapability {
        account,
        shard_id: fra.clone(),
        capability_id: fra_cap.capability_id,
        amount: AmountMicros(args.topup),
        nonce: 2,
    };
    let topup_cert = committee.sign_with(&topup_op, &[0, 2, 3]).unwrap();
    committee.consume(&topup_cert, REGIONAL_QUORUM).unwrap();
    match &topup_cert.op {
        RegionalOp::TopUpCapability {
            account,
            shard_id,
            capability_id,
            amount,
            ..
        } => {
            auth.debit_for_top_up(*account, shard_id.clone(), *amount)
                .unwrap();
            for eng in cluster.engines.iter() {
                eng.top_up_capability(*capability_id, *amount).await.unwrap();
            }
        }
        other => panic!("unexpected op {other:?}"),
    }
    assert_eq!(
        cluster.leader().remaining(&fra_cap.capability_id).await.unwrap(),
        AmountMicros(9 + args.topup)
    );

    println!(
        "fastpay_sim OK reallocate={} topup={} ams_cap_total={} fra_remaining={}",
        args.reallocate,
        args.topup,
        ams_cap.maximum_total.0,
        9 + args.topup
    );
}
