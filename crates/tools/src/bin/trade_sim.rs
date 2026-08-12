//! Demo: tokenize an instrument and trade via limit order book (partial fills supported).
//! Not a regulated securities exchange.

use blockai_execute::GlobalState;
use blockai_types::{AccountId, AmountMicros, AssetId, L1Tx, OrderId, Side};
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
    /// Limit price per unit (EURC micros) for the ask; bid crosses at +20%.
    #[arg(long, default_value_t = 2_500)]
    price: u128,
}

fn main() {
    let args = Args::parse();
    let mut state = GlobalState::new(2);
    let issuer = AccountId([1u8; 32]);
    let buyer = AccountId([2u8; 32]);
    let asset_id = AssetId(*blake3::hash(args.symbol.as_bytes()).as_bytes());
    let bid_price = args.price.saturating_mul(12) / 10; // 20% above ask → crosses
    let buyer_cash = bid_price.saturating_mul(args.units).saturating_mul(2);

    state
        .apply(&L1Tx::GenesisFund {
            account: issuer,
            amount: AmountMicros(0),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: buyer,
            amount: AmountMicros(buyer_cash),
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

    let ask_id = OrderId(*blake3::hash(b"ask-1").as_bytes());
    let bid_id = OrderId(*blake3::hash(b"bid-1").as_bytes());

    // Seller rests an ask for more units than the bid → partial fill on the ask.
    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: ask_id,
            asset_id,
            trader: issuer,
            side: Side::Sell,
            price: AmountMicros(args.price),
            units: args.units.saturating_mul(3).max(args.units),
        })
        .unwrap();
    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: bid_id,
            asset_id,
            trader: buyer,
            side: Side::Buy,
            price: AmountMicros(bid_price),
            units: args.units,
        })
        .unwrap();

    state.check_conservation().unwrap();
    let sym = state.assets[&asset_id].symbol.clone();
    let fill = state.fills.last().expect("expected a fill");
    println!(
        "ok book symbol={} fills={} last_fill_units={} last_fill_price={} buyer_units={} seller_units={} ask_remaining={} buyer_eurc={} seller_eurc={}",
        sym,
        state.fills.len(),
        fill.units,
        fill.price.0,
        state.holding(buyer, asset_id),
        state.holding(issuer, asset_id),
        state.orders[&ask_id].remaining,
        state.accounts[&buyer].balance_available.0,
        state.accounts[&issuer].balance_available.0
    );
}
