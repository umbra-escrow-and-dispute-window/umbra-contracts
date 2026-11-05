use soroban_sdk::contracterror;

/// Every reachable failure returns one of these codes. Codes are stable across
/// releases so the SDK and the app can branch on them; do not renumber.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EscrowNotFound = 1,
    NotAuthorized = 2,
    InvalidState = 3,
    InvalidAmount = 4,
    InvalidWindow = 5,
    /// `dispute()` was called on an escrow whose arbiter is `None`, so no one
    /// could ever resolve it.
    ArbiterRequired = 6,
    /// The requested arbiter variant is declared but not implemented in v0.1.
    Unimplemented = 7,
    /// `dispute()` after the dispute window closed.
    DisputeWindowClosed = 8,
    /// `claim_timeout()` before the confirm window elapsed.
    ConfirmWindowOpen = 9,
    /// Split verdict with `buyer_bps` outside 0..=10000.
    InvalidVerdict = 10,
    Overflow = 11,
}
