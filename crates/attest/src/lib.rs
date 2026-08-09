use blockai_crypto::{verifying_key_from_bytes, Keypair};
use blockai_types::encode_cbor;
use ed25519_dalek::{Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Software RATS-style attestation evidence (stub → real hardware later).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationEvidence {
    pub binary_hash: [u8; 32],
    pub config_hash: [u8; 32],
    pub version: String,
    pub hardware_id: [u8; 32],
    pub platform_pubkey: [u8; 32],
    pub platform_signature: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct AttestationPolicy {
    pub approved_binary_hashes: HashSet<[u8; 32]>,
    pub approved_config_hashes: HashSet<[u8; 32]>,
    pub approved_versions: HashSet<String>,
    pub approved_hardware_ids: HashSet<[u8; 32]>,
    pub platform_pubkey: [u8; 32],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttestError {
    #[error("attestation signature invalid")]
    BadSignature,
    #[error("binary hash not approved")]
    BinaryRejected,
    #[error("config hash not approved")]
    ConfigRejected,
    #[error("version not approved")]
    VersionRejected,
    #[error("hardware id not approved")]
    HardwareRejected,
    #[error("platform key mismatch")]
    PlatformMismatch,
    #[error("cbor encode failed")]
    CborEncode,
}

#[derive(Serialize)]
struct EvidenceBody<'a> {
    domain: &'static str,
    /// Platform attestation domain — never QUIC/TLS or PAY key material.
    key_domain: &'static str,
    binary_hash: [u8; 32],
    config_hash: [u8; 32],
    version: &'a str,
    hardware_id: [u8; 32],
    platform_pubkey: [u8; 32],
}

fn evidence_bytes(ev: &AttestationEvidence) -> Result<Vec<u8>, AttestError> {
    let body = EvidenceBody {
        domain: "ATTESTATION",
        key_domain: "platform-attest",
        binary_hash: ev.binary_hash,
        config_hash: ev.config_hash,
        version: &ev.version,
        hardware_id: ev.hardware_id,
        platform_pubkey: ev.platform_pubkey,
    };
    encode_cbor(&body).map_err(|_| AttestError::CborEncode)
}

pub fn sign_evidence(platform: &Keypair, mut ev: AttestationEvidence) -> AttestationEvidence {
    ev.platform_pubkey = platform.verifying_key_bytes();
    let bytes = evidence_bytes(&ev).expect("evidence encodes");
    ev.platform_signature = platform.signing_key().sign(&bytes).to_bytes().to_vec();
    ev
}

pub fn verify_evidence(
    policy: &AttestationPolicy,
    evidence: &AttestationEvidence,
) -> Result<(), AttestError> {
    if evidence.platform_pubkey != policy.platform_pubkey {
        return Err(AttestError::PlatformMismatch);
    }
    if !policy.approved_binary_hashes.contains(&evidence.binary_hash) {
        return Err(AttestError::BinaryRejected);
    }
    if !policy.approved_config_hashes.contains(&evidence.config_hash) {
        return Err(AttestError::ConfigRejected);
    }
    if !policy.approved_versions.contains(&evidence.version) {
        return Err(AttestError::VersionRejected);
    }
    if !policy
        .approved_hardware_ids
        .contains(&evidence.hardware_id)
    {
        return Err(AttestError::HardwareRejected);
    }
    if evidence.platform_signature.is_empty() {
        return Err(AttestError::BadSignature);
    }
    let bytes = evidence_bytes(evidence)?;
    let vk = verifying_key_from_bytes(&evidence.platform_pubkey)
        .map_err(|_| AttestError::BadSignature)?;
    let sig_bytes: [u8; 64] = evidence
        .platform_signature
        .as_slice()
        .try_into()
        .map_err(|_| AttestError::BadSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);
    vk.verify(&bytes, &sig)
        .map_err(|_| AttestError::BadSignature)?;
    Ok(())
}

/// Deterministic test platform + matching policy for unit/integration tests.
pub struct TestPlatform {
    pub platform: Keypair,
    pub policy: AttestationPolicy,
    pub binary_hash: [u8; 32],
    pub config_hash: [u8; 32],
    pub hardware_id: [u8; 32],
    pub version: String,
}

impl TestPlatform {
    pub fn new() -> Self {
        let platform = Keypair::generate();
        let binary_hash = *blake3::hash(b"blockai-edge-bin-v1").as_bytes();
        let config_hash = *blake3::hash(b"blockai-edge-cfg-v1").as_bytes();
        let hardware_id = *blake3::hash(b"sim-hw-001").as_bytes();
        let version = "0.1.0-sim".to_string();
        let mut approved_binary_hashes = HashSet::new();
        approved_binary_hashes.insert(binary_hash);
        let mut approved_config_hashes = HashSet::new();
        approved_config_hashes.insert(config_hash);
        let mut approved_versions = HashSet::new();
        approved_versions.insert(version.clone());
        let mut approved_hardware_ids = HashSet::new();
        approved_hardware_ids.insert(hardware_id);
        let policy = AttestationPolicy {
            approved_binary_hashes,
            approved_config_hashes,
            approved_versions,
            approved_hardware_ids,
            platform_pubkey: platform.verifying_key_bytes(),
        };
        Self {
            platform,
            policy,
            binary_hash,
            config_hash,
            hardware_id,
            version,
        }
    }

    pub fn evidence(&self) -> AttestationEvidence {
        sign_evidence(
            &self.platform,
            AttestationEvidence {
                binary_hash: self.binary_hash,
                config_hash: self.config_hash,
                version: self.version.clone(),
                hardware_id: self.hardware_id,
                platform_pubkey: [0u8; 32],
                platform_signature: vec![],
            },
        )
    }
}

impl Default for TestPlatform {
    fn default() -> Self {
        Self::new()
    }
}
