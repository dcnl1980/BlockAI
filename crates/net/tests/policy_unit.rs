use blockai_net::{admit_frame, AdmitError, AppFrame};
use blockai_types::{
    AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence,
};

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
        ..Default::default()
    }
}

#[test]
fn pay_rejected_on_early_data() {
    let frame = AppFrame::Pay { pay: sample_pay() };
    assert_eq!(
        admit_frame(true, &frame).unwrap_err(),
        AdmitError::ZeroRttPayForbidden
    );
}

#[test]
fn pay_allowed_on_1rtt() {
    let frame = AppFrame::Pay { pay: sample_pay() };
    admit_frame(false, &frame).unwrap();
}

#[test]
fn idempotent_read_allowed_on_0rtt() {
    let frame = AppFrame::IdempotentRead {
        path: "/price".into(),
    };
    admit_frame(true, &frame).unwrap();
}
