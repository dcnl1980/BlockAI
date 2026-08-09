use blockai_shard::{merkle_proof, merkle_root, verify_merkle_proof};

#[test]
fn merkle_inclusion_proof_verifies() {
    let leaves = [[1u8; 32], [2u8; 32], [3u8; 32]];
    let root = merkle_root(&leaves);
    let proof = merkle_proof(&leaves, 1).unwrap();
    assert!(verify_merkle_proof(leaves[1], &proof, root));
    assert!(!verify_merkle_proof(leaves[0], &proof, root));
}
