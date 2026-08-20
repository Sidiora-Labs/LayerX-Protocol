#!/bin/sh
set -eu

emulator_url=${1:?emulator URL is required}
testnet_url=${2:?testnet URL is required}
corpus=${3:?canonical activity corpus is required}

if [ ! -f "$corpus" ]; then
    echo "conformance corpus does not exist: $corpus" >&2
    exit 2
fi

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT HUP INT TERM

outcome() {
    response=$1
    status=$2
    result=$(sed -n 's/.*"result_code":[[:space:]]*\(-\{0,1\}[0-9][0-9]*\).*/result:\1/p' "$response")
    if [ -z "$result" ]; then
        result=$(sed -n 's/.*"code":[[:space:]]*"\([^"]*\)".*/error:\1/p' "$response")
    fi
    printf '%s|%s\n' "$(cat "$status")" "$result"
}

line_number=0
while IFS= read -r activity || [ -n "$activity" ]; do
    line_number=$((line_number + 1))
    case "$activity" in
        ''|'#'*) continue ;;
    esac
    case "$activity" in
        *[!0123456789abcdefABCDEF]*)
            echo "non-hex activity at corpus line $line_number" >&2
            exit 2
            ;;
    esac
    payload="{\"activity\":\"$activity\"}"
    curl --fail-with-body --silent --show-error \
        --output "$work/emulator-$line_number.json" \
        --write-out '%{http_code}' \
        --header 'Content-Type: application/json' \
        --data "$payload" "$emulator_url/v1/activities" \
        >"$work/emulator-$line_number.status" || true
    curl --fail-with-body --silent --show-error \
        --output "$work/testnet-$line_number.json" \
        --write-out '%{http_code}' \
        --header 'Content-Type: application/json' \
        --data "$payload" "$testnet_url/v1/activities" \
        >"$work/testnet-$line_number.status" || true
    emulator_outcome=$(outcome "$work/emulator-$line_number.json" "$work/emulator-$line_number.status")
    testnet_outcome=$(outcome "$work/testnet-$line_number.json" "$work/testnet-$line_number.status")
    if [ "$emulator_outcome" != "$testnet_outcome" ]; then
        echo "behaviour divergence at corpus line $line_number: emulator=$emulator_outcome testnet=$testnet_outcome" >&2
        exit 1
    fi
done <"$corpus"
