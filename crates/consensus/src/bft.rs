use crate::dag::{DagBlock, DagMempool};
use crate::ConsensusError;
use blockai_crypto::Keypair;
use blockai_execute::{ExecuteError, GlobalState};
use blockai_types::L1Tx;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Clone, Debug)]
enum Msg {
    Propose {
        block: DagBlock,
        #[allow(dead_code)]
        leader: u8,
    },
    Vote {
        digest: [u8; 32],
        voter: u8,
        signature: Vec<u8>,
    },
    Commit {
        digest: [u8; 32],
        votes: Vec<(u8, Vec<u8>)>,
    },
}

struct ValidatorInner {
    id: u8,
    key: Keypair,
    peer_keys: HashMap<u8, VerifyingKey>,
    dag: DagMempool,
    state: GlobalState,
    alive: bool,
    inbox: mpsc::UnboundedReceiver<Msg>,
    outboxes: HashMap<u8, mpsc::UnboundedSender<Msg>>,
}

pub struct GlobalValidator {
    inner: Arc<Mutex<ValidatorInner>>,
    id: u8,
}

#[derive(Clone, Debug)]
pub struct CommitOutcome {
    pub digest: [u8; 32],
    pub applied: usize,
    pub height_hint: u64,
}

impl GlobalValidator {
    pub fn id(&self) -> u8 {
        self.id
    }

    pub async fn state_snapshot(&self) -> GlobalState {
        self.inner.lock().await.state.clone()
    }

    pub async fn kill(&self) {
        self.inner.lock().await.alive = false;
    }

    pub async fn pump(&self) {
        let mut inner = self.inner.lock().await;
        if !inner.alive {
            return;
        }
        while let Ok(msg) = inner.inbox.try_recv() {
            let _ = handle_msg(&mut inner, msg);
        }
    }

    pub async fn submit_and_commit(&self, txs: Vec<L1Tx>) -> Result<CommitOutcome, ConsensusError> {
        let block = {
            let mut inner = self.inner.lock().await;
            if !inner.alive {
                return Err(ConsensusError::ValidatorKilled);
            }
            let leader_id = inner.id;
            let block = inner.dag.propose(leader_id, txs);
            broadcast(
                &inner,
                Msg::Propose {
                    block: block.clone(),
                    leader: leader_id,
                },
            );
            let digest = block.digest();
            let sig = sign_digest(&inner.key, &digest);
            broadcast(
                &inner,
                Msg::Vote {
                    digest,
                    voter: leader_id,
                    signature: sig,
                },
            );
            block
        };

        let digest = block.digest();
        let need = 3usize;
        let mut votes: HashMap<u8, Vec<u8>> = HashMap::new();
        {
            let inner = self.inner.lock().await;
            votes.insert(self.id, sign_digest(&inner.key, &digest));
        }

        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        while votes.len() < need {
            if tokio::time::Instant::now() > deadline {
                return Err(ConsensusError::QuorumFailed);
            }
            let mut inner = self.inner.lock().await;
            while let Ok(msg) = inner.inbox.try_recv() {
                match msg {
                    Msg::Vote {
                        digest: d,
                        voter,
                        signature,
                    } if d == digest => {
                        let vk = if voter == inner.id {
                            inner.key.verifying_key()
                        } else if let Some(vk) = inner.peer_keys.get(&voter) {
                            *vk
                        } else {
                            continue;
                        };
                        if verify_digest(&vk, &d, &signature).is_ok() {
                            votes.insert(voter, signature);
                        }
                    }
                    other => {
                        let _ = handle_msg(&mut inner, other);
                    }
                }
            }
            drop(inner);
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        let vote_vec: Vec<(u8, Vec<u8>)> = votes.into_iter().collect();
        {
            let mut inner = self.inner.lock().await;
            broadcast(
                &inner,
                Msg::Commit {
                    digest,
                    votes: vote_vec,
                },
            );
            apply_block_txs(&mut inner, &block)?;
        }

        // Wait briefly for followers to apply
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        Ok(CommitOutcome {
            digest,
            applied: block.txs.len(),
            height_hint: block.round,
        })
    }
}

fn broadcast(inner: &ValidatorInner, msg: Msg) {
    for tx in inner.outboxes.values() {
        let _ = tx.send(msg.clone());
    }
}

fn handle_msg(inner: &mut ValidatorInner, msg: Msg) -> Result<(), ConsensusError> {
    if !inner.alive {
        return Ok(());
    }
    match msg {
        Msg::Propose { block, .. } => {
            let digest = block.digest();
            inner.dag.insert(block);
            let signature = sign_digest(&inner.key, &digest);
            broadcast(
                inner,
                Msg::Vote {
                    digest,
                    voter: inner.id,
                    signature,
                },
            );
        }
        Msg::Commit { digest, votes } => {
            let mut good = 0usize;
            for (voter, sig) in &votes {
                let vk = if *voter == inner.id {
                    inner.key.verifying_key()
                } else if let Some(vk) = inner.peer_keys.get(voter) {
                    *vk
                } else {
                    continue;
                };
                if verify_digest(&vk, &digest, sig).is_ok() {
                    good += 1;
                }
            }
            if good >= 3 {
                if let Some(block) = inner.dag.blocks.get(&digest).cloned() {
                    apply_block_txs(inner, &block)?;
                }
            }
        }
        Msg::Vote { .. } => {}
    }
    Ok(())
}

fn apply_block_txs(inner: &mut ValidatorInner, block: &DagBlock) -> Result<(), ConsensusError> {
    for tx in &block.txs {
        match inner.state.apply(tx) {
            Ok(()) => {}
            Err(ExecuteError::CheckpointAlreadyFinalized) => {}
            Err(e) => return Err(ConsensusError::Execute(e.to_string())),
        }
    }
    Ok(())
}

fn sign_digest(kp: &Keypair, digest: &[u8; 32]) -> Vec<u8> {
    kp.signing_key().sign(digest).to_bytes().to_vec()
}

fn verify_digest(vk: &VerifyingKey, digest: &[u8; 32], sig: &[u8]) -> Result<(), ConsensusError> {
    let bytes: [u8; 64] = sig.try_into().map_err(|_| ConsensusError::BadSignature)?;
    vk.verify(digest, &Signature::from_bytes(&bytes))
        .map_err(|_| ConsensusError::BadSignature)
}

pub struct GlobalCluster {
    pub validators: Vec<Arc<GlobalValidator>>,
    pumps: Vec<tokio::task::JoinHandle<()>>,
}

impl GlobalCluster {
    pub fn leader(&self) -> Arc<GlobalValidator> {
        self.validators[0].clone()
    }
}

impl Drop for GlobalCluster {
    fn drop(&mut self) {
        for p in &self.pumps {
            p.abort();
        }
    }
}

pub async fn cluster4(min_witnesses: usize) -> GlobalCluster {
    let secrets: Vec<[u8; 32]> = (0..4)
        .map(|_| Keypair::generate().signing_key().to_bytes())
        .collect();
    let peer_map: HashMap<u8, VerifyingKey> = secrets
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u8, Keypair::from_bytes(s).verifying_key()))
        .collect();

    let mut senders = HashMap::new();
    let mut receivers = HashMap::new();
    for i in 0..4u8 {
        let (tx, rx) = mpsc::unbounded_channel();
        senders.insert(i, tx);
        receivers.insert(i, rx);
    }

    let mut validators = Vec::new();
    for i in 0..4u8 {
        let mut outboxes = HashMap::new();
        for (peer, tx) in &senders {
            if *peer != i {
                outboxes.insert(*peer, tx.clone());
            }
        }
        let inner = ValidatorInner {
            id: i,
            key: Keypair::from_bytes(&secrets[i as usize]),
            peer_keys: peer_map
                .iter()
                .filter(|(id, _)| **id != i)
                .map(|(id, vk)| (*id, *vk))
                .collect(),
            dag: DagMempool::new(),
            state: GlobalState::new(min_witnesses),
            alive: true,
            inbox: receivers.remove(&i).unwrap(),
            outboxes,
        };
        validators.push(Arc::new(GlobalValidator {
            inner: Arc::new(Mutex::new(inner)),
            id: i,
        }));
    }

    let mut pumps = Vec::new();
    for v in validators.iter().skip(1).cloned() {
        pumps.push(tokio::spawn(async move {
            loop {
                v.pump().await;
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        }));
    }

    GlobalCluster { validators, pumps }
}
