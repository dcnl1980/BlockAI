use blockai_crypto::{dual_sign_root_op, verify_root_op_pq, Keypair, PqKeypair};
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
    #[serde(default)]
    pub pq_pubkey: Vec<u8>,
    #[serde(default)]
    pub pq_signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThresholdSignature {
    pub op: RootOp,
    pub shares: Vec<ShareSig>,
}

/// Offline ceremony export for ops / audit (production HSM seam).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyTranscript {
    pub share_pubkeys: Vec<[u8; 32]>,
    pub quorum: usize,
    pub hybrid: bool,
    pub created_unix_ms: u64,
    /// BLAKE3 over sorted share pubkeys || quorum || hybrid flag.
    pub root_commitment: [u8; 32],
}

pub fn ceremony_root_commitment(share_pubkeys: &[[u8; 32]], quorum: usize, hybrid: bool) -> [u8; 32] {
    let mut keys = share_pubkeys.to_vec();
    keys.sort();
    let mut h = blake3::Hasher::new();
    for k in &keys {
        h.update(k);
    }
    h.update(&(quorum as u64).to_le_bytes());
    h.update(&[u8::from(hybrid)]);
    *h.finalize().as_bytes()
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
    pq_shares: Option<Vec<PqKeypair>>,
}

impl SoftHsm3of5 {
    pub fn generate() -> Self {
        Self {
            shares: (0..HSM_SHARES).map(|_| Keypair::generate()).collect(),
            pq_shares: None,
        }
    }

    pub fn generate_hybrid() -> Self {
        Self {
            shares: (0..HSM_SHARES).map(|_| Keypair::generate()).collect(),
            pq_shares: Some((0..HSM_SHARES).map(|_| PqKeypair::generate()).collect()),
        }
    }

    pub fn share_pubkeys(&self) -> Vec<[u8; 32]> {
        self.shares
            .iter()
            .map(|k| k.verifying_key_bytes())
            .collect()
    }

    pub fn export_ceremony(&self, created_unix_ms: u64) -> CeremonyTranscript {
        let share_pubkeys = self.share_pubkeys();
        let hybrid = self.pq_shares.is_some();
        let root_commitment = ceremony_root_commitment(&share_pubkeys, HSM_QUORUM, hybrid);
        CeremonyTranscript {
            share_pubkeys,
            quorum: HSM_QUORUM,
            hybrid,
            created_unix_ms,
            root_commitment,
        }
    }

    pub fn verify_ceremony_transcript(t: &CeremonyTranscript) -> Result<(), HsmError> {
        if t.share_pubkeys.len() != HSM_SHARES || t.quorum != HSM_QUORUM {
            return Err(HsmError::UnknownShare);
        }
        let expected = ceremony_root_commitment(&t.share_pubkeys, t.quorum, t.hybrid);
        if expected != t.root_commitment {
            return Err(HsmError::BadShareSignature);
        }
        Ok(())
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
            let (signature, pq_pubkey, pq_signature) = if let Some(pq_shares) = &self.pq_shares {
                let pq = pq_shares.get(idx).ok_or(HsmError::UnknownShare)?;
                dual_sign_root_op(kp, pq, &bytes, op).map_err(|_| HsmError::BadShareSignature)?
            } else {
                (
                    kp.signing_key().sign(&bytes).to_bytes().to_vec(),
                    vec![],
                    vec![],
                )
            };
            out.push(ShareSig {
                share_id: id,
                pubkey: kp.verifying_key_bytes(),
                signature,
                pq_pubkey,
                pq_signature,
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
            if self.pq_shares.is_some()
                || !share.pq_pubkey.is_empty()
                || !share.pq_signature.is_empty()
            {
                verify_root_op_pq(&share.pubkey, &share.pq_pubkey, &share.pq_signature, &sig.op)
                    .map_err(|_| HsmError::BadShareSignature)?;
            }
        }
        Ok(())
    }
}
