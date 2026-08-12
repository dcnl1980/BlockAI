use blockai_execute::GlobalState;
use blockai_types::{
    AccountId, AmountMicros, AssetId, L1Tx, OrderId, OrderStatus, Side,
};

fn fund(state: &mut GlobalState, account: AccountId, amount: u128) {
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(amount),
        })
        .unwrap();
}

fn setup_acme(state: &mut GlobalState) -> (AccountId, AccountId, AssetId) {
    let issuer = AccountId([1u8; 32]);
    let buyer = AccountId([2u8; 32]);
    let asset_id = AssetId([0xACu8; 32]);
    fund(state, issuer, 0);
    fund(state, buyer, 1_000_000);
    state
        .apply(&L1Tx::RegisterAsset {
            asset_id,
            issuer,
            symbol: "ACME".into(),
            name: "Acme".into(),
            decimals: 0,
            max_supply: 1_000,
        })
        .unwrap();
    state
        .apply(&L1Tx::MintAsset {
            asset_id,
            issuer,
            to: issuer,
            units: 100,
        })
        .unwrap();
    (issuer, buyer, asset_id)
}

#[test]
fn limit_orders_match_with_partial_fill_and_history() {
    let mut state = GlobalState::new(2);
    let (seller, buyer, asset_id) = setup_acme(&mut state);

    let sell_id = OrderId([0x51u8; 32]);
    let buy_id = OrderId([0x42u8; 32]);

    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: sell_id,
            asset_id,
            trader: seller,
            side: Side::Sell,
            price: AmountMicros(1_000),
            units: 30,
        })
        .unwrap();
    assert_eq!(state.holding(seller, asset_id), 70);

    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: buy_id,
            asset_id,
            trader: buyer,
            side: Side::Buy,
            price: AmountMicros(1_200),
            units: 10,
        })
        .unwrap();

    assert_eq!(state.fills.len(), 1);
    assert_eq!(state.fills[0].units, 10);
    assert_eq!(state.fills[0].price, AmountMicros(1_000)); // maker (sell) price
    assert_eq!(state.holding(buyer, asset_id), 10);
    assert_eq!(state.orders[&sell_id].status, OrderStatus::Partial);
    assert_eq!(state.orders[&sell_id].remaining, 20);
    assert_eq!(state.orders[&buy_id].status, OrderStatus::Filled);
    // buyer locked 1200*10=12000, paid 1000*10=10000, refunded 2000
    assert_eq!(state.accounts[&buyer].balance_available.0, 1_000_000 - 10_000);
    assert_eq!(state.accounts[&seller].balance_available.0, 10_000);
    state.check_conservation().unwrap();
}

#[test]
fn cancel_resting_sell_returns_units() {
    let mut state = GlobalState::new(2);
    let (seller, _buyer, asset_id) = setup_acme(&mut state);
    let sell_id = OrderId([0x99u8; 32]);
    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: sell_id,
            asset_id,
            trader: seller,
            side: Side::Sell,
            price: AmountMicros(5_000),
            units: 40,
        })
        .unwrap();
    assert_eq!(state.holding(seller, asset_id), 60);
    state
        .apply(&L1Tx::CancelOrder {
            order_id: sell_id,
            trader: seller,
        })
        .unwrap();
    assert_eq!(state.holding(seller, asset_id), 100);
    assert_eq!(state.orders[&sell_id].status, OrderStatus::Cancelled);
    state.check_conservation().unwrap();
}

#[test]
fn non_crossing_orders_rest_on_book() {
    let mut state = GlobalState::new(2);
    let (seller, buyer, asset_id) = setup_acme(&mut state);
    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: OrderId([1u8; 32]),
            asset_id,
            trader: seller,
            side: Side::Sell,
            price: AmountMicros(2_000),
            units: 5,
        })
        .unwrap();
    state
        .apply(&L1Tx::PlaceLimitOrder {
            order_id: OrderId([2u8; 32]),
            asset_id,
            trader: buyer,
            side: Side::Buy,
            price: AmountMicros(1_000),
            units: 5,
        })
        .unwrap();
    assert!(state.fills.is_empty());
    assert_eq!(state.orders[&OrderId([1u8; 32])].status, OrderStatus::Open);
    assert_eq!(state.orders[&OrderId([2u8; 32])].status, OrderStatus::Open);
}
