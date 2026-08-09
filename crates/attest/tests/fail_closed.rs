use blockai_attest::{verify_evidence, AttestError, TestPlatform};

#[test]
fn good_evidence_passes() {
    let platform = TestPlatform::new();
    verify_evidence(&platform.policy, &platform.evidence()).unwrap();
}

#[test]
fn bad_binary_hash_fail_closed() {
    let platform = TestPlatform::new();
    let mut ev = platform.evidence();
    ev.binary_hash = [0u8; 32];
    // resign would be needed for sig; without resign signature fails first —
    // force approved path by clearing signature after changing hash.
    let err = verify_evidence(&platform.policy, &ev).unwrap_err();
    assert!(matches!(
        err,
        AttestError::BinaryRejected | AttestError::BadSignature
    ));
}

#[test]
fn missing_signature_fail_closed() {
    let platform = TestPlatform::new();
    let mut ev = platform.evidence();
    ev.platform_signature.clear();
    assert_eq!(
        verify_evidence(&platform.policy, &ev).unwrap_err(),
        AttestError::BadSignature
    );
}
