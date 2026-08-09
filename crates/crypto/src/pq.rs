use crate::sign::CryptoError;
use ml_dsa::{
    EncodedSignature, EncodedVerifyingKey, Generate, Keypair as _, MlDsa65, Signature, SigningKey,
    Signer, Verifier,
};
use serde::{Deserialize, Serialize};

/// ML-DSA-65 keypair (PQ path for capabilities / checkpoints / root artifacts).
pub struct PqKeypair {
    signing: SigningKey<MlDsa65>,
}

impl PqKeypair {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::<MlDsa65>::generate(),
        }
    }

    pub fn verifying_key_bytes(&self) -> Vec<u8> {
        self.signing.verifying_key().encode().to_vec()
    }

    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        let sig: Signature<MlDsa65> = self.signing.sign(msg);
        sig.encode().to_vec()
    }
}

pub fn verify_pq(pubkey: &[u8], msg: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
    let vk_arr = EncodedVerifyingKey::<MlDsa65>::try_from(pubkey)
        .map_err(|_| CryptoError::InvalidVerifyingKey)?;
    let vk = ml_dsa::VerifyingKey::<MlDsa65>::decode(&vk_arr);
    let sig_arr = EncodedSignature::<MlDsa65>::try_from(signature)
        .map_err(|_| CryptoError::InvalidSignature)?;
    let sig = Signature::<MlDsa65>::decode(&sig_arr).ok_or(CryptoError::InvalidSignature)?;
    vk.verify(msg, &sig)
        .map_err(|_| CryptoError::InvalidSignature)
}

/// Serializable PQ public material (for tests / envelopes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PqPublicKey(pub Vec<u8>);
