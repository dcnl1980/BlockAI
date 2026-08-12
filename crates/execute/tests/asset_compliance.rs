use blockai_execute::{ExecuteError, GlobalState};
use blockai_types::{AccountId, AmountMicros, AssetId, L1Tx, OrderId, Side};

fn setup() -> (GlobalState, AccountId, AccountId, AssetId) {
    let mut state = GlobalState::new(2);
    let issuer = AccountId([1u8; 32]);
    let alice = AccountId([2u8; 32]);
    let asset_id = AssetId([0xCCu8; 32]);
    state
        .apply(&L1Tx::GenesisFund {
            account: issuer,
            amount: AmountMicros(0),
        })
        .unwrap();
    state
        .apply(&L1Tx::GenesisFund {
            account: alice,
            amount: AmountMicros(100_000),
        })
        .unwrap();
    state
        .apply(&L1Tx::RegisterAsset {
            asset_id,
            issuer,
            symbol: "COMP".into(),
            name: "Compliance Co".into(),
            decimals: 0,
            max_supply: 100,
        })
        .unwrap();
    state
        .apply(&L1Tx::MintAsset {
            asset_id,
            issuer,
            to: issuer,
            units: 50,
        })
        .unwrap();
    (state, issuer, alice, asset_id)
}

#[test]
fn frozen_asset_blocks_transfer_and_orders() {
    let (mut state, issuer, alice, asset_id) = setup();
    state
        .apply(&L1Tx::SetAssetFrozen {
            asset_id,
            issuer,
            frozen: true,
        })
        .unwrap();
    assert_eq!(
        state
            .apply(&L1Tx::TransferAsset {
                asset_id,
                from: issuer,
                to: alice,
                units: 1,
            })
            .unwrap_err(),
        ExecuteError::AssetFrozen
    );
    assert_eq!(
        state
            .apply(&L1Tx::PlaceLimitOrder {
                order_id: OrderId([9u8; 32]),
                asset_id,
                trader: issuer,
                side: Side::Sell,
                price: AmountMicros(1),
                units: 1,
            })
            .unwrap_err(),
        ExecuteError::AssetFrozen
    );
}

#[test]
fn allowlist_blocks_non_members() {
    let (mut state, issuer, alice, asset_id) = setup();
    state
        .apply(&L1Tx::SetAssetAllowlistEnabled {
            asset_id,
            issuer,
            enabled: true,
        })
        .unwrap();
    // Issuer not yet on list → cannot transfer to alice either; mint already done before enable.
    assert_eq!(
        state
            .apply(&L1Tx::TransferAsset {
                asset_id,
                from: issuer,
                to: alice,
                units: 1,
            })
            .unwrap_err(),
        ExecuteError::NotAllowlisted
    );
    state
        .apply(&L1Tx::SetAssetAllowlistMember {
            asset_id,
            issuer,
            account: issuer,
            allowed: true,
        })
        .unwrap();
    state
        .apply(&L1Tx::SetAssetAllowlistMember {
            asset_id,
            issuer,
            account: alice,
            allowed: true,
        })
        .unwrap();
    state
        .apply(&L1Tx::TransferAsset {
            asset_id,
            from: issuer,
            to: alice,
            units: 5,
        })
        .unwrap();
    assert_eq!(state.holding(alice, asset_id), 5);
}
