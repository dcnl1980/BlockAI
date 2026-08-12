use blockai_hsm::{SoftHsm3of5, HSM_QUORUM, HSM_SHARES};

#[test]
fn ceremony_transcript_roundtrip() {
    let hsm = SoftHsm3of5::generate_hybrid();
    let t = hsm.export_ceremony(1_700_000_000_000);
    assert_eq!(t.share_pubkeys.len(), HSM_SHARES);
    assert_eq!(t.quorum, HSM_QUORUM);
    assert!(t.hybrid);
    SoftHsm3of5::verify_ceremony_transcript(&t).unwrap();
    let mut bad = t.clone();
    bad.root_commitment[0] ^= 1;
    assert!(SoftHsm3of5::verify_ceremony_transcript(&bad).is_err());
}
