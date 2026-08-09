use blockai_types::{AmountMicros, CapabilityId, Epoch, Pay, Sequence};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayCommitBody {
    pub capability_id: CapabilityId,
    pub epoch: Epoch,
    pub sequence: Sequence,
    pub amount: AmountMicros,
    pub tx_id: [u8; 32],
    pub request_hash: [u8; 32],
    pub agent_id: [u8; 32],
    pub service_id: String,
}

impl PayCommitBody {
    pub fn from_pay(pay: &Pay, tx_id: [u8; 32]) -> Self {
        Self {
            capability_id: pay.capability_id,
            epoch: pay.epoch,
            sequence: pay.sequence,
            amount: pay.amount,
            tx_id,
            request_hash: pay.request_hash,
            agent_id: pay.agent_id.0,
            service_id: pay.service_id.clone(),
        }
    }

    pub fn digest(&self) -> [u8; 32] {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("digest encode");
        *blake3::hash(&buf).as_bytes()
    }
}

#[derive(Clone, Debug)]
pub enum BftMessage {
    Propose {
        body: PayCommitBody,
        pay: Pay,
        leader: u8,
        now_ms: u64,
    },
    Vote {
        digest: [u8; 32],
        voter: u8,
        signature: Vec<u8>,
    },
    Commit {
        body: PayCommitBody,
        votes: Vec<(u8, Vec<u8>)>,
    },
    Fence {
        epoch: Epoch,
        leader: u8,
    },
    FenceVote {
        epoch: Epoch,
        voter: u8,
    },
    FenceCommit {
        epoch: Epoch,
        voters: Vec<u8>,
    },
    DurableAck {
        digest: [u8; 32],
        validator: u8,
    },
    FenceDurableAck {
        epoch: Epoch,
        validator: u8,
    },
}

pub fn quorum_threshold(n: usize) -> usize {
    // 3-of-4 for n=4; generally floor(2n/3)+1 for small committees in Plan 1
    match n {
        4 => 3,
        _ => (2 * n) / 3 + 1,
    }
}
