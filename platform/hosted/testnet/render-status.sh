#!/bin/sh
set -eu
: "${LAYERX_STATUS_OUTPUT:=/var/www/status/components.json}"
: "${LAYERX_TESTNET_HEALTH:=http://layerx-testnet:9402/healthz}"
: "${LAYERX_GATEWAY_HEALTH:=http://layerx-gateway:9420/healthz}"
: "${LAYERX_CORE_HEALTH:=http://layerx-testnet:9402/v1/state}"
: "${LAYERX_PAXEER_HEALTH:=https://rpc.testnet.paxeer.network/health}"

probe() { if curl --fail --silent --max-time 5 "$1" >/dev/null; then printf operational; else printf degraded; fi; }
testnet=$(probe "$LAYERX_TESTNET_HEALTH")
gateway=$(probe "$LAYERX_GATEWAY_HEALTH")
core=$(probe "$LAYERX_CORE_HEALTH")
paxeer=$(probe "$LAYERX_PAXEER_HEALTH")
generated=$(date -u +%Y-%m-%dT%H:%M:%SZ)
temporary="${LAYERX_STATUS_OUTPUT}.new"
umask 027
mkdir -p "$(dirname "$LAYERX_STATUS_OUTPUT")"
printf '{"generated_at":"%s","components":{"testnet":"%s","gateway":"%s","core":"%s","paxeer":"%s"}}\n' \
  "$generated" "$testnet" "$gateway" "$core" "$paxeer" >"$temporary"
mv "$temporary" "$LAYERX_STATUS_OUTPUT"
