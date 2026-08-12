use blockai_net::{
    make_server_endpoint, race_connect, recv_admitted_frame, send_frame, AppFrame,
};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};
use std::time::Duration;

#[tokio::test]
async fn race_connect_picks_first_live_path() {
    let (server_a, cert_a) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr_a = server_a.local_addr().unwrap();
    // Dead path: nothing listening.
    let dead: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();

    let server_task = tokio::spawn(async move {
        let incoming = server_a.accept().await.expect("incoming");
        let conn = incoming.await.expect("accept");
        let mut recv = conn.accept_uni().await.expect("uni");
        let frame = recv_admitted_frame(&mut recv, false).await.expect("admit");
        assert!(matches!(frame, AppFrame::Pay { .. }));
    });

    let (conn, won) = race_connect(
        vec![(dead, cert_a.clone()), (addr_a, cert_a)],
        "localhost",
        Duration::from_millis(500),
    )
    .await
    .expect("race");
    assert_eq!(won, addr_a);

    let mut send = conn.open_uni().await.unwrap();
    let pay = Pay {
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
    ..Default::default()
    };
    send_frame(&mut send, &AppFrame::Pay { pay }).await.unwrap();
    send.finish().unwrap();
    server_task.await.unwrap();
}
