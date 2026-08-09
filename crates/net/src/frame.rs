use blockai_types::{encode_cbor, Pay};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Application frames on the SEEF QUIC dataplane.
///
/// TLS/QUIC keys authenticate the transport only — never payment authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppFrame {
    /// May be sent as QUIC 0-RTT early data.
    IdempotentRead {
        path: String,
    },
    /// Must never be accepted as 0-RTT early data.
    Pay {
        pay: Pay,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("cbor encode failed")]
    Encode,
    #[error("cbor decode failed")]
    Decode,
    #[error("frame too large")]
    TooLarge,
}

const MAX_FRAME: usize = 256 * 1024;

pub fn encode_frame(frame: &AppFrame) -> Result<Vec<u8>, FrameError> {
    let body = encode_cbor(frame).map_err(|_| FrameError::Encode)?;
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode_frame(bytes: &[u8]) -> Result<AppFrame, FrameError> {
    if bytes.len() < 4 {
        return Err(FrameError::Decode);
    }
    let len = u32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
    if len > MAX_FRAME || bytes.len() != 4 + len {
        return Err(FrameError::Decode);
    }
    ciborium::from_reader(&bytes[4..]).map_err(|_| FrameError::Decode)
}
