#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, IntoVal,
};

use crate::{ArbiterConfig, Error, EscrowContract, EscrowContractClient, State, Verdict};

const AMOUNT: i128 = 1_000;
const CONFIRM_WINDOW: u64 = 1_000;
const DISPUTE_WINDOW: u64 = 2_000;
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

impl<'a> Ctx<'a> {
    fn create(&self, arbiter: ArbiterConfig) -> u64 {
        self.client.create_escrow(
            &self.buyer,
            &self.seller,
            &self.token,
            &AMOUNT,
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

    c.client.resolve(&id, &Verdict::RefundBuyer, &None);

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

    c.client.resolve(&id, &Verdict::ReleaseSeller, &None);

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
    c.client.resolve(&id, &Verdict::Split(4_000), &None);

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

    c.client.resolve(&id, &Verdict::Split(3_333), &None);

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
        c.client.try_resolve(&id, &Verdict::Split(10_001), &None),
        Err(Ok(Error::InvalidVerdict))
    );
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
        c.client.try_resolve(&id, &Verdict::ReleaseSeller, &None),
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

// --- Unimplemented arbiter variants ---

#[test]
fn resolve_panel_arbiter_is_unimplemented() {
    let c = setup();
    let panel = soroban_sdk::vec![&c.env, c.arbiter.clone()];
    let id = c.create(ArbiterConfig::Panel(panel, 1));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    assert_eq!(
        c.client.try_resolve(&id, &Verdict::ReleaseSeller, &None),
        Err(Ok(Error::Unimplemented))
    );
}

#[test]
fn resolve_signed_arbiter_is_unimplemented() {
    let c = setup();
    let id = c.create(ArbiterConfig::Signed(hash(&c.env, 9)));
    c.client.fund(&id);
    c.client.mark_delivered(&id, &hash(&c.env, 2));
    c.client.dispute(&id, &hash(&c.env, 3));

    assert_eq!(
        c.client.try_resolve(&id, &Verdict::RefundBuyer, &None),
        Err(Ok(Error::Unimplemented))
    );
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
        &(START_TS + 500),
        &0,
        &DISPUTE_WINDOW,
        &ArbiterConfig::None,
        &hash(&c.env, 1),
    );
    assert_eq!(r, Err(Ok(Error::InvalidWindow)));
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
                args: (id, Verdict::ReleaseSeller, None::<BytesN<64>>).into_val(&c.env),
                sub_invokes: &[],
            },
        }])
        .try_resolve(&id, &Verdict::ReleaseSeller, &None);
    assert!(r.is_err());
}
