use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_types::{AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId};
use clap::Parser;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "pay_sim")]
struct Args {
    #[arg(long, default_value_t = 50)]
    pays: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let shard = ShardId::new("FRA-004").expect("shard");
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());
    auth.fund(account, AmountMicros(1_000_000)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(1_000_000))
        .unwrap();
    let __evidence = auth.passing_attestation();
    let cap = auth
        .issue_capability(IssueRequest {
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
        }, &__evidence)
        .unwrap();

    let cluster = cluster4_with_issuer_bytes(shard, auth.issuer_signing_bytes_for_tests()).await;
    for eng in cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }

    let mut latencies = Vec::new();
    for i in 1..=args.pays {
        let mut pay = Pay {
            capability_id: cap.capability_id,
            epoch: cap.epoch,
            sequence: Sequence(i),
            agent_id: agent,
            service_id: "inference/x".into(),
            amount: AmountMicros(1),
            currency: "EURC".into(),
            request_hash: {
                let mut h = [0u8; 32];
                h[0..8].copy_from_slice(&i.to_le_bytes());
                h
            },
            price_quote_hash: [5u8; 32],
            max_amount: AmountMicros(5),
            pricing_schedule_version: 1,
            expiry_unix_ms: 9_999_999_999,
            agent_signature: vec![],
        ..Default::default()
        };
        pay.agent_signature = sign_pay(&agent_kp, &pay);
        let start = Instant::now();
        cluster
            .leader()
            .handle_pay(pay, 1_100)
            .await
            .unwrap_or_else(|e| panic!("pay {i} failed: {e}"));
        latencies.push(start.elapsed().as_micros());
    }

    latencies.sort_unstable();
    let p50 = latencies[latencies.len() / 2];
    let remaining = AmountMicros(1_000_000 - args.pays as u128);
    println!(
        "ok pays={} p50_us={} remaining={}",
        args.pays, p50, remaining.0
    );
}
