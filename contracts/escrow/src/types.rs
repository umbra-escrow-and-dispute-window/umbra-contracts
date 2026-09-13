use soroban_sdk::{contracttype, Address, BytesN, Vec};

/// Lifecycle state. Terminal states (Released, Refunded, ResolvedBuyer,
/// ResolvedSeller, ResolvedSplit) accept no further transitions. The normative
/// transition table lives in docs/protocol.md; code follows it.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Created = 0,
    Funded = 1,
    Delivered = 2,
    Disputed = 3,
    Released = 4,
    Refunded = 5,
    ResolvedBuyer = 6,
    ResolvedSeller = 7,
    ResolvedSplit = 8,
}

/// Who may resolve a dispute. `Panel` and `Signed` are declared so the storage
/// layout and the interface are stable when panel arbitration and the signed
/// adjudicator land; v0.1 implements only `None` and `Single` and returns
/// `Unimplemented` for the rest at resolve time.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArbiterConfig {
    /// Timeout only. dispute() is rejected because nothing could resolve it.
    None,
    Single(Address),
    Panel(Vec<Address>, u32),
    Signed(BytesN<32>),
}

/// An arbiter's decision. Amounts split with `Split` round the remainder to the
/// seller; see docs/protocol.md.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    RefundBuyer,
    ReleaseSeller,
    /// buyer_bps in 0..=10000 is the buyer's share; the seller takes the rest.
    Split(u32),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub asset: Address,
    pub amount: i128,
    pub deliver_by: u64,
    pub confirm_window: u64,
    pub dispute_window: u64,
    pub arbiter: ArbiterConfig,
    pub terms_hash: BytesN<32>,
    pub state: State,
    pub created_at: u64,
    /// Ledger time of mark_delivered; the confirm and dispute windows count
    /// from here. Zero until delivery.
    pub delivered_at: u64,
    pub proof_hash: BytesN<32>,
    pub reason_hash: BytesN<32>,
}

#[contracttype]
pub enum DataKey {
    Escrow(u64),
    NextId,
}
