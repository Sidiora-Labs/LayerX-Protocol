#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
metadata_file=$(mktemp)
license_file=$(mktemp)
trap 'rm -f -- "$metadata_file" "$license_file"' EXIT HUP INT TERM

cargo metadata --manifest-path "$repo_root/human/Cargo.toml" \
    --locked --format-version 1 >"$metadata_file"

banned='^(bindgen|libsqlite3-sys|rusqlite|sqlx-sqlite|layerx-core|layerx-protocol-core|ctor|inventory)$'
if jq -r '.packages[].name' "$metadata_file" | grep -Eq "$banned"; then
    echo "human dependency policy: forbidden boundary or global-initialisation crate" >&2
    exit 1
fi

jq -r '.packages[] | select(.source != null) | [.name, (.license // "")] | @tsv' \
    "$metadata_file" >"$license_file"
while IFS="$(printf '\t')" read -r package license; do
    if [ -z "$license" ]; then
        echo "human dependency policy: $package has no SPDX license" >&2
        exit 1
    fi
    tokens=$(printf '%s\n' "$license" | sed 's/[()]/ /g; s|/| |g; s/ AND / /g; s/ OR / /g; s/ WITH / /g')
    for token in $tokens; do
        case "$token" in
            Apache-2.0|BSD-1-Clause|BSD-2-Clause|BSD-3-Clause|CC0-1.0|ISC|MIT|Unicode-3.0|Zlib|LLVM-exception)
                ;;
            *)
                echo "human dependency policy: $package uses non-allowlisted license $token" >&2
                exit 1
                ;;
        esac
    done
done <"$license_file"

if rg -n --glob '*.rs' '\bunsafe\s+(fn|trait|impl|extern)|\bunsafe\s*\{' "$repo_root/human/crates"; then
    echo "human unsafe policy: unsafe code is forbidden" >&2
    exit 1
fi

echo "human dependency and unsafe policies passed"
