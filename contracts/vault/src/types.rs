use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    AuthorizedCaller(Address),
    SellerCollateral(Address, Address), // (Seller, Asset) -> i128
    SellerLocked(Address, Address),     // (Seller, Asset) -> i128
    EscrowLock(Address, u64),           // (Seller, EscrowId) -> LockRecord
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockRecord {
    pub escrow_id: u64,
    pub asset: Address,
    pub amount: i128,
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerAccountSummary {
    pub total_collateral: i128,
    pub locked_collateral: i128,
    pub available_collateral: i128,
}
