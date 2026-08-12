//! Plan 13 ops surface: economics fees/rewards + stake-weighted governance.

use blockai_attest::{Attestor, HardwareAttestor, TestPlatform};
use blockai_dataplane::{select_backend, DataplaneBackend};
use blockai_execute::GlobalState;
use blockai_hsm::SoftHsm3of5;
use blockai_types::{
    AccountId, AmountMicros, GovernanceAction, L1Tx, ProposalStatus,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "econ_gov_sim")]
struct Args {
    #[arg(long, default_value_t = 5)]
    fees: u64,
    #[arg(long, default_value_t = 50)]
    new_min_stake: u128,
}

fn main() {
    let args = Args::parse();
    let mut state = GlobalState::new(2);
    let treasury_payer = AccountId([1u8; 32]);
    let validator = AccountId([2u8; 32]);
    let voter = AccountId([3u8; 32]);

    state
        .apply(&L1Tx::GenesisFund {
            account: treasury_payer,
            amount: AmountMicros(10_000),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: validator,
            amount: AmountMicros(100),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: voter,
            amount: AmountMicros(1_000),
        })
        .unwrap();
    state
        .apply(&L1Tx::Stake {
            account: voter,
            amount: AmountMicros(200),
        })
        .unwrap();

    for _ in 0..args.fees {
        state
            .apply(&L1Tx::ChargeBaseFee {
                payer: treasury_payer,
            })
            .unwrap();
    }
    let treasury = state.fee_treasury.0;
    state
        .apply(&L1Tx::DistributeRewards {
            recipients: vec![validator],
            total: AmountMicros(treasury),
        })
        .unwrap();

    let id = *blake3::hash(b"gov-set-min-stake").as_bytes();
    state
        .apply(&L1Tx::ProposeGovernance {
            id,
            proposer: treasury_payer,
            action: GovernanceAction::SetMinStake {
                value: AmountMicros(args.new_min_stake),
            },
        })
        .unwrap();
    state
        .apply(&L1Tx::VoteGovernance {
            id,
            voter,
            approve: true,
        })
        .unwrap();
    state.apply(&L1Tx::FinalizeGovernance { id }).unwrap();
    assert_eq!(state.economics.min_stake.0, args.new_min_stake);
    assert_eq!(state.proposals[&id].status, ProposalStatus::Executed);
    state.check_conservation().unwrap();

    // Production seams smoke.
    let backend = select_backend(DataplaneBackend::AfXdp);
    assert_eq!(backend, DataplaneBackend::Userspace);
    let hsm = SoftHsm3of5::generate();
    let ceremony = hsm.export_ceremony(1);
    SoftHsm3of5::verify_ceremony_transcript(&ceremony).unwrap();
    let hw = HardwareAttestor::with_measurement(TestPlatform::new());
    let ev = hw.collect().unwrap();
    assert_eq!(ev.pcrs.len(), 2);

    println!(
        "econ_gov_sim OK fees={} treasury_paid={} min_stake={} backend={:?} hsm_shares={} pcrs={}",
        args.fees,
        treasury,
        state.economics.min_stake.0,
        backend,
        ceremony.share_pubkeys.len(),
        ev.pcrs.len()
    );
}
