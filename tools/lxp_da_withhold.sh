#!/bin/sh
set -eu

mode=${1:-all}
binary=${2:-build/tests/test_da_unavailable}

run_class() {
    case "$1" in
        activities|receipts|oracle|state-diff|recovery) ;;
        *) echo "unknown DA class: $1" >&2; exit 2 ;;
    esac
    "$binary" "$1"
}

if [ "$mode" = all ]; then
    for class in activities receipts oracle state-diff recovery; do
        run_class "$class"
    done
else
    run_class "$mode"
fi
