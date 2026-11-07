#!/usr/bin/env bash
# Build, optimise, and deploy the escrow contract to Stellar Mainnet.
# Gated behind explicit safety flags; see docs/mainnet-checklist.md.
#
#   ALLOW_MAINNET=1 CONFIRM_MAINNET_CHECKLIST=1 STELLAR_IDENTITY=<id> ./scripts/deploy_mainnet.sh [output-file]
set -euo pipefail

if [ "${ALLOW_MAINNET:-0}" != "1" ]; then
  echo "Error: Mainnet deployment is disabled by default." >&2
  echo "Review docs/mainnet-checklist.md and set ALLOW_MAINNET=1 to proceed." >&2
  exit 1
fi

if [ "${CONFIRM_MAINNET_CHECKLIST:-0}" != "1" ]; then
  echo "Error: CONFIRM_MAINNET_CHECKLIST=1 must be set to confirm all checklist items are signed off." >&2
  exit 1
fi

if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  echo "Error: Working tree is dirty. Mainnet deploys must run from a clean, committed state." >&2
  exit 1
fi

OUT="${1:-deployments/mainnet.json}"
IDENTITY="${STELLAR_IDENTITY:?Error: STELLAR_IDENTITY must be set explicitly for mainnet deployment}"
NETWORK="mainnet"
PASSPHRASE="Public Global Stellar Network ; September 2015"
WASM="target/wasm32v1-none/release/umbra_escrow.wasm"

echo "Running pre-deploy test suite..."
cargo test --workspace

echo "Building contract..."
stellar contract build

echo "Optimising..."
stellar contract optimize --wasm "$WASM" >/dev/null 2>&1 || true
OPTIMISED="target/wasm32v1-none/release/umbra_escrow.optimized.wasm"
[ -f "$OPTIMISED" ] || OPTIMISED="$WASM"

echo "Deploying to $NETWORK as identity '$IDENTITY'..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$OPTIMISED" \
  --source "$IDENTITY" \
  --network "$NETWORK")

WASM_HASH=$(sha256sum "$OPTIMISED" | cut -d' ' -f1)
COMMIT=$(git rev-parse --short HEAD)
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

mkdir -p "$(dirname "$OUT")"
cat >"$OUT" <<JSON
{
  "network": "$NETWORK",
  "networkPassphrase": "$PASSPHRASE",
  "deployedAt": "$NOW",
  "commit": "$COMMIT",
  "wasmSha256": "$WASM_HASH",
  "contracts": {
    "escrow": "$CONTRACT_ID"
  }
}
JSON

echo "Deployed escrow to $CONTRACT_ID on Mainnet"
echo "Wrote $OUT"
