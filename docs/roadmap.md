# Umbra Protocol Roadmap

This document tracks the evolution of `umbra-contracts` from its initial state-machine baseline (v0.1) through full multi-mode arbitration and anti-griefing (v0.2) to post-release clawback settlement (v0.3).

---

## Completed in v0.1 — Core Escrow & Dispute Window

- **8-Transition State Machine**: `create_escrow`, `fund`, `mark_delivered`, `confirm`, `claim_timeout`, `dispute`, `resolve`, and `cancel`.
- **Single-Arbiter Resolution**: `ArbiterConfig::Single(Address)` with `RefundBuyer`, `ReleaseSeller`, and `Split(buyer_bps)`.
- **Off-Chain Hash Anchoring**: 32-byte SHA-256 digests for `terms_hash`, `proof_hash`, and `reason_hash`.
- **Stellar Testnet Deployment**: Reproducible WASM build and `deployments/testnet.json` synchronisation with `umbra-sdk` and `umbra-app`.

---

## Completed in v0.2 — Multi-Mode Arbitration, Bonds & Attestations

- **Window Ordering Invariant (Issue 1)**: Enforces `confirm_window >= dispute_window` at creation (`Error::InvalidWindowOrdering`).
- **Event Schema & Indexer Support (Issue 2)**: Normative specification in `docs/events.md` and integration test coverage for all `Transition` events.
- **Split Rounding Property Suite (Issue 3)**: Mathematical invariant and contract conservation tests across amounts and basis-point boundaries.
- **Dispute Bonds (Issue 4)**: Optional per-escrow `dispute_bond` posted by the buyer on `dispute` and refunded, forfeited, or split on `resolve`.
- **Astrolabe Terminal Attestations (Issue 5)**: Emits `AttestationRecord` under `["astrolabe", "record", escrow_id]` on every terminal outcome with logarithmic volume buckets.
- **Mainnet Gate & Checklist (Issue 6)**: `docs/mainnet-checklist.md` and flag-gated `scripts/deploy_mainnet.sh`.
- **Panel Arbitration (Issue 7)**: `ArbiterConfig::Panel(members, threshold)` requiring `threshold` distinct member signatures via `resolve` / `resolve_panel`.
- **Signed Adjudicator Verdicts (Issue 8)**: `ArbiterConfig::Signed(public_key)` verifying Ed25519 signatures over domain-separated 32-byte payloads (`umbra:adjudicate:v1`) via `resolve` / `resolve_signed`.

---

## Planned for v0.3 — Post-Release Reversal & Collateral Vault (~10% Remaining)

### 1. Collateral Vault, `UMBR` Receipt Asset, and Clawback Settlement (Issue 9 / M6)

In v0.2, an escrow can only be disputed while funds remain locked inside the contract (`confirm_window >= dispute_window`). Some high-throughput machine-to-machine workflows require immediate settlement upon delivery while retaining a post-release dispute window.

#### Architectural Invariants
Per `docs/threat-model.md` ("What clawback will and will not do"), post-release reversal must obey three non-negotiable constraints:
1. **Scoped to `UMBR` Only**: Clawback applies exclusively to Umbra's own `UMBR` receipt asset issued with `AUTH_CLAWBACK_ENABLED`. It never touches external Stellar assets (such as USDC or XLM) in the seller's wallet.
2. **Time-Bounded**: Reversal is only callable before `delivered_at + dispute_window` elapses.
3. **Explicit Seller Opt-In**: A seller opts in by staking collateral into the Umbra Collateral Vault prior to accepting instant-release escrows.

#### Planned Components
- **`contracts/vault`**: Seller collateral vault tracking staked balance, active window liabilities, and slashable reserves.
- **`UMBR` Receipt Asset**: Stellar Asset Contract configured with `AUTH_CLAWBACK_ENABLED` where the escrow contract acts as the clawback authority during the dispute window.
- **`resolve_post_release` Entry Point**: When a dispute is upheld after an early release, the contract claws back the `UMBR` receipt token and draws the refund from the seller's locked collateral in the vault.

### 2. Economic Analysis of Collateral Sizing (Issue 10 / M9)

Before lifting the per-seller window cap in the collateral vault, a formal model in `docs/` will specify:
- Minimum collateral-to-volume ratios (`C >= k * V_open`) as a function of `dispute_window` length and historical Astrolabe dispute rates.
- Liquidation and top-up thresholds when a seller runs concurrent escrows against a shared collateral pool.
