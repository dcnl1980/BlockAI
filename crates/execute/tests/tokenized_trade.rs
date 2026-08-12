use blockai_execute::{ExecuteError, GlobalState};
use blockai_types::{AccountId, AmountMicros, AssetId, L1Tx};

fn fund(state: &mut GlobalState, account: AccountId, amount: u128) {
    state
        .apply(&L1Tx::GenesisFund {
            account,
            amount: AmountMicros(amount),
        })
        .unwrap();
}

#[test]
fn register_mint_transfer_and_spot_trade() {
    let mut state = GlobalState::new(2);
    let issuer = AccountId([1u8; 32]);
    let buyer = AccountId([2u8; 32]);
    let asset_id = AssetId([0xACu8; 32]);

    fund(&mut state, issuer, 0);
    fund(&mut state, buyer, 50_000);

    state
        .apply(&L1Tx::RegisterAsset {
            asset_id,
            issuer,
            symbol: "acme".into(),
            name: "Acme Robotics".into(),
            decimals: 0,
            max_supply: 1_000,
        })
        .unwrap();
    assert_eq!(state.assets[&asset_id].symbol, "ACME");

    state
        .apply(&L1Tx::MintAsset {
            asset_id,
            issuer,
            to: issuer,
            units: 100,
        })
        .unwrap();
    assert_eq!(state.holding(issuer, asset_id), 100);

    state
        .apply(&L1Tx::SpotTrade {
            asset_id,
            buyer,
            seller: issuer,
            asset_units: 10,
            price_total: AmountMicros(25_000),
        })
        .unwrap();

    assert_eq!(state.holding(buyer, asset_id), 10);
    assert_eq!(state.holding(issuer, asset_id), 90);
    assert_eq!(state.accounts[&buyer].balance_available.0, 25_000);
    assert_eq!(state.accounts[&issuer].balance_available.0, 25_000);
    state.check_conservation().unwrap();
}

#[test]
fn mint_beyond_cap_and_bad_issuer_fail_closed() {
    let mut state = GlobalState::new(2);
    let issuer = AccountId([1u8; 32]);
    let other = AccountId([9u8; 32]);
    let asset_id = AssetId([0x11u8; 32]);
    fund(&mut state, issuer, 0);
    fund(&mut state, other, 0);
    state
        .apply(&L1Tx::RegisterAsset {
            asset_id,
            issuer,
            symbol: "BETA".into(),
            name: "Beta".into(),
            decimals: 0,
            max_supply: 5,
        })
        .unwrap();
    assert_eq!(
        state
            .apply(&L1Tx::MintAsset {
                asset_id,
                issuer: other,
                to: other,
                units: 1,
            })
            .unwrap_err(),
        ExecuteError::NotAssetIssuer
    );
    state
        .apply(&L1Tx::MintAsset {
            asset_id,
            issuer,
            to: issuer,
            units: 5,
        })
        .unwrap();
    assert_eq!(
        state
            .apply(&L1Tx::MintAsset {
                asset_id,
                issuer,
                to: issuer,
                units: 1,
            })
            .unwrap_err(),
        ExecuteError::ExceedsMaxSupply
    );
}

#[test]
fn spot_trade_insufficient_cash_or_units_fails() {
    let mut state = GlobalState::new(2);
    let seller = AccountId([1u8; 32]);
    let buyer = AccountId([2u8; 32]);
    let asset_id = AssetId([0x22u8; 32]);
    fund(&mut state, seller, 0);
    fund(&mut state, buyer, 100);
    state
        .apply(&L1Tx::RegisterAsset {
            asset_id,
            issuer: seller,
            symbol: "GAMMA".into(),
            name: "Gamma".into(),
            decimals: 0,
            max_supply: 10,
        })
        .unwrap();
    state
        .apply(&L1Tx::MintAsset {
            asset_id,
            issuer: seller,
            to: seller,
            units: 2,
        })
        .unwrap();
    assert_eq!(
        state
            .apply(&L1Tx::SpotTrade {
                asset_id,
                buyer,
                seller,
                asset_units: 1,
                price_total: AmountMicros(1_000),
            })
            .unwrap_err(),
        ExecuteError::InsufficientAvailable
    );
    fund(&mut state, buyer, 10_000);
    assert_eq!(
        state
            .apply(&L1Tx::SpotTrade {
                asset_id,
                buyer,
                seller,
                asset_units: 5,
                price_total: AmountMicros(1),
            })
            .unwrap_err(),
        ExecuteError::InsufficientAssetUnits
    );
}
