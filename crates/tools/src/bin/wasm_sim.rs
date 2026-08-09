use blockai_execute::GlobalState;
use blockai_types::{AccountId, AgentId, AmountMicros, L1Tx};
use blockai_wasm::code_hash;
use clap::Parser;

const ADD_WAT: &str = r#"
(module
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add))
"#;

#[derive(Parser, Debug)]
#[command(name = "wasm_sim")]
struct Args {
    #[arg(long, default_value_t = 2)]
    a: i32,
    #[arg(long, default_value_t = 40)]
    b: i32,
}

fn main() {
    let args = Args::parse();
    let mut state = GlobalState::new(2);
    let account = AccountId([1u8; 32]);
    let agent = AgentId([9u8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(100),
        })
        .unwrap();
    state
        .apply(&L1Tx::RegisterAgent {
            account,
            agent_id: agent,
            metadata_hash: [3u8; 32],
        })
        .unwrap();
    let code = ADD_WAT.as_bytes().to_vec();
    let hash = code_hash(&code);
    state
        .apply(&L1Tx::DeployContract {
            deployer: account,
            code_hash: hash,
            code,
        })
        .unwrap();
    state
        .apply(&L1Tx::CallContract {
            caller: account,
            code_hash: hash,
            export: "add".into(),
            args: (args.a, args.b),
            fuel: 50_000,
        })
        .unwrap();
    println!(
        "ok result={} reputation={} contracts={}",
        state.last_call_result.unwrap(),
        state.agents[&agent].reputation,
        state.contracts.len()
    );
}
