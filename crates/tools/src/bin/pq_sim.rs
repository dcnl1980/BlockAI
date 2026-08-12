use blockai_crypto::{
    seal_pay_hybrid, verify_capability_hybrid, verify_pay_hybrid, AlgorithmId, Keypair, PqKeypair,
};
use blockai_hsm::{RootOp, SoftHsm3of5, HSM_QUORUM};
use blockai_shard::CheckpointSealer;
use blockai_types::{
    AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence, ShardId,
};
use blockai_witness::Witness;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "pq_sim")]
struct Args {
    /// Micros for the hybrid PAY
    #[arg(long, default_value_t = 10)]
    amount: u128,
}

fn main() {
    let args = Args::parse();

    let agent = Keypair::generate();
    let agent_pq = PqKeypair::generate();
    let mut pay = Pay {
        capability_id: CapabilityId([9u8; 32]),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId(agent.verifying_key_bytes()),
        service_id: "inference/pq".into(),
        amount: AmountMicros(args.amount),
        currency: "EURC".into(),
        request_hash: [1u8; 32],
        price_quote_hash: [2u8; 32],
        max_amount: AmountMicros(args.amount),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![],
        ..Default::default()
    };
    seal_pay_hybrid(&agent, &agent_pq, &mut pay).expect("seal pay");
    verify_pay_hybrid(&pay).expect("verify pay");
    assert_eq!(
        AlgorithmId::from_u16(pay.agent_alg),
        Some(AlgorithmId::HybridEd25519MlDsa65)
    );

    // Capability hybrid still works (authority default path).
    let issuer = Keypair::generate();
    let issuer_pq = PqKeypair::generate();
    let mut cap = blockai_types::SpendCapability {
        capability_id: CapabilityId([1u8; 32]),
        account_id: blockai_types::AccountId([2u8; 32]),
        agent_id: AgentId(agent.verifying_key_bytes()),
        shard_id: ShardId::new("FRA-004").unwrap(),
        epoch: Epoch(1),
        currency: "EURC".into(),
        maximum_total: AmountMicros(100),
        maximum_per_call: AmountMicros(args.amount),
        service_scope: vec!["inference/*".into()],
        policy_hash: [3u8; 32],
        sequence_start: Sequence(1),
        sequence_end: Sequence(10),
        valid_from_unix_ms: 0,
        valid_until_unix_ms: 9_999_999_999,
        region: "EU".into(),
        issuer_alg: AlgorithmId::Ed25519.as_u16(),
        issuer_pubkey: [0u8; 32],
        issuer_signature: vec![],
        issuer_pq_pubkey: vec![],
        issuer_pq_signature: vec![],
    };
    blockai_crypto::seal_capability_hybrid(&issuer, &issuer_pq, &mut cap).expect("seal cap");
    verify_capability_hybrid(&cap).expect("verify cap");

    // Hybrid checkpoint + witness.
    let shard_kp = Keypair::generate();
    let shard_pq = PqKeypair::generate();
    let mut log = blockai_shard::ReceiptLog::default();
    // Minimal leaf via empty proof path isn't available; seal needs non-empty log.
    // Use force_seal_with_pq after appending a dummy proof through payment helpers is heavy —
    // instead construct header seal via CheckpointSealer with a real receipt leaf from types.
    let proof = blockai_types::PaymentProof {
        agent: blockai_types::AgentAuthorization {
            pay_cbor_hash: [4u8; 32],
            agent_signature: pay.agent_signature.clone(),
        },
        edge: blockai_types::EdgeAcceptance {
            agent_auth_hash: [5u8; 32],
            commit_index: 1,
            tx_id: [6u8; 32],
            edge_pubkey: [7u8; 32],
            edge_signature: vec![0u8; 64],
            edge_pq_pubkey: vec![],
            edge_pq_signature: vec![],
        },
        service: blockai_types::ServiceReceipt {
            edge_accept_hash: [8u8; 32],
            execution_hash: [9u8; 32],
            actual_amount: AmountMicros(args.amount),
            service_pubkey: [10u8; 32],
            service_signature: vec![0u8; 64],
            service_pq_pubkey: vec![],
            service_pq_signature: vec![],
        },
    };
    log.append(proof).expect("append");
    let mut sealer = CheckpointSealer::new(1, AmountMicros(1_000_000));
    let checkpoint = sealer
        .force_seal_with_pq(
            &mut log,
            &shard_kp,
            Some(&shard_pq),
            ShardId::new("FRA-004").unwrap(),
            Epoch(1),
            1_700_000_000_000,
        )
        .expect("seal checkpoint");
    assert!(!checkpoint.shard_pq_signature.is_empty());
    blockai_shard::verify_signed_checkpoint(&checkpoint).expect("verify checkpoint");

    let witness = Witness::new_hybrid(Keypair::generate(), PqKeypair::generate());
    let wsig = witness.countersign(&checkpoint).expect("witness");
    assert!(!wsig.witness_pq_signature.is_empty());
    blockai_witness::verify_witness_sig(&checkpoint, &wsig).expect("verify witness");

    // Hybrid HSM root.
    let hsm = SoftHsm3of5::generate_hybrid();
    let op = RootOp::AuthorizeIssuer {
        issuer_pubkey: issuer.verifying_key_bytes(),
    };
    let thresh = hsm.sign_with(&op, &[0, 2, 4]).expect("hsm sign");
    hsm.verify(&thresh, HSM_QUORUM).expect("hsm verify");
    assert!(thresh.shares.iter().all(|s| !s.pq_signature.is_empty()));

    println!("pq_sim OK hybrid_pay+cap+checkpoint+witness+hsm amount_micros={}", args.amount);
}
