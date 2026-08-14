#!/bin/sh
set -eu

make --no-print-directory test-replay-golden-local
make --no-print-directory BUILD_DIR=build/replay-clang CC=clang-18 \
    OPT_LEVEL=-O0 test-replay-golden-local

if [ "${LXP_REQUIRE_NON_X86:-0}" = 1 ]; then
    machine=$(uname -m)
    case "$machine" in
        x86_64|i?86) echo "non-x86 replay runner required" >&2; exit 1 ;;
    esac
fi
