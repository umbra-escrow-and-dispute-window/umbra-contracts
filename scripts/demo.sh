#!/usr/bin/env bash
# End-to-end proof of life against Stellar Testnet. Runs a full happy path
# (create, fund, deliver, confirm) and a full dispute path (create, fund,
# deliver, dispute, resolve as refund), printing each transition and the escrow
# state after it. This is the script the README quotes.
#
#   ./scripts/demo.sh
#
# Requires: stellar CLI (>= 27); a deployed contract in deployments/testnet.json
# (run scripts/deploy_testnet.sh first); four funded Testnet identities named
# umbra, umbra-buyer, umbra-seller, umbra-arbiter. Create and fund them with:
#   for id in umbra umbra-buyer umbra-seller umbra-arbiter; do \
#     stellar keys generate "$id" --network testnet --fund; done
#
# The escrow asset in this demo is native XLM wrapped as a Stellar Asset
# Contract, so no custom token issuance is needed.
set -euo pipefail

NETWORK="testnet"
DEPLOYMENTS="deployments/testnet.json"
AMOUNT="500000000" # 50 XLM in stroops
CONFIRM_WINDOW="86400"
DISPUTE_WINDOW="86400"
TERMS="1111111111111111111111111111111111111111111111111111111111111111"
PROOF="2222222222222222222222222222222222222222222222222222222222222222"
REASON="3333333333333333333333333333333333333333333333333333333333333333"

CID=$(grep -oE '"escrow"[[:space:]]*:[[:space:]]*"[^"]+"' "$DEPLOYMENTS" | grep -oE 'C[A-Z0-9]{55}')
BUYER=$(stellar keys address umbra-buyer)
SELLER=$(stellar keys address umbra-seller)
ARBITER=$(stellar keys address umbra-arbiter)

echo "Contract: $CID"
echo "Buyer:    $BUYER"
echo "Seller:   $SELLER"
echo "Arbiter:  $ARBITER"
echo

# Wrap native XLM as a Stellar Asset Contract. Idempotent: ignore if it exists.
stellar contract asset deploy --asset native --source umbra --network "$NETWORK" >/dev/null 2>&1 || true
ASSET=$(stellar contract id asset --asset native --network "$NETWORK")
echo "Asset (native SAC): $ASSET"
echo

invoke() { stellar contract invoke --id "$CID" --network "$NETWORK" "$@"; }

# State is serialised as its numeric discriminant; map it to the protocol.md name.
STATE_NAMES=(Created Funded Delivered Disputed Released Refunded ResolvedBuyer ResolvedSeller ResolvedSplit)
show() {
  local json n i
  for i in 1 2 3; do
    json=$(invoke --source umbra -- get_escrow --escrow_id "$2" 2>/dev/null || true)
    n=$(printf '%s' "$json" | grep -oE '"state":[0-9]+' | grep -oE '[0-9]+' || true)
    [ -n "$n" ] && break
    sleep 2
  done
  if [ -z "$n" ]; then
    echo "  state after $1: (read unavailable)"
  else
    echo "  state after $1: ${STATE_NAMES[$n]:-unknown($n)}"
  fi
}

echo "=== Happy path ==="
ID=$(invoke --source umbra-buyer -- create_escrow \
  --buyer "$BUYER" --seller "$SELLER" --asset "$ASSET" --amount "$AMOUNT" \
  --deliver_by 9999999999 --confirm_window "$CONFIRM_WINDOW" --dispute_window "$DISPUTE_WINDOW" \
  --arbiter '"None"' --terms_hash "$TERMS")
echo "create_escrow -> id $ID"; show create "$ID"

invoke --source umbra-buyer -- fund --escrow_id "$ID" >/dev/null
echo "fund"; show fund "$ID"

invoke --source umbra-seller -- mark_delivered --escrow_id "$ID" --proof_hash "$PROOF" >/dev/null
echo "mark_delivered"; show mark_delivered "$ID"

invoke --source umbra-buyer -- confirm --escrow_id "$ID" >/dev/null
echo "confirm"; show confirm "$ID"
echo

echo "=== Dispute path ==="
ID=$(invoke --source umbra-buyer -- create_escrow \
  --buyer "$BUYER" --seller "$SELLER" --asset "$ASSET" --amount "$AMOUNT" \
  --deliver_by 9999999999 --confirm_window "$CONFIRM_WINDOW" --dispute_window "$DISPUTE_WINDOW" \
  --arbiter "{\"Single\":\"$ARBITER\"}" --terms_hash "$TERMS")
echo "create_escrow -> id $ID"; show create "$ID"

invoke --source umbra-buyer -- fund --escrow_id "$ID" >/dev/null
echo "fund"; show fund "$ID"

invoke --source umbra-seller -- mark_delivered --escrow_id "$ID" --proof_hash "$PROOF" >/dev/null
echo "mark_delivered"; show mark_delivered "$ID"

invoke --source umbra-buyer -- dispute --escrow_id "$ID" --reason_hash "$REASON" >/dev/null
echo "dispute"; show dispute "$ID"

invoke --source umbra-arbiter -- resolve --escrow_id "$ID" --verdict '"RefundBuyer"' >/dev/null
echo "resolve (RefundBuyer)"; show resolve "$ID"
echo

echo "Demo complete."
