use blockai_crypto::Keypair;
use blockai_types::encode_cbor;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Soft offline-root HSM: 5 key shares, quorum 3.
pub const HSM_SHARES: usize = 5;
pub const HSM_QUORUM: usize = 3;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HsmError {
    #[error("insufficient shares: have {have} need {need}")]
    InsufficientShares { have: usize, need: usize },
    #[error("duplicate share")]
    DuplicateShare,
    #[error("bad share signature")]
    BadShareSignature,
    #[error("unknown share")]
    UnknownShare,
    #[error("cbor encode failed")]
    Cbor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RootOp {
    AuthorizeIssuer { issuer_pubkey: [u8; 32] },
    RotateRoot { epoch: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareSig {
    pub share_id: u8,
    pub pubkey: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThresholdSignature {
    pub op: RootOp,
    pub shares: Vec<ShareSig>,
}

#[derive(Serialize)]
struct RootSignBody<'a> {
    domain: &'static str,
    op: &'a RootOp,
}

fn op_bytes(op: &RootOp) -> Result<Vec<u8>, HsmError> {
    encode_cbor(&RootSignBody {
        domain: "HSM_ROOT_OP",
        op,
    })
    .map_err(|_| HsmError::Cbor)
}

/// Software simulation of an offline 3-of-5 root HSM ceremony.
pub struct SoftHsm3of5 {
    shares: Vec<Keypair>,
}

impl SoftHsm3of5 {
    pub fn generate() -> Self {
        Self {
            shares: (0..HSM_SHARES).map(|_| Keypair::generate()).collect(),
        }
    }

    pub fn share_pubkeys(&self) -> Vec<[u8; 32]> {
        self.shares
            .iter()
            .map(|k| k.verifying_key_bytes())
            .collect()
    }

    /// Sign with selected share indices (must be unique, size ≥ quorum when verified).
    pub fn sign_with(
        &self,
        op: &RootOp,
        share_ids: &[u8],
    ) -> Result<ThresholdSignature, HsmError> {
        let bytes = op_bytes(op)?;
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &id in share_ids {
            if !seen.insert(id) {
                return Err(HsmError::DuplicateShare);
            }
            let idx = id as usize;
            let kp = self.shares.get(idx).ok_or(HsmError::UnknownShare)?;
            out.push(ShareSig {
                share_id: id,
                pubkey: kp.verifying_key_bytes(),
                signature: kp.signing_key().sign(&bytes).to_bytes().to_vec(),
            });
        }
        Ok(ThresholdSignature {
            op: op.clone(),
            shares: out,
        })
    }

    pub fn verify(
        &self,
        sig: &ThresholdSignature,
        quorum: usize,
    ) -> Result<(), HsmError> {
        if sig.shares.len() < quorum {
            return Err(HsmError::InsufficientShares {
                have: sig.shares.len(),
                need: quorum,
            });
        }
        let allowed: std::collections::HashSet<[u8; 32]> =
            self.share_pubkeys().into_iter().collect();
        let bytes = op_bytes(&sig.op)?;
        let mut seen = std::collections::HashSet::new();
        for share in &sig.shares {
            if !seen.insert(share.share_id) {
                return Err(HsmError::DuplicateShare);
            }
            if !allowed.contains(&share.pubkey) {
                return Err(HsmError::UnknownShare);
            }
            let vk = VerifyingKey::from_bytes(&share.pubkey).map_err(|_| HsmError::BadShareSignature)?;
            let sig_bytes: [u8; 64] = share
                .signature
                .as_slice()
                .try_into()
                .map_err(|_| HsmError::BadShareSignature)?;
            vk.verify(&bytes, &Signature::from_bytes(&sig_bytes))
                .map_err(|_| HsmError::BadShareSignature)?;
        }
        Ok(())
    }
}
