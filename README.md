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

Unaudited and Testnet-only. Do not deploy to mainnet. The contract implements the
full escrow state machine with single-arbiter resolution. The collateral vault,
the `UMBR` receipt asset, and clawback settlement are not built yet; they are
filed in `ISSUES.md`. Until they land, a payment can only be reversed before it is
released, because the contract still holds the funds. What clawback will and will
not do is described in `docs/threat-model.md`, and it is deliberately narrow: only
Umbra's own receipt asset, only inside the agreed window, always opt-in.

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
  events.rs     One event per transition
  test.rs       Every legal and illegal transition, plus unauthorised paths
docs/
  protocol.md       Normative state machine and error codes
  threat-model.md   Arbiter capture, griefing, timeout races, clawback limits
  multi-repo.md     Cross-repository contract (shared across all three repos)
scripts/
  deploy_testnet.sh Build, deploy, write deployments/testnet.json
  demo.sh           Full happy and dispute paths against Testnet
deployments/
  testnet.json      Deployment truth vendored by the SDK
```

## Contributing

Read `CONTRIBUTING.md`. It covers setup from a cold start, the invariants a
reviewer will send a pull request back for, and a list of unclaimed work ordered
easiest to hardest. Report security issues privately per `SECURITY.md`, never in a
public issue.

## License

Apache-2.0. See `LICENSE`.
