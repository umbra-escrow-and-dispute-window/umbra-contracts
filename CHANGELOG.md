# Changelog

All notable changes to `umbra-contracts` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-11-07

### Added
- **Panel Arbitration (Issue 7)**: Implemented `ArbiterConfig::Panel(members, threshold)` requiring at least `threshold` distinct member signatures on `resolve`, plus companion entry point `resolve_panel`. Added error codes `BelowThreshold` (13), `DuplicateSigner` (14), and `InvalidThreshold` (15).
- **Signed Adjudicator Verdicts (Issue 8)**: Implemented `ArbiterConfig::Signed(public_key)` verifying Ed25519 signatures over the canonical 32-byte payload `umbra:adjudicate:v1 || escrow_id || verdict_tag || buyer_bps`, plus companion entry point `resolve_signed` and helper `signed_verdict_payload`.
- **Dispute Bonds (Issue 4)**: Added `dispute_bond: i128` to `Escrow` and `create_escrow`. Bonds are posted by the buyer on `dispute` and refunded on `RefundBuyer`, forfeited to the seller on `ReleaseSeller`, or apportioned on `Split(buyer_bps)`.
- **Astrolabe Terminal Attestations (Issue 5)**: Emits `AttestationRecord` under topics `["astrolabe", "record", escrow_id]` on all terminal transitions (`Released`, `Refunded`, `ResolvedBuyer`, `ResolvedSeller`, `ResolvedSplit`) with logarithmic amount buckets (`0..=4`).
- **Window Ordering Validation (Issue 1)**: `create_escrow` enforces `confirm_window >= dispute_window` and returns `Error::InvalidWindowOrdering` (12) when violated.
- **Storage TTL Maintenance**: Added `extend_escrow_ttl` entry point to bump persistent and instance TTL for active escrows.
- **Mainnet Gate & Checklist (Issue 6)**: Added `docs/mainnet-checklist.md` and flag-gated `scripts/deploy_mainnet.sh` (`ALLOW_MAINNET=1` and `CONFIRM_MAINNET_CHECKLIST=1`).
- **Documentation & Governance**: Added `docs/events.md`, `docs/roadmap.md`, `CODE_OF_CONDUCT.md`, `FUNDING.json`, and GitHub issue and pull request templates.

### Changed
- Converted `next_id` counter increment to checked arithmetic (`checked_add(1).ok_or(Error::Overflow)`).
- Converted basis-point split calculations to lossless `i128::from(...)` conversions.
- Expanded test suite from 28 baseline unit tests to 55 unit, property, event-schema, and concurrency tests.

### Removed
- Removed unused `ZERO_HASH` variable from `scripts/demo.sh`.

## [0.1.0] - 2026-03-29

### Added
- Initial Soroban escrow contract (`contracts/escrow`) implementing the 8-transition state machine (`Created`, `Funded`, `Delivered`, `Disputed`, `Released`, `Refunded`, `ResolvedBuyer`, `ResolvedSeller`, `ResolvedSplit`).
- Single-arbiter dispute resolution (`ArbiterConfig::Single`) with full refund, full release, and basis-point split verdicts.
- Normative protocol specification (`docs/protocol.md`), threat model (`docs/threat-model.md`), and cross-repository contract (`docs/multi-repo.md`).
- Testnet deployment script (`scripts/deploy_testnet.sh`), end-to-end demo script (`scripts/demo.sh`), and deployment record (`deployments/testnet.json`).
