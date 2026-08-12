use blockai_net::{
    make_client_endpoint, make_server_endpoint, recv_admitted_frame, send_frame, AppFrame,
};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "quic_sim")]
struct Args {
    #[arg(long, default_value_t = 1)]
    amount: u128,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let (server, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).expect("server");
    let addr = server.local_addr().unwrap();
    let client = make_client_endpoint(cert).expect("client");

    let server_task = tokio::spawn(async move {
        let incoming = server.accept().await.expect("incoming");
        let conn = incoming.await.expect("accept");
        let mut recv = conn.accept_uni().await.expect("uni");
        let frame = recv_admitted_frame(&mut recv, false)
            .await
            .expect("1-rtt pay admitted");
        match frame {
            AppFrame::Pay { pay } => {
                println!("ok pay_amount={} via=1rtt", pay.amount.0);
            }
            AppFrame::IdempotentRead { path } => {
                println!("ok read path={path}");
            }
        }
    });

    let conn = client
        .connect(addr, "localhost")
        .unwrap()
        .await
        .expect("connect");
    let mut send = conn.open_uni().await.expect("open");
    let pay = Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId([2u8; 32]),
        service_id: "inference/x".into(),
        amount: AmountMicros(args.amount),
        currency: "EURC".into(),
        request_hash: [3u8; 32],
        price_quote_hash: [4u8; 32],
        max_amount: AmountMicros(args.amount.saturating_mul(2)),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![0u8; 64],
    ..Default::default()
    };
    send_frame(&mut send, &AppFrame::Pay { pay })
        .await
        .expect("send");
    send.finish().unwrap();
    server_task.await.unwrap();
}
