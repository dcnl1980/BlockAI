use crate::{
    sign_evidence, verify_evidence, AttestError, AttestationEvidence, AttestationPolicy,
    TestPlatform,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttestorError {
    #[error(transparent)]
    Verify(#[from] AttestError),
    #[error("hardware measurement unavailable")]
    MeasurementUnavailable,
    #[error("attestation backend unavailable")]
    BackendUnavailable,
}

/// Pluggable attestation collector (software today, TPM/RATS hardware later).
pub trait Attestor {
    fn name(&self) -> &'static str;
    fn collect(&self) -> Result<AttestationEvidence, AttestorError>;
    fn verify_against(
        &self,
        policy: &AttestationPolicy,
        evidence: &AttestationEvidence,
    ) -> Result<(), AttestorError> {
        verify_evidence(policy, evidence).map_err(AttestorError::Verify)
    }
}

/// Always produces policy-matching software evidence (lab).
pub struct SoftwareAttestor {
    platform: TestPlatform,
}

impl SoftwareAttestor {
    pub fn new() -> Self {
        Self {
            platform: TestPlatform::new(),
        }
    }

    pub fn policy(&self) -> &AttestationPolicy {
        &self.platform.policy
    }
}

impl Default for SoftwareAttestor {
    fn default() -> Self {
        Self::new()
    }
}

impl Attestor for SoftwareAttestor {
    fn name(&self) -> &'static str {
        "software"
    }

    fn collect(&self) -> Result<AttestationEvidence, AttestorError> {
        Ok(self.platform.evidence())
    }
}

/// Hardware-shaped attestor: fails closed until measured boot evidence is supplied.
pub struct HardwareAttestor {
    platform: TestPlatform,
    /// When false, collect() fails — production default without TPM quote.
    pub measured: bool,
}

impl HardwareAttestor {
    pub fn unmeasured(platform: TestPlatform) -> Self {
        Self {
            platform,
            measured: false,
        }
    }

    /// Lab helper: simulate a successful measured boot matching policy.
    pub fn with_measurement(platform: TestPlatform) -> Self {
        Self {
            platform,
            measured: true,
        }
    }

    pub fn policy(&self) -> &AttestationPolicy {
        &self.platform.policy
    }
}

impl Attestor for HardwareAttestor {
    fn name(&self) -> &'static str {
        "hardware-tpm-stub"
    }

    fn collect(&self) -> Result<AttestationEvidence, AttestorError> {
        if !self.measured {
            return Err(AttestorError::MeasurementUnavailable);
        }
        // Re-sign under platform key as a stand-in for a TPM quote over PCRs.
        Ok(sign_evidence(
            &self.platform.platform,
            AttestationEvidence {
                binary_hash: self.platform.binary_hash,
                config_hash: self.platform.config_hash,
                version: self.platform.version.clone(),
                hardware_id: self.platform.hardware_id,
                platform_pubkey: [0u8; 32],
                platform_signature: vec![],
            },
        ))
    }
}
