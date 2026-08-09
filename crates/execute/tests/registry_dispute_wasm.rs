use blockai_execute::GlobalState;
use blockai_types::{AccountId, AgentId, AmountMicros, L1Tx};
use blockai_wasm::code_hash;

const ADD_WAT: &str = r#"
(module
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add))
"#;

#[test]
fn register_reputation_and_dispute_flow() {
    let mut state = GlobalState::new(2);
    let plaintiff = AccountId([1u8; 32]);
    let defendant = AccountId([2u8; 32]);
    let agent = AgentId([9u8; 32]);

    state
        .apply(&L1Tx::GenesisFund {
            account: plaintiff,
            amount: AmountMicros(100),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: defendant,
            amount: AmountMicros(50),
        })
        .unwrap();
    state
        .apply(&L1Tx::RegisterAgent {
            account: plaintiff,
            agent_id: agent,
            metadata_hash: [3u8; 32],
        })
        .unwrap();
    state
        .apply(&L1Tx::UpdateReputation {
            agent_id: agent,
            delta: 5,
            reason_hash: [4u8; 32],
        })
        .unwrap();
    assert_eq!(state.agents[&agent].reputation, 5);

    let dispute_id = [7u8; 32];
    state
        .apply(&L1Tx::OpenDispute {
            id: dispute_id,
            plaintiff,
            defendant,
            bond: AmountMicros(10),
            evidence_hash: [8u8; 32],
        })
        .unwrap();
    assert_eq!(state.accounts[&plaintiff].balance_available, AmountMicros(90));
    state
        .apply(&L1Tx::ResolveDispute {
            id: dispute_id,
            for_plaintiff: true,
        })
        .unwrap();
    assert_eq!(state.accounts[&plaintiff].balance_available, AmountMicros(100));
    assert!(state.agents[&agent].reputation >= 6);
    state.check_conservation().unwrap();
}

#[test]
fn deploy_and_call_wasm_contract() {
    let mut state = GlobalState::new(2);
    let deployer = AccountId([1u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account: deployer,
            amount: AmountMicros(10),
        })
        .unwrap();
    let code = ADD_WAT.as_bytes().to_vec();
    let hash = code_hash(&code);
    state
        .apply(&L1Tx::DeployContract {
            deployer,
            code_hash: hash,
            code,
        })
        .unwrap();
    state
        .apply(&L1Tx::CallContract {
            caller: deployer,
            code_hash: hash,
            export: "add".into(),
            args: (20, 22),
            fuel: 50_000,
        })
        .unwrap();
    assert_eq!(state.last_call_result, Some(42));
    state.check_conservation().unwrap();
}

#[test]
fn suspend_agent_blocks_stake() {
    let mut state = GlobalState::new(2);
    let account = AccountId([1u8; 32]);
    let agent = AgentId([9u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(20),
        })
        .unwrap();
    state
        .apply(&L1Tx::RegisterAgent {
            account,
            agent_id: agent,
            metadata_hash: [1u8; 32],
        })
        .unwrap();
    state
        .apply(&L1Tx::SuspendAgent { agent_id: agent })
        .unwrap();
    let err = state
        .apply(&L1Tx::Stake {
            account,
            amount: AmountMicros(5),
        })
        .unwrap_err();
    assert!(matches!(err, blockai_execute::ExecuteError::Suspended));
}
