## Summary
Describe what this pull request changes and why. Reference the issue number from `ISSUES.md` if applicable.

## Protocol & Threat Model Checklist
- [ ] `docs/protocol.md` and `docs/events.md` match the implementation (or were updated in lockstep).
- [ ] Any change to `require_auth` or fund movement cites the affected section of `docs/threat-model.md`.
- [ ] Every new or modified transition has legal-path, illegal-path, and unauthorised-path unit tests.
- [ ] All amount arithmetic uses checked operations (`checked_add`, `checked_sub`, `checked_mul`).
- [ ] No hardcoded contract or asset IDs were introduced (`deployments/testnet.json` untouched unless deploying).

## Verification
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo build --target wasm32v1-none --release`
