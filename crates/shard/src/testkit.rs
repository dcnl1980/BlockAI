use crate::bft::BftMessage;
use crate::engine::{open_engine, ShardEngine, ValidatorConfig};
use blockai_crypto::Keypair;
use blockai_types::ShardId;
use ed25519_dalek::VerifyingKey;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

pub struct Cluster4 {
    pub engines: Vec<Arc<ShardEngine>>,
    pub issuer_vk: VerifyingKey,
    _dir: TempDir,
    pumps: Vec<JoinHandle<()>>,
    leader_idx: usize,
}

impl Cluster4 {
    pub fn leader(&self) -> Arc<ShardEngine> {
        self.engines[self.leader_idx].clone()
    }

    pub async fn kill(&self, validator_id: u8) {
        for eng in &self.engines {
            if eng.id() == validator_id {
                eng.kill().await;
            }
        }
    }
}

impl Drop for Cluster4 {
    fn drop(&mut self) {
        for p in &self.pumps {
            p.abort();
        }
    }
}

/// Build a 4-validator in-process shard cluster using the authority issuer key bytes.
pub async fn cluster4_with_issuer_bytes(shard: ShardId, issuer_signing_bytes: [u8; 32]) -> Cluster4 {
    let issuer = Keypair::from_bytes(&issuer_signing_bytes);
    let issuer_vk = issuer.verifying_key();
    let dir = tempfile::tempdir().expect("tempdir");

    let secrets: Vec<[u8; 32]> = (0..4)
        .map(|_| Keypair::generate().signing_key().to_bytes())
        .collect();
    let peer_map: HashMap<u8, VerifyingKey> = secrets
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u8, Keypair::from_bytes(s).verifying_key()))
        .collect();

    let mut senders: HashMap<u8, mpsc::UnboundedSender<BftMessage>> = HashMap::new();
    let mut receivers: HashMap<u8, mpsc::UnboundedReceiver<BftMessage>> = HashMap::new();
    let mut notifies: HashMap<u8, Arc<Notify>> = HashMap::new();
    for i in 0..4u8 {
        let (tx, rx) = mpsc::unbounded_channel();
        senders.insert(i, tx);
        receivers.insert(i, rx);
        notifies.insert(i, Arc::new(Notify::new()));
    }

    let mut engines = Vec::new();
    for i in 0..4u8 {
        let mut outboxes = HashMap::new();
        for (peer, tx) in &senders {
            if *peer != i {
                outboxes.insert(*peer, tx.clone());
            }
        }
        let peer_notifies = notifies
            .iter()
            .filter(|(id, _)| **id != i)
            .map(|(id, n)| (*id, n.clone()))
            .collect();
        let cfg = ValidatorConfig {
            id: i,
            shard_id: shard.clone(),
            key: Keypair::from_bytes(&secrets[i as usize]),
            peer_keys: peer_map
                .iter()
                .filter(|(id, _)| **id != i)
                .map(|(id, vk)| (*id, *vk))
                .collect(),
            issuer_vk,
        };
        let rx = receivers.remove(&i).expect("rx");
        let wal_path = dir.path().join(format!("v{i}.wal"));
        let eng = open_engine(
            cfg,
            wal_path,
            rx,
            outboxes,
            peer_notifies,
            notifies[&i].clone(),
        )
        .expect("open engine");
        engines.push(Arc::new(eng));
    }

    // Pump followers only. The leader drains its own inbox inside handle_pay/fence_epoch.
    // A leader background pump would drop Vote/DurableAck messages before the leader loop sees them.
    let mut pumps = Vec::new();
    for eng in engines.iter().skip(1).cloned() {
        pumps.push(tokio::spawn(async move {
            loop {
                let _ = eng.wait_and_pump().await;
            }
        }));
    }

    Cluster4 {
        engines,
        issuer_vk,
        _dir: dir,
        pumps,
        leader_idx: 0,
    }
}

pub async fn cluster4(shard: ShardId) -> Cluster4 {
    let issuer = Keypair::generate();
    cluster4_with_issuer_bytes(shard, issuer.signing_key().to_bytes()).await
}
