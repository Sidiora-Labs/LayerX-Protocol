#!/bin/sh
set -eu
: "${LAYERX_TESTNET_LISTEN:=0.0.0.0:9402}"
: "${LAYERX_TESTNET_NETWORK_ID:=402}"
: "${LAYERX_TESTNET_STATE:=/var/lib/layerx-testnet/state.snapshot}"
: "${LAYERX_TESTNET_SNAPSHOT_SECONDS:=15}"

mkdir -p "$(dirname "$LAYERX_TESTNET_STATE")"
layerx emulator up --listen "$LAYERX_TESTNET_LISTEN" --network-id "$LAYERX_TESTNET_NETWORK_ID" &
testnet_pid=$!
trap 'kill "$testnet_pid" 2>/dev/null || true; wait "$testnet_pid" 2>/dev/null || true' EXIT INT TERM

until curl --fail --silent --show-error "http://127.0.0.1:${LAYERX_TESTNET_LISTEN##*:}/healthz" >/dev/null; do
  kill -0 "$testnet_pid"
done

if test -s "$LAYERX_TESTNET_STATE"; then
  curl --fail --silent --show-error --request PUT --header 'Content-Type: application/vnd.layerx.emulator-snapshot' \
    --data-binary "@$LAYERX_TESTNET_STATE" "http://127.0.0.1:${LAYERX_TESTNET_LISTEN##*:}/__emulator/snapshot" >/dev/null
fi

while kill -0 "$testnet_pid" 2>/dev/null; do
  sleep "$LAYERX_TESTNET_SNAPSHOT_SECONDS"
  snapshot="${LAYERX_TESTNET_STATE}.new"
  if curl --fail --silent --show-error "http://127.0.0.1:${LAYERX_TESTNET_LISTEN##*:}/__emulator/snapshot" --output "$snapshot"; then
    chmod 0600 "$snapshot"
    mv "$snapshot" "$LAYERX_TESTNET_STATE"
  else
    rm -f "$snapshot"
  fi
done
wait "$testnet_pid"
