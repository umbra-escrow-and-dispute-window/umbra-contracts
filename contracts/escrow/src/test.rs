#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, IntoVal,
};

use crate::{ArbiterConfig, Error, EscrowContract, EscrowContractClient, State, Verdict};

const AMOUNT: i128 = 1_000;
const CONFIRM_WINDOW: u64 = 2_000;
const DISPUTE_WINDOW: u64 = 1_000;
const START_TS: u64 = 10_000;

struct Ctx<'a> {
    env: Env,
    client: EscrowContractClient<'a>,
    contract_id: Address,
    token: Address,
    token_client: TokenClient<'a>,
    buyer: Address,
    seller: Address,
    arbiter: Address,
}

fn setup<'a>() -> Ctx<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = START_TS);

    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token = sac.address();

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    // Fund the buyer so real transfers succeed.
    StellarAssetClient::new(&env, &token).mint(&buyer, &(AMOUNT * 10));

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    Ctx {
        token_client: TokenClient::new(&env, &token),
        env,
        client,
        contract_id,
        token,
        buyer,
        seller,
        arbiter,
    }
}

fn hash(env: &Env, b: u8) -> BytesN<32> {
    BytesN::from_array(env, &[b; 32])
}

impl Ctx<'_> {
    fn create(&self, arbiter: ArbiterConfig) -> u64 {
        self.create_with_bond(arbiter, 0)
    }

    fn create_with_bond(&self, arbiter: ArbiterConfig, dispute_bond: i128) -> u64 {
        self.client.create_escrow(
            &self.buyer,
            &self.seller,
            &self.token,
            &AMOUNT,
            &dispute_bond,
            &(START_TS + 500),
            &CONFIRM_WINDOW,
            &DISPUTE_WINDOW,
            &arbiter,
            &hash(&self.env, 1),
        )
    }

    fn single(&self) -> ArbiterConfig {
        ArbiterConfig::Single(self.arbiter.clone())
    }

    fn advance(&self, by: u64) {
        self.env
            .ledger()
            .with_mut(|li| li.timestamp = START_TS + by);
    }
}

// --- Legal path: happy (confirm) ---

#[test]
fn happy_path_confirm_releases_to_seller() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    assert_eq!(c.token_client.balance(&c.contract_id), AMOUNT);

    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.confirm(&id);

    assert_eq!(c.client.get_escrow(&id).unwrap().state, State::Released);
    assert_eq!(c.token_client.balance(&c.seller), AMOUNT);
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
}

// --- Legal path: timeout ---

#[test]
fn timeout_after_confirm_window_releases_to_seller() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));

    c.advance(CONFIRM_WINDOW + 1);
    c.client.claim_timeout(&id);

    assert_eq!(c.client.get_escrow(&id).unwrap().state, State::Released);
    assert_eq!(c.token_client.balance(&c.seller), AMOUNT);
}

#[test]
fn timeout_before_confirm_window_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));

    c.advance(CONFIRM_WINDOW - 1);
    assert_eq!(
        c.client.try_claim_timeout(&id),
        Err(Ok(Error::ConfirmWindowOpen))
    );
}

// --- Legal path: dispute + resolve variants ---

#[test]
fn dispute_then_refund_buyer() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    c.client.resolve(&id, &Verdict::RefundBuyer, &None, &None);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedBuyer
    );
    assert_eq!(c.token_client.balance(&c.buyer), AMOUNT * 10);
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
}

#[test]
fn dispute_then_release_seller() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    c.client.resolve(&id, &Verdict::ReleaseSeller, &None, &None);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedSeller
    );
    assert_eq!(c.token_client.balance(&c.seller), AMOUNT);
}

#[test]
fn split_even_amount_distributes_exactly() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // 40% to buyer, 60% to seller of 1000.
    c.client.resolve(&id, &Verdict::Split(4_000), &None, &None);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedSplit
    );
    assert_eq!(c.token_client.balance(&c.buyer), AMOUNT * 10 - AMOUNT + 400);
    assert_eq!(c.token_client.balance(&c.seller), 600);
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
}

#[test]
fn split_odd_amount_rounds_remainder_to_seller() {
    let c = setup();
    // buyer_bps 3333 of 1000 = 333.3 -> buyer floored to 333, seller 667.
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    c.client.resolve(&id, &Verdict::Split(3_333), &None, &None);

    let buyer_share = 333;
    let seller_share = AMOUNT - buyer_share; // 667, remainder to seller
    assert_eq!(
        c.token_client.balance(&c.buyer),
        AMOUNT * 10 - AMOUNT + buyer_share
    );
    assert_eq!(c.token_client.balance(&c.seller), seller_share);
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
}

#[test]
fn split_rejects_bps_over_denominator() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    assert_eq!(
        c.client
            .try_resolve(&id, &Verdict::Split(10_001), &None, &None),
        Err(Ok(Error::InvalidVerdict))
    );
}

#[test]
fn split_zero_bps_pays_entire_amount_to_seller() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    c.client.resolve(&id, &Verdict::Split(0), &None, &None);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedSplit
    );
    assert_eq!(c.token_client.balance(&c.buyer), AMOUNT * 10 - AMOUNT);
    assert_eq!(c.token_client.balance(&c.seller), AMOUNT);
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
}

#[test]
fn split_full_bps_pays_entire_amount_to_buyer() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    c.client.resolve(&id, &Verdict::Split(10_000), &None, &None);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedSplit
    );
    assert_eq!(c.token_client.balance(&c.buyer), AMOUNT * 10);
    assert_eq!(c.token_client.balance(&c.seller), 0);
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
}

// --- Legal path: cancel before delivery ---

#[test]
fn cancel_when_funded_refunds_buyer() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);

    c.client.cancel(&id, &c.buyer);

    assert_eq!(c.client.get_escrow(&id).unwrap().state, State::Refunded);
    assert_eq!(c.token_client.balance(&c.buyer), AMOUNT * 10);
}

#[test]
fn cancel_when_created_closes_without_transfer() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);

    c.client.cancel(&id, &c.seller);

    assert_eq!(c.client.get_escrow(&id).unwrap().state, State::Refunded);
}

#[test]
fn cancel_after_delivery_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));

    assert_eq!(
        c.client.try_cancel(&id, &c.buyer),
        Err(Ok(Error::InvalidState))
    );
}

#[test]
fn cancel_by_non_party_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    let stranger = Address::generate(&c.env);

    assert_eq!(
        c.client.try_cancel(&id, &stranger),
        Err(Ok(Error::NotAuthorized))
    );
}

// --- Illegal state transitions ---

#[test]
fn fund_twice_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    assert_eq!(c.client.try_fund(&id), Err(Ok(Error::InvalidState)));
}

#[test]
fn mark_delivered_before_fund_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    assert_eq!(
        c.client.try_mark_delivered(&id, &hash(&c.env, 2)),
        Err(Ok(Error::InvalidState))
    );
}

#[test]
fn confirm_before_delivery_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    assert_eq!(c.client.try_confirm(&id), Err(Ok(Error::InvalidState)));
}

#[test]
fn dispute_without_arbiter_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    assert_eq!(
        c.client.try_dispute(&id, &hash(&c.env, 3)),
        Err(Ok(Error::ArbiterRequired))
    );
}

#[test]
fn dispute_after_window_fails() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));

    c.advance(DISPUTE_WINDOW + 1);
    assert_eq!(
        c.client.try_dispute(&id, &hash(&c.env, 3)),
        Err(Ok(Error::DisputeWindowClosed))
    );
}

#[test]
fn resolve_without_dispute_fails() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    assert_eq!(
        c.client
            .try_resolve(&id, &Verdict::ReleaseSeller, &None, &None),
        Err(Ok(Error::InvalidState))
    );
}

#[test]
fn get_missing_escrow_returns_none() {
    let c = setup();
    assert!(c.client.get_escrow(&999).is_none());
}

#[test]
fn missing_escrow_call_fails() {
    let c = setup();
    assert_eq!(c.client.try_fund(&999), Err(Ok(Error::EscrowNotFound)));
}

// --- Arbiter variants ---

#[test]
fn resolve_panel_with_threshold_signatures_succeeds() {
    let c = setup();
    let m1 = Address::generate(&c.env);
    let m2 = Address::generate(&c.env);
    let m3 = Address::generate(&c.env);

    let panel = soroban_sdk::vec![&c.env, m1.clone(), m2.clone(), m3.clone()];
    let id = c.create(ArbiterConfig::Panel(panel, 2));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Threshold is 2 of 3. Authorize with m1 and m3.
    let signers = soroban_sdk::vec![&c.env, m1.clone(), m3.clone()];
    c.client
        .resolve_panel(&id, &Verdict::ReleaseSeller, &signers);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedSeller
    );
    assert_eq!(c.token_client.balance(&c.seller), AMOUNT);
}

#[test]
fn resolve_panel_below_threshold_fails() {
    let c = setup();
    let m1 = Address::generate(&c.env);
    let m2 = Address::generate(&c.env);

    let panel = soroban_sdk::vec![&c.env, m1.clone(), m2.clone()];
    let id = c.create(ArbiterConfig::Panel(panel, 2));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Only 1 signer supplied when 2 required
    let signers = soroban_sdk::vec![&c.env, m1];
    assert_eq!(
        c.client
            .try_resolve_panel(&id, &Verdict::ReleaseSeller, &signers),
        Err(Ok(Error::BelowThreshold))
    );
}

#[test]
fn resolve_panel_duplicate_signers_fails() {
    let c = setup();
    let m1 = Address::generate(&c.env);
    let m2 = Address::generate(&c.env);

    let panel = soroban_sdk::vec![&c.env, m1.clone(), m2];
    let id = c.create(ArbiterConfig::Panel(panel, 2));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Duplicate signer m1 repeated to fake threshold
    let signers = soroban_sdk::vec![&c.env, m1.clone(), m1];
    assert_eq!(
        c.client
            .try_resolve_panel(&id, &Verdict::ReleaseSeller, &signers),
        Err(Ok(Error::DuplicateSigner))
    );
}

#[test]
fn resolve_panel_non_member_signer_fails() {
    let c = setup();
    let m1 = Address::generate(&c.env);
    let m2 = Address::generate(&c.env);
    let stranger = Address::generate(&c.env);

    let panel = soroban_sdk::vec![&c.env, m1.clone(), m2];
    let id = c.create(ArbiterConfig::Panel(panel, 2));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    let signers = soroban_sdk::vec![&c.env, m1, stranger];
    assert_eq!(
        c.client
            .try_resolve_panel(&id, &Verdict::ReleaseSeller, &signers),
        Err(Ok(Error::NotAuthorized))
    );
}

#[test]
fn resolve_panel_unauthorized_member_signature_fails() {
    let c = setup();
    let m1 = Address::generate(&c.env);
    let m2 = Address::generate(&c.env);

    let panel = soroban_sdk::vec![&c.env, m1.clone(), m2.clone()];
    let id = c.create(ArbiterConfig::Panel(panel, 2));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    let signers = soroban_sdk::vec![&c.env, m1.clone(), m2.clone()];

    // Only m1 authorizes; m2 signature is missing
    let r = c
        .client
        .mock_auths(&[MockAuth {
            address: &m1,
            invoke: &MockAuthInvoke {
                contract: &c.contract_id,
                fn_name: "resolve",
                args: (
                    id,
                    Verdict::ReleaseSeller,
                    None::<BytesN<64>>,
                    Some(signers.clone()),
                )
                    .into_val(&c.env),
                sub_invokes: &[],
            },
        }])
        .try_resolve_panel(&id, &Verdict::ReleaseSeller, &signers);
    assert!(r.is_err());
}

#[test]
fn create_panel_rejects_invalid_threshold_or_duplicate_members() {
    let c = setup();
    let m1 = Address::generate(&c.env);
    let m2 = Address::generate(&c.env);

    // Threshold 0 rejected
    let panel = soroban_sdk::vec![&c.env, m1.clone(), m2.clone()];
    let r0 = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &AMOUNT,
        &0,
        &(START_TS + 500),
        &CONFIRM_WINDOW,
        &DISPUTE_WINDOW,
        &ArbiterConfig::Panel(panel, 0),
        &hash(&c.env, 1),
    );
    assert_eq!(r0, Err(Ok(Error::InvalidThreshold)));

    // Threshold > len rejected
    let panel2 = soroban_sdk::vec![&c.env, m1.clone(), m2.clone()];
    let r_too_high = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &AMOUNT,
        &0,
        &(START_TS + 500),
        &CONFIRM_WINDOW,
        &DISPUTE_WINDOW,
        &ArbiterConfig::Panel(panel2, 3),
        &hash(&c.env, 1),
    );
    assert_eq!(r_too_high, Err(Ok(Error::InvalidThreshold)));

    // Duplicate member in panel config rejected
    let panel_dup = soroban_sdk::vec![&c.env, m1.clone(), m1];
    let r_dup = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &AMOUNT,
        &0,
        &(START_TS + 500),
        &CONFIRM_WINDOW,
        &DISPUTE_WINDOW,
        &ArbiterConfig::Panel(panel_dup, 2),
        &hash(&c.env, 1),
    );
    assert_eq!(r_dup, Err(Ok(Error::DuplicateSigner)));
}

#[test]
fn resolve_signed_without_signature_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::Signed(hash(&c.env, 9)));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    assert_eq!(
        c.client
            .try_resolve(&id, &Verdict::RefundBuyer, &None, &None),
        Err(Ok(Error::NotAuthorized))
    );
}

#[test]
fn resolve_signed_with_valid_signature_succeeds() {
    use ed25519_dalek::{Signer, SigningKey};

    let c = setup();
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes: BytesN<32> = BytesN::from_array(&c.env, &verifying_key.to_bytes());

    let id = c.create(ArbiterConfig::Signed(pubkey_bytes));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    let verdict = Verdict::RefundBuyer;
    let payload = EscrowContract::signed_verdict_payload(&c.env, id, &verdict);
    let mut raw_payload = [0u8; 32];
    payload.copy_into_slice(&mut raw_payload);
    let sig = signing_key.sign(&raw_payload);
    let sig_bytes: BytesN<64> = BytesN::from_array(&c.env, &sig.to_bytes());

    c.client.resolve_signed(&id, &verdict, &sig_bytes);

    assert_eq!(
        c.client.get_escrow(&id).unwrap().state,
        State::ResolvedBuyer
    );
    assert_eq!(c.token_client.balance(&c.buyer), 10_000);
}

#[test]
fn resolve_signed_with_wrong_key_fails() {
    use ed25519_dalek::{Signer, SigningKey};

    let c = setup();
    let configured_key = SigningKey::from_bytes(&[42u8; 32]);
    let other_key = SigningKey::from_bytes(&[7u8; 32]);

    let pubkey_bytes: BytesN<32> =
        BytesN::from_array(&c.env, &configured_key.verifying_key().to_bytes());

    let id = c.create(ArbiterConfig::Signed(pubkey_bytes));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    let verdict = Verdict::ReleaseSeller;
    let payload = EscrowContract::signed_verdict_payload(&c.env, id, &verdict);
    let mut raw_payload = [0u8; 32];
    payload.copy_into_slice(&mut raw_payload);
    // Signed by wrong key
    let bad_sig = other_key.sign(&raw_payload);
    let sig_bytes: BytesN<64> = BytesN::from_array(&c.env, &bad_sig.to_bytes());

    let r = c.client.try_resolve_signed(&id, &verdict, &sig_bytes);
    assert!(r.is_err());
}

#[test]
fn resolve_signed_with_tampered_payload_fails() {
    use ed25519_dalek::{Signer, SigningKey};

    let c = setup();
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let pubkey_bytes: BytesN<32> =
        BytesN::from_array(&c.env, &signing_key.verifying_key().to_bytes());

    let id = c.create(ArbiterConfig::Signed(pubkey_bytes));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Sign for ReleaseSeller
    let legitimate_verdict = Verdict::ReleaseSeller;
    let payload = EscrowContract::signed_verdict_payload(&c.env, id, &legitimate_verdict);
    let mut raw_payload = [0u8; 32];
    payload.copy_into_slice(&mut raw_payload);
    let sig = signing_key.sign(&raw_payload);
    let sig_bytes: BytesN<64> = BytesN::from_array(&c.env, &sig.to_bytes());

    // Adversary attempts to use signature with RefundBuyer
    let tampered_verdict = Verdict::RefundBuyer;
    let r = c
        .client
        .try_resolve_signed(&id, &tampered_verdict, &sig_bytes);
    assert!(r.is_err());
}

// --- Validation at creation ---

#[test]
fn create_rejects_non_positive_amount() {
    let c = setup();
    let r = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &0,
        &0,
        &(START_TS + 500),
        &CONFIRM_WINDOW,
        &DISPUTE_WINDOW,
        &ArbiterConfig::None,
        &hash(&c.env, 1),
    );
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn create_rejects_negative_dispute_bond() {
    let c = setup();
    let r = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &AMOUNT,
        &-1,
        &(START_TS + 500),
        &CONFIRM_WINDOW,
        &DISPUTE_WINDOW,
        &ArbiterConfig::None,
        &hash(&c.env, 1),
    );
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn create_rejects_zero_window() {
    let c = setup();
    let r = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &AMOUNT,
        &0,
        &(START_TS + 500),
        &0,
        &DISPUTE_WINDOW,
        &ArbiterConfig::None,
        &hash(&c.env, 1),
    );
    assert_eq!(r, Err(Ok(Error::InvalidWindow)));
}

#[test]
fn create_rejects_confirm_window_shorter_than_dispute_window() {
    let c = setup();
    let r = c.client.try_create_escrow(
        &c.buyer,
        &c.seller,
        &c.token,
        &AMOUNT,
        &0,
        &(START_TS + 500),
        &1_000,
        &2_000,
        &ArbiterConfig::None,
        &hash(&c.env, 1),
    );
    assert_eq!(r, Err(Ok(Error::InvalidWindowOrdering)));
}

// --- Unauthorised (wrong signer) paths ---
// These authorise only a non-required address, proving each entry point
// requires the specific party's signature. The required party never signs, so
// its require_auth has no matching entry and the call fails.

#[test]
fn fund_by_non_buyer_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    let r = c
        .client
        .mock_auths(&[MockAuth {
            address: &c.seller,
            invoke: &MockAuthInvoke {
                contract: &c.contract_id,
                fn_name: "fund",
                args: (id,).into_val(&c.env),
                sub_invokes: &[],
            },
        }])
        .try_fund(&id);
    assert!(r.is_err());
}

#[test]
fn mark_delivered_by_non_seller_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    let proof = hash(&c.env, 2);
    let r = c
        .client
        .mock_auths(&[MockAuth {
            address: &c.buyer,
            invoke: &MockAuthInvoke {
                contract: &c.contract_id,
                fn_name: "mark_delivered",
                args: (id, proof.clone()).into_val(&c.env),
                sub_invokes: &[],
            },
        }])
        .try_mark_delivered(&id, &proof);
    assert!(r.is_err());
}

#[test]
fn confirm_by_seller_fails() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    let r = c
        .client
        .mock_auths(&[MockAuth {
            address: &c.seller,
            invoke: &MockAuthInvoke {
                contract: &c.contract_id,
                fn_name: "confirm",
                args: (id,).into_val(&c.env),
                sub_invokes: &[],
            },
        }])
        .try_confirm(&id);
    assert!(r.is_err());
}

#[test]
fn resolve_by_non_arbiter_fails() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));
    let r = c
        .client
        .mock_auths(&[MockAuth {
            address: &c.buyer,
            invoke: &MockAuthInvoke {
                contract: &c.contract_id,
                fn_name: "resolve",
                args: (
                    id,
                    Verdict::ReleaseSeller,
                    None::<BytesN<64>>,
                    None::<soroban_sdk::Vec<Address>>,
                )
                    .into_val(&c.env),
                sub_invokes: &[],
            },
        }])
        .try_resolve(&id, &Verdict::ReleaseSeller, &None, &None);
    assert!(r.is_err());
}

// --- Event schema verification (docs/events.md) ---

#[test]
fn events_happy_path_matches_schema() {
    use crate::events::{AttestationRecord, Transition};
    use soroban_sdk::events::Event as _;

    let c = setup();
    let id = c.create(ArbiterConfig::None);
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [Transition {
            kind: symbol_short!("created"),
            escrow: id,
            state: State::Created,
        }
        .to_xdr(&c.env, &c.contract_id)]
    );

    c.client.fund(&id);
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [Transition {
            kind: symbol_short!("funded"),
            escrow: id,
            state: State::Funded,
        }
        .to_xdr(&c.env, &c.contract_id)]
    );

    c.client.mark_delivered(&id, &hash(&c.env, 2));
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [Transition {
            kind: symbol_short!("delivered"),
            escrow: id,
            state: State::Delivered,
        }
        .to_xdr(&c.env, &c.contract_id)]
    );

    c.client.confirm(&id);
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [
            Transition {
                kind: symbol_short!("confirmed"),
                escrow: id,
                state: State::Released,
            }
            .to_xdr(&c.env, &c.contract_id),
            AttestationRecord {
                escrow: id,
                buyer: c.buyer.clone(),
                seller: c.seller.clone(),
                outcome: State::Released,
                amount_bucket: 0,
            }
            .to_xdr(&c.env, &c.contract_id),
        ]
    );
}

#[test]
fn events_dispute_path_matches_schema() {
    use crate::events::{AttestationRecord, Transition};
    use soroban_sdk::events::Event as _;

    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));

    c.client.dispute(&id, &hash(&c.env, 3));
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [Transition {
            kind: symbol_short!("disputed"),
            escrow: id,
            state: State::Disputed,
        }
        .to_xdr(&c.env, &c.contract_id)]
    );

    c.client.resolve(&id, &Verdict::RefundBuyer, &None, &None);
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [
            Transition {
                kind: symbol_short!("resolved"),
                escrow: id,
                state: State::ResolvedBuyer,
            }
            .to_xdr(&c.env, &c.contract_id),
            AttestationRecord {
                escrow: id,
                buyer: c.buyer.clone(),
                seller: c.seller.clone(),
                outcome: State::ResolvedBuyer,
                amount_bucket: 0,
            }
            .to_xdr(&c.env, &c.contract_id),
        ]
    );
}

#[test]
fn events_cancel_and_timeout_paths_match_schema() {
    use crate::events::{AttestationRecord, Transition};
    use soroban_sdk::events::Event as _;

    let c = setup();
    let id = c.create(ArbiterConfig::None);
    c.client.cancel(&id, &c.buyer);
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [
            Transition {
                kind: symbol_short!("cancelled"),
                escrow: id,
                state: State::Refunded,
            }
            .to_xdr(&c.env, &c.contract_id),
            AttestationRecord {
                escrow: id,
                buyer: c.buyer.clone(),
                seller: c.seller.clone(),
                outcome: State::Refunded,
                amount_bucket: 0,
            }
            .to_xdr(&c.env, &c.contract_id),
        ]
    );

    let id2 = c.create(ArbiterConfig::None);
    c.client.fund(&id2);
    c.client.mark_delivered(&id2, &hash(&c.env, 2));
    c.advance(CONFIRM_WINDOW + 1);
    c.client.claim_timeout(&id2);
    assert_eq!(
        c.env.events().all().filter_by_contract(&c.contract_id),
        [
            Transition {
                kind: symbol_short!("timeout"),
                escrow: id2,
                state: State::Released,
            }
            .to_xdr(&c.env, &c.contract_id),
            AttestationRecord {
                escrow: id2,
                buyer: c.buyer.clone(),
                seller: c.seller.clone(),
                outcome: State::Released,
                amount_bucket: 0,
            }
            .to_xdr(&c.env, &c.contract_id),
        ]
    );
}

// --- Property tests for split rounding (Issue 3 / protocol.md) ---

#[test]
fn property_split_rounding_invariants_hold_across_ranges() {
    let amounts: [i128; 14] = [
        1,
        2,
        3,
        7,
        10,
        99,
        100,
        1_000,
        9_999,
        10_000,
        10_001,
        100_000,
        1_000_000_000,
        i128::MAX / 10_000,
    ];
    let bps_samples: [u32; 11] = [
        0, 1, 333, 1_000, 3_333, 4_999, 5_000, 5_001, 6_667, 9_999, 10_000,
    ];

    for &amount in &amounts {
        for &bps in &bps_samples {
            let buyer_amount = amount * i128::from(bps) / 10_000;
            let seller_amount = amount - buyer_amount;

            // Invariant 1: Total conservation of funds
            assert_eq!(
                buyer_amount + seller_amount,
                amount,
                "conservation failed for amount={amount}, bps={bps}"
            );

            // Invariant 2: Buyer share is floored division
            assert_eq!(
                buyer_amount,
                (amount * i128::from(bps)) / 10_000,
                "buyer flooring failed for amount={amount}, bps={bps}"
            );

            // Invariant 3: Neither share is negative
            assert!(buyer_amount >= 0, "negative buyer share");
            assert!(seller_amount >= 0, "negative seller share");

            // Invariant 4: Remainder stroop goes to seller
            if (amount * i128::from(bps)) % 10_000 != 0 {
                let exact_unrounded_buyer_num = amount * i128::from(bps);
                let floored_buyer_num = buyer_amount * 10_000;
                assert!(exact_unrounded_buyer_num > floored_buyer_num);
            }
        }
    }
}

#[test]
fn property_split_contract_execution_conserves_balances() {
    let test_cases: [(i128, u32); 6] = [
        (1, 5_000),      // 1 stroop: buyer gets 0, seller gets 1
        (7, 3_333),      // 7 stroops: 7 * 3333 / 10000 = 23331 / 10000 = 2 -> buyer 2, seller 5
        (100, 2_500),    // exact 25/75 split
        (999, 3_333),    // 999 * 3333 / 10000 = 3329667 / 10000 = 332 -> buyer 332, seller 667
        (10_000, 1),     // 10000 * 1 / 10000 = 1 -> buyer 1, seller 9999
        (10_000, 9_999), // 10000 * 9999 / 10000 = 9999 -> buyer 9999, seller 1
    ];

    for (amount, bps) in test_cases {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|li| li.timestamp = START_TS);

        let admin = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token = sac.address();
        let token_client = TokenClient::new(&env, &token);

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let arbiter = Address::generate(&env);

        StellarAssetClient::new(&env, &token).mint(&buyer, &(amount * 2));

        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        let id = client.create_escrow(
            &buyer,
            &seller,
            &token,
            &amount,
            &0,
            &(START_TS + 500),
            &CONFIRM_WINDOW,
            &DISPUTE_WINDOW,
            &ArbiterConfig::Single(arbiter.clone()),
            &hash(&env, 1),
        );

        client.fund(&id);
        client.mark_delivered(&id, &hash(&env, 2));
        client.dispute(&id, &hash(&env, 3));
        client.resolve(&id, &Verdict::Split(bps), &None, &None);

        let expected_buyer = amount * i128::from(bps) / 10_000;
        let expected_seller = amount - expected_buyer;

        assert_eq!(token_client.balance(&contract_id), 0);
        assert_eq!(token_client.balance(&seller), expected_seller);
        assert_eq!(
            token_client.balance(&buyer),
            (amount * 2) - amount + expected_buyer
        );
    }
}

// --- Dispute Bond Tests (Issue 4) ---

#[test]
fn dispute_with_bond_transfers_from_buyer_to_contract() {
    let c = setup();
    let bond = 200;
    let id = c.create_with_bond(c.single(), bond);

    c.client.fund(&id);
    assert_eq!(c.token_client.balance(&c.buyer), 9_000);
    assert_eq!(c.token_client.balance(&c.contract_id), AMOUNT);

    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Buyer posted the 200 bond upon dispute
    assert_eq!(c.token_client.balance(&c.buyer), 9_000 - bond);
    assert_eq!(c.token_client.balance(&c.contract_id), AMOUNT + bond);
}

#[test]
fn dispute_bond_refunded_to_buyer_on_refund_verdict() {
    let c = setup();
    let bond = 200;
    let id = c.create_with_bond(c.single(), bond);

    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Resolve in buyer's favor: full amount + bond refunded
    c.client.resolve(&id, &Verdict::RefundBuyer, &None, &None);

    assert_eq!(c.token_client.balance(&c.contract_id), 0);
    assert_eq!(c.token_client.balance(&c.buyer), 10_000);
    assert_eq!(c.token_client.balance(&c.seller), 0);
}

#[test]
fn dispute_bond_forfeited_to_seller_on_release_verdict() {
    let c = setup();
    let bond = 250;
    let id = c.create_with_bond(c.single(), bond);

    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Resolve in seller's favor: buyer forfeits bond, seller gets amount + bond
    c.client.resolve(&id, &Verdict::ReleaseSeller, &None, &None);

    assert_eq!(c.token_client.balance(&c.contract_id), 0);
    assert_eq!(c.token_client.balance(&c.buyer), 10_000 - AMOUNT - bond);
    assert_eq!(c.token_client.balance(&c.seller), AMOUNT + bond);
}

#[test]
fn dispute_bond_split_proportionally_on_split_verdict() {
    let c = setup();
    let bond = 400;
    let id = c.create_with_bond(c.single(), bond);

    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    // Total funds in escrow: 1000 + 400 = 1400.
    // 50% split (5000 bps)
    c.client.resolve(&id, &Verdict::Split(5_000), &None, &None);

    assert_eq!(c.token_client.balance(&c.contract_id), 0);
    // Buyer gets back 700 of the 1400 they deposited (1000 principal + 400 bond)
    assert_eq!(c.token_client.balance(&c.buyer), 10_000 - 1_400 + 700);
    assert_eq!(c.token_client.balance(&c.seller), 700);
}

#[test]
fn astrolabe_attestation_buckets_match_spec() {
    use crate::events::compute_amount_bucket;

    assert_eq!(compute_amount_bucket(0), 0);
    assert_eq!(compute_amount_bucket(100_000_000), 0);
    assert_eq!(compute_amount_bucket(100_000_001), 1);
    assert_eq!(compute_amount_bucket(1_000_000_000), 1);
    assert_eq!(compute_amount_bucket(1_000_000_001), 2);
    assert_eq!(compute_amount_bucket(10_000_000_000), 2);
    assert_eq!(compute_amount_bucket(10_000_000_001), 3);
    assert_eq!(compute_amount_bucket(100_000_000_000), 3);
    assert_eq!(compute_amount_bucket(100_000_000_001), 4);
}

// --- Hardening & Edge Case Tests ---

#[test]
fn extend_escrow_ttl_succeeds_for_existing_and_fails_for_missing() {
    let c = setup();
    let id = c.create(ArbiterConfig::None);

    c.client.extend_escrow_ttl(&id);

    assert_eq!(
        c.client.try_extend_escrow_ttl(&999),
        Err(Ok(Error::EscrowNotFound))
    );
}

#[test]
fn multiple_concurrent_escrows_isolate_state_and_funds() {
    let c = setup();
    let id1 = c.create(c.single());
    let id2 = c.create(c.single());
    let id3 = c.create_with_bond(c.single(), 200);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);

    c.client.fund(&id1);
    c.client.fund(&id2);
    c.client.fund(&id3);
    assert_eq!(c.token_client.balance(&c.contract_id), AMOUNT * 3);

    // Escrow 1: happy path confirm
    c.client.mark_delivered(&id1, &hash(&c.env, 2));
    c.client.confirm(&id1);
    assert_eq!(c.client.get_escrow(&id1).unwrap().state, State::Released);

    // Escrow 2: funded cancel
    c.client.cancel(&id2, &c.seller);
    assert_eq!(c.client.get_escrow(&id2).unwrap().state, State::Refunded);

    // Escrow 3: dispute + 50% split with bond
    c.client.mark_delivered(&id3, &hash(&c.env, 2));
    c.client.dispute(&id3, &hash(&c.env, 3));
    c.client.resolve(&id3, &Verdict::Split(5_000), &None, &None);
    assert_eq!(
        c.client.get_escrow(&id3).unwrap().state,
        State::ResolvedSplit
    );

    // Contract balance is fully drained
    assert_eq!(c.token_client.balance(&c.contract_id), 0);
    // Seller got 1000 from id1 + 600 from id3 (50% of 1000+200) = 1600
    assert_eq!(c.token_client.balance(&c.seller), 1_600);
    // Buyer started with 10_000, paid 1000 on id1, net 0 on id2, paid 1200 and got 600 back on id3 -> 8_400
    assert_eq!(c.token_client.balance(&c.buyer), 8_400);
}

#[test]
fn timestamp_overflow_on_window_deadline_returns_overflow_error() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);

    // Set ledger timestamp near u64::MAX before delivery
    c.env.ledger().with_mut(|li| li.timestamp = u64::MAX - 10);
    c.client.mark_delivered(&id, &hash(&c.env, 2));

    // Confirm and dispute windows are 200 and 100 respectively, so delivered_at + window overflows u64
    assert_eq!(c.client.try_claim_timeout(&id), Err(Ok(Error::Overflow)));
    assert_eq!(
        c.client.try_dispute(&id, &hash(&c.env, 3)),
        Err(Ok(Error::Overflow))
    );
}

#[test]
fn terminal_states_reject_all_subsequent_transitions() {
    let c = setup();
    let id = c.create(c.single());
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.confirm(&id);

    assert_eq!(c.client.get_escrow(&id).unwrap().state, State::Released);

    assert_eq!(c.client.try_fund(&id), Err(Ok(Error::InvalidState)));
    assert_eq!(
        c.client.try_mark_delivered(&id, &hash(&c.env, 2)),
        Err(Ok(Error::InvalidState))
    );
    assert_eq!(c.client.try_confirm(&id), Err(Ok(Error::InvalidState)));
    assert_eq!(
        c.client.try_claim_timeout(&id),
        Err(Ok(Error::InvalidState))
    );
    assert_eq!(
        c.client.try_dispute(&id, &hash(&c.env, 3)),
        Err(Ok(Error::InvalidState))
    );
    assert_eq!(
        c.client
            .try_resolve(&id, &Verdict::RefundBuyer, &None, &None),
        Err(Ok(Error::InvalidState))
    );
    assert_eq!(
        c.client.try_cancel(&id, &c.buyer),
        Err(Ok(Error::InvalidState))
    );
}
