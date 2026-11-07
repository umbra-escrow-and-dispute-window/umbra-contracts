# umbra-contracts

The Soroban escrow contract behind Umbra, an escrow and dispute-window protocol
for machine-to-machine payments on Stellar. A buyer funds an escrow, a seller
delivers, and the buyer confirms, lets the payment auto-release after a timeout,
or disputes within a window. An arbiter chosen by both parties resolves a dispute
with a refund, a release, or a split.

This repository is the protocol: the contract, its specification, its tests, and
the Testnet deployment. Two sibling repositories build on it:

- [umbra-sdk](https://github.com/umbra-escrow-and-dispute-window/umbra-sdk) — the TypeScript client and the off-chain hashing conventions.
- [umbra-app](https://github.com/umbra-escrow-and-dispute-window/umbra-app) — the escrow explorer, dispute filing, and arbiter console.

Dependencies point one way: protocol, then SDK, then application. See
`docs/multi-repo.md`.

## Status

Unaudited and Testnet-only. Mainnet promotion is gated behind `docs/mainnet-checklist.md`
and `scripts/deploy_mainnet.sh` (`ALLOW_MAINNET=1`).

The contract implements the full escrow state machine with:

- **Arbitration modes**: Single named arbiter (`Single`), multi-party M-of-N threshold
  panel (`Panel`), off-chain Ed25519 signed adjudicator (`Signed`), and timeout-only (`None`).
- **Dispute bonds**: Optional per-escrow `dispute_bond` posted by the buyer on `dispute`,
  refunded on `RefundBuyer`, forfeited to the seller on `ReleaseSeller`, or apportioned on `Split(bps)`.
- **Window safety**: Enforces `confirm_window >= dispute_window` at creation to prevent timeout races.
- **Indexer & Astrolabe events**: Emits structured `Transition` events on every transition and
  `AttestationRecord` events on every terminal outcome (`docs/events.md`).

The collateral vault, the `UMBR` receipt asset, and post-release clawback settlement
are tracked on the roadmap in `ISSUES.md` (Issues 9 and 10). Until they land, a payment
can only be reversed before it is released, while the contract still holds the funds.
What clawback will and will not do is described in `docs/threat-model.md`, and it is
deliberately narrow: only Umbra's own receipt asset, only inside the agreed window,
always opt-in.

## Protocol Capabilities

| Feature | Entry Point / Config | Description |
|---------|----------------------|-------------|
| Escrow Creation | `create_escrow` | Registers terms, windows, `dispute_bond`, and `ArbiterConfig`. |
| Funding | `fund` | Transfers `amount` of `asset` from buyer to contract. |
| Delivery Proof | `mark_delivered` | Seller records 32-byte `proof_hash` and starts both windows. |
| Buyer Release | `confirm` | Buyer releases funds to seller immediately. |
| Timeout Release | `claim_timeout` | Seller claims funds once `delivered_at + confirm_window` elapses. |
| Dispute Filing | `dispute` | Buyer files `reason_hash` inside `dispute_window` and posts `dispute_bond`. |
| Resolution | `resolve` / `resolve_panel` / `resolve_signed` | Distributes principal and bond per `RefundBuyer`, `ReleaseSeller`, or `Split(bps)`. |
| Cancellation | `cancel` | Either party cancels before delivery; refunds buyer if funded. |
| TTL Extension | `extend_escrow_ttl` | Extends persistent and instance storage TTL for long-lived agreements. |

## The state machine

```
Created -> Funded -> Delivered -> Released
                          |            (via confirm or claim_timeout)
                          +-> Disputed -> ResolvedBuyer   (refund)
                          |            -> ResolvedSeller  (release)
                          |            -> ResolvedSplit   (split)
                          +-> (cancel before delivery) -> Refunded
```

The normative specification, including the actor and precondition for every
transition and the error codes, is `docs/protocol.md`. Every test names the row
it exercises.

## Proof of life

The contract is deployed to Testnet and `scripts/demo.sh` walks both a full happy
path and a full dispute path against it from the command line:

```
=== Happy path ===
create_escrow -> id 4
  state after create: Created
fund
  state after fund: Funded
mark_delivered
  state after mark_delivered: Delivered
confirm
  state after confirm: Released

=== Dispute path ===
create_escrow -> id 5
  state after create: Created
fund
  state after fund: Funded
mark_delivered
  state after mark_delivered: Delivered
dispute
  state after dispute: Disputed
resolve (RefundBuyer)
  state after resolve: ResolvedBuyer
```

The escrow asset in the demo is native XLM wrapped as a Stellar Asset Contract, so
no custom token issuance is required.

## Testnet deployment

The current deployment is in `deployments/testnet.json`:

| Contract | Address |
|----------|---------|
| escrow | `CA3W2JO6TD3JCPTNUSNSMNROYM6NSSQID55H5BKO6GLQQKNZ5KKEIZZG` |

The SDK vendors this file so no consumer hardcodes a contract id.

## Build and test

Prerequisites: Rust stable (1.84 or newer) with the `wasm32v1-none` target, and
the [stellar CLI](https://developers.stellar.org/docs/tools/cli) 27 or newer.

```bash
rustup target add wasm32v1-none

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --target wasm32v1-none --release
```

On Windows, if cargo cannot reach crates.io with a certificate-revocation error,
set `CARGO_HTTP_CHECK_REVOKE=false`.

## Deploy your own and run the demo

```bash
# One funded identity for deploys, plus three parties for the demo.
for id in umbra umbra-buyer umbra-seller umbra-arbiter; do
  stellar keys generate "$id" --network testnet --fund
done

STELLAR_IDENTITY=umbra ./scripts/deploy_testnet.sh   # writes deployments/testnet.json
./scripts/demo.sh
```

## Repository map

```
contracts/escrow/src/
  lib.rs        Contract entry points, one per transition
  types.rs      Escrow, State, ArbiterConfig, Verdict, storage keys
  error.rs      Typed error codes (stable; do not renumber)
  events.rs     Transition and Astrolabe terminal attestation events
  test.rs       Legal, illegal, unauthorised, property, and concurrency tests
docs/
  protocol.md           Normative state machine and error codes
  events.md             Event schema for indexers and Astrolabe attestations
  threat-model.md       Arbiter capture, griefing, timeout races, clawback limits
  mainnet-checklist.md  Pre-deployment audit and release gate for Mainnet
  roadmap.md            Completed v0.1/v0.2 milestones and v0.3 clawback architecture
  multi-repo.md         Cross-repository contract (shared across all three repos)
scripts/
  deploy_testnet.sh     Build, deploy, write deployments/testnet.json
  deploy_mainnet.sh     Flag-gated Mainnet deploy script
  demo.sh               Full happy and dispute paths against Testnet
deployments/
  testnet.json          Deployment truth vendored by the SDK
```

## Roadmap

See `docs/roadmap.md` for completed v0.1 and v0.2 milestones and the architectural
design for v0.3 (the collateral vault, `UMBR` receipt asset, and post-release
clawback settlement tracked in `ISSUES.md`).

## Contributing

Read `CONTRIBUTING.md`. It covers setup from a cold start, the invariants a
reviewer will send a pull request back for, and a list of unclaimed work ordered
easiest to hardest. Report security issues privately per `SECURITY.md`, never in a
public issue.

## License

Apache-2.0. See `LICENSE`.
