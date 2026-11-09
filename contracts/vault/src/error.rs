use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    AmountMustBePositive = 1,
    InsufficientCollateral = 2,
    LockedLiabilityExists = 3,
    LockNotFound = 4,
    LockAlreadyExists = 5,
    LockExpired = 6,
    Unauthorized = 7,
    Overflow = 8,
    EscrowIdMustBePositive = 9,
}
