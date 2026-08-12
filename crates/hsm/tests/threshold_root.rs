use blockai_hsm::{HsmError, RootOp, SoftHsm3of5, HSM_QUORUM};

#[test]
fn three_of_five_signs_and_verifies() {
    let hsm = SoftHsm3of5::generate();
    let op = RootOp::AuthorizeIssuer {
        issuer_pubkey: [7u8; 32],
    };
    let sig = hsm.sign_with(&op, &[0, 2, 4]).unwrap();
    hsm.verify(&sig, HSM_QUORUM).unwrap();
}

#[test]
fn hybrid_three_of_five_requires_pq_half() {
    let hsm = SoftHsm3of5::generate_hybrid();
    let op = RootOp::AuthorizeIssuer {
        issuer_pubkey: [7u8; 32],
    };
    let mut sig = hsm.sign_with(&op, &[0, 1, 2]).unwrap();
    hsm.verify(&sig, HSM_QUORUM).unwrap();
    sig.shares[0].pq_signature.clear();
    assert_eq!(hsm.verify(&sig, HSM_QUORUM).unwrap_err(), HsmError::BadShareSignature);
}

#[test]
fn two_shares_fail_quorum() {
    let hsm = SoftHsm3of5::generate();
    let op = RootOp::RotateRoot { epoch: 3 };
    let sig = hsm.sign_with(&op, &[1, 3]).unwrap();
    assert_eq!(
        hsm.verify(&sig, HSM_QUORUM).unwrap_err(),
        HsmError::InsufficientShares {
            have: 2,
            need: HSM_QUORUM
        }
    );
}
