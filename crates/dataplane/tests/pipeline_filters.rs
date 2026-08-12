use blockai_dataplane::{
    AfXdpSocket, DataplaneError, DataplanePipeline, DpdkPort, DpdkStub, IngressPacket,
    PipelineConfig, UserspaceXdp,
};
use blockai_net::{encode_frame, AppFrame};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};

fn pay_frame(cap: [u8; 32]) -> Vec<u8> {
    let pay = Pay {
        capability_id: CapabilityId(cap),
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
    };
    encode_frame(&AppFrame::Pay { pay }).unwrap()
}

#[test]
fn userspace_xdp_pipeline_admits_1rtt_pay_with_warm_cache() {
    let cap = [9u8; 32];
    let mut xdp = UserspaceXdp::new();
    xdp.inject(IngressPacket {
        bytes: pay_frame(cap),
        is_early_data: false,
        src_hint: "10.0.0.1".into(),
    });
    let mut pipe = DataplanePipeline::new(PipelineConfig::default());
    pipe.warm_capability(cap);
    let pkt = xdp.recv().unwrap().unwrap();
    let frame = pipe.process(&pkt).unwrap();
    assert!(matches!(frame, AppFrame::Pay { .. }));
    assert_eq!(xdp.name(), "userspace-xdp");
}

#[test]
fn rejects_0rtt_pay_and_cold_cache_and_dpdk_stub() {
    let cap = [9u8; 32];
    let mut pipe = DataplanePipeline::new(PipelineConfig {
        max_frame_bytes: 64 * 1024,
        rate_limit_per_src: 2,
    });
    let early = IngressPacket {
        bytes: pay_frame(cap),
        is_early_data: true,
        src_hint: "a".into(),
    };
    assert!(matches!(
        pipe.process(&early).unwrap_err(),
        DataplaneError::Admit(_)
    ));
    pipe.warm_capability(cap);
    // rate limit
    for _ in 0..2 {
        pipe.process(&IngressPacket {
            bytes: pay_frame(cap),
            is_early_data: false,
            src_hint: "b".into(),
        })
        .unwrap();
    }
    assert_eq!(
        pipe.process(&IngressPacket {
            bytes: pay_frame(cap),
            is_early_data: false,
            src_hint: "b".into(),
        })
        .unwrap_err(),
        DataplaneError::RateLimited
    );

    let mut dpdk = DpdkStub::default();
    assert_eq!(
        dpdk.rx_burst(8).unwrap_err(),
        DataplaneError::DpdkNotAvailable
    );
    assert!(!dpdk.started());
}
