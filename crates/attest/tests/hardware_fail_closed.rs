use blockai_attest::{
    Attestor, AttestorError, HardwareAttestor, SoftwareAttestor, TestPlatform,
};

#[test]
fn software_attestor_passes_policy() {
    let a = SoftwareAttestor::new();
    let ev = a.collect().unwrap();
    a.verify_against(a.policy(), &ev).unwrap();
}

#[test]
fn hardware_attestor_fails_without_measurement() {
    let platform = TestPlatform::new();
    let a = HardwareAttestor::unmeasured(platform);
    assert_eq!(
        a.collect().unwrap_err(),
        AttestorError::MeasurementUnavailable
    );
}

#[test]
fn hardware_attestor_passes_when_measured() {
    let platform = TestPlatform::new();
    let a = HardwareAttestor::with_measurement(platform);
    let ev = a.collect().unwrap();
    a.verify_against(a.policy(), &ev).unwrap();
}
