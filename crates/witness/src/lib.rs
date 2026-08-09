use blockai_crypto::Keypair;
use blockai_shard::verify_signed_checkpoint;
use blockai_types::{
    encode_cbor, SignedCheckpoint, WitnessSig, WitnessedCheckpoint,
};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::Serialize;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WitnessError {
    #[error("invalid shard checkpoint signature")]
    BadShardSignature,
    #[error("invalid witness signature")]
    BadWitnessSignature,
    #[error("duplicate witness")]
    DuplicateWitness,
    #[error("insufficient witnesses: have {have} need {need}")]
    InsufficientWitnesses { have: usize, need: usize },
    #[error("conflicting checkpoint for shard/epoch/height")]
    ConflictingCheckpoint,
    #[error("cbor error")]
    Cbor,
}

pub struct Witness {
    key: Keypair,
}

impl Witness {
    pub fn new(key: Keypair) -> Self {
        Self { key }
    }

    pub fn generate() -> Self {
        Self {
            key: Keypair::generate(),
        }
    }

    pub fn pubkey(&self) -> [u8; 32] {
        self.key.verifying_key_bytes()
    }

    pub fn countersign(&self, checkpoint: &SignedCheckpoint) -> Result<WitnessSig, WitnessError> {
        verify_signed_checkpoint(checkpoint).map_err(|_| WitnessError::BadShardSignature)?;
        let body = witness_body(checkpoint);
        let bytes = encode_cbor(&body).map_err(|_| WitnessError::Cbor)?;
        let signature = self.key.signing_key().sign(&bytes).to_bytes().to_vec();
        Ok(WitnessSig {
            witness_pubkey: self.pubkey(),
            signature,
        })
    }
}

#[derive(Serialize)]
struct WitnessSignBody {
    domain: &'static str,
    shard_id: String,
    epoch: u64,
    root: [u8; 32],
    height: u64,
    shard_signature: Vec<u8>,
}

fn witness_body(checkpoint: &SignedCheckpoint) -> WitnessSignBody {
    WitnessSignBody {
        domain: "WITNESS_CHECKPOINT",
        shard_id: checkpoint.header.shard_id.as_str().to_string(),
        epoch: checkpoint.header.epoch.0,
        root: checkpoint.header.root,
        height: checkpoint.header.height,
        shard_signature: checkpoint.shard_signature.clone(),
    }
}

pub fn verify_witness_sig(
    checkpoint: &SignedCheckpoint,
    sig: &WitnessSig,
) -> Result<(), WitnessError> {
    let vk = VerifyingKey::from_bytes(&sig.witness_pubkey)
        .map_err(|_| WitnessError::BadWitnessSignature)?;
    let body = witness_body(checkpoint);
    let bytes = encode_cbor(&body).map_err(|_| WitnessError::Cbor)?;
    let sig_bytes: [u8; 64] = sig
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| WitnessError::BadWitnessSignature)?;
    vk.verify(&bytes, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| WitnessError::BadWitnessSignature)
}

pub fn verify_witnessed(
    witnessed: &WitnessedCheckpoint,
    witnesses_required: usize,
) -> Result<(), WitnessError> {
    verify_signed_checkpoint(&witnessed.checkpoint)
        .map_err(|_| WitnessError::BadShardSignature)?;
    let mut seen = HashSet::new();
    for sig in &witnessed.witness_sigs {
        if !seen.insert(sig.witness_pubkey) {
            return Err(WitnessError::DuplicateWitness);
        }
        verify_witness_sig(&witnessed.checkpoint, sig)?;
    }
    if witnessed.witness_sigs.len() < witnesses_required {
        return Err(WitnessError::InsufficientWitnesses {
            have: witnessed.witness_sigs.len(),
            need: witnesses_required,
        });
    }
    Ok(())
}

/// Tracks accepted checkpoint roots to reject conflicting history for the same key.
#[derive(Default)]
pub struct WitnessSet {
    accepted: HashSet<(String, u64, u64, [u8; 32])>,
    // key without root for conflict detection
    by_key: std::collections::HashMap<(String, u64, u64), [u8; 32]>,
}

impl WitnessSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accept(
        &mut self,
        witnessed: &WitnessedCheckpoint,
        witnesses_required: usize,
    ) -> Result<(), WitnessError> {
        verify_witnessed(witnessed, witnesses_required)?;
        let h = &witnessed.checkpoint.header;
        let key = (
            h.shard_id.as_str().to_string(),
            h.epoch.0,
            h.height,
        );
        if let Some(existing) = self.by_key.get(&key) {
            if *existing != h.root {
                return Err(WitnessError::ConflictingCheckpoint);
            }
        } else {
            self.by_key.insert(key.clone(), h.root);
            self.accepted
                .insert((key.0, key.1, key.2, h.root));
        }
        Ok(())
    }
}
