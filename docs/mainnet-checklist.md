# Mainnet Configuration and Release Checklist

This checklist governs the promotion of the Umbra escrow contract from Stellar Testnet to Stellar Public Network (Mainnet). No Mainnet deployment may proceed unless every item below is verified and signed off by the maintainers.

---

## 1. Pre-Deployment Security & Code Audit

- [ ] **External Audit**: An independent security review of `contracts/escrow` has been completed and all critical, high, and medium findings are resolved.
- [ ] **Zero Unimplemented Paths**: All reachable transitions in `contracts/escrow/src/lib.rs` are implemented and covered by unit and property tests.
- [ ] **Invariant Verification**:
  - [ ] `confirm_window >= dispute_window` enforced at creation (`Error::InvalidWindowOrdering`).
  - [ ] Checked arithmetic used for all balance, basis-point, and deadline calculations (`Error::Overflow`).
  - [ ] Split rounding remainder deterministically assigned to the seller (`docs/protocol.md`).
  - [ ] Panel arbitration rejects duplicate signers and enforces `0 < threshold <= members.len()`.
  - [ ] Signed adjudicator verifies Ed25519 signatures over domain-separated 32-byte payloads (`umbra:adjudicate:v1`).
  - [ ] Dispute bonds are conserved across refund, release, and split outcomes.
- [ ] **Storage TTL**: `ESCROW_TTL` (90 days) and `ESCROW_TTL_THRESHOLD` (30 days) are verified against expected dispute durations, and `extend_escrow_ttl` is tested.

---

## 2. Build & Reproducibility Verification

- [ ] **Toolchain Pin**: `rust-toolchain.toml` pins the exact Rust release and `wasm32v1-none` target.
- [ ] **Clean Quality Suite**:
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  stellar contract build
  ```
- [ ] **WASM Hash Verification**: Record the SHA-256 digest of `target/wasm32v1-none/release/umbra_escrow.optimized.wasm` and confirm deterministic output across two independent build environments.

---

## 3. Key Management & Operational Readiness

- [ ] **Deployer Identity**: The deployment account is backed by hardware key storage or a multi-signature operational setup; raw secret keys are never stored in `.env` or CI variables.
- [ ] **Network Configuration**:
  - Network: `mainnet`
  - Passphrase: `Public Global Stellar Network ; September 2015`
- [ ] **Explicit Safety Gate**: `scripts/deploy_mainnet.sh` requires `ALLOW_MAINNET=1` and `CONFIRM_MAINNET_CHECKLIST=1` in the environment. Calling the script without both flags aborts immediately before any network call.

---

## 4. Deployment Execution (`scripts/deploy_mainnet.sh`)

Run from a clean working tree on a tagged release commit:

```bash
ALLOW_MAINNET=1 CONFIRM_MAINNET_CHECKLIST=1 STELLAR_IDENTITY=<mainnet-identity> ./scripts/deploy_mainnet.sh
```

- [ ] Verify `deployments/mainnet.json` is written with:
  - `network`: `"mainnet"`
  - `networkPassphrase`: `"Public Global Stellar Network ; September 2015"`
  - `commit`: release commit SHA
  - `wasmSha256`: verified SHA-256 digest
  - `contracts.escrow`: deployed `C...` contract address
- [ ] Smoke-test read-only `get_escrow --escrow_id 1` against the deployed contract ID on Mainnet.

---

## 5. Post-Deployment & Cross-Repo Synchronisation

- [ ] Commit `deployments/mainnet.json` and tag the release in git.
- [ ] Vendor `deployments/mainnet.json` into `umbra-sdk` and run `scripts/check-sync.mjs` (`docs/multi-repo.md`).
- [ ] Update `README.md` and `SECURITY.md` to remove the "Testnet-only" notice once the Mainnet deployment is live and monitored.
