//! Plan 11 assurance harness: drills + release p50 gate (SEEF §10.3).

use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_shard::ShardError;
use blockai_types::{AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId};
use clap::Parser;
use std::process::ExitCode;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "assurance_sim")]
struct Args {
    /// PAY samples for p50 measurement
    #[arg(long, default_value_t = 50)]
    pays: u64,
    /// Max allowed p50 in microseconds (v1: single-digit ms → 10000)
    #[arg(long, default_value_t = 10_000)]
    max_p50_us: u64,
}

fn signed_pay(
    agent_kp: &Keypair,
    agent: AgentId,
    cap_id: blockai_types::CapabilityId,
    epoch: Epoch,
    seq: u64,
    amount: AmountMicros,
) -> Pay {
    let mut pay = Pay {
        capability_id: cap_id,
        epoch,
        sequence: Sequence(seq),
        agent_id: agent,
        service_id: "inference/x".into(),
        amount,
        currency: "EURC".into(),
        request_hash: {
            let mut h = [0u8; 32];
            h[0..8].copy_from_slice(&seq.to_le_bytes());
            h
        },
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
        ..Default::default()
    };
    pay.agent_signature = sign_pay(agent_kp, &pay);
    pay
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let mut checklist: Vec<(&'static str, bool)> = Vec::new();

    // --- Drill: partition-bounded allocation ---
    {
        let mut auth = Authority::new_for_tests();
        let account = AccountId([1u8; 32]);
        auth.fund(account, AmountMicros(50)).unwrap();
        let fra = ShardId::new("FRA-004").unwrap();
        let ams = ShardId::new("AMS-001").unwrap();
        auth.allocate(account, fra, AmountMicros(30)).unwrap();
        let over = auth.allocate(account, ams, AmountMicros(25)).is_err();
        checklist.push(("partition_allocation_bounded", over));
    }

    // --- Drill: kill-one + replay + key-theft bound + kill-two quorum ---
    {
        let shard = ShardId::new("FRA-004").unwrap();
        let mut auth = Authority::new_for_tests();
        let account = AccountId([2u8; 32]);
        let agent_kp = Keypair::generate();
        let agent = AgentId(agent_kp.verifying_key_bytes());
        auth.fund(account, AmountMicros(1_000)).unwrap();
        auth.allocate(account, shard.clone(), AmountMicros(20))
            .unwrap();
        let evidence = auth.passing_attestation();
        let cap = auth
            .issue_capability(
                IssueRequest {
                    account_id: account,
                    agent_id: agent,
                    shard_id: shard.clone(),
                    epoch: Epoch(1),
                    maximum_total: AmountMicros(20),
                    maximum_per_call: AmountMicros(5),
                    service_scope: vec!["inference/*".into()],
                    policy_hash: [9u8; 32],
                    sequence_start: Sequence(1),
                    sequence_end: Sequence(1_000),
                    ttl_ms: 600_000,
                    region: "EU".into(),
                    now_unix_ms: 1_000,
                },
                &evidence,
            )
            .unwrap();
        let cluster =
            cluster4_with_issuer_bytes(shard.clone(), auth.issuer_signing_bytes_for_tests()).await;
        for eng in cluster.engines.iter() {
            eng.activate_capability(cap.clone()).await.unwrap();
        }

        let pay1 = signed_pay(&agent_kp, agent, cap.capability_id, cap.epoch, 1, AmountMicros(1));
        cluster.leader().handle_pay(pay1.clone(), 1_100).await.unwrap();
        cluster.kill(2).await;
        let replay_closed = matches!(
            cluster.leader().handle_pay(pay1, 1_101).await.unwrap_err(),
            ShardError::Replay { .. }
        );
        checklist.push(("kill_one_replay_closed", replay_closed));

        // Drain remaining lease with stolen key (same material).
        let mut seq = 2u64;
        while cluster
            .leader()
            .remaining(&cap.capability_id)
            .await
            .unwrap()
            .0
            > 0
        {
            let amt = AmountMicros(
                cluster
                    .leader()
                    .remaining(&cap.capability_id)
                    .await
                    .unwrap()
                    .0
                    .min(5),
            );
            cluster
                .leader()
                .handle_pay(
                    signed_pay(&agent_kp, agent, cap.capability_id, cap.epoch, seq, amt),
                    1_200 + seq,
                )
                .await
                .unwrap();
            seq += 1;
        }
        let theft_bound = matches!(
            cluster
                .leader()
                .handle_pay(
                    signed_pay(
                        &agent_kp,
                        agent,
                        cap.capability_id,
                        cap.epoch,
                        seq,
                        AmountMicros(1)
                    ),
                    2_000
                )
                .await
                .unwrap_err(),
            ShardError::InsufficientRemaining { .. }
        );
        checklist.push(("agent_key_theft_loss_bounded", theft_bound));

        // Fresh cluster for kill-two (prior lease exhausted / one validator dead).
        let mut auth2 = Authority::new_for_tests();
        let account2 = AccountId([3u8; 32]);
        let agent2_kp = Keypair::generate();
        let agent2 = AgentId(agent2_kp.verifying_key_bytes());
        auth2.fund(account2, AmountMicros(100)).unwrap();
        auth2
            .allocate(account2, shard.clone(), AmountMicros(20))
            .unwrap();
        let evidence2 = auth2.passing_attestation();
        let cap2 = auth2
            .issue_capability(
                IssueRequest {
                    account_id: account2,
                    agent_id: agent2,
                    shard_id: shard.clone(),
                    epoch: Epoch(1),
                    maximum_total: AmountMicros(20),
                    maximum_per_call: AmountMicros(5),
                    service_scope: vec!["inference/*".into()],
                    policy_hash: [9u8; 32],
                    sequence_start: Sequence(1),
                    sequence_end: Sequence(100),
                    ttl_ms: 60_000,
                    region: "EU".into(),
                    now_unix_ms: 1_000,
                },
                &evidence2,
            )
            .unwrap();
        let cluster2 =
            cluster4_with_issuer_bytes(shard, auth2.issuer_signing_bytes_for_tests()).await;
        for eng in cluster2.engines.iter() {
            eng.activate_capability(cap2.clone()).await.unwrap();
        }
        cluster2.kill(1).await;
        cluster2.kill(2).await;
        let quorum_fail = matches!(
            cluster2
                .leader()
                .handle_pay(
                    signed_pay(
                        &agent2_kp,
                        agent2,
                        cap2.capability_id,
                        cap2.epoch,
                        1,
                        AmountMicros(1)
                    ),
                    1_100
                )
                .await
                .unwrap_err(),
            ShardError::BftQuorumFailed
        );
        checklist.push(("kill_two_quorum_fail_closed", quorum_fail));
    }

    // --- Release p50 ---
    let p50 = {
        let shard = ShardId::new("FRA-004").unwrap();
        let mut auth = Authority::new_for_tests();
        let account = AccountId([9u8; 32]);
        let agent_kp = Keypair::generate();
        let agent = AgentId(agent_kp.verifying_key_bytes());
        auth.fund(account, AmountMicros(1_000_000)).unwrap();
        auth.allocate(account, shard.clone(), AmountMicros(1_000_000))
            .unwrap();
        let evidence = auth.passing_attestation();
        let cap = auth
            .issue_capability(
                IssueRequest {
                    account_id: account,
                    agent_id: agent,
                    shard_id: shard.clone(),
                    epoch: Epoch(1),
                    maximum_total: AmountMicros(1_000_000),
                    maximum_per_call: AmountMicros(5),
                    service_scope: vec!["inference/*".into()],
                    policy_hash: [9u8; 32],
                    sequence_start: Sequence(1),
                    sequence_end: Sequence(args.pays + 10),
                    ttl_ms: 600_000,
                    region: "EU".into(),
                    now_unix_ms: 1_000,
                },
                &evidence,
            )
            .unwrap();
        let cluster = cluster4_with_issuer_bytes(shard, auth.issuer_signing_bytes_for_tests()).await;
        for eng in cluster.engines.iter() {
            eng.activate_capability(cap.clone()).await.unwrap();
        }
        let mut latencies = Vec::with_capacity(args.pays as usize);
        for i in 1..=args.pays {
            let pay = signed_pay(
                &agent_kp,
                agent,
                cap.capability_id,
                cap.epoch,
                i,
                AmountMicros(1),
            );
            let start = Instant::now();
            cluster.leader().handle_pay(pay, 1_100).await.unwrap();
            latencies.push(start.elapsed().as_micros() as u64);
        }
        latencies.sort_unstable();
        latencies[latencies.len() / 2]
    };
    // v1 p50 gate is a release/lab metric; debug builds report but do not fail the gate.
    let enforce_p50 = !cfg!(debug_assertions);
    let p50_ok = p50 < args.max_p50_us;
    if enforce_p50 {
        checklist.push(("pay_authorize_p50_single_digit_ms", p50_ok));
    } else if p50_ok {
        checklist.push(("pay_authorize_p50_single_digit_ms", true));
    } else {
        println!(
            "drill SKIP pay_authorize_p50_single_digit_ms (debug p50_us={p50}; use --release)"
        );
    }

    let mut all_ok = true;
    for (name, ok) in &checklist {
        let mark = if *ok { "PASS" } else { "FAIL" };
        if !ok {
            all_ok = false;
        }
        println!("drill {mark} {name}");
    }
    println!(
        "p50_us={p50} max_p50_us={} pays={} enforce_p50={enforce_p50}",
        args.max_p50_us, args.pays
    );

    if all_ok {
        println!("assurance_sim OK");
        ExitCode::SUCCESS
    } else {
        println!("assurance_sim FAIL");
        ExitCode::FAILURE
    }
}
