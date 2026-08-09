use crate::bft::{quorum_threshold, BftMessage, PayCommitBody};
use crate::state::ShardState;
use crate::wal::{Wal, WalRecord};
use crate::ShardError;
use blockai_crypto::{verify_capability, verify_pay, verifying_key_from_bytes, Keypair};
use blockai_types::{tx_id, Epoch, EpochState, Pay, ShardId, SpendCapability};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, Notify};

#[derive(Clone, Debug)]
pub struct EdgeAccept {
    pub commit_index: u64,
    pub tx_id: [u8; 32],
    pub edge_signature: Vec<u8>,
}

pub struct ValidatorConfig {
    pub id: u8,
    pub shard_id: ShardId,
    pub key: Keypair,
    pub peer_keys: HashMap<u8, VerifyingKey>,
    pub issuer_vk: VerifyingKey,
}

struct Inner {
    cfg: ValidatorConfig,
    state: ShardState,
    wal: Wal,
    capabilities: HashMap<blockai_types::CapabilityId, SpendCapability>,
    alive: bool,
    inbox: mpsc::UnboundedReceiver<BftMessage>,
    outboxes: HashMap<u8, mpsc::UnboundedSender<BftMessage>>,
    peer_notifies: HashMap<u8, Arc<Notify>>,
}

pub struct ShardEngine {
    inner: Arc<Mutex<Inner>>,
    shard_id: ShardId,
    id: u8,
    notify: Arc<Notify>,
}

impl ShardEngine {
    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn shard_id(&self) -> &ShardId {
        &self.shard_id
    }

    pub async fn activate_capability(&self, cap: SpendCapability) -> Result<(), ShardError> {
        let mut inner = self.inner.lock().await;
        if !inner.alive {
            return Err(ShardError::ValidatorKilled);
        }
        verify_capability(&inner.cfg.issuer_vk, &cap).map_err(|_| ShardError::BadSignature)?;
        inner.wal.append(&WalRecord::ActivateCapability {
            capability_id: cap.capability_id,
            epoch: cap.epoch,
            remaining: cap.maximum_total,
            sequence_start: cap.sequence_start,
            sequence_end: cap.sequence_end,
        })?;
        inner.state.activate_capability(
            cap.capability_id,
            cap.epoch,
            cap.maximum_total,
            cap.sequence_start,
            cap.sequence_end,
        );
        inner.capabilities.insert(cap.capability_id, cap);
        Ok(())
    }

    pub async fn fence_epoch(&self, epoch: Epoch) -> Result<(), ShardError> {
        let need = {
            let inner = self.inner.lock().await;
            if !inner.alive {
                return Err(ShardError::ValidatorKilled);
            }
            let n = inner.outboxes.len() + 1;
            Self::broadcast(
                &inner,
                BftMessage::FenceCommit {
                    epoch,
                    voters: vec![inner.cfg.id],
                },
            );
            quorum_threshold(n)
        };

        // Local durable fence
        {
            let mut inner = self.inner.lock().await;
            if !matches!(inner.state.epoch_state(epoch), EpochState::Fenced) {
                inner.wal.append(&WalRecord::FenceEpoch { epoch })?;
                inner.state.fence_epoch(epoch);
            }
            Self::broadcast(
                &inner,
                BftMessage::FenceDurableAck {
                    epoch,
                    validator: inner.cfg.id,
                },
            );
        }

        let mut acks = vec![self.id];
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        while acks.len() < need {
            if tokio::time::Instant::now() > deadline {
                return Err(ShardError::BftQuorumFailed);
            }
            let mut inner = self.inner.lock().await;
            while let Ok(msg) = inner.inbox.try_recv() {
                match msg {
                    BftMessage::FenceDurableAck {
                        epoch: e,
                        validator,
                    } if e == epoch => {
                        if !acks.contains(&validator) {
                            acks.push(validator);
                        }
                    }
                    other => Self::handle_message(&mut inner, other)?,
                }
            }
            drop(inner);
            self.wait_for_peer_message().await;
        }
        Ok(())
    }

    pub async fn handle_pay(&self, pay: Pay, now_ms: u64) -> Result<EdgeAccept, ShardError> {
        let body = {
            let inner = self.inner.lock().await;
            if !inner.alive {
                return Err(ShardError::ValidatorKilled);
            }
            Self::validate_pay(&inner, &pay, now_ms)?;
            let tid = tx_id(&pay);
            let body = PayCommitBody::from_pay(&pay, tid);
            Self::broadcast(
                &inner,
                BftMessage::Propose {
                    body: body.clone(),
                    pay: pay.clone(),
                    leader: inner.cfg.id,
                    now_ms,
                },
            );
            let digest = body.digest();
            let vote_sig = sign_digest(&inner.cfg.key, &digest);
            Self::broadcast(
                &inner,
                BftMessage::Vote {
                    digest,
                    voter: inner.cfg.id,
                    signature: vote_sig,
                },
            );
            body
        };

        let digest = body.digest();
        let need = {
            let inner = self.inner.lock().await;
            quorum_threshold(inner.outboxes.len() + 1)
        };
        let mut votes: HashMap<u8, Vec<u8>> = HashMap::new();
        {
            let inner = self.inner.lock().await;
            votes.insert(self.id, sign_digest(&inner.cfg.key, &digest));
        }

        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        while votes.len() < need {
            if tokio::time::Instant::now() > deadline {
                return Err(ShardError::BftQuorumFailed);
            }
            // Do not call pump() here: it would drop Vote messages in handle_message.
            let mut inner = self.inner.lock().await;
            while let Ok(msg) = inner.inbox.try_recv() {
                match msg {
                    BftMessage::Vote {
                        digest: d,
                        voter,
                        signature,
                    } if d == digest => {
                        let vk_opt = if voter == inner.cfg.id {
                            Some(inner.cfg.key.verifying_key())
                        } else {
                            inner.cfg.peer_keys.get(&voter).copied()
                        };
                        if let Some(vk) = vk_opt {
                            if verify_digest(&vk, &d, &signature).is_ok() {
                                votes.insert(voter, signature);
                            }
                        }
                    }
                    other => Self::handle_message(&mut inner, other)?,
                }
            }
            drop(inner);
            if votes.len() < need {
                self.wait_for_peer_message().await;
            }
        }

        let vote_vec: Vec<(u8, Vec<u8>)> = votes.into_iter().collect();
        {
            let mut inner = self.inner.lock().await;
            Self::broadcast(
                &inner,
                BftMessage::Commit {
                    body: body.clone(),
                    votes: vote_vec,
                },
            );
            if !inner
                .state
                .is_consumed(body.capability_id, body.epoch, body.sequence)
            {
                Self::durable_commit(&mut inner, &body)?;
            }
            Self::broadcast(
                &inner,
                BftMessage::DurableAck {
                    digest,
                    validator: inner.cfg.id,
                },
            );
        }

        let mut acks = vec![self.id];
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        while acks.len() < need {
            if tokio::time::Instant::now() > deadline {
                return Err(ShardError::BftQuorumFailed);
            }
            let mut inner = self.inner.lock().await;
            while let Ok(msg) = inner.inbox.try_recv() {
                match msg {
                    BftMessage::DurableAck {
                        digest: d,
                        validator,
                    } if d == digest => {
                        if !acks.contains(&validator) {
                            acks.push(validator);
                        }
                    }
                    other => Self::handle_message(&mut inner, other)?,
                }
            }
            drop(inner);
            if acks.len() < need {
                self.wait_for_peer_message().await;
            }
        }

        let inner = self.inner.lock().await;
        let commit_index = inner.state.commit_index;
        let edge_signature = sign_edge_accept(&inner.cfg.key, digest, commit_index);
        Ok(EdgeAccept {
            commit_index,
            tx_id: body.tx_id,
            edge_signature,
        })
    }

    pub async fn kill(&self) {
        let mut inner = self.inner.lock().await;
        inner.alive = false;
    }

    pub async fn pump(&self) -> Result<(), ShardError> {
        let mut inner = self.inner.lock().await;
        if !inner.alive {
            return Ok(());
        }
        while let Ok(msg) = inner.inbox.try_recv() {
            Self::handle_message(&mut inner, msg)?;
        }
        Ok(())
    }

    /// Wait for a peer wakeup (or a short safety timeout), then pump inbox.
    pub async fn wait_and_pump(&self) -> Result<(), ShardError> {
        self.wait_for_peer_message().await;
        self.pump().await
    }

    async fn wait_for_peer_message(&self) {
        tokio::select! {
            _ = self.notify.notified() => {}
            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
        }
    }

    fn broadcast(inner: &Inner, msg: BftMessage) {
        for (peer, tx) in &inner.outboxes {
            let _ = tx.send(msg.clone());
            if let Some(n) = inner.peer_notifies.get(peer) {
                n.notify_one();
            }
        }
    }

    fn handle_message(inner: &mut Inner, msg: BftMessage) -> Result<(), ShardError> {
        if !inner.alive {
            return Ok(());
        }
        match msg {
            BftMessage::Propose {
                body, pay, now_ms, ..
            } => {
                if Self::validate_pay(inner, &pay, now_ms).is_ok() {
                    let digest = body.digest();
                    let signature = sign_digest(&inner.cfg.key, &digest);
                    Self::broadcast(
                        inner,
                        BftMessage::Vote {
                            digest,
                            voter: inner.cfg.id,
                            signature,
                        },
                    );
                }
            }
            BftMessage::Commit { body, votes } => {
                let digest = body.digest();
                let need = quorum_threshold(inner.outboxes.len() + 1);
                let mut good = 0usize;
                for (voter, sig) in &votes {
                    let vk = if *voter == inner.cfg.id {
                        inner.cfg.key.verifying_key()
                    } else if let Some(vk) = inner.cfg.peer_keys.get(voter) {
                        *vk
                    } else {
                        continue;
                    };
                    if verify_digest(&vk, &digest, sig).is_ok() {
                        good += 1;
                    }
                }
                if good >= need {
                    if !inner
                        .state
                        .is_consumed(body.capability_id, body.epoch, body.sequence)
                    {
                        Self::durable_commit(inner, &body)?;
                    }
                    Self::broadcast(
                        inner,
                        BftMessage::DurableAck {
                            digest,
                            validator: inner.cfg.id,
                        },
                    );
                }
            }
            BftMessage::FenceCommit { epoch, .. } => {
                if !matches!(inner.state.epoch_state(epoch), EpochState::Fenced) {
                    inner.wal.append(&WalRecord::FenceEpoch { epoch })?;
                    inner.state.fence_epoch(epoch);
                }
                Self::broadcast(
                    inner,
                    BftMessage::FenceDurableAck {
                        epoch,
                        validator: inner.cfg.id,
                    },
                );
            }
            BftMessage::Fence { epoch, .. } => {
                Self::broadcast(
                    inner,
                    BftMessage::FenceVote {
                        epoch,
                        voter: inner.cfg.id,
                    },
                );
            }
            BftMessage::Vote { .. }
            | BftMessage::DurableAck { .. }
            | BftMessage::FenceVote { .. }
            | BftMessage::FenceDurableAck { .. } => {}
        }
        Ok(())
    }

    fn durable_commit(inner: &mut Inner, body: &PayCommitBody) -> Result<(), ShardError> {
        inner.wal.append(&WalRecord::ConsumePay {
            tx_id: body.tx_id,
            capability_id: body.capability_id,
            epoch: body.epoch,
            sequence: body.sequence,
            amount: body.amount,
        })?;
        inner
            .state
            .consume_pay(body.capability_id, body.epoch, body.sequence, body.amount)?;
        Ok(())
    }

    fn validate_pay(inner: &Inner, pay: &Pay, now_ms: u64) -> Result<(), ShardError> {
        let cap = inner
            .capabilities
            .get(&pay.capability_id)
            .ok_or(ShardError::UnknownCapability)?;

        if cap.shard_id != inner.cfg.shard_id {
            return Err(ShardError::WrongShard {
                capability_shard: cap.shard_id.as_str().to_string(),
                engine_shard: inner.cfg.shard_id.as_str().to_string(),
            });
        }
        verify_capability(&inner.cfg.issuer_vk, cap).map_err(|_| ShardError::BadSignature)?;
        if pay.agent_id != cap.agent_id {
            return Err(ShardError::AgentMismatch);
        }
        let agent_vk =
            verifying_key_from_bytes(&pay.agent_id.0).map_err(|_| ShardError::BadSignature)?;
        verify_pay(&agent_vk, pay).map_err(|_| ShardError::BadSignature)?;

        match inner.state.epoch_state(pay.epoch) {
            EpochState::Fenced => return Err(ShardError::EpochFenced { epoch: pay.epoch }),
            EpochState::Expired => return Err(ShardError::EpochExpired { epoch: pay.epoch }),
            EpochState::Active => {}
        }
        if now_ms < cap.valid_from_unix_ms {
            return Err(ShardError::NotYetValid);
        }
        if now_ms > cap.valid_until_unix_ms {
            return Err(ShardError::CapabilityExpired);
        }
        if now_ms > pay.expiry_unix_ms {
            return Err(ShardError::PayExpired);
        }
        if pay.currency != cap.currency {
            return Err(ShardError::CurrencyMismatch);
        }
        if pay.amount.0 > cap.maximum_per_call.0 {
            return Err(ShardError::ExceedsPerCall {
                amount: pay.amount,
                maximum_per_call: cap.maximum_per_call,
            });
        }
        if pay.amount.0 > pay.max_amount.0 {
            return Err(ShardError::ExceedsMaxAmount);
        }
        if !service_in_scope(&pay.service_id, &cap.service_scope) {
            return Err(ShardError::ServiceOutOfScope);
        }
        if inner
            .state
            .is_consumed(pay.capability_id, pay.epoch, pay.sequence)
        {
            return Err(ShardError::Replay {
                capability_id: pay.capability_id,
                epoch: pay.epoch,
                sequence: pay.sequence,
            });
        }
        let remaining = inner.state.remaining(&pay.capability_id)?;
        if pay.amount.0 > remaining.0 {
            return Err(ShardError::InsufficientRemaining {
                remaining,
                requested: pay.amount,
            });
        }
        Ok(())
    }
}

fn service_in_scope(service_id: &str, scope: &[String]) -> bool {
    scope.iter().any(|pattern| {
        if let Some(prefix) = pattern.strip_suffix("/*") {
            service_id == prefix || service_id.starts_with(&format!("{prefix}/"))
        } else {
            service_id == pattern
        }
    })
}

fn sign_digest(kp: &Keypair, digest: &[u8; 32]) -> Vec<u8> {
    kp.signing_key().sign(digest).to_bytes().to_vec()
}

fn verify_digest(vk: &VerifyingKey, digest: &[u8; 32], sig: &[u8]) -> Result<(), ShardError> {
    let sig_bytes: [u8; 64] = sig.try_into().map_err(|_| ShardError::BadSignature)?;
    let signature = Signature::from_bytes(&sig_bytes);
    vk.verify(digest, &signature)
        .map_err(|_| ShardError::BadSignature)
}

fn sign_edge_accept(kp: &Keypair, digest: [u8; 32], commit_index: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&digest);
    buf.extend_from_slice(&commit_index.to_le_bytes());
    kp.signing_key().sign(&buf).to_bytes().to_vec()
}

pub fn open_engine(
    cfg: ValidatorConfig,
    wal_path: impl AsRef<Path>,
    inbox: mpsc::UnboundedReceiver<BftMessage>,
    outboxes: HashMap<u8, mpsc::UnboundedSender<BftMessage>>,
    peer_notifies: HashMap<u8, Arc<Notify>>,
    notify: Arc<Notify>,
) -> Result<ShardEngine, ShardError> {
    let wal = Wal::open(wal_path)?;
    let state = wal.replay()?;
    let shard_id = cfg.shard_id.clone();
    let id = cfg.id;
    Ok(ShardEngine {
        inner: Arc::new(Mutex::new(Inner {
            cfg,
            state,
            wal,
            capabilities: HashMap::new(),
            alive: true,
            inbox,
            outboxes,
            peer_notifies,
        })),
        shard_id,
        id,
        notify,
    })
}
