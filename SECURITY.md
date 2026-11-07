# Security policy

Umbra is unaudited and Testnet-only. Do not use it to hold value you cannot
afford to lose, and do not deploy it to mainnet.

## Reporting a vulnerability

Report privately to **security@umbra-escrow.dev**. Do not open a public issue for
a vulnerability. Include what you found, how to reproduce it, and the impact you
see. You will get an acknowledgement, and we will agree a disclosure timeline
with you before anything is made public.

The same address is used for all three Umbra repositories.

## Sensitive surfaces in this repository

This repository is the escrow contract, so the sensitive surface is the contract
itself. Pay particular attention to:

- **Authorisation checks.** Each entry point requires a specific party's
  signature (`docs/protocol.md`). A missing or wrong `require_auth` is a fund-loss
  bug. Any pull request that changes one must say which `docs/threat-model.md`
  item it affects.
- **The time source.** Every deadline uses the ledger timestamp. A change that
  accepted a caller-supplied time would break the timeout and dispute windows.
- **Amount arithmetic and the split rounding.** All arithmetic is checked and the
  split remainder goes to the seller. A change here can misdirect funds.
- **State transitions.** The contract must never move funds except along the
  transition table. A new path out of a terminal state is a double-spend.
- **Panel threshold and signed verdicts.** Panel resolution must enforce distinct
  member signatures meeting `threshold`, and signed adjudicator resolution must
  verify Ed25519 signatures over the canonical domain-separated payload.
- **Dispute bond conservation.** Dispute bonds posted by buyers must be conserved
  and distributed strictly according to the resolution verdict.

## Scope

In scope: the contract in `contracts/escrow` (including single, panel, and signed
adjudication, dispute bonds, and terminal attestations), the deploy and demo scripts,
and the deployment record. Out of scope: the collateral vault, the `UMBR` receipt
asset, and post-release clawback settlement (tracked on the v0.3 roadmap).
