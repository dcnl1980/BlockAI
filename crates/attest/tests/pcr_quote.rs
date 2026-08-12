use blockai_attest::{
    verify_evidence, AttestError, Attestor, HardwareAttestor, TestPlatform,
};

#[test]
fn measured_hardware_includes_pcrs_and_policy_can_require_them() {
    let platform = TestPlatform::new();
    let attestor = HardwareAttestor::with_measurement(platform);
    let mut policy = attestor.policy().clone();
    let ev = attestor.collect().unwrap();
    assert_eq!(ev.pcrs.len(), 2);
    assert_ne!(ev.quote_nonce, [0u8; 32]);
    policy.required_pcrs = ev.pcrs.clone();
    verify_evidence(&policy, &ev).unwrap();
    policy.required_pcrs[0][0] ^= 1;
    assert_eq!(
        verify_evidence(&policy, &ev).unwrap_err(),
        AttestError::PcrRejected
    );
}
