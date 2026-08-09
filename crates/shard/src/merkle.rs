#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerkleProof {
    pub index: usize,
    pub siblings: Vec<[u8; 32]>,
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return *blake3::hash(b"empty-merkle").as_bytes();
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            let last = *level.last().unwrap();
            level.push(last);
        }
        let mut next = Vec::with_capacity(level.len() / 2);
        for chunk in level.chunks(2) {
            next.push(hash_pair(&chunk[0], &chunk[1]));
        }
        level = next;
    }
    level[0]
}

pub fn merkle_proof(leaves: &[[u8; 32]], index: usize) -> Option<MerkleProof> {
    if leaves.is_empty() || index >= leaves.len() {
        return None;
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    let mut idx = index;
    let mut siblings = Vec::new();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            let last = *level.last().unwrap();
            level.push(last);
        }
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        siblings.push(level[sibling_idx]);
        let mut next = Vec::with_capacity(level.len() / 2);
        for chunk in level.chunks(2) {
            next.push(hash_pair(&chunk[0], &chunk[1]));
        }
        level = next;
        idx /= 2;
    }
    Some(MerkleProof { index, siblings })
}

pub fn verify_merkle_proof(leaf: [u8; 32], proof: &MerkleProof, root: [u8; 32]) -> bool {
    let mut hash = leaf;
    let mut idx = proof.index;
    for sibling in &proof.siblings {
        if idx % 2 == 0 {
            hash = hash_pair(&hash, sibling);
        } else {
            hash = hash_pair(sibling, &hash);
        }
        idx /= 2;
    }
    hash == root
}
