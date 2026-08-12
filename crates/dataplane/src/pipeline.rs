use blockai_net::{admit_frame, decode_frame, AdmitError, AppFrame};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DataplaneError {
    #[error("not bound")]
    NotBound,
    #[error("dpdk not available in this environment")]
    DpdkNotAvailable,
    #[error("frame too large")]
    TooLarge,
    #[error("rate limited")]
    RateLimited,
    #[error("admit: {0}")]
    Admit(#[from] AdmitError),
    #[error("decode failed")]
    Decode,
    #[error("capability cache miss")]
    CapCacheMiss,
}

#[derive(Clone, Debug)]
pub struct IngressPacket {
    pub bytes: Vec<u8>,
    pub is_early_data: bool,
    pub src_hint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipelineStage {
    CheapFilter,
    RateLimit,
    CapCache,
    Admit,
    Ready(AppFrame),
}

#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub max_frame_bytes: usize,
    pub rate_limit_per_src: u32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: 64 * 1024,
            rate_limit_per_src: 100,
        }
    }
}

/// SEEF §5.4 dataplane: cheap rejects before expensive work.
pub struct DataplanePipeline {
    pub cfg: PipelineConfig,
    hits: HashMap<String, u32>,
    /// Capability IDs (first 8 bytes hex) known warm in cache.
    cap_cache: HashMap<[u8; 32], ()>,
}

impl DataplanePipeline {
    pub fn new(cfg: PipelineConfig) -> Self {
        Self {
            cfg,
            hits: HashMap::new(),
            cap_cache: HashMap::new(),
        }
    }

    pub fn warm_capability(&mut self, capability_id: [u8; 32]) {
        self.cap_cache.insert(capability_id, ());
    }

    pub fn process(&mut self, pkt: &IngressPacket) -> Result<AppFrame, DataplaneError> {
        if pkt.bytes.len() > self.cfg.max_frame_bytes {
            return Err(DataplaneError::TooLarge);
        }
        let count = self.hits.entry(pkt.src_hint.clone()).or_insert(0);
        *count += 1;
        if *count > self.cfg.rate_limit_per_src {
            return Err(DataplaneError::RateLimited);
        }
        let frame = decode_frame(&pkt.bytes).map_err(|_| DataplaneError::Decode)?;
        // Reject 0-RTT PAY before spending cache/crypto budget.
        admit_frame(pkt.is_early_data, &frame)?;
        if let AppFrame::Pay { pay } = &frame {
            if !self.cap_cache.contains_key(&pay.capability_id.0) {
                return Err(DataplaneError::CapCacheMiss);
            }
        }
        Ok(frame)
    }
}
