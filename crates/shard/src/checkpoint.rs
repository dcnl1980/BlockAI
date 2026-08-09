use crate::merkle::merkle_root;
use crate::receipt_log::ReceiptLog;
use crate::ShardError;
use blockai_crypto::Keypair;
use blockai_types::{
    encode_cbor, AmountMicros, CheckpointHeader, Epoch, ShardId, SignedCheckpoint,
};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct CheckpointSealer {
    pub max_txs: u64,
    pub max_exposure: AmountMicros,
    pub next_height: u64,
}

impl CheckpointSealer {
    pub fn new(max_txs: u64, max_exposure: AmountMicros) -> Self {
        Self {
            max_txs,
            max_exposure,
            next_height: 1,
        }
    }

    pub fn should_seal(&self, log: &ReceiptLog) -> bool {
        if log.is_empty() {
            return false;
        }
        log.len() as u64 >= self.max_txs || log.exposure().0 >= self.max_exposure.0
    }

    pub fn maybe_seal(
        &mut self,
        log: &mut ReceiptLog,
        shard_kp: &Keypair,
        shard_id: ShardId,
        epoch: Epoch,
        now_unix_ms: u64,
    ) -> Result<Option<SignedCheckpoint>, ShardError> {
        if !self.should_seal(log) {
            return Ok(None);
        }
        Ok(Some(self.force_seal(log, shard_kp, shard_id, epoch, now_unix_ms)?))
    }

    pub fn force_seal(
        &mut self,
        log: &mut ReceiptLog,
        shard_kp: &Keypair,
        shard_id: ShardId,
        epoch: Epoch,
        now_unix_ms: u64,
    ) -> Result<SignedCheckpoint, ShardError> {
        if log.is_empty() {
            return Err(ShardError::EmptyReceiptLog);
        }
        let root = merkle_root(log.leaves());
        let header = CheckpointHeader {
            shard_id,
            epoch,
            root,
            height: self.next_height,
            tx_count: log.len() as u64,
            exposure: log.exposure(),
            sealed_at_unix_ms: now_unix_ms,
        };
        let sig = sign_checkpoint(shard_kp, &header)?;
        let signed = SignedCheckpoint {
            header,
            shard_signer_pubkey: shard_kp.verifying_key_bytes(),
            shard_signature: sig,
        };
        self.next_height += 1;
        log.clear_sealed();
        Ok(signed)
    }
}

#[derive(Serialize)]
struct CheckpointSignBody {
    domain: &'static str,
    shard_id: String,
    epoch: u64,
    root: [u8; 32],
    height: u64,
    tx_count: u64,
    exposure: u128,
    sealed_at_unix_ms: u64,
}

fn sign_checkpoint(kp: &Keypair, header: &CheckpointHeader) -> Result<Vec<u8>, ShardError> {
    let body = CheckpointSignBody {
        domain: "CHECKPOINT",
        shard_id: header.shard_id.as_str().to_string(),
        epoch: header.epoch.0,
        root: header.root,
        height: header.height,
        tx_count: header.tx_count,
        exposure: header.exposure.0,
        sealed_at_unix_ms: header.sealed_at_unix_ms,
    };
    let bytes = encode_cbor(&body).map_err(|_| ShardError::Cbor)?;
    Ok(kp.signing_key().sign(&bytes).to_bytes().to_vec())
}

pub fn verify_signed_checkpoint(
    checkpoint: &SignedCheckpoint,
) -> Result<(), ShardError> {
    let vk = VerifyingKey::from_bytes(&checkpoint.shard_signer_pubkey)
        .map_err(|_| ShardError::BadSignature)?;
    let body = CheckpointSignBody {
        domain: "CHECKPOINT",
        shard_id: checkpoint.header.shard_id.as_str().to_string(),
        epoch: checkpoint.header.epoch.0,
        root: checkpoint.header.root,
        height: checkpoint.header.height,
        tx_count: checkpoint.header.tx_count,
        exposure: checkpoint.header.exposure.0,
        sealed_at_unix_ms: checkpoint.header.sealed_at_unix_ms,
    };
    let bytes = encode_cbor(&body).map_err(|_| ShardError::Cbor)?;
    let sig_bytes: [u8; 64] = checkpoint
        .shard_signature
        .as_slice()
        .try_into()
        .map_err(|_| ShardError::BadSignature)?;
    vk.verify(&bytes, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| ShardError::BadSignature)
}
