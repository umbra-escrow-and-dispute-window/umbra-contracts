# Umbra threat model

Scope is the v0.1 escrow contract: `create_escrow`, `fund`, `mark_delivered`,
`confirm`, `claim_timeout`, `dispute`, `resolve`, `cancel`. The collateral vault,
the `UMBR` receipt asset, and clawback settlement are not built yet; where they
change a conclusion below, that is called out.

A pull request that changes an authorisation check must state which item here it
affects.

## Assets at risk

The escrowed `amount` of `asset`, held by the contract between `fund` and a
terminal state. Nothing else. The contract never holds a key, never custodies an
asset it was not funded with, and cannot move funds except along the transition
table in `docs/protocol.md`.

## Arbiter capture

The arbiter decides who gets the money in a dispute, so a compromised or biased
arbiter is the largest single risk. Mitigations in v0.1:

- The arbiter is chosen per escrow by the buyer at creation and is visible in the
  escrow terms before funding. A buyer who does not trust the arbiter does not
  fund.
- `resolve` requires the arbiter's signature; no one else can resolve.
- Every resolution emits an event, so verdicts are public and attributable.
- Panel arbitration (`ArbiterConfig::Panel(members, threshold)`) requires `threshold`
  distinct member signatures to resolve, eliminating any single arbiter as a single point of failure.

## Griefing by disputes

A buyer who disputes in bad faith freezes the seller's payment until the arbiter
resolves. Mitigations:

- Dispute records are public and attach to the buyer as much as the seller, so a
  serial disputer builds a visible record.
- `dispute` is only legal inside the dispute window and only in `Delivered`, so a
  buyer cannot dispute before delivery or reopen a settled escrow.

Residual risk: there is no bond required to dispute in v0.1, so disputing is
cheap. A dispute bond is a candidate mitigation and is left as an open issue.

## Timeout races

`claim_timeout` (seller) and `dispute` (buyer) both act on a `Delivered` escrow,
and their windows can overlap. The contract resolves the race by state, not by
intent: whichever call lands first moves the escrow out of `Delivered`, and the
other then fails with `InvalidState`. Because both windows are measured from the
same `delivered_at` using ledger time, neither party can manufacture a favourable
clock. Parties who need the dispute path to stay open until the seller can claim
set `dispute_window <= confirm_window`; `docs/protocol.md` states this.

The remaining sharp edge is a `confirm_window` shorter than `dispute_window`,
which lets a seller claim while a dispute was still nominally allowed. This is a
configuration choice, not a contract flaw, but the SDK should warn on it and the
app should not offer it by default. Tracked in `ISSUES.md`.

## Time source

All deadlines use `env.ledger().timestamp()`. No entry point accepts a
caller-supplied time. This is the single most important invariant for the timeout
and dispute logic; a change that introduced a client-supplied timestamp would
break both windows and must be rejected.

## Reentrancy and arithmetic

Token transfers are the only external calls. State is written after the transfer
in the paying transitions, but each transition first checks and leaves a terminal
or advanced state, so a reentrant call re-enters into a state that rejects it.
All amount arithmetic is checked; the split calculation cannot overflow for any
`amount` and `buyer_bps <= 10000`.

## What clawback will and will not do when it arrives

Today, reversal is only possible before release, because the contract still holds
the funds. The harder case is a seller who was already paid and a dispute upheld
afterwards. That is what the `UMBR` receipt asset and clawback (M6) address, and
its limits are deliberate:

- Clawback will only ever affect Umbra's own `UMBR` receipt asset, never a user's
  other holdings. The receipt carries no value of its own; it represents the
  reversible portion for the length of the window.
- Clawback will be time-bounded to the window both parties agreed to at creation.
  After the window closes the receipt is burned and the payment is final.
- Clawback will be opt-in per escrow. A seller who posts no collateral receives no
  receipt and offers no post-release reversal, and buyers can see that from the
  terms.

**Never widen clawback beyond Umbra's own receipt asset inside an agreed window.**
This is the invariant the whole project's credibility rests on. Any change that
touched it would be a different, and far more dangerous, product.
