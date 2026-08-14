#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work="$root/build/qualification/replay"
corpus="$work/replay-10m.lxq"
ledger="$work/replay-10m.roots"
expected="$root/tests/vectors/qualification_replay_10m.digest"
mkdir -p "$work"

sources="tests/qualification/lxp_qual_replay.c \
src/protocol/lxp_activity.c src/codec/lxp_codec.c \
src/protocol/lxp_arena.c src/protocol/lxp_u128.c \
src/protocol/lxp_result.c src/crypto/lxp_hash.c src/crypto/lxp_ct.c \
src/protocol/lxp_protocol.c"
flags="-Iinclude -std=c17 -pedantic -Werror -Wall -Wextra -Wconversion \
-Wshadow -Wvla -fno-strict-aliasing -ffp-contract=off"

cd "$root"
command -v gcc-13 >/dev/null 2>&1 || {
    echo "required compiler unavailable: gcc-13" >&2
    exit 1
}
command -v clang-18 >/dev/null 2>&1 || {
    echo "required compiler unavailable: clang-18" >&2
    exit 1
}

gcc-13 $flags -O2 tools/lxp_corpus_gen.c $sources \
    -o "$work/lxp-corpus-gen"
if [ ! -s "$corpus" ] || [ ! -s "$ledger" ] || \
   [ tests/qualification/lxp_qual_replay.c -nt "$corpus" ] || \
   [ tools/lxp_corpus_gen.c -nt "$corpus" ]; then
    "$work/lxp-corpus-gen" "$corpus" "$ledger"
fi

for case_name in gcc-o0 gcc-o2 clang-o0 clang-o2; do
    case "$case_name" in
        gcc-o0) compiler=gcc-13; optimisation=-O0 ;;
        gcc-o2) compiler=gcc-13; optimisation=-O2 ;;
        clang-o0) compiler=clang-18; optimisation=-O0 ;;
        clang-o2) compiler=clang-18; optimisation=-O2 ;;
    esac
    $compiler $flags "$optimisation" tests/test_qual_replay.c $sources \
        -o "$work/$case_name"
done

command -v docker >/dev/null 2>&1 || {
    echo "docker is required for independent musl and AArch64 runners" >&2
    exit 1
}

runner_pids=""
for case_name in gcc-o0 gcc-o2 clang-o0 clang-o2; do
    "$work/$case_name" "$corpus" "$ledger" >"$work/$case_name.digest" &
    runner_pids="$runner_pids $!"
done

docker run --rm --platform linux/amd64 \
    -v "$root:/src:ro" -v "$work:/out" alpine:3.20 sh -ec '
        apk add --no-cache build-base >/dev/null
        cd /src
        cc -Iinclude -std=c17 -pedantic -Werror -Wall -Wextra \
            -Wconversion -Wshadow -Wvla -fno-strict-aliasing \
            -ffp-contract=off -O2 tests/test_qual_replay.c \
            tests/qualification/lxp_qual_replay.c \
            src/protocol/lxp_activity.c src/codec/lxp_codec.c \
            src/protocol/lxp_arena.c src/protocol/lxp_u128.c \
            src/protocol/lxp_result.c src/crypto/lxp_hash.c \
            src/crypto/lxp_ct.c src/protocol/lxp_protocol.c \
            -o /out/musl-o2
        /out/musl-o2 /out/replay-10m.lxq /out/replay-10m.roots \
            >/out/musl-o2.digest
    ' &
runner_pids="$runner_pids $!"

docker run --rm --platform linux/amd64 \
    -v "$root:/src:ro" -v "$work:/out" \
    debian:bookworm-slim sh -ec '
        apt-get update >/dev/null
        DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
            gcc-aarch64-linux-gnu libc6-dev-arm64-cross qemu-user >/dev/null
        cd /src
        aarch64-linux-gnu-gcc -Iinclude -std=c17 -pedantic -Werror \
            -Wall -Wextra -Wconversion -Wshadow -Wvla \
            -fno-strict-aliasing -ffp-contract=off -O2 \
            tests/test_qual_replay.c \
            tests/qualification/lxp_qual_replay.c \
            src/protocol/lxp_activity.c src/codec/lxp_codec.c \
            src/protocol/lxp_arena.c src/protocol/lxp_u128.c \
            src/protocol/lxp_result.c src/crypto/lxp_hash.c \
            src/crypto/lxp_ct.c src/protocol/lxp_protocol.c \
            -o /out/aarch64-o2
        qemu-aarch64 -L /usr/aarch64-linux-gnu /out/aarch64-o2 \
            /out/replay-10m.lxq /out/replay-10m.roots \
            >/out/aarch64-o2.digest
    ' &
runner_pids="$runner_pids $!"

for runner_pid in $runner_pids; do
    wait "$runner_pid"
done

cp "$ledger" "$work/replay-10m-mutated.roots"
printf '\001' | dd of="$work/replay-10m-mutated.roots" bs=1 seek=48 \
    conv=notrunc status=none
if "$work/gcc-o2" "$corpus" "$work/replay-10m-mutated.roots" \
    >"$work/mutated.digest" 2>"$work/mutated.failure"; then
    echo "single-byte root-ledger mutation was accepted" >&2
    exit 1
fi
rg -q 'status=-1002 sequence=10000' "$work/mutated.failure" || {
    cat "$work/mutated.failure" >&2
    echo "single-byte divergence did not fail at sequence 10000" >&2
    exit 1
}

for digest_file in "$work"/*.digest; do
    [ "$digest_file" = "$work/mutated.digest" ] && continue
    cmp "$work/gcc-o2.digest" "$digest_file" || {
        echo "qualification replay byte divergence: $digest_file" >&2
        exit 1
    }
done

if [ ! -f "$expected" ]; then
    echo "published qualification corpus digest is missing: $expected" >&2
    exit 1
fi
cmp "$expected" "$work/gcc-o2.digest"
cat "$work/gcc-o2.digest"
