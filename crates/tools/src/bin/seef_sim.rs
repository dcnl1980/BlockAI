//! Full SEEF vertical slice:
//! attest → hybrid issue → QUIC PAY frame (1-RTT) → local BFT authorize →
//! three-party receipt → checkpoint → witnesses → L1 apply → conservation.

use blockai_authority::{Authority, IssueRequest};
use blockai_consensus::cluster4 as l1_cluster4;
use blockai_crypto::{sign_pay, verify_capability_hybrid, Keypair};
use blockai_net::{
    make_client_endpoint, make_server_endpoint, recv_admitted_frame, send_frame, AppFrame,
};
use blockai_shard::testkit::cluster4_with_issuer_bytes;
use blockai_shard::{complete_payment_proof, CheckpointSealer, ReceiptLog};
use blockai_types::{
    AccountId, AgentId, AmountMicros, Epoch, L1Tx, Pay, Sequence, ShardId, WitnessedCheckpoint,
};
use blockai_witness::Witness;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "seef_sim")]
struct Args {
    #[arg(long, default_value_t = 10)]
    amount: u128,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let shard = ShardId::new("FRA-004").expect("shard");
    let mut auth = Authority::new_for_tests();
    let account = AccountId([1u8; 32]);
    let agent_kp = Keypair::generate();
    let agent = AgentId(agent_kp.verifying_key_bytes());

    auth.fund(account, AmountMicros(1_000)).unwrap();
    auth.allocate(account, shard.clone(), AmountMicros(100))
        .unwrap();
    let evidence = auth.passing_attestation();
    let cap = auth
        .issue_capability(
            IssueRequest {
                account_id: account,
                agent_id: agent,
                shard_id: shard.clone(),
                epoch: Epoch(1),
                maximum_total: AmountMicros(100),
                maximum_per_call: AmountMicros(args.amount),
                service_scope: vec!["inference/*".into()],
                policy_hash: [9u8; 32],
                sequence_start: Sequence(1),
                sequence_end: Sequence(100),
                ttl_ms: 600_000,
                region: "EU".into(),
                now_unix_ms: 1_000,
            },
            &evidence,
        )
        .expect("issue");
    verify_capability_hybrid(&cap).expect("hybrid seal");

    let mut pay = Pay {
        capability_id: cap.capability_id,
        epoch: cap.epoch,
        sequence: Sequence(1),
        agent_id: agent,
        service_id: "inference/x".into(),
        amount: AmountMicros(args.amount),
        currency: "EURC".into(),
        request_hash: [4u8; 32],
        price_quote_hash: [5u8; 32],
        max_amount: AmountMicros(args.amount),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
    ..Default::default()
    };
    pay.agent_signature = sign_pay(&agent_kp, &pay);

    // QUIC dataplane: PAY on 1-RTT only.
    let (server, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).expect("quic srv");
    let addr = server.local_addr().unwrap();
    let client = make_client_endpoint(cert).expect("quic cli");
    let pay_for_net = pay.clone();
    let quic_task = tokio::spawn(async move {
        let incoming = server.accept().await.expect("incoming");
        let conn = incoming.await.expect("accept");
        let mut recv = conn.accept_uni().await.expect("uni");
        let frame = recv_admitted_frame(&mut recv, false)
            .await
            .expect("admit 1-rtt pay");
        match frame {
            AppFrame::Pay { pay } => pay,
            other => panic!("expected Pay, got {other:?}"),
        }
    });
    let conn = client
        .connect(addr, "localhost")
        .unwrap()
        .await
        .expect("connect");
    let mut send = conn.open_uni().await.expect("open");
    send_frame(&mut send, &AppFrame::Pay { pay: pay_for_net })
        .await
        .expect("send");
    send.finish().unwrap();
    let pay = quic_task.await.expect("quic join");

    // Local shard BFT authorize.
    let shard_cluster =
        cluster4_with_issuer_bytes(shard.clone(), auth.issuer_signing_bytes_for_tests()).await;
    for eng in shard_cluster.engines.iter() {
        eng.activate_capability(cap.clone()).await.unwrap();
    }
    let edge = shard_cluster
        .leader()
        .handle_pay(pay.clone(), 1_100)
        .await
        .expect("authorize");

    // Receipt → checkpoint → witnesses.
    let edge_kp = Keypair::generate();
    let service_kp = Keypair::generate();
    let mut log = ReceiptLog::new();
    let mut sealer = CheckpointSealer::new(1, AmountMicros(u128::MAX));
    complete_payment_proof(
        &pay,
        &edge,
        &edge_kp,
        [9u8; 32],
        AmountMicros(args.amount),
        &service_kp,
        &mut log,
    )
    .unwrap();
    let sealed = sealer
        .force_seal(&mut log, &edge_kp, shard.clone(), Epoch(1), 2_000)
        .unwrap();
    let w1 = Witness::generate();
    let w2 = Witness::generate();
    let witnessed = WitnessedCheckpoint {
        checkpoint: sealed.clone(),
        witness_sigs: vec![
            w1.countersign(&sealed).unwrap(),
            w2.countersign(&sealed).unwrap(),
        ],
    };

    // Global L1 settle.
    let l1 = l1_cluster4(2).await;
    l1.leader()
        .submit_and_commit(vec![
            L1Tx::GenesisFund {
                account,
                amount: AmountMicros(1_000),
            },
            L1Tx::AllocateShardAllowance {
                account,
                shard_id: shard,
                amount: AmountMicros(100),
            },
        ])
        .await
        .expect("fund l1");
    let outcome = l1
        .leader()
        .submit_and_commit(vec![L1Tx::CheckpointFinalized {
            checkpoint: witnessed,
            funding_account: account,
        }])
        .await
        .expect("checkpoint commit");
    let state = l1.leader().state_snapshot().await;
    state.check_conservation().expect("conservation");

    println!(
        "ok seef_e2e amount={} commit_index={} applied={} supply={} available={} outstanding={} locked={}",
        args.amount,
        edge.commit_index,
        outcome.applied,
        state.total_supply.0,
        state.available_sum().0,
        state.shard_outstanding_sum().0,
        state.accounts[&account].balance_locked.0
    );
}
