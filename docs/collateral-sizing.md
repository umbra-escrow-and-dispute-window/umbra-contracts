# Economic Analysis of Collateral Sizing for Post-Release Reversal

## 1. Problem Statement & Incentive Landscape

In the Umbra protocol, instant-release escrows provide immediate liquidity to sellers upon milestone delivery. To preserve buyer protection, a post-release dispute window allows adjudicators to reverse disputed settlements. Because funds have already left contract custody, reversals must be backed by pre-staked seller collateral in `contracts/vault`.

This introduces an economic trade-off:
- **Under-collateralisation ($C < L_{\text{active}}$)**: Exposes buyers to default risk if an arbiter upholds a refund but the seller's unencumbered balance is insufficient to cover the verdict.
- **Over-collateralisation ($C \gg L_{\text{active}}$)**: Traps working capital, pricing out legitimate micro-agents and small enterprises from offering instant-release services.

This document establishes the formal sizing model, reserve ratio formulas, assumptions, and failure modes for the Umbra Collateral Vault.

---

## 2. Mathematical Sizing Model

### 2.1 Variables & Notation

| Symbol | Description | Units |
|---|---|---|
| $V_i$ | Principal volume of active escrow $i$ | Stroops / Asset Units |
| $T_{\text{window}, i}$ | Duration of post-release dispute window for escrow $i$ | Seconds |
| $L_{\text{active}}$ | Total concurrent active liability: $\sum_{i \in \text{Open}} V_i$ | Stroops |
| $p_{\text{dispute}}$ | Historical empirical dispute rate for seller persona | Dimensionless $[0, 1]$ |
| $\alpha_{\text{loss}}$ | Historical arbiter loss rate conditional on dispute ($P(\text{Refund} \mid \text{Dispute})$) | Dimensionless $[0, 1]$ |
| $\lambda$ | Security parameter / tail-risk multiplier | Dimensionless ($\ge 1.0$) |
| $C_{\text{min}}$ | Minimum required unencumbered collateral | Stroops |

### 2.2 Discrete Per-Escrow Staking (Strict Safety Baseline)

Under the strict per-escrow safety baseline implemented in `contracts/vault`:
$$C_{\text{locked}, i} = V_i$$

Each instant-release escrow locks 100% of its volume in unencumbered vault collateral until $T_{\text{expiry}} = \text{delivered\_at} + T_{\text{window}}$ elapses:
$$C \ge \sum_{i \in \text{Open}} V_i = L_{\text{active}}$$

This provides deterministic solvency ($P(\text{Default}) = 0$) regardless of external market shocks or dispute correlation.

### 2.3 Fractional Pooled Staking for Reputable Sellers (v0.3 Roadmap Model)

For sellers with established Astrolabe reputation records ($\ge 100$ completed escrows and dispute rate $p_{\text{dispute}} < 0.02$), the required vault collateral scales with aggregate value-at-risk:

$$C_{\text{req}} = \lambda \cdot \sqrt{\sum_{i=1}^N V_i^2} + k \cdot L_{\text{active}} \cdot p_{\text{dispute}} \cdot \alpha_{\text{loss}}$$

Where:
- $\sqrt{\sum V_i^2}$ captures sub-additive portfolio variance across non-correlated escrows.
- $k$ represents the dispute resolution latency buffer ($k = 1 + \frac{T_{\text{resolution}}}{T_{\text{window}}}$).
- $\lambda = 3.0$ provides $99.87\%$ confidence against correlated claim spikes under Chebyshev's inequality.

---

## 3. Assumptions & Boundary Conditions

1. **Asset Homogeneity**: Collateral is deposited in the exact same asset as the escrowed payment (e.g. USDC for USDC escrows). Cross-asset volatility risk is completely eliminated.
2. **Deterministic Ledger Time**: Expiration timestamps rely exclusively on `env.ledger().timestamp()`. Sizing formulas are invariant to miner time manipulation within standard Soroban consensus limits (+/- 5 seconds).
3. **Astrolabe Attribution**: Volume tiers and dispute outcomes emitted via `AttestationRecord` feed the off-chain credit evaluation that parameterizes $\lambda$ and $p_{\text{dispute}}$.

---

## 4. Failure Modes & Mitigations

| Failure Mode | Mechanism | Mitigation in Protocol |
|---|---|---|
| **Correlated Dispute Cascade** | Sybil buyer attacks or systemic seller delivery failure causes multiple simultaneous disputes. | Strict 100% per-escrow lock invariant in `lock_collateral` prevents over-allocation. |
| **Capital Lockup Exhaustion** | Long dispute windows lock seller capital, preventing new jobs. | Sellers can configure shorter dispute windows ($T_{\text{window}}$) in exchange for smaller escrow sizes. |
| **Re-entrancy During Slashing** | Malicious recipient attempts to drain vault during transfer callback. | `UmbraVaultContract` updates internal balance storage **before** invoking external token transfers. |
| **Orphaned Locks** | Buyer neither confirms nor disputes, leaving collateral locked. | `release_lock` is permissionless for the seller after `expires_at` without requiring counterparty consent. |

---

## 5. Summary & Recommendation

For protocol v0.3:
1. The **deterministic 1:1 ratio** ($C_{\text{locked}} = V$) is enforced on-chain in `contracts/vault`.
2. Fractional collateral pools are reserved for future governance extensions once Astrolabe multi-party reputation feeds achieve full mainnet maturity.
