#!/bin/sh
set -eu
: "${LAYERX_TESTNET_STATE:=/var/lib/layerx-testnet/state.snapshot}"
: "${LAYERX_TESTNET_RESET_TOKEN_FILE:?LAYERX_TESTNET_RESET_TOKEN_FILE is required}"
test -r "$LAYERX_TESTNET_RESET_TOKEN_FILE"
IFS= read -r provided
test -n "$provided"
expected=$(sha256sum "$LAYERX_TESTNET_RESET_TOKEN_FILE" | cut -d' ' -f1)
test "$(printf '%s' "$provided" | sha256sum | cut -d' ' -f1)" = "$expected"
rm -f "$LAYERX_TESTNET_STATE" "${LAYERX_TESTNET_STATE}.new"
kill -TERM 1
