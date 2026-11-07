# Umbra escrow protocol

This is the normative specification of the escrow state machine. The contract in
`contracts/escrow` implements it, and every test names the row it exercises. If
the code and this document disagree, one of them is a bug; say which in the pull
request.

## Actors

- **Buyer** funds the escrow and either confirms delivery or disputes it.
- **Seller** delivers the work and is paid on confirmation or timeout.
- **Arbiter** resolves a dispute. Chosen by both parties at creation. In v0.1
  only a single named arbiter is implemented.

## States

```
Created -> Funded -> Delivered -> Released
                          |            (via confirm or claim_timeout)
                          +-> Disputed -> ResolvedBuyer   (refund)
                          |            -> ResolvedSeller  (release)
                          |            -> ResolvedSplit   (split)
                          +-> (cancel before delivery) -> Refunded
```

`Released`, `Refunded`, `ResolvedBuyer`, `ResolvedSeller` and `ResolvedSplit` are
terminal. No transition leaves a terminal state.

## Transition table

Every entry point checks the current state first and returns `InvalidState`
(code 3) if the escrow is not in a listed `from` state. Authorisation is by
Soroban `require_auth` on the named actor; a call missing that signature fails
before any state change.

| ID | Call | From | To | Actor | Precondition | Effect |
|----|------|------|----|-------|--------------|--------|
| T1 | `create_escrow` | (none) | Created | buyer | `amount > 0`; `confirm_window > 0`; `dispute_window > 0`; `confirm_window >= dispute_window` | Stores terms; assigns id. |
| T2 | `fund` | Created | Funded | buyer | buyer signs | Transfers `amount` of `asset` from buyer into the contract. |
| T3 | `mark_delivered` | Funded | Delivered | seller | seller signs | Stores `proof_hash`; sets `delivered_at` to ledger time. Confirm and dispute windows start here. |
| T4 | `confirm` | Delivered | Released | buyer | buyer signs | Pays `amount` to seller. |
| T5 | `claim_timeout` | Delivered | Released | seller | `now >= delivered_at + confirm_window` | Pays `amount` to seller. |
| T6 | `dispute` | Delivered | Disputed | buyer | arbiter is not `None`; `now <= delivered_at + dispute_window` | Stores `reason_hash`. |
| T7 | `resolve` | Disputed | ResolvedBuyer / ResolvedSeller / ResolvedSplit | arbiter | arbiter signs | Distributes per verdict. |
| T8 | `cancel` | Created or Funded | Refunded | buyer or seller | caller is a party | Refunds buyer if funds are held. |

`get_escrow` reads and never transitions.

## Time

Every deadline is computed from the ledger timestamp (`env.ledger().timestamp()`),
never from a value supplied by a caller. The confirm and dispute windows are both
measured from `delivered_at`, the ledger time of T3.

`claim_timeout` (T5) requires the confirm window to have elapsed. `dispute` (T6)
requires the dispute window not to have elapsed. If `confirm_window` is shorter
than `dispute_window`, a seller can claim payment while the buyer could still
have disputed; parties choose both windows at creation and are expected to set
`dispute_window <= confirm_window` when the dispute path must stay open until the
seller can claim. The reversible-after-release case is out of scope for v0.1 and
is what the collateral and clawback path (M6) will address.

## Verdicts and rounding

`resolve` takes a `Verdict`:

- `RefundBuyer` pays the full `amount` to the buyer.
- `ReleaseSeller` pays the full `amount` to the seller.
- `Split(buyer_bps)` where `buyer_bps` is in `0..=10000`. The buyer's share is
  `amount * buyer_bps / 10000` using integer division, so it is floored. The
  seller receives `amount - buyer_share`. **The rounded-off stroop always goes to
  the seller.** For `amount = 1000` and `buyer_bps = 3333`, the buyer receives
  333 and the seller receives 667. `buyer_bps > 10000` returns `InvalidVerdict`.

All amount arithmetic is checked; an overflow returns `Overflow` rather than
wrapping.

## Arbiter configuration

`ArbiterConfig` is one of:

- `None` — timeout only. `dispute` is rejected with `ArbiterRequired`, because no
  one could resolve it.
- `Single(Address)` — one named arbiter. Implemented.
- `Panel(Vec<Address>, threshold)` — multi-party panel arbitration. Implemented. Requires at least `threshold` distinct member signatures.
- `Signed(BytesN<32>)` — an off-chain signed verdict from a known key, for the
  automated adjudicator. Requires `signature: Option<BytesN<64>>` containing an
  Ed25519 signature over the canonical 32-byte payload:
  - Bytes 0..19: domain prefix `umbra:adjudicate:v1`
  - Bytes 19..27: `escrow_id` as u64 big-endian
  - Byte 27: verdict code (`0` = `RefundBuyer`, `1` = `ReleaseSeller`, `2` = `Split`)
  - Bytes 28..32: `buyer_bps` as u32 big-endian (`0` for non-split verdicts)
  A companion entry point `resolve_signed` is provided.

The `Panel` and `Signed` variants exist in the type and the storage layout now so
that adding them later is not a breaking change.

## Errors

| Code | Name | Meaning |
|------|------|---------|
| 1 | EscrowNotFound | No escrow with that id. |
| 2 | NotAuthorized | Caller is not a party to this escrow. |
| 3 | InvalidState | Call is not legal from the current state. |
| 4 | InvalidAmount | `amount <= 0` at creation. |
| 5 | InvalidWindow | A window was zero at creation. |
| 6 | ArbiterRequired | Dispute attempted on an escrow with no arbiter. |
| 7 | Unimplemented | Arbiter variant declared but not implemented in v0.1. |
| 8 | DisputeWindowClosed | Dispute attempted after the window closed. |
| 9 | ConfirmWindowOpen | Timeout claimed before the confirm window elapsed. |
| 10 | InvalidVerdict | `Split` with `buyer_bps > 10000`. |
| 11 | Overflow | Checked arithmetic overflowed. |
| 12 | InvalidWindowOrdering | `confirm_window < dispute_window` at creation. |
| 13 | BelowThreshold | Number of authorizing panel signers is below threshold. |
| 14 | DuplicateSigner | Duplicate address in panel configuration or signers list. |
| 15 | InvalidThreshold | Panel threshold is zero or exceeds member count. |

## Deviation from the PRD signature

- `cancel` takes an extra `caller: Address` argument not shown in PRD section 4.2.
  Authenticating "either party" requires knowing which party is calling; the extra
  argument is that party, and it is authenticated and checked against the escrow.
- `resolve` takes `signers: Option<Vec<Address>>` for `Panel` arbitration and
  `signature: Option<BytesN<64>>` for `Signed` adjudicators. Companion entry points
  `resolve_panel` and `resolve_signed` are also provided.

## Events

Every transition publishes one `Transition` event with topics `("umbra", kind,
escrow_id)` and the resulting state as data, so an indexer can rebuild history
without replaying calls.
