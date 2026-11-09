#![cfg(test)]

use crate::{error::VaultError, types::LockRecord, UmbraVaultContract, UmbraVaultContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let token_address = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    (
        TokenClient::new(env, &token_address),
        StellarAssetClient::new(env, &token_address),
    )
}

fn setup_test<'a>() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TokenClient<'a>,
    StellarAssetClient<'a>,
    UmbraVaultContractClient<'a>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let escrow_contract = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (token_client, token_admin) = create_token(&env, &admin);
    token_admin.mint(&seller, &100_000);

    let vault_id = env.register(UmbraVaultContract, ());
    let vault_client = UmbraVaultContractClient::new(&env, &vault_id);
    vault_client.initialize(&admin);
    vault_client.set_authorized(&escrow_contract, &true);

    (
        env,
        admin,
        escrow_contract,
        seller,
        buyer,
        token_client,
        token_admin,
        vault_client,
    )
}

#[test]
fn test_initialize_and_authorization() {
    let (env, _admin, escrow_contract, _seller, _buyer, _, _, vault_client) = setup_test();

    assert!(vault_client.is_authorized(&escrow_contract));

    let other = Address::generate(&env);
    assert!(!vault_client.is_authorized(&other));

    vault_client.set_authorized(&other, &true);
    assert!(vault_client.is_authorized(&other));

    vault_client.set_authorized(&other, &false);
    assert!(!vault_client.is_authorized(&other));
}

#[test]
fn test_deposit_and_withdraw_unencumbered() {
    let (_env, _admin, _escrow, seller, _buyer, token_client, _, vault_client) = setup_test();

    assert_eq!(token_client.balance(&seller), 100_000);

    vault_client.deposit(&seller, &token_client.address, &40_000);
    assert_eq!(token_client.balance(&seller), 60_000);

    let summary = vault_client.get_summary(&seller, &token_client.address);
    assert_eq!(summary.total_collateral, 40_000);
    assert_eq!(summary.locked_collateral, 0);
    assert_eq!(summary.available_collateral, 40_000);

    vault_client.withdraw(&seller, &token_client.address, &15_000);
    assert_eq!(token_client.balance(&seller), 75_000);

    let summary_after = vault_client.get_summary(&seller, &token_client.address);
    assert_eq!(summary_after.total_collateral, 25_000);
    assert_eq!(summary_after.available_collateral, 25_000);
}

#[test]
fn test_withdraw_insufficient_collateral_fails() {
    let (_env, _admin, _escrow, seller, _buyer, token_client, _, vault_client) = setup_test();

    vault_client.deposit(&seller, &token_client.address, &20_000);

    let err = vault_client.try_withdraw(&seller, &token_client.address, &25_000);
    assert_eq!(err, Err(Ok(VaultError::InsufficientCollateral)));
}

#[test]
fn test_lock_collateral_and_query() {
    let (env, _admin, escrow, seller, _buyer, token_client, _, vault_client) = setup_test();

    vault_client.deposit(&seller, &token_client.address, &50_000);

    let expires_at = env.ledger().timestamp() + 3600;
    vault_client.lock_collateral(
        &escrow,
        &seller,
        &token_client.address,
        &20_000,
        &1,
        &expires_at,
    );

    let summary = vault_client.get_summary(&seller, &token_client.address);
    assert_eq!(summary.total_collateral, 50_000);
    assert_eq!(summary.locked_collateral, 20_000);
    assert_eq!(summary.available_collateral, 30_000);

    let lock = vault_client.get_lock(&seller, &1);
    assert_eq!(
        lock,
        Some(LockRecord {
            escrow_id: 1,
            asset: token_client.address.clone(),
            amount: 20_000,
            expires_at,
        })
    );

    // Cannot withdraw locked collateral
    let err = vault_client.try_withdraw(&seller, &token_client.address, &35_000);
    assert_eq!(err, Err(Ok(VaultError::InsufficientCollateral)));

    // Can withdraw available amount
    vault_client.withdraw(&seller, &token_client.address, &30_000);
    assert_eq!(token_client.balance(&seller), 80_000);
}

#[test]
fn test_release_lock_lifecycle() {
    let (env, _admin, escrow, seller, _buyer, token_client, _, vault_client) = setup_test();

    vault_client.deposit(&seller, &token_client.address, &50_000);

    let expires_at = env.ledger().timestamp() + 1000;
    vault_client.lock_collateral(
        &escrow,
        &seller,
        &token_client.address,
        &25_000,
        &1,
        &expires_at,
    );

    // Seller cannot unlock before expiry
    let err = vault_client.try_release_lock(&seller, &seller, &1);
    assert_eq!(err, Err(Ok(VaultError::LockedLiabilityExists)));

    // Advance time past expiry
    env.ledger().set_timestamp(expires_at + 1);

    // Now seller can unlock
    vault_client.release_lock(&seller, &seller, &1);

    let summary = vault_client.get_summary(&seller, &token_client.address);
    assert_eq!(summary.locked_collateral, 0);
    assert_eq!(summary.available_collateral, 50_000);
    assert_eq!(vault_client.get_lock(&seller, &1), None);
}

#[test]
fn test_slash_and_refund_buyer() {
    let (env, _admin, escrow, seller, buyer, token_client, _, vault_client) = setup_test();

    vault_client.deposit(&seller, &token_client.address, &50_000);

    let expires_at = env.ledger().timestamp() + 1000;
    vault_client.lock_collateral(
        &escrow,
        &seller,
        &token_client.address,
        &30_000,
        &42,
        &expires_at,
    );

    assert_eq!(token_client.balance(&buyer), 0);

    // Escrow contract slashes 20_000 of locked collateral to refund buyer
    vault_client.slash_and_refund(&escrow, &seller, &buyer, &42, &20_000);

    assert_eq!(token_client.balance(&buyer), 20_000);

    let summary = vault_client.get_summary(&seller, &token_client.address);
    assert_eq!(summary.total_collateral, 30_000);
    assert_eq!(summary.locked_collateral, 10_000);
    assert_eq!(summary.available_collateral, 20_000);

    let remaining_lock = vault_client.get_lock(&seller, &42);
    assert_eq!(
        remaining_lock,
        Some(LockRecord {
            escrow_id: 42,
            asset: token_client.address.clone(),
            amount: 10_000,
            expires_at,
        })
    );
}

#[test]
fn test_unauthorized_slash_fails() {
    let (env, _admin, escrow, seller, buyer, token_client, _, vault_client) = setup_test();

    vault_client.deposit(&seller, &token_client.address, &50_000);

    let expires_at = env.ledger().timestamp() + 1000;
    vault_client.lock_collateral(
        &escrow,
        &seller,
        &token_client.address,
        &30_000,
        &42,
        &expires_at,
    );

    let intruder = Address::generate(&env);
    let err = vault_client.try_slash_and_refund(&intruder, &seller, &buyer, &42, &10_000);
    assert_eq!(err, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_multi_seller_isolation() {
    let (env, _admin, escrow, seller1, _buyer, token_client, token_admin, vault_client) =
        setup_test();

    let seller2 = Address::generate(&env);
    token_admin.mint(&seller2, &80_000);

    vault_client.deposit(&seller1, &token_client.address, &40_000);
    vault_client.deposit(&seller2, &token_client.address, &60_000);

    let expires_at = env.ledger().timestamp() + 2000;
    vault_client.lock_collateral(
        &escrow,
        &seller1,
        &token_client.address,
        &15_000,
        &101,
        &expires_at,
    );

    let s1 = vault_client.get_summary(&seller1, &token_client.address);
    let s2 = vault_client.get_summary(&seller2, &token_client.address);

    assert_eq!(s1.total_collateral, 40_000);
    assert_eq!(s1.locked_collateral, 15_000);
    assert_eq!(s1.available_collateral, 25_000);

    assert_eq!(s2.total_collateral, 60_000);
    assert_eq!(s2.locked_collateral, 0);
    assert_eq!(s2.available_collateral, 60_000);
}
