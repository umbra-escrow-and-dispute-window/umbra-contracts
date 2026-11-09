use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["vault", "deposit"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultDepositEvent {
    #[topic]
    pub seller: Address,
    pub asset: Address,
    pub amount: i128,
}

#[contractevent(topics = ["vault", "withdraw"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultWithdrawEvent {
    #[topic]
    pub seller: Address,
    pub asset: Address,
    pub amount: i128,
}

#[contractevent(topics = ["vault", "lock"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultLockEvent {
    #[topic]
    pub seller: Address,
    pub escrow_id: u64,
    pub amount: i128,
    pub expires_at: u64,
}

#[contractevent(topics = ["vault", "unlock"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultUnlockEvent {
    #[topic]
    pub seller: Address,
    pub escrow_id: u64,
    pub amount: i128,
}

#[contractevent(topics = ["vault", "slash"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultSlashEvent {
    #[topic]
    pub seller: Address,
    pub recipient: Address,
    pub escrow_id: u64,
    pub amount: i128,
}

pub fn emit_deposit(env: &Env, seller: &Address, asset: &Address, amount: i128) {
    VaultDepositEvent {
        seller: seller.clone(),
        asset: asset.clone(),
        amount,
    }
    .publish(env);
}

pub fn emit_withdraw(env: &Env, seller: &Address, asset: &Address, amount: i128) {
    VaultWithdrawEvent {
        seller: seller.clone(),
        asset: asset.clone(),
        amount,
    }
    .publish(env);
}

pub fn emit_lock(env: &Env, seller: &Address, escrow_id: u64, amount: i128, expires_at: u64) {
    VaultLockEvent {
        seller: seller.clone(),
        escrow_id,
        amount,
        expires_at,
    }
    .publish(env);
}

pub fn emit_release_lock(env: &Env, seller: &Address, escrow_id: u64, amount: i128) {
    VaultUnlockEvent {
        seller: seller.clone(),
        escrow_id,
        amount,
    }
    .publish(env);
}

pub fn emit_slash(env: &Env, seller: &Address, recipient: &Address, escrow_id: u64, amount: i128) {
    VaultSlashEvent {
        seller: seller.clone(),
        recipient: recipient.clone(),
        escrow_id,
        amount,
    }
    .publish(env);
}
