use blockai_types::{encode_cbor, L1Tx};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DagBlock {
    pub author: u8,
    pub round: u64,
    pub parents: Vec<[u8; 32]>,
    pub txs: Vec<L1Tx>,
}

impl DagBlock {
    pub fn digest(&self) -> [u8; 32] {
        let bytes = encode_cbor(self).expect("dag block encodes");
        *blake3::hash(&bytes).as_bytes()
    }
}

#[derive(Default)]
pub struct DagMempool {
    pub blocks: HashMap<[u8; 32], DagBlock>,
    pub tips: Vec<[u8; 32]>,
    next_round: u64,
}

impl DagMempool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, block: DagBlock) -> [u8; 32] {
        let d = block.digest();
        self.blocks.insert(d, block);
        self.tips.push(d);
        if self.tips.len() > 16 {
            self.tips.drain(0..self.tips.len() - 16);
        }
        d
    }

    pub fn propose(&mut self, author: u8, txs: Vec<L1Tx>) -> DagBlock {
        let parents = self.tips.clone();
        let round = self.next_round;
        self.next_round += 1;
        let block = DagBlock {
            author,
            round,
            parents,
            txs,
        };
        self.insert(block.clone());
        block
    }

    pub fn ordered_txs_from_anchor(&self, anchor: &[u8; 32]) -> Vec<L1Tx> {
        let mut out = Vec::new();
        let mut stack = vec![*anchor];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            if let Some(block) = self.blocks.get(&id) {
                for p in block.parents.iter().rev() {
                    stack.push(*p);
                }
                out.extend(block.txs.clone());
            }
        }
        out
    }
}
