# Umbra Event Schema

This document specifies the event topics and data payloads emitted by the Umbra escrow contract. Every state machine transition publishes exactly one `Transition` event so that off-chain indexers, explorers, and the `umbra-app` can reconstruct escrow history without replaying ledger transactions.

---

## Event Structure

All transition events use the Soroban contract event structure with `topics` and `data`:

```rust
#[contractevent(topics = ["umbra"])]
pub struct Transition {
    #[topic]
    pub kind: Symbol,
    #[topic]
    pub escrow: u64,
    pub state: State,
}
```

### Topics Layout

| Topic Index | Field | Type | Description |
|---|---|---|---|
| 0 | Protocol namespace | `Symbol` | Always `"umbra"`. Used to filter all Umbra events. |
| 1 | `kind` | `Symbol` | The transition action name (see Transition Table below). |
| 2 | `escrow` | `u64` | The unique numeric ID of the escrow instance. |

### Data Payload

| Field | Type | Description |
|---|---|---|
| `state` | `State` | The new lifecycle state resulting from the transition. |

---

## Lifecycle State Values

`State` is serialised as an integer discriminant matching `contracts/escrow/src/types.rs`:

| Value | Identifier | State Type | Description |
|---|---|---|---|
| `0` | `Created` | Intermediate | Terms agreed, awaiting funding. |
| `1` | `Funded` | Intermediate | Buyer funds held in escrow. |
| `2` | `Delivered` | Intermediate | Seller delivered work; windows running. |
| `3` | `Disputed` | Intermediate | Buyer disputed; awaiting arbiter resolution. |
| `4` | `Released` | Terminal | Funds paid to seller (via confirm or timeout). |
| `5` | `Refunded` | Terminal | Funds returned to buyer (via cancel). |
| `6` | `ResolvedBuyer` | Terminal | Arbiter resolved in buyer's favour (full refund). |
| `7` | `ResolvedSeller` | Terminal | Arbiter resolved in seller's favour (full release). |
| `8` | `ResolvedSplit` | Terminal | Arbiter resolved with split between parties. |

---

## Transition Events Table

| Transition | Call | `kind` (Topic 1) | Resulting `state` (Data) | Description |
|---|---|---|---|---|
| T1 | `create_escrow` | `created` | `Created` (0) | Escrow terms stored and ID assigned. |
| T2 | `fund` | `funded` | `Funded` (1) | Buyer transferred asset into contract. |
| T3 | `mark_delivered` | `delivered` | `Delivered` (2) | Seller submitted proof hash; clocks started. |
| T4 | `confirm` | `confirmed` | `Released` (4) | Buyer released funds to seller. |
| T5 | `claim_timeout` | `timeout` | `Released` (4) | Seller claimed payment after confirm window. |
| T6 | `dispute` | `disputed` | `Disputed` (3) | Buyer initiated dispute with reason hash. |
| T7 | `resolve` | `resolved` | `ResolvedBuyer` (6) / `ResolvedSeller` (7) / `ResolvedSplit` (8) | Arbiter delivered verdict. |
| T8 | `cancel` | `cancelled` | `Refunded` (5) | Escrow cancelled before delivery; buyer refunded if funded. |

---

## Indexer Subscription Recipes

### 1. Track All Events For a Specific Escrow
Filter topics: `["umbra", *, <escrow_id>]`

### 2. Track All Open Disputes
Filter topics: `["umbra", "disputed", *]`

### 3. Track All Terminal Resolutions
Filter topics: `["umbra", "resolved", *]`
