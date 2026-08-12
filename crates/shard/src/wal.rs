use crate::state::ShardState;
use crate::ShardError;
use blockai_types::{
    decode_cbor, encode_cbor, AmountMicros, CapabilityId, Epoch, Sequence,
};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WalRecord {
    ActivateCapability {
        capability_id: CapabilityId,
        epoch: Epoch,
        remaining: AmountMicros,
        sequence_start: Sequence,
        sequence_end: Sequence,
    },
    ConsumePay {
        tx_id: [u8; 32],
        capability_id: CapabilityId,
        epoch: Epoch,
        sequence: Sequence,
        amount: AmountMicros,
    },
    FenceEpoch {
        epoch: Epoch,
    },
    TopUpCapability {
        capability_id: CapabilityId,
        amount: AmountMicros,
    },
}

pub struct Wal {
    path: PathBuf,
    file: File,
}

impl Wal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ShardError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ShardError::Io(e.to_string()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)
            .map_err(|e| ShardError::Io(e.to_string()))?;
        Ok(Self { path, file })
    }

    pub fn append(&mut self, record: &WalRecord) -> Result<(), ShardError> {
        let bytes = encode_cbor(record).map_err(|_| ShardError::Cbor)?;
        let len = (bytes.len() as u32).to_le_bytes();
        self.file
            .write_all(&len)
            .and_then(|_| self.file.write_all(&bytes))
            .and_then(|_| self.file.flush())
            .and_then(|_| self.file.sync_all())
            .map_err(|e| ShardError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn replay(&self) -> Result<ShardState, ShardError> {
        let mut file = File::open(&self.path).map_err(|e| ShardError::Io(e.to_string()))?;
        let mut state = ShardState::new();
        loop {
            let mut len_buf = [0u8; 4];
            match file.read_exact(&mut len_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(ShardError::Io(e.to_string())),
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            file.read_exact(&mut buf)
                .map_err(|e| ShardError::Io(e.to_string()))?;
            let record: WalRecord = decode_cbor(&buf).map_err(|_| ShardError::Cbor)?;
            apply_record(&mut state, &record)?;
        }
        Ok(state)
    }
}

fn apply_record(state: &mut ShardState, record: &WalRecord) -> Result<(), ShardError> {
    match record {
        WalRecord::ActivateCapability {
            capability_id,
            epoch,
            remaining,
            sequence_start,
            sequence_end,
        } => {
            state.activate_capability(
                *capability_id,
                *epoch,
                *remaining,
                *sequence_start,
                *sequence_end,
            );
        }
        WalRecord::ConsumePay {
            capability_id,
            epoch,
            sequence,
            amount,
            ..
        } => {
            state.consume_pay(*capability_id, *epoch, *sequence, *amount)?;
        }
        WalRecord::FenceEpoch { epoch } => {
            state.fence_epoch(*epoch);
        }
        WalRecord::TopUpCapability {
            capability_id,
            amount,
        } => {
            state.top_up(*capability_id, *amount)?;
        }
    }
    Ok(())
}
