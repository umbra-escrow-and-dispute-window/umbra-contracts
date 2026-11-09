# Collateral Vault Specification & Integration Guide

`contracts/vault` provides seller collateral custody and instant-release liability management for the Umbra protocol.

## 1. Overview & Architecture

Instant-release escrows pay sellers immediately upon milestone delivery. To preserve dispute protection, reversals are backed by seller collateral locked inside `contracts/vault`.

```
[Seller] --deposit--> [UmbraVaultContract] <--lock_collateral-- [UmbraEscrowContract]
                            |
           +----------------+----------------+
           | (Dispute Upheld)                | (Dispute Window Expires)
           v                                 v
   [slash_and_refund]                 [release_lock]
   Buyer refunded from collateral     Seller unencumbered balance restored
```

## 2. Public Entry Points

### Administration & Authorization
- `initialize(admin: Address)`: Configures the administrative key.
- `set_authorized(caller: Address, authorized: bool)`: Authorizes external contracts (such as `umbra-escrow`) to lock and slash collateral.
- `is_authorized(caller: Address) -> bool`: Checks authorization status.

### Seller Collateral Operations
- `deposit(seller: Address, asset: Address, amount: i128)`: Transfers `amount` of `asset` from seller to vault. Increases total and available collateral.
- `withdraw(seller: Address, asset: Address, amount: i128)`: Withdraws unencumbered collateral (`amount <= available`). Transfers tokens back to seller.
- `get_summary(seller: Address, asset: Address) -> SellerAccountSummary`: Returns `total_collateral`, `locked_collateral`, and `available_collateral`.

### Escrow Liability Locks
- `lock_collateral(caller, seller, asset, amount, escrow_id, expires_at)`: Locks `amount` of seller's available collateral for a given `escrow_id` until `expires_at`. Callable by authorized contracts or seller.
- `release_lock(caller, seller, escrow_id)`: Frees the lock. Callable by authorized contracts at any time, or by seller after `expires_at`.
- `slash_and_refund(caller, seller, recipient, escrow_id, amount)`: Authorized-only. Slashes up to `amount` from the locked collateral and transfers tokens directly to `recipient` (e.g. buyer). Decreases total and locked collateral.

## 3. Storage Layout & Invariants

1. **Conservation Invariant**: At all times, the vault contract's token balance for any asset equals the sum of `total_collateral` across all sellers holding that asset.
2. **Availability Invariant**: `available_collateral = total_collateral - locked_collateral`. A withdrawal can never exceed `available_collateral`.
3. **Lock Exclusivity**: An `escrow_id` lock record is unique per seller. Attempting to duplicate returns `LockAlreadyExists`.
