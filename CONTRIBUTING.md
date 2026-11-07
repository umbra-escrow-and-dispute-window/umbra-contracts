# Contributing to umbra-contracts

Thanks for helping build Umbra. This file gets a contributor who has never
touched Stellar from a clone to an open pull request in a day. Read it end to end
once; after that, the "Where to start" list is the part you will come back to.

## What this repository is, and the other two

Umbra is an escrow and dispute-window protocol for machine-to-machine payments on
Stellar's Soroban platform. A buyer funds an escrow, a seller delivers, and the
buyer confirms, lets it auto-release after a timeout, or disputes within a window.
An arbiter chosen by both parties resolves a dispute.

Umbra is three repositories:

- **umbra-contracts** (this one) — the Rust escrow contract, its specification,
  its tests, and the Testnet deployment.
- [umbra-sdk](https://github.com/umbra-escrow-and-dispute-window/umbra-sdk) — the
  TypeScript client and the off-chain hashing conventions.
- [umbra-app](https://github.com/umbra-escrow-and-dispute-window/umbra-app) — the
  escrow explorer, dispute filing, and arbiter console.

Dependencies point one way: **protocol, then SDK, then application, never
reversed.** A change here can ripple outward to the SDK and app; a change in the
app never reaches back into this repository. `docs/multi-repo.md` is the full
cross-repository contract.

## The domain in two minutes

Four concepts carry the whole design:

- **Escrow.** The contract holds the buyer's funds between funding and a terminal
  outcome. It never holds anything else and never moves funds except along the
  state machine.
- **Windows.** After the seller marks delivery, two clocks start from the same
  ledger timestamp: a confirm window, after which the seller can claim payment,
  and a dispute window, inside which the buyer can dispute.
- **Arbiter.** Chosen per escrow at creation. Supports a single named arbiter
  (`Single`), a multi-party threshold panel (`Panel`), an automated Ed25519
  adjudicator (`Signed`), or timeout-only (`None`).
- **Hashes, not documents.** Terms, delivery proofs, and dispute reasons live off
  chain. The contract stores only their 32-byte hashes.

The normative document is `docs/protocol.md`: the state table, the actor and
precondition for each transition, and the error codes. Code follows it, and every
test names the row it exercises. If code and `protocol.md` disagree, one is a bug.

## Repository map

```
contracts/escrow/src/
  lib.rs        Contract entry points, one per transition (cites protocol.md rows)
  types.rs      Escrow, State, ArbiterConfig, Verdict, storage keys
  error.rs      Typed error codes; stable, do not renumber
  events.rs     Transition and Astrolabe terminal attestation events
  test.rs       Every legal, illegal, unauthorised, and property test
docs/
  protocol.md           Normative state machine and error codes
  events.md             Event schema for indexers and Astrolabe attestations
  threat-model.md       Arbiter capture, griefing, timeout races, clawback limits
  mainnet-checklist.md  Pre-deployment audit and release gate for Mainnet
  multi-repo.md         Cross-repository contract, shared across all three repos
scripts/
  deploy_testnet.sh     Build, deploy, write deployments/testnet.json
  deploy_mainnet.sh     Flag-gated Mainnet deploy script
  demo.sh               Full happy and dispute paths against Testnet
deployments/
  testnet.json          Deployment truth vendored by the SDK
```

Nothing here is generated; every file is edited by hand. The build output under
`target/` is ignored.

## Getting set up

Prerequisites:

- Rust stable, 1.84 or newer, with the WASM target:
  `rustup target add wasm32v1-none`
- The [stellar CLI](https://developers.stellar.org/docs/tools/cli), 27 or newer:
  `stellar --version`

Clone and prove the setup works without deploying anything of your own. The unit
tests run entirely in a simulated environment and need no network:

```bash
git clone https://github.com/umbra-escrow-and-dispute-window/umbra-contracts
cd umbra-contracts

cargo test          # 55 tests, all in-memory, no network
```

On Windows, if cargo cannot reach crates.io with a certificate-revocation error,
set `CARGO_HTTP_CHECK_REVOKE=false` in your environment.

To exercise the contract against Testnet you need funded identities. A Testnet key
is free and is funded by Friendbot; the stellar CLI does both in one command:

```bash
for id in umbra umbra-buyer umbra-seller umbra-arbiter; do
  stellar keys generate "$id" --network testnet --fund
done
```

Then deploy your own copy and run the end-to-end demo, which completes a full
dispute cycle on Testnet:

```bash
STELLAR_IDENTITY=umbra ./scripts/deploy_testnet.sh   # writes deployments/testnet.json
./scripts/demo.sh                                    # happy path and dispute path
```

If `demo.sh` prints `ResolvedBuyer` at the end, your setup is complete.

## Where to start

Issues carry one of three difficulty labels: `good first issue`, `intermediate`,
`advanced`. Area labels (`contract`, `docs`, `tooling`) narrow further. The full
list with acceptance criteria is `ISSUES.md`; this is the same list, ordered
easiest to hardest, and the two must agree.

1. **Warn on a confirm window shorter than the dispute window** (`good first
   issue`, **Resolved**). Enforced in `create_escrow` with `InvalidWindowOrdering`.
2. **Document and test the event schema** (`good first issue`, **Resolved**).
   Documented in `docs/events.md` and verified in integration tests.
3. **Property tests for split rounding** (`intermediate`, **Resolved**). Verified
   across wide amount and basis-point ranges in `test.rs`.
4. **Dispute bond to deter griefing** (`intermediate`, **Resolved**). Per-escrow
   `dispute_bond` posted on `dispute` and settled on `resolve`.
5. **Astrolabe attestation on terminal states** (`intermediate`, **Resolved**).
   Emits `AttestationRecord` on every terminal outcome.
6. **Mainnet configuration and release checklist** (`intermediate`, **Resolved**).
   Documented in `docs/mainnet-checklist.md` and gated in `scripts/deploy_mainnet.sh`.
7. **Panel arbitration with threshold signatures** (`advanced`, **Resolved**).
   Implemented `ArbiterConfig::Panel(members, threshold)` and `resolve_panel`.
8. **Signed adjudicator verdicts** (`advanced`, **Resolved**). Implemented
   `ArbiterConfig::Signed(key)` Ed25519 verification and `resolve_signed`.
9. **Collateral vault, `UMBR` receipt asset, and clawback settlement**
   (`advanced`). **This is the single largest unclaimed piece**: the post-release
   reversal case, the reason clawback exists at all. Read the threat model first.
10. **Economic analysis of collateral sizing** (`advanced`, research). The open
    research question of how much collateral an escrow should require; it blocks
    lifting the window cap in issue 9.

Claim an issue by commenting on it so two people do not build the same thing. For
anything that changes the contract's authorisation, storage layout, or the
protocol itself (issues 7, 8, 9), open a Discussion with a short written proposal
and wait for one maintainer to sign off before you start; see "Community" below.

## Rules that matter here

A reviewer will send a pull request back for any of these:

- **Ledger time only.** Every deadline uses `env.ledger().timestamp()`. Never
  accept a caller-supplied time; it would break both windows.
- **No hardcoded contract or asset ids in the contract.** Ids travel through
  `deployments/testnet.json` and the SDK; a feature pull request does not edit
  that file.
- **Checked arithmetic, documented rounding.** All amount maths is checked, and
  the split remainder goes to the seller as `docs/protocol.md` states. Do not
  change the rounding direction without changing the doc and the tests together.
- **Every transition needs a legal-path test and an illegal-path test**, and every
  new entry point needs an unauthorised-path test proving it requires the right
  signature. This is the most important rule in the repository.
- **Never widen clawback beyond Umbra's own receipt asset inside the agreed
  window.** This is the invariant the whole project's credibility rests on. Any
  change touching issue 9 is reviewed against it first.
- **Documents never go on chain, only their hashes.**

## Code style

Rust, formatted with `rustfmt` and linted with `clippy` at deny-warnings. No
`unsafe`. Commits follow [Conventional Commits](https://www.conventionalcommits.org)
(`feat:`, `fix:`, `docs:`, `test:`, `chore:`). Branch names are
`type/short-description`, for example `feat/dispute-bond`.

CI runs exactly these, and they must pass:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --target wasm32v1-none --release
```

Testnet integration tests are not a pull-request gate; Testnet resets and rate
limits make them flaky as a required check. Run them locally against your own
deployment.

## Pull request checklist

- [ ] `cargo fmt --all --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` green
- [ ] `cargo build --target wasm32v1-none --release` succeeds
- [ ] New or changed transitions have both a legal-path and an illegal-path test
- [ ] New entry points have an unauthorised-path test
- [ ] `docs/protocol.md` updated if behaviour changed
- [ ] A pull request that changes an authorisation check names the affected
      `docs/threat-model.md` item
- [ ] If this change spans repositories, the dependent pull request in the SDK or
      app is linked (see `docs/multi-repo.md` section 4)

Small pull requests get reviewed faster than large ones.

## Releases

Maintainers cut releases. A release is a git tag `vX.Y.Z`: it builds the WASM,
attaches it to a GitHub release with its sha256, and is the version the SDK's
compatibility table points at. Before 1.0, breaking changes may land in a minor.

Do not touch `deployments/testnet.json` in a feature pull request; a redeployment
is a separate, deliberate step owned by maintainers, and it ripples into the SDK
and app in dependency order.

## Security

Report vulnerabilities privately per `SECURITY.md`, never in a public issue. The
sensitive surfaces here are the authorisation checks, the time source, the amount
arithmetic, and the state transitions. The project is unaudited and Testnet-only.

## Community

Design discussion happens in this repository's GitHub Discussions. Protocol
changes need a written proposal there and one maintainer sign-off before
implementation begins; this is stricter than the app repository on purpose,
because a contract bug loses funds.

Two merged pull requests earn triage rights on request. Commit rights on a
protocol repository are granted slowly and only after real contract-review work,
which is also stricter than the app side. We will not promise a review time we
cannot keep, but smaller pull requests are always reviewed sooner.
