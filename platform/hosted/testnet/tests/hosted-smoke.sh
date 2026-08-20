#!/bin/sh
set -eu
: "${LAYERX_TESTNET_URL:?LAYERX_TESTNET_URL is required}"
: "${LAYERX_FAUCET_URL:?LAYERX_FAUCET_URL is required}"
: "${LAYERX_TEST_IDENTITY:?LAYERX_TEST_IDENTITY is required}"
: "${LAYERX_TEST_PUBLIC_KEY:?LAYERX_TEST_PUBLIC_KEY is required}"
: "${LAYERX_TEST_DID:?LAYERX_TEST_DID is required}"
curl --fail --silent --show-error "$LAYERX_TESTNET_URL/healthz" >/dev/null
curl --fail --silent --show-error --request POST "$LAYERX_FAUCET_URL/v1/faucet/claims" \
  --header "X-LayerX-Principal: $LAYERX_TEST_IDENTITY" --header "X-LayerX-Client-IP: scheduled-ci" \
  --header 'Content-Type: application/json' \
  --data "{\"did\":\"$LAYERX_TEST_DID\",\"public_key\":\"$LAYERX_TEST_PUBLIC_KEY\"}" >/dev/null
sh platform/emulator/tests/conformance.sh "$LAYERX_TESTNET_URL" "$LAYERX_TESTNET_URL" "$LAYERX_CONFORMANCE_ACTIVITIES"
