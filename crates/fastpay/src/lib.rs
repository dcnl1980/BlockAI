//! FastPay-style regional consistent broadcast (SEEF §6.7).
//!
//! Not on the PAY hot path — used for cross-shard reallocations and capability top-ups.

use blockai_crypto::Keypair;
use blockai_types::{
    encode_cbor, AccountId, AmountMicros, CapabilityId, ShardId,
};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

pub const REGIONAL_AUTHORITIES: usize = 4;
pub const REGIONAL_QUORUM: usize = 3;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FastPayError {
    #[error("insufficient shares: have {have} need {need}")]
    InsufficientShares { have: usize, need: usize },
    #[error("duplicate authority")]
    DuplicateAuthority,
    #[error("unknown authority")]
    UnknownAuthority,
    #[error("bad signature")]
    BadSignature,
    #[error("nonce already consumed")]
    NonceConsumed,
    #[error("cbor encode failed")]
    Cbor,
    #[error("same source and destination shard")]
    SameShard,
    #[error("zero amount")]
    ZeroAmount,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionalOp {
    /// Move unused authority/L1 shard allowance between shards.
    Reallocate {
        account: AccountId,
        from_shard: ShardId,
        to_shard: ShardId,
        amount: AmountMicros,
        nonce: u64,
    },
    /// Increase remaining on an activated capability, funded from shard allowance.
    TopUpCapability {
        account: AccountId,
        shard_id: ShardId,
        capability_id: CapabilityId,
        amount: AmountMicros,
        nonce: u64,
    },
}

impl RegionalOp {
    pub fn nonce(&self) -> u64 {
        match self {
            RegionalOp::Reallocate { nonce, .. } => *nonce,
            RegionalOp::TopUpCapability { nonce, .. } => *nonce,
        }
    }

    pub fn validate(&self) -> Result<(), FastPayError> {
        match self {
            RegionalOp::Reallocate {
                from_shard,
                to_shard,
                amount,
                ..
            } => {
                if from_shard == to_shard {
                    return Err(FastPayError::SameShard);
                }
                if amount.0 == 0 {
                    return Err(FastPayError::ZeroAmount);
                }
            }
            RegionalOp::TopUpCapability { amount, .. } => {
                if amount.0 == 0 {
                    return Err(FastPayError::ZeroAmount);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityShareSig {
    pub authority_id: u8,
    pub pubkey: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionalCertificate {
    pub op: RegionalOp,
    pub shares: Vec<AuthorityShareSig>,
}

#[derive(Serialize)]
struct RegionalSignBody<'a> {
    domain: &'static str,
    op: &'a RegionalOp,
}

pub fn op_digest(op: &RegionalOp) -> Result<[u8; 32], FastPayError> {
    let bytes = encode_cbor(&RegionalSignBody {
        domain: "REGIONAL_OP",
        op,
    })
    .map_err(|_| FastPayError::Cbor)?;
    Ok(*blake3::hash(&bytes).as_bytes())
}

/// Soft regional committee: 4 authorities, quorum 3 (lab FastPay).
pub struct RegionalCommittee {
    keys: Vec<Keypair>,
    consumed_nonces: HashSet<u64>,
}

impl RegionalCommittee {
    pub fn generate() -> Self {
        Self {
            keys: (0..REGIONAL_AUTHORITIES).map(|_| Keypair::generate()).collect(),
            consumed_nonces: HashSet::new(),
        }
    }

    pub fn pubkeys(&self) -> Vec<[u8; 32]> {
        self.keys.iter().map(|k| k.verifying_key_bytes()).collect()
    }

    pub fn sign_with(
        &self,
        op: &RegionalOp,
        authority_ids: &[u8],
    ) -> Result<RegionalCertificate, FastPayError> {
        op.validate()?;
        let digest = op_digest(op)?;
        let mut shares = Vec::new();
        let mut seen = HashSet::new();
        for &id in authority_ids {
            if !seen.insert(id) {
                return Err(FastPayError::DuplicateAuthority);
            }
            let kp = self
                .keys
                .get(id as usize)
                .ok_or(FastPayError::UnknownAuthority)?;
            shares.push(AuthorityShareSig {
                authority_id: id,
                pubkey: kp.verifying_key_bytes(),
                signature: kp.signing_key().sign(&digest).to_bytes().to_vec(),
            });
        }
        Ok(RegionalCertificate {
            op: op.clone(),
            shares,
        })
    }

    pub fn verify(
        &self,
        cert: &RegionalCertificate,
        quorum: usize,
    ) -> Result<(), FastPayError> {
        cert.op.validate()?;
        if cert.shares.len() < quorum {
            return Err(FastPayError::InsufficientShares {
                have: cert.shares.len(),
                need: quorum,
            });
        }
        if self.consumed_nonces.contains(&cert.op.nonce()) {
            return Err(FastPayError::NonceConsumed);
        }
        let allowed: HashMap<[u8; 32], ()> =
            self.pubkeys().into_iter().map(|pk| (pk, ())).collect();
        let digest = op_digest(&cert.op)?;
        let mut seen = HashSet::new();
        for share in &cert.shares {
            if !seen.insert(share.authority_id) {
                return Err(FastPayError::DuplicateAuthority);
            }
            if !allowed.contains_key(&share.pubkey) {
                return Err(FastPayError::UnknownAuthority);
            }
            let vk = VerifyingKey::from_bytes(&share.pubkey)
                .map_err(|_| FastPayError::BadSignature)?;
            let sig_bytes: [u8; 64] = share
                .signature
                .as_slice()
                .try_into()
                .map_err(|_| FastPayError::BadSignature)?;
            vk.verify(&digest, &Signature::from_bytes(&sig_bytes))
                .map_err(|_| FastPayError::BadSignature)?;
        }
        Ok(())
    }

    /// Verify and mark nonce consumed (single-use certificate).
    pub fn consume(
        &mut self,
        cert: &RegionalCertificate,
        quorum: usize,
    ) -> Result<(), FastPayError> {
        self.verify(cert, quorum)?;
        self.consumed_nonces.insert(cert.op.nonce());
        Ok(())
    }
}
