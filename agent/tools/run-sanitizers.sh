#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
manifest="$repo_root/agent/Cargo.toml"
target=x86_64-unknown-linux-gnu

RUSTC_BOOTSTRAP=1 RUSTFLAGS='-Zsanitizer=address' \
    cargo test --manifest-path "$manifest" --locked --workspace --target "$target"

probe_dir=$(mktemp -d)
probe_log="$probe_dir/thread-sanitizer.log"
trap 'rm -rf -- "$probe_dir"' EXIT HUP INT TERM
if CARGO_TARGET_DIR="$probe_dir/target" RUSTC_BOOTSTRAP=1 \
    RUSTFLAGS='-Zsanitizer=thread' \
    cargo test --manifest-path "$manifest" --locked --workspace \
        --target "$target" --no-run >"$probe_log" 2>&1; then
    CARGO_TARGET_DIR="$probe_dir/target" RUSTC_BOOTSTRAP=1 \
        RUSTFLAGS='-Zsanitizer=thread' \
        cargo test --manifest-path "$manifest" --locked --workspace \
            --target "$target"
elif grep -q 'ABI mismatch' "$probe_log"; then
    echo "thread sanitizer unavailable: pinned standard library is not TSan-instrumented"
else
    cat "$probe_log" >&2
    exit 1
fi
