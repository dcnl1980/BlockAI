use crate::ShardError;
use blockai_types::{AmountMicros, CapabilityId, Epoch, EpochState, Sequence};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct ActiveCapability {
    pub remaining: AmountMicros,
    pub sequence_start: Sequence,
    pub sequence_end: Sequence,
    pub epoch: Epoch,
}

#[derive(Clone, Debug, Default)]
pub struct ShardState {
    capabilities: HashMap<CapabilityId, ActiveCapability>,
    consumed: HashSet<(CapabilityId, u64, u64)>,
    epoch_states: HashMap<u64, EpochState>,
    pub commit_index: u64,
}

impl ShardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn activate_capability(
        &mut self,
        capability_id: CapabilityId,
        epoch: Epoch,
        remaining: AmountMicros,
        sequence_start: Sequence,
        sequence_end: Sequence,
    ) {
        self.capabilities.insert(
            capability_id,
            ActiveCapability {
                remaining,
                sequence_start,
                sequence_end,
                epoch,
            },
        );
        self.epoch_states
            .entry(epoch.0)
            .or_insert(EpochState::Active);
    }

    pub fn fence_epoch(&mut self, epoch: Epoch) {
        self.epoch_states.insert(epoch.0, EpochState::Fenced);
    }

    pub fn epoch_state(&self, epoch: Epoch) -> EpochState {
        self.epoch_states
            .get(&epoch.0)
            .copied()
            .unwrap_or(EpochState::Expired)
    }

    pub fn remaining(&self, capability_id: &CapabilityId) -> Result<AmountMicros, ShardError> {
        self.capabilities
            .get(capability_id)
            .map(|c| c.remaining)
            .ok_or(ShardError::UnknownCapability)
    }

    /// Increase remaining on an activated capability (FastPay top-up).
    pub fn top_up(
        &mut self,
        capability_id: CapabilityId,
        amount: AmountMicros,
    ) -> Result<(), ShardError> {
        if amount.0 == 0 {
            return Err(ShardError::InsufficientRemaining {
                remaining: AmountMicros(0),
                requested: amount,
            });
        }
        let cap = self
            .capabilities
            .get_mut(&capability_id)
            .ok_or(ShardError::UnknownCapability)?;
        cap.remaining = AmountMicros(cap.remaining.0 + amount.0);
        Ok(())
    }

    pub fn is_consumed(&self, capability_id: CapabilityId, epoch: Epoch, sequence: Sequence) -> bool {
        self.consumed
            .contains(&(capability_id, epoch.0, sequence.0))
    }

    pub fn try_mark_consumed(
        &mut self,
        capability_id: CapabilityId,
        epoch: Epoch,
        sequence: Sequence,
    ) -> Result<(), ShardError> {
        if !self.consumed.insert((capability_id, epoch.0, sequence.0)) {
            return Err(ShardError::Replay {
                capability_id,
                epoch,
                sequence,
            });
        }
        Ok(())
    }

    pub fn consume_pay(
        &mut self,
        capability_id: CapabilityId,
        epoch: Epoch,
        sequence: Sequence,
        amount: AmountMicros,
    ) -> Result<u64, ShardError> {
        match self.epoch_state(epoch) {
            EpochState::Fenced => return Err(ShardError::EpochFenced { epoch }),
            EpochState::Expired => return Err(ShardError::EpochExpired { epoch }),
            EpochState::Active => {}
        }

        if self.is_consumed(capability_id, epoch, sequence) {
            return Err(ShardError::Replay {
                capability_id,
                epoch,
                sequence,
            });
        }

        let cap = self
            .capabilities
            .get_mut(&capability_id)
            .ok_or(ShardError::UnknownCapability)?;

        if epoch != cap.epoch {
            return Err(ShardError::EpochMismatch);
        }
        if sequence.0 < cap.sequence_start.0 || sequence.0 > cap.sequence_end.0 {
            return Err(ShardError::SequenceOutOfRange { sequence });
        }
        if amount.0 > cap.remaining.0 {
            return Err(ShardError::InsufficientRemaining {
                remaining: cap.remaining,
                requested: amount,
            });
        }

        cap.remaining = AmountMicros(cap.remaining.0 - amount.0);
        self.consumed.insert((capability_id, epoch.0, sequence.0));
        self.commit_index += 1;
        Ok(self.commit_index)
    }
}
