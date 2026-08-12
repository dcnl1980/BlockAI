//! Demo: register a tokenized instrument, mint shares, execute an atomic EURC spot trade.
//! Not a regulated securities exchange.

use blockai_execute::GlobalState;
use blockai_types::{AccountId, AmountMicros, AssetId, L1Tx};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "trade_sim")]
struct Args {
    #[arg(long, default_value = "ACME")]
    symbol: String,
    #[arg(long, default_value_t = 100)]
    mint: u128,
    #[arg(long, default_value_t = 10)]
    units: u128,
    #[arg(long, default_value_t = 25_000)]
    price: u128,
}

fn main() {
    let args = Args::parse();
    let mut state = GlobalState::new(2);
    let issuer = AccountId([1u8; 32]);
    let buyer = AccountId([2u8; 32]);
    let asset_id = AssetId(*blake3::hash(args.symbol.as_bytes()).as_bytes());

    state
        .apply(&L1Tx::GenesisFund {
            account: issuer,
            amount: AmountMicros(0),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: buyer,
            amount: AmountMicros(args.price.saturating_mul(2)),
        })
        .unwrap();
    state
        .apply(&L1Tx::RegisterAsset {
            asset_id,
            issuer,
            symbol: args.symbol.clone(),
            name: format!("{} Tokenized Shares", args.symbol.to_ascii_uppercase()),
            decimals: 0,
            max_supply: args.mint.saturating_mul(10).max(args.mint),
        })
        .unwrap();
    state
        .apply(&L1Tx::MintAsset {
            asset_id,
            issuer,
            to: issuer,
            units: args.mint,
        })
        .unwrap();
    state
        .apply(&L1Tx::SpotTrade {
            asset_id,
            buyer,
            seller: issuer,
            asset_units: args.units,
            price_total: AmountMicros(args.price),
        })
        .unwrap();
    state.check_conservation().unwrap();

    let sym = state.assets[&asset_id].symbol.clone();
    println!(
        "ok trade symbol={} buyer_units={} seller_units={} buyer_eurc={} seller_eurc={} minted={}",
        sym,
        state.holding(buyer, asset_id),
        state.holding(issuer, asset_id),
        state.accounts[&buyer].balance_available.0,
        state.accounts[&issuer].balance_available.0,
        state.assets[&asset_id].minted
    );
}
