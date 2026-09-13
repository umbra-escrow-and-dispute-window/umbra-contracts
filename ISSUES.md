# Open issues

The work left in this repository after v0.1, ordered easiest to hardest. Each has
acceptance criteria and a difficulty label matching the scheme in
`CONTRIBUTING.md`: `good first issue`, `intermediate`, `advanced`. This list and
the "Where to start" section of `CONTRIBUTING.md` must agree; if they drift, one
is wrong.

Milestone tags (M6..M10) refer to `docs` in the SDK and app repositories' shared
PRD.

---

## 1. Warn on a confirm window shorter than the dispute window
Label: good first issue

`create_escrow` accepts any positive windows, but a `confirm_window` shorter than
`dispute_window` lets a seller claim payment while the buyer could still dispute
(`docs/threat-model.md`, "Timeout races"). The contract should not silently allow
the footgun.

Acceptance: `create_escrow` returns a distinct, documented error when
`confirm_window < dispute_window`, or a design note in `docs/protocol.md` argues
why it is allowed and the SDK carries the warning instead. A test covers the
chosen behaviour.

## 2. Document and test the event schema for indexers
Label: good first issue

Every transition emits a `Transition` event, but there is no consumer-facing
description of the topic and data layout.

Acceptance: `docs/events.md` describes each event's topics and data; a test asserts
the emitted events for a full happy path and a full dispute path match the doc.

## 3. Property tests for split rounding
Label: intermediate

The split verdict floors the buyer's share and gives the remainder to the seller.
Only two example amounts are tested.

Acceptance: a property test asserts, for a range of amounts and `buyer_bps`, that
`buyer_share + seller_share == amount` and `buyer_share == amount * buyer_bps /
10000`, with no panics. Reference `docs/protocol.md`, "Verdicts and rounding".

## 4. Dispute bond to deter griefing
Label: intermediate

Disputing is free in v0.1, so a buyer can dispute in bad faith at no cost
(`docs/threat-model.md`, "Griefing by disputes").

Acceptance: an optional per-escrow dispute bond, posted on `dispute` and forfeited
or returned per verdict, with legal- and illegal-path tests and a threat-model
update. The bond amount is part of the escrow terms.

## 5. Astrolabe attestation on terminal states
Label: intermediate

Every terminal state should write an `agent-dispute-record` attestation so an
agent marketplace can rank by dispute rate (PRD section 4.4).

Acceptance: on reaching a terminal state the contract writes an attestation with
escrow id, counterparty, outcome, and amount bucket; integration test on Testnet;
depends on the Astrolabe schema being available.

## 6. Mainnet configuration and release checklist
Label: intermediate

The repository is Testnet-only. Mainnet needs a deliberate, reviewed switch.

Acceptance: a documented mainnet deploy path gated behind an explicit flag, a
release checklist in `docs`, and a README change that removes "Testnet-only" only
when the maintainers decide. No mainnet deploy happens as part of this issue.

## 7. Panel arbitration with threshold signatures
Label: advanced

`ArbiterConfig::Panel(members, threshold)` is declared but `resolve` returns
`Unimplemented`. Implement resolution requiring `threshold` distinct member
signatures (M7).

Acceptance: `resolve` accepts a panel verdict when at least `threshold` members
authorise; rejects below threshold and duplicate signers; legal- and
illegal-path tests including the unauthorised variant; `docs/protocol.md` and
`docs/threat-model.md` updated for the reduced arbiter-capture risk.

## 8. Signed adjudicator verdicts
Label: advanced

`ArbiterConfig::Signed(key)` is declared but `resolve` returns `Unimplemented`.
Implement verification of an off-chain verdict signed by the configured key, so an
automated adjudicator can plug in (PRD section 4.5).

Acceptance: `resolve` verifies the signature over a canonical verdict payload
against the configured key; rejects a wrong key or a tampered payload; the payload
canonicalisation is documented and matches what the SDK produces; full tests.

## 9. Collateral vault, UMBR receipt asset, and clawback settlement
Label: advanced — this is the single largest unclaimed piece

The post-release reversal case. A seller who was already paid, with a dispute
upheld inside the window, must be made to refund. This needs a collateral vault
the seller stakes into, the `UMBR` receipt asset with `AUTH_CLAWBACK_ENABLED`, and
the settlement path that claws back the receipt and draws on collateral (PRD
section 4.3, M6). Read `docs/threat-model.md`, "What clawback will and will not
do", first: the invariant is that clawback never reaches beyond Umbra's own
receipt asset inside the agreed window, and that constraint is not negotiable.

Acceptance: a seller can opt in by posting collateral at creation; on release the
seller receives the underlying asset plus a time-bounded `UMBR` receipt; a dispute
upheld within the window claws back the receipt and refunds the buyer from
collateral; after the window the receipt is burned and settlement is final;
exhaustive tests including the unauthorised and out-of-window paths; a threat-model
update. Open a discussion with a written proposal before implementation.

## 10. Economic analysis of collateral sizing
Label: advanced — open research question

Under-collateralised sellers make the reversal window meaningless, while
over-collateralisation prices out honest sellers. There is no model yet for how
much collateral a given escrow should require, and the window is capped until
there is one.

Acceptance: a written analysis in `docs` proposing a collateral-to-escrow ratio
(or a formula) with its assumptions and failure modes, reviewed in a discussion.
This is research, not code; it blocks removing the window cap in issue 9.
