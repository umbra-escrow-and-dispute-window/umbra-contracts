# Open issues

The work left in this repository after v0.1, ordered easiest to hardest. Each has
acceptance criteria and a difficulty label matching the scheme in
`CONTRIBUTING.md`: `good first issue`, `intermediate`, `advanced`. This list and
the "Where to start" section of `CONTRIBUTING.md` must agree; if they drift, one
is wrong.

Milestone tags (M6..M10) refer to `docs` in the SDK and app repositories' shared
PRD.

---

## 1. Warn on a confirm window shorter than the dispute window (Resolved)
Label: good first issue

Resolved: `create_escrow` enforces `confirm_window >= dispute_window` and returns
`Error::InvalidWindowOrdering` (code 12) if violated. Covered by tests and documented in
`docs/protocol.md`.

## 2. Document and test the event schema for indexers (Resolved)
Label: good first issue

Resolved: `docs/events.md` documents all transition event topics and data layouts.
Integration tests in `contracts/escrow/src/test.rs` assert event emissions across
happy, dispute, timeout, and cancellation paths.

## 3. Property tests for split rounding (Resolved)
Label: intermediate

Resolved: `property_split_rounding_invariants_hold_across_ranges` and
`property_split_contract_execution_conserves_balances` assert invariant preservation
across wide amount and basis point ranges without panics.

## 4. Dispute bond to deter griefing (Resolved)
Label: intermediate

Resolved: `create_escrow` accepts `dispute_bond: i128` (validating `dispute_bond >= 0`). `dispute` transfers
the bond from buyer to contract. `resolve` distributes the bond according to the verdict: refunded to buyer
on `RefundBuyer`, forfeited to seller on `ReleaseSeller`, or split per basis points. Documented in `docs/protocol.md`
and `docs/threat-model.md`, and covered by unit tests.

## 5. Astrolabe attestation on terminal states (Resolved)
Label: intermediate

Resolved: On reaching any terminal state (`Released`, `Refunded`, `ResolvedBuyer`, `ResolvedSeller`, `ResolvedSplit`),
the contract emits an `AttestationRecord` event under topics `["astrolabe", "record", escrow_id]` capturing buyer,
seller, outcome state, and logarithmic amount bucket. Documented in `docs/events.md` and verified with unit tests.

## 6. Mainnet configuration and release checklist (Resolved)
Label: intermediate

Resolved: Authored `docs/mainnet-checklist.md` detailing pre-deployment audit, reproducibility, key management,
and cross-repo sync steps, and added `scripts/deploy_mainnet.sh` gated behind `ALLOW_MAINNET=1` and
`CONFIRM_MAINNET_CHECKLIST=1`.

## 7. Panel arbitration with threshold signatures (Resolved)
Label: advanced

Resolved: `ArbiterConfig::Panel(members, threshold)` enforces threshold requirements and
rejects duplicate signers or unauthorized callers. Covered by legal- and illegal-path unit tests,
and documented in `docs/protocol.md` and `docs/threat-model.md`.

## 8. Signed adjudicator verdicts (Resolved)
Label: advanced

Resolved: `resolve` verifies Ed25519 signatures over the canonical 32-byte payload format
`umbra:adjudicate:v1 || escrow_id || verdict_tag || buyer_bps`. Unit tests verify valid signatures,
unauthorized callers without signatures, wrong public keys, and tampered verdicts. Exposed companion
entry point `resolve_signed`. Documented in `docs/protocol.md`.

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
