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
    /// Retained for error-code stability across releases (v0.1 returned this
    /// for `Panel` and `Signed` before both were implemented in v0.2).
    Unimplemented = 7,
    /// `dispute()` after the dispute window closed.
    DisputeWindowClosed = 8,
    /// `claim_timeout()` before the confirm window elapsed.
    ConfirmWindowOpen = 9,
    /// Split verdict with `buyer_bps` outside 0..=10000.
    InvalidVerdict = 10,
    Overflow = 11,
    /// `confirm_window` is shorter than `dispute_window`, which would create a timeout race footgun.
    InvalidWindowOrdering = 12,
    /// Number of authorizing panel signers is below the required threshold.
    BelowThreshold = 13,
    /// Duplicate signer encountered in panel configuration or authorization list.
    DuplicateSigner = 14,
    /// Panel threshold is zero or exceeds the number of panel members.
    InvalidThreshold = 15,
}
