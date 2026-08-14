SANITIZER_CFLAGS := -O1 -g -fno-omit-frame-pointer -fno-optimize-sibling-calls

.PHONY: build-asan build-ubsan build-tsan test-asan test-ubsan test-tsan \
	test-sanitizers

build-asan:
	$(MAKE) --no-print-directory BUILD_DIR=build/asan \
		EXTRA_CFLAGS="$(SANITIZER_CFLAGS) -fsanitize=address" \
		EXTRA_LDFLAGS="-fsanitize=address" build

build-ubsan:
	$(MAKE) --no-print-directory BUILD_DIR=build/ubsan \
		EXTRA_CFLAGS="$(SANITIZER_CFLAGS) -fsanitize=undefined" \
		EXTRA_LDFLAGS="-fsanitize=undefined" build

build-tsan:
	$(MAKE) --no-print-directory BUILD_DIR=build/tsan \
		EXTRA_CFLAGS="$(SANITIZER_CFLAGS) -fsanitize=thread" \
		EXTRA_LDFLAGS="-fsanitize=thread" build

test-asan:
	ASAN_OPTIONS=abort_on_error=1:detect_leaks=1:strict_string_checks=1 \
	$(MAKE) --no-print-directory BUILD_DIR=build/asan \
		EXTRA_CFLAGS="$(SANITIZER_CFLAGS) -fsanitize=address" \
		EXTRA_LDFLAGS="-fsanitize=address" test-sanitizer-suite

test-ubsan:
	UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
	$(MAKE) --no-print-directory BUILD_DIR=build/ubsan \
		EXTRA_CFLAGS="$(SANITIZER_CFLAGS) -fsanitize=undefined" \
		EXTRA_LDFLAGS="-fsanitize=undefined" test-sanitizer-suite

test-tsan:
	TSAN_OPTIONS=halt_on_error=1 \
	$(MAKE) --no-print-directory BUILD_DIR=build/tsan \
		EXTRA_CFLAGS="$(SANITIZER_CFLAGS) -fsanitize=thread" \
		EXTRA_LDFLAGS="-fsanitize=thread" \
		RUN_PREFIX="setarch $(shell uname -m) -R" test-sanitizer-suite

test-sanitizers: test-asan test-ubsan test-tsan
