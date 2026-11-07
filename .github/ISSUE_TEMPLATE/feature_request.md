---
name: Feature proposal
about: Propose a protocol enhancement, tooling improvement, or open issue implementation
title: "[feat]: "
labels: enhancement
assignees: ''
---

## Problem or Issue Reference
If claiming an item from `ISSUES.md`, cite the issue number and title. Otherwise describe the problem this proposal solves.

## Proposed Design
Describe any changes to:
- Contract entry points (`contracts/escrow/src/lib.rs`)
- Storage types (`contracts/escrow/src/types.rs`)
- Error codes (`contracts/escrow/src/error.rs`)
- Event schema (`docs/events.md`)

## Threat Model Impact
Which section of `docs/threat-model.md` does this affect? Confirm that clawback invariants and authorisation rules remain intact.

## Test Plan
List the legal-path, illegal-path, and unauthorised-path tests that will accompany the change.
