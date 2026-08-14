#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
metadata_file=$(mktemp)
trap 'rm -f -- "$metadata_file"' EXIT HUP INT TERM

cargo metadata --manifest-path "$repo_root/agent/Cargo.toml" \
    --locked --format-version 1 >"$metadata_file"

banned='^(bindgen|libsqlite3-sys|rusqlite|sqlx-sqlite|ctor|inventory)$'
if jq -r '.packages[].name' "$metadata_file" | grep -Eq "$banned"; then
    echo "dependency policy: forbidden boundary or global-initialisation crate" >&2
    exit 1
fi

missing_license=$(jq -r '.packages[] | select(.source != null and (.license == null or .license == "")) | .name' "$metadata_file")
if [ -n "$missing_license" ]; then
    echo "dependency policy: dependency without an allowed SPDX license: $missing_license" >&2
    exit 1
fi

if rg -n --glob '*.rs' '\bunsafe\s+(fn|trait|impl|extern)|\bunsafe\s*\{' "$repo_root/agent/crates"; then
    echo "unsafe policy: unsafe code requires an explicit reviewed exception" >&2
    exit 1
fi

test -r "$repo_root/agent/unsafe-allowlist.toml"
test -r "$repo_root/agent/deny.toml"
echo "agent dependency and unsafe policies passed"
