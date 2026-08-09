use crate::frame::AppFrame;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdmitError {
    #[error("QUIC 0-RTT PAY forbidden")]
    ZeroRttPayForbidden,
}

/// Enforce SEEF transport rules: no 0-RTT for irreversible economic frames.
pub fn admit_frame(is_early_data: bool, frame: &AppFrame) -> Result<(), AdmitError> {
    match frame {
        AppFrame::Pay { .. } if is_early_data => Err(AdmitError::ZeroRttPayForbidden),
        AppFrame::Pay { .. } | AppFrame::IdempotentRead { .. } => Ok(()),
    }
}
