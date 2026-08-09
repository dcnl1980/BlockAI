use blockai_net::{
    make_client_endpoint, make_server_endpoint, recv_admitted_frame, send_frame, AppFrame,
};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};

fn sample_pay() -> Pay {
    Pay {
        capability_id: CapabilityId([1u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId([2u8; 32]),
        service_id: "inference/x".into(),
        amount: AmountMicros(1),
        currency: "EURC".into(),
        request_hash: [3u8; 32],
        price_quote_hash: [4u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999,
        agent_signature: vec![0u8; 64],
    }
}

#[tokio::test]
async fn pay_over_1rtt_is_admitted() {
    let (server, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = server.local_addr().unwrap();
    let client = make_client_endpoint(cert).unwrap();

    let server_task = tokio::spawn(async move {
        let incoming = server.accept().await.expect("incoming");
        let conn = incoming.await.expect("accept");
        let mut recv = conn.accept_uni().await.expect("uni");
        // Fresh connection: application data is 1-RTT.
        let frame = recv_admitted_frame(&mut recv, false).await.expect("admit");
        match frame {
            AppFrame::Pay { pay } => assert_eq!(pay.amount, AmountMicros(1)),
            other => panic!("unexpected {other:?}"),
        }
    });

    let conn = client
        .connect(addr, "localhost")
        .unwrap()
        .await
        .expect("connect");
    let mut send = conn.open_uni().await.expect("open");
    send_frame(&mut send, &AppFrame::Pay { pay: sample_pay() })
        .await
        .unwrap();
    send.finish().unwrap();
    server_task.await.unwrap();
}
