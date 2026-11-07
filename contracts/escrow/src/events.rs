use soroban_sdk::{contractevent, symbol_short, Address, Env, Symbol};

use crate::types::State;

/// One event per transition, so an indexer can rebuild an escrow's history
/// without replaying calls. `kind` names the transition and `escrow` is the id;
/// both are topics so a consumer can subscribe to one escrow or one kind.
#[contractevent(topics = ["umbra"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    #[topic]
    pub kind: Symbol,
    #[topic]
    pub escrow: u64,
    pub state: State,
}

/// Astrolabe dispute and terminal settlement attestation record event.
/// Topics: ["astrolabe", "record", escrow_id]
/// Data: AttestationRecord { buyer, seller, outcome, amount_bucket }
#[contractevent(topics = ["astrolabe", "record"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationRecord {
    #[topic]
    pub escrow: u64,
    pub buyer: Address,
    pub seller: Address,
    pub outcome: State,
    pub amount_bucket: u32,
}

pub fn compute_amount_bucket(amount: i128) -> u32 {
    if amount <= 100_000_000 {
        0
    } else if amount <= 1_000_000_000 {
        1
    } else if amount <= 10_000_000_000 {
        2
    } else if amount <= 100_000_000_000 {
        3
    } else {
        4
    }
}

pub fn attestation(
    env: &Env,
    escrow: u64,
    buyer: Address,
    seller: Address,
    outcome: State,
    amount: i128,
) {
    AttestationRecord {
        escrow,
        buyer,
        seller,
        outcome,
        amount_bucket: compute_amount_bucket(amount),
    }
    .publish(env);
}

fn emit(env: &Env, kind: Symbol, escrow: u64, state: State) {
    Transition {
        kind,
        escrow,
        state,
    }
    .publish(env);
}

pub fn created(env: &Env, id: u64) {
    emit(env, symbol_short!("created"), id, State::Created);
}
pub fn funded(env: &Env, id: u64) {
    emit(env, symbol_short!("funded"), id, State::Funded);
}
pub fn delivered(env: &Env, id: u64) {
    emit(env, symbol_short!("delivered"), id, State::Delivered);
}
pub fn confirmed(env: &Env, id: u64) {
    emit(env, symbol_short!("confirmed"), id, State::Released);
}
pub fn timed_out(env: &Env, id: u64) {
    emit(env, symbol_short!("timeout"), id, State::Released);
}
pub fn disputed(env: &Env, id: u64) {
    emit(env, symbol_short!("disputed"), id, State::Disputed);
}
pub fn resolved(env: &Env, id: u64, state: State) {
    emit(env, symbol_short!("resolved"), id, state);
}
pub fn cancelled(env: &Env, id: u64) {
    emit(env, symbol_short!("cancelled"), id, State::Refunded);
}
