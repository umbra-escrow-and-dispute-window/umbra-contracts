#![no_std]

pub mod error;
pub mod events;
pub mod types;

#[cfg(test)]
mod test;

use error::VaultError;
use soroban_sdk::{contract, contractimpl, token, Address, Env};
use types::{DataKey, LockRecord, SellerAccountSummary};

#[contract]
pub struct UmbraVaultContract;

#[contractimpl]
impl UmbraVaultContract {
    /// Initialize the vault with an administrative address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), VaultError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(VaultError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Authorize or revoke an external contract (e.g., escrow contract) to lock/slash collateral.
    pub fn set_authorized(env: Env, caller: Address, authorized: bool) -> Result<(), VaultError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(VaultError::Unauthorized)?;
        admin.require_auth();

        if authorized {
            env.storage()
                .instance()
                .set(&DataKey::AuthorizedCaller(caller), &true);
        } else {
            env.storage()
                .instance()
                .remove(&DataKey::AuthorizedCaller(caller));
        }
        Ok(())
    }

    /// Returns whether a caller is authorized to lock and slash collateral.
    pub fn is_authorized(env: Env, caller: Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::AuthorizedCaller(caller))
            .unwrap_or(false)
    }

    /// Deposit collateral into the vault.
    pub fn deposit(
        env: Env,
        seller: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::AmountMustBePositive);
        }
        seller.require_auth();

        let contract = env.current_contract_address();
        let client = token::Client::new(&env, &asset);
        client.transfer(&seller, &contract, &amount);

        let key = DataKey::SellerCollateral(seller.clone(), asset.clone());
        let current_balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new_balance = current_balance
            .checked_add(amount)
            .ok_or(VaultError::Overflow)?;

        env.storage().persistent().set(&key, &new_balance);
        events::emit_deposit(&env, &seller, &asset, amount);
        Ok(())
    }

    /// Withdraw unencumbered collateral from the vault.
    pub fn withdraw(
        env: Env,
        seller: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::AmountMustBePositive);
        }
        seller.require_auth();

        let collateral_key = DataKey::SellerCollateral(seller.clone(), asset.clone());
        let locked_key = DataKey::SellerLocked(seller.clone(), asset.clone());

        let total_collateral: i128 = env.storage().persistent().get(&collateral_key).unwrap_or(0);
        let locked_collateral: i128 = env.storage().persistent().get(&locked_key).unwrap_or(0);

        let available = total_collateral
            .checked_sub(locked_collateral)
            .ok_or(VaultError::Overflow)?;

        if amount > available {
            return Err(VaultError::InsufficientCollateral);
        }

        let new_balance = total_collateral
            .checked_sub(amount)
            .ok_or(VaultError::Overflow)?;

        if new_balance == 0 {
            env.storage().persistent().remove(&collateral_key);
        } else {
            env.storage()
                .persistent()
                .set(&collateral_key, &new_balance);
        }

        let contract = env.current_contract_address();
        let client = token::Client::new(&env, &asset);
        client.transfer(&contract, &seller, &amount);

        events::emit_withdraw(&env, &seller, &asset, amount);
        Ok(())
    }

    /// Lock seller collateral for an instant-release escrow liability.
    pub fn lock_collateral(
        env: Env,
        caller: Address,
        seller: Address,
        asset: Address,
        amount: i128,
        escrow_id: u64,
        expires_at: u64,
    ) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::AmountMustBePositive);
        }
        if escrow_id == 0 {
            return Err(VaultError::EscrowIdMustBePositive);
        }

        // Caller must be an authorized contract or the seller itself
        caller.require_auth();
        let is_auth = Self::is_authorized(env.clone(), caller.clone());
        if !is_auth && caller != seller {
            return Err(VaultError::Unauthorized);
        }

        let lock_key = DataKey::EscrowLock(seller.clone(), escrow_id);
        if env.storage().persistent().has(&lock_key) {
            return Err(VaultError::LockAlreadyExists);
        }

        let collateral_key = DataKey::SellerCollateral(seller.clone(), asset.clone());
        let locked_key = DataKey::SellerLocked(seller.clone(), asset.clone());

        let total_collateral: i128 = env.storage().persistent().get(&collateral_key).unwrap_or(0);
        let current_locked: i128 = env.storage().persistent().get(&locked_key).unwrap_or(0);

        let available = total_collateral
            .checked_sub(current_locked)
            .ok_or(VaultError::Overflow)?;

        if amount > available {
            return Err(VaultError::InsufficientCollateral);
        }

        let new_locked = current_locked
            .checked_add(amount)
            .ok_or(VaultError::Overflow)?;

        env.storage().persistent().set(&locked_key, &new_locked);
        let record = LockRecord {
            escrow_id,
            asset: asset.clone(),
            amount,
            expires_at,
        };
        env.storage().persistent().set(&lock_key, &record);

        events::emit_lock(&env, &seller, escrow_id, amount, expires_at);
        Ok(())
    }

    /// Release locked collateral after dispute window expiry.
    pub fn release_lock(
        env: Env,
        caller: Address,
        seller: Address,
        escrow_id: u64,
    ) -> Result<(), VaultError> {
        caller.require_auth();

        let lock_key = DataKey::EscrowLock(seller.clone(), escrow_id);
        let record: LockRecord = env
            .storage()
            .persistent()
            .get(&lock_key)
            .ok_or(VaultError::LockNotFound)?;

        let is_auth = Self::is_authorized(env.clone(), caller.clone());
        let now = env.ledger().timestamp();
        // Authorized contracts can unlock at any time; seller can unlock only after expires_at
        if !is_auth {
            if caller != seller {
                return Err(VaultError::Unauthorized);
            }
            if now < record.expires_at {
                return Err(VaultError::LockedLiabilityExists);
            }
        }

        let locked_key = DataKey::SellerLocked(seller.clone(), record.asset.clone());
        let current_locked: i128 = env.storage().persistent().get(&locked_key).unwrap_or(0);
        let new_locked = current_locked
            .checked_sub(record.amount)
            .ok_or(VaultError::Overflow)?;

        if new_locked == 0 {
            env.storage().persistent().remove(&locked_key);
        } else {
            env.storage().persistent().set(&locked_key, &new_locked);
        }
        env.storage().persistent().remove(&lock_key);

        events::emit_release_lock(&env, &seller, escrow_id, record.amount);
        Ok(())
    }

    /// Slash locked collateral following an arbiter dispute verdict and refund the buyer.
    pub fn slash_and_refund(
        env: Env,
        caller: Address,
        seller: Address,
        recipient: Address,
        escrow_id: u64,
        amount: i128,
    ) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::AmountMustBePositive);
        }
        caller.require_auth();

        let is_auth = Self::is_authorized(env.clone(), caller.clone());
        if !is_auth {
            return Err(VaultError::Unauthorized);
        }

        let lock_key = DataKey::EscrowLock(seller.clone(), escrow_id);
        let record: LockRecord = env
            .storage()
            .persistent()
            .get(&lock_key)
            .ok_or(VaultError::LockNotFound)?;

        if amount > record.amount {
            return Err(VaultError::InsufficientCollateral);
        }

        let collateral_key = DataKey::SellerCollateral(seller.clone(), record.asset.clone());
        let locked_key = DataKey::SellerLocked(seller.clone(), record.asset.clone());

        let total_collateral: i128 = env.storage().persistent().get(&collateral_key).unwrap_or(0);
        let current_locked: i128 = env.storage().persistent().get(&locked_key).unwrap_or(0);

        let new_collateral = total_collateral
            .checked_sub(amount)
            .ok_or(VaultError::Overflow)?;
        let new_locked = current_locked
            .checked_sub(amount)
            .ok_or(VaultError::Overflow)?;

        if new_collateral == 0 {
            env.storage().persistent().remove(&collateral_key);
        } else {
            env.storage()
                .persistent()
                .set(&collateral_key, &new_collateral);
        }

        if new_locked == 0 {
            env.storage().persistent().remove(&locked_key);
        } else {
            env.storage().persistent().set(&locked_key, &new_locked);
        }

        let remaining_lock_amount = record
            .amount
            .checked_sub(amount)
            .ok_or(VaultError::Overflow)?;
        if remaining_lock_amount == 0 {
            env.storage().persistent().remove(&lock_key);
        } else {
            let updated_record = LockRecord {
                amount: remaining_lock_amount,
                ..record.clone()
            };
            env.storage().persistent().set(&lock_key, &updated_record);
        }

        let contract = env.current_contract_address();
        let client = token::Client::new(&env, &record.asset);
        client.transfer(&contract, &recipient, &amount);

        events::emit_slash(&env, &seller, &recipient, escrow_id, amount);
        Ok(())
    }

    /// Retrieve summary of a seller's collateral position for a given asset.
    pub fn get_summary(env: Env, seller: Address, asset: Address) -> SellerAccountSummary {
        let collateral_key = DataKey::SellerCollateral(seller.clone(), asset.clone());
        let locked_key = DataKey::SellerLocked(seller, asset);

        let total_collateral: i128 = env.storage().persistent().get(&collateral_key).unwrap_or(0);
        let locked_collateral: i128 = env.storage().persistent().get(&locked_key).unwrap_or(0);
        let available_collateral = total_collateral.saturating_sub(locked_collateral);

        SellerAccountSummary {
            total_collateral,
            locked_collateral,
            available_collateral,
        }
    }

    /// Retrieve an individual escrow lock record.
    pub fn get_lock(env: Env, seller: Address, escrow_id: u64) -> Option<LockRecord> {
        let lock_key = DataKey::EscrowLock(seller, escrow_id);
        env.storage().persistent().get(&lock_key)
    }
}
