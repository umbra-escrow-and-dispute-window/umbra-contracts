#!/usr/bin/env bash
# Build, install and deploy the escrow contract to Stellar Testnet, then write
# the deployment record that the SDK vendors. Run from the repository root.
#
#   ./scripts/deploy_testnet.sh [output-file]
#
# Default output is deployments/testnet.json. For local cross-repo work, pass
# deployments/local-testnet.json and point the app at it with
# CONSTELLATION_DEPLOYMENTS (see docs/multi-repo.md section 5).
#
# Requires: stellar CLI (>= 27), a funded Testnet identity named by
# STELLAR_IDENTITY (default: umbra). Create and fund one with:
#   stellar keys generate umbra --network testnet --fund
set -euo pipefail

OUT="${1:-deployments/testnet.json}"
IDENTITY="${STELLAR_IDENTITY:-umbra}"
NETWORK="testnet"
PASSPHRASE="Test SDF Network ; September 2015"
WASM="target/wasm32v1-none/release/umbra_escrow.wasm"

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
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
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

echo "Deployed escrow to $CONTRACT_ID"
echo "Wrote $OUT"
