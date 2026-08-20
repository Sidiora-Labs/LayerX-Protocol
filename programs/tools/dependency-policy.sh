#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
metadata_file=$(mktemp)
license_file=$(mktemp)
trap 'rm -f -- "$metadata_file" "$license_file"' EXIT HUP INT TERM

cd "$repo_root/programs"
cargo metadata --manifest-path "$repo_root/programs/Cargo.toml" \
    --locked --format-version 1 >"$metadata_file"

banned='^(bindgen|libsqlite3-sys|rusqlite|sqlx-sqlite|ctor|inventory|getrandom|rand|rand_core|chrono|time|instant|tokio|mio|socket2|wasi|wasmtime)$'
if jq -r '.packages[].name' "$metadata_file" | grep -Eq "$banned"; then
    echo "programs dependency policy: forbidden boundary, clock, randomness or network crate" >&2
    exit 1
fi

allowlisted_license_token() {
    case "$1" in
        Apache-2.0|BSD-1-Clause|BSD-2-Clause|BSD-3-Clause|CC0-1.0|ISC|MIT|Unicode-3.0|Zlib|LLVM-exception)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

jq -r '.packages[] | select(.source != null) | [.name, (.license // "")] | @tsv' \
    "$metadata_file" >"$license_file"
while IFS="$(printf '\t')" read -r package license; do
    if [ -z "$license" ]; then
        echo "programs dependency policy: $package has no SPDX license" >&2
        exit 1
    fi
    case "$license" in
        *" AND "*)
            tokens=$(printf '%s\n' "$license" | sed 's/[()]/ /g; s|/| |g; s/ AND / /g; s/ OR / /g; s/ WITH / /g')
            for token in $tokens; do
                if ! allowlisted_license_token "$token"; then
                    echo "programs dependency policy: $package uses non-allowlisted license $license" >&2
                    exit 1
                fi
            done
            ;;
        *)
            alternatives=$(printf '%s\n' "$license" | sed 's/[()]//g; s|/|\n|g; s/ OR /\n/g')
            satisfied=0
            while IFS= read -r alternative; do
                [ -n "$alternative" ] || continue
                alternative_ok=1
                for token in $(printf '%s\n' "$alternative" | sed 's/ WITH / /g'); do
                    if ! allowlisted_license_token "$token"; then
                        alternative_ok=0
                    fi
                done
                if [ "$alternative_ok" -eq 1 ]; then
                    satisfied=1
                fi
            done <<LICENSE_ALTERNATIVES
$alternatives
LICENSE_ALTERNATIVES
            if [ "$satisfied" -ne 1 ]; then
                echo "programs dependency policy: $package uses non-allowlisted license $license" >&2
                exit 1
            fi
            ;;
    esac
done <"$license_file"

jq -r '.packages[] | select(.source != null) | .name' "$metadata_file" | while IFS= read -r package; do
    if [ ! -f "$repo_root/programs/vendor/$package/.cargo-checksum.json" ]; then
        echo "programs vendoring policy: $package is not vendored under programs/vendor" >&2
        exit 1
    fi
done

if ! grep -q '^wasmi = "=0\.31\.2"$' "$repo_root/programs/Cargo.toml"; then
    echo "programs vendoring policy: the WASM engine must stay pinned to an exact revision" >&2
    exit 1
fi
test -r "$repo_root/programs/.cargo/config.toml"
if ! grep -q 'replace-with = "vendored-sources"' "$repo_root/programs/.cargo/config.toml"; then
    echo "programs vendoring policy: builds must resolve the engine from programs/vendor" >&2
    exit 1
fi

if command -v rg >/dev/null 2>&1; then
    unsafe_hits=$(rg -n --glob '*.rs' '\bunsafe\s+(fn|trait|impl|extern)|\bunsafe\s*\{' "$repo_root/programs/crates" || true)
else
    unsafe_hits=$(grep -rEn --include='*.rs' '\bunsafe[[:space:]]+(fn|trait|impl|extern)|\bunsafe[[:space:]]*\{' "$repo_root/programs/crates" || true)
fi
if [ -n "$unsafe_hits" ]; then
    printf '%s\n' "$unsafe_hits" >&2
    echo "programs unsafe policy: unsafe code is forbidden" >&2
    exit 1
fi

if command -v rg >/dev/null 2>&1; then
    float_hits=$(rg -n --glob '*.rs' '\b(f32|f64)\b' "$repo_root/programs/crates" || true)
else
    float_hits=$(grep -rEn --include='*.rs' '\b(f32|f64)\b' "$repo_root/programs/crates" || true)
fi
if [ -n "$float_hits" ]; then
    printf '%s\n' "$float_hits" >&2
    echo "programs integer-only policy: floating-point types are forbidden in consensus-adjacent code" >&2
    exit 1
fi

test -r "$repo_root/programs/deny.toml"
echo "programs dependency, vendoring, unsafe and integer-only policies passed"
