use blockai_authority::{Authority, IssueRequest};
use blockai_crypto::{sign_pay, Keypair};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_types::{
    AccountId, AgentId, AmountMicros, Epoch, Pay, Sequence, ShardId,
};
use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn bench_pay_authorize(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let shard = ShardId::new("FRA-004").unwrap();
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
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
                sequence_end: Sequence(1_000_000),
                ttl_ms: 600_000,
                region: "EU".into(),
                now_unix_ms: 1_000,
            },
            &evidence,
        )
        .unwrap();
    let issuer_bytes = auth.issuer_signing_bytes_for_tests();
    let cluster = rt.block_on(cluster4_with_issuer_bytes(shard, issuer_bytes));
    for eng in cluster.engines.iter() {
        rt.block_on(eng.activate_capability(cap.clone())).unwrap();
    }
    let leader = cluster.leader();
    let mut seq = 1u64;

    let mut group = c.benchmark_group("pay_authorize");
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("handle_pay_p50_lab", |b| {
        b.iter(|| {
            seq += 1;
            let mut pay = Pay {
                capability_id: cap.capability_id,
                epoch: cap.epoch,
                sequence: Sequence(seq),
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
            rt.block_on(leader.handle_pay(pay, 2_000)).expect("pay");
        })
    });
    group.finish();
}

criterion_group!(benches, bench_pay_authorize);
criterion_main!(benches);
