#!/bin/sh
set -eu

program_dir=$(cd "$(dirname "$0")" && pwd)
sdk_dir=$(cd "$program_dir/../.." && pwd)
build_dir="$program_dir/build"
artifact="$build_dir/paid-counter.wasm"
lint="$build_dir/determinism-lint"

CLANG=${CLANG:-clang}
HOSTCC=${HOSTCC:-cc}
WARNINGS="-std=c17 -pedantic -Wall -Wextra -Wconversion -Wshadow -Wvla"
if [ "${STRICT:-0}" = "1" ]; then
	WARNINGS="$WARNINGS -Werror"
fi

sources="$sdk_dir/src/abi.c $sdk_dir/src/amount.c $sdk_dir/src/bytes.c \
$sdk_dir/src/calls.c $sdk_dir/src/capability.c $sdk_dir/src/entry.c \
$sdk_dir/src/events.c \
$sdk_dir/src/receipts.c $sdk_dir/src/runtime.c $sdk_dir/src/storage.c \
$sdk_dir/src/transfers.c $program_dir/src/program.c"

build_lint() {
	mkdir -p "$build_dir"
	"$HOSTCC" -std=c17 -O2 -Wall -Wextra -o "$lint" \
		"$sdk_dir/tools/determinism_lint.c"
}

lint_sources() {
	# shellcheck disable=SC2086
	"$lint" $sources "$sdk_dir/src/host.h" "$sdk_dir/src/internal.h" \
		"$sdk_dir/include/layerx/program.h"
}

compile() {
	mkdir -p "$build_dir"
	# shellcheck disable=SC2086
	"$CLANG" \
		--target=wasm32-unknown-unknown \
		$WARNINGS \
		-Oz \
		-ffreestanding \
		-fno-builtin \
		-ffunction-sections \
		-fdata-sections \
		-nostdlib \
		-I "$sdk_dir/include" \
		-Wl,--no-entry \
		-Wl,--export-memory \
		-Wl,--gc-sections \
		-Wl,--strip-all \
		-o "$artifact" \
		$sources
}

case "${1:-all}" in
all)
	build_lint
	lint_sources
	compile
	"$lint" "$artifact"
	printf '%s\n' "$artifact"
	;;
compile)
	compile
	printf '%s\n' "$artifact"
	;;
lint)
	build_lint
	lint_sources
	"$lint" "$artifact"
	;;
clean)
	rm -rf "$build_dir"
	;;
*)
	printf 'usage: build.sh [all|compile|lint|clean]\n' >&2
	exit 2
	;;
esac
