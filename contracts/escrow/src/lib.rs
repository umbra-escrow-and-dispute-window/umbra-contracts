#![no_std]

//! Umbra escrow and dispute-window contract.
//!
//! A buyer funds an escrow, a seller delivers, and the buyer confirms, lets it
//! auto-release after a timeout, or disputes within the window. An arbiter
//! chosen at creation resolves a dispute with a refund, a release, or a split.
//!
//! The state machine and the authorisation rules are specified in
//! docs/protocol.md. Every transition below cites the table row it implements.

mod error;
pub mod events;
mod types;

pub use error::Error;
pub use types::{ArbiterConfig, DataKey, Escrow, State, Verdict};

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env};

// Basis points denominator for split verdicts.
const BPS_DENOM: u32 = 10_000;

// TTL bumps. Ledgers close roughly every five seconds, so a day is ~17280.
// Live escrows are extended well past a plausible dispute window so an active
// agreement is never archived out from under its parties.
const DAY: u32 = 17_280;
const ESCROW_TTL: u32 = 90 * DAY;
const ESCROW_TTL_THRESHOLD: u32 = 30 * DAY;

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// protocol.md T1. Records an agreement in `Created`. The buyer authorises,
    /// because the buyer is the party who will move funds in `fund`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        asset: Address,
        amount: i128,
        deliver_by: u64,
        confirm_window: u64,
        dispute_window: u64,
        arbiter: ArbiterConfig,
        terms_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        buyer.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        // A zero window is a footgun: it would let a seller claim the instant
        // after delivery, or leave no room to dispute. Reject both.
        if confirm_window == 0 || dispute_window == 0 {
            return Err(Error::InvalidWindow);
        }
        // A confirm window shorter than the dispute window is a footgun:
        // it would let a seller claim payment while the dispute window remains open.
        if confirm_window < dispute_window {
            return Err(Error::InvalidWindowOrdering);
        }

        if let ArbiterConfig::Panel(members, threshold) = &arbiter {
            if *threshold == 0 || *threshold > members.len() {
                return Err(Error::InvalidThreshold);
            }
            for i in 0..members.len() {
                for j in (i + 1)..members.len() {
                    if members.get(i) == members.get(j) {
                        return Err(Error::DuplicateSigner);
                    }
                }
            }
        }

        let id = Self::next_id(&env);
        let escrow = Escrow {
            id,
            buyer,
            seller,
            asset,
            amount,
            deliver_by,
            confirm_window,
            dispute_window,
            arbiter,
            terms_hash,
            state: State::Created,
            created_at: env.ledger().timestamp(),
            delivered_at: 0,
            proof_hash: BytesN::from_array(&env, &[0u8; 32]),
            reason_hash: BytesN::from_array(&env, &[0u8; 32]),
        };
        Self::put(&env, &escrow);
        events::created(&env, id);
        Ok(id)
    }

    /// protocol.md T2. The buyer transfers `amount` of `asset` into the
    /// contract. Only the buyer, only from `Created`.
    pub fn fund(env: Env, escrow_id: u64) -> Result<(), Error> {
        let mut e = Self::load(&env, escrow_id)?;
        e.buyer.require_auth();
        if e.state != State::Created {
            return Err(Error::InvalidState);
        }
        let contract = env.current_contract_address();
        token::TokenClient::new(&env, &e.asset).transfer(&e.buyer, &contract, &e.amount);
        e.state = State::Funded;
        Self::put(&env, &e);
        events::funded(&env, escrow_id);
        Ok(())
    }

    /// protocol.md T3. The seller records an off-chain delivery proof by its
    /// hash and starts the confirm and dispute windows.
    pub fn mark_delivered(env: Env, escrow_id: u64, proof_hash: BytesN<32>) -> Result<(), Error> {
        let mut e = Self::load(&env, escrow_id)?;
        e.seller.require_auth();
        if e.state != State::Funded {
            return Err(Error::InvalidState);
        }
        e.proof_hash = proof_hash;
        e.delivered_at = env.ledger().timestamp();
        e.state = State::Delivered;
        Self::put(&env, &e);
        events::delivered(&env, escrow_id);
        Ok(())
    }

    /// protocol.md T4. The buyer accepts delivery and releases funds to the
    /// seller immediately.
    pub fn confirm(env: Env, escrow_id: u64) -> Result<(), Error> {
        let mut e = Self::load(&env, escrow_id)?;
        e.buyer.require_auth();
        if e.state != State::Delivered {
            return Err(Error::InvalidState);
        }
        Self::pay(&env, &e, &e.seller, e.amount);
        e.state = State::Released;
        Self::put(&env, &e);
        events::confirmed(&env, escrow_id);
        Ok(())
    }

    /// protocol.md T5. After the confirm window the seller may claim payment
    /// even without an explicit confirm. Time is the ledger's, never a caller's.
    pub fn claim_timeout(env: Env, escrow_id: u64) -> Result<(), Error> {
        let mut e = Self::load(&env, escrow_id)?;
        e.seller.require_auth();
        if e.state != State::Delivered {
            return Err(Error::InvalidState);
        }
        let deadline = e
            .delivered_at
            .checked_add(e.confirm_window)
            .ok_or(Error::Overflow)?;
        if env.ledger().timestamp() < deadline {
            return Err(Error::ConfirmWindowOpen);
        }
        Self::pay(&env, &e, &e.seller, e.amount);
        e.state = State::Released;
        Self::put(&env, &e);
        events::timed_out(&env, escrow_id);
        Ok(())
    }

    /// protocol.md T6. The buyer disputes within the dispute window, recording
    /// the reason by its hash. Rejected when the escrow has no arbiter, since
    /// nothing could then resolve it.
    pub fn dispute(env: Env, escrow_id: u64, reason_hash: BytesN<32>) -> Result<(), Error> {
        let mut e = Self::load(&env, escrow_id)?;
        e.buyer.require_auth();
        if e.state != State::Delivered {
            return Err(Error::InvalidState);
        }
        if e.arbiter == ArbiterConfig::None {
            return Err(Error::ArbiterRequired);
        }
        let deadline = e
            .delivered_at
            .checked_add(e.dispute_window)
            .ok_or(Error::Overflow)?;
        if env.ledger().timestamp() > deadline {
            return Err(Error::DisputeWindowClosed);
        }
        e.reason_hash = reason_hash;
        e.state = State::Disputed;
        Self::put(&env, &e);
        events::disputed(&env, escrow_id);
        Ok(())
    }

    /// protocol.md T7. The escrow's arbiter resolves a dispute. `signature` is
    /// used for the `Signed` adjudicator variant, and `signers` supplies the
    /// authorizing members for the `Panel` arbiter variant.
    pub fn resolve(
        env: Env,
        escrow_id: u64,
        verdict: Verdict,
        signature: Option<BytesN<64>>,
        signers: Option<soroban_sdk::Vec<Address>>,
    ) -> Result<(), Error> {
        let mut e = Self::load(&env, escrow_id)?;
        if e.state != State::Disputed {
            return Err(Error::InvalidState);
        }
        match &e.arbiter {
            ArbiterConfig::Single(arbiter) => arbiter.require_auth(),
            ArbiterConfig::Panel(members, threshold) => {
                let signers = signers.ok_or(Error::BelowThreshold)?;
                if signers.len() < *threshold {
                    return Err(Error::BelowThreshold);
                }
                for i in 0..signers.len() {
                    for j in (i + 1)..signers.len() {
                        if signers.get(i) == signers.get(j) {
                            return Err(Error::DuplicateSigner);
                        }
                    }
                }
                for signer in signers.iter() {
                    let mut found = false;
                    for member in members.iter() {
                        if member == signer {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(Error::NotAuthorized);
                    }
                    signer.require_auth();
                }
            }
            // Unreachable: `dispute()` rejects `ArbiterConfig::None`. Kept explicit
            // so a future edit to `dispute()` cannot silently open this path.
            ArbiterConfig::None => return Err(Error::ArbiterRequired),
            ArbiterConfig::Signed(public_key) => {
                let sig = signature.ok_or(Error::NotAuthorized)?;
                let payload = Self::signed_verdict_payload(&env, escrow_id, &verdict);
                env.crypto().ed25519_verify(public_key, &payload, &sig);
            }
        }

        let new_state = match verdict {
            Verdict::RefundBuyer => {
                Self::pay(&env, &e, &e.buyer, e.amount);
                State::ResolvedBuyer
            }
            Verdict::ReleaseSeller => {
                Self::pay(&env, &e, &e.seller, e.amount);
                State::ResolvedSeller
            }
            Verdict::Split(buyer_bps) => {
                if buyer_bps > BPS_DENOM {
                    return Err(Error::InvalidVerdict);
                }
                // Rounding direction: the buyer's share is floored, so the
                // remaining stroop always goes to the seller. Documented in
                // protocol.md so tests and reviewers agree on the odd-amount case.
                let buyer_amount = e
                    .amount
                    .checked_mul(i128::from(buyer_bps))
                    .ok_or(Error::Overflow)?
                    / i128::from(BPS_DENOM);
                let seller_amount = e.amount.checked_sub(buyer_amount).ok_or(Error::Overflow)?;
                if buyer_amount > 0 {
                    Self::pay(&env, &e, &e.buyer, buyer_amount);
                }
                if seller_amount > 0 {
                    Self::pay(&env, &e, &e.seller, seller_amount);
                }
                State::ResolvedSplit
            }
        };
        e.state = new_state;
        Self::put(&env, &e);
        events::resolved(&env, escrow_id, new_state);
        Ok(())
    }

    /// Companion entry point for resolving disputes governed by `ArbiterConfig::Panel`.
    pub fn resolve_panel(
        env: Env,
        escrow_id: u64,
        verdict: Verdict,
        signers: soroban_sdk::Vec<Address>,
    ) -> Result<(), Error> {
        Self::resolve(env, escrow_id, verdict, None, Some(signers))
    }

    /// Companion entry point for resolving disputes governed by `ArbiterConfig::Signed`.
    pub fn resolve_signed(
        env: Env,
        escrow_id: u64,
        verdict: Verdict,
        signature: BytesN<64>,
    ) -> Result<(), Error> {
        Self::resolve(env, escrow_id, verdict, Some(signature), None)
    }

    /// Canonical payload for signed adjudicator verdicts (32 bytes).
    pub fn signed_verdict_payload(
        env: &Env,
        escrow_id: u64,
        verdict: &Verdict,
    ) -> soroban_sdk::Bytes {
        let mut payload = soroban_sdk::Bytes::new(env);
        payload.extend_from_array(b"umbra:adjudicate:v1");
        payload.extend_from_array(&escrow_id.to_be_bytes());
        match verdict {
            Verdict::RefundBuyer => {
                payload.push_back(0);
                payload.extend_from_array(&0u32.to_be_bytes());
            }
            Verdict::ReleaseSeller => {
                payload.push_back(1);
                payload.extend_from_array(&0u32.to_be_bytes());
            }
            Verdict::Split(bps) => {
                payload.push_back(2);
                payload.extend_from_array(&bps.to_be_bytes());
            }
        }
        payload
    }

    /// protocol.md T8. Either party cancels before delivery. Any funds return
    /// to the buyer. `caller` is authenticated and must be a party; the PRD
    /// signature omits it, but authenticating "either party" needs to know who.
    pub fn cancel(env: Env, escrow_id: u64, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        let mut e = Self::load(&env, escrow_id)?;
        if caller != e.buyer && caller != e.seller {
            return Err(Error::NotAuthorized);
        }
        match e.state {
            State::Created => {}
            State::Funded => Self::pay(&env, &e, &e.buyer, e.amount),
            _ => return Err(Error::InvalidState),
        }
        e.state = State::Refunded;
        Self::put(&env, &e);
        events::cancelled(&env, escrow_id);
        Ok(())
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage().persistent().get(&DataKey::Escrow(escrow_id))
    }

    // --- internal helpers ---

    fn pay(env: &Env, e: &Escrow, to: &Address, amount: i128) {
        let contract = env.current_contract_address();
        token::TokenClient::new(env, &e.asset).transfer(&contract, to, &amount);
    }

    fn load(env: &Env, escrow_id: u64) -> Result<Escrow, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(Error::EscrowNotFound)
    }

    fn put(env: &Env, e: &Escrow) {
        let key = DataKey::Escrow(e.id);
        env.storage().persistent().set(&key, e);
        env.storage()
            .persistent()
            .extend_ttl(&key, ESCROW_TTL_THRESHOLD, ESCROW_TTL);
    }

    fn next_id(env: &Env) -> u64 {
        let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
        env.storage().instance().set(&DataKey::NextId, &(id + 1));
        env.storage()
            .instance()
            .extend_ttl(ESCROW_TTL_THRESHOLD, ESCROW_TTL);
        id
    }
}

#[cfg(test)]
mod test;
