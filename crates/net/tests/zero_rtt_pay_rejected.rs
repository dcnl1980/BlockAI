use blockai_net::{admit_frame, AdmitError, AppFrame};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};

/// Application-layer enforcement of the hard rule (used when quinn delivers early data).
#[test]
fn zero_rtt_pay_fail_closed() {
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
    };
    let err = admit_frame(true, &AppFrame::Pay { pay }).unwrap_err();
    assert_eq!(err, AdmitError::ZeroRttPayForbidden);
}
