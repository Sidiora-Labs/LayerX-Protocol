#include "lxp_test_harness.h"

#include <inttypes.h>
#include <stdio.h>
#include <string.h>

enum { LXP_TEST_CAPACITY = 1024 };

typedef struct lxp_test_case {
    const char *name;
    lxp_test_function function;
} lxp_test_case;

static lxp_test_case registry[LXP_TEST_CAPACITY];
static size_t registry_length;

int lxp_test_register(const char *name, lxp_test_function function)
{
    if (name == NULL || name[0] == '\0' || function == NULL ||
        registry_length >= (size_t)LXP_TEST_CAPACITY) {
        return 1;
    }
    registry[registry_length].name = name;
    registry[registry_length].function = function;
    ++registry_length;
    return 0;
}

int lxp_test_run_all(int list_only)
{
    size_t i;
    int failures = 0;

    for (i = 0U; i < registry_length; ++i) {
        if (list_only != 0) {
            (void)printf("%s\n", registry[i].name);
            continue;
        }
        if (registry[i].function() != 0) {
            (void)fprintf(stderr, "FAIL %s\n", registry[i].name);
            ++failures;
        } else {
            (void)printf("PASS %s\n", registry[i].name);
        }
    }
    return failures == 0 ? 0 : 1;
}

int lxp_test_assert_u64_eq(uint64_t expected, uint64_t produced,
                           const char *file, int line)
{
    if (expected == produced) return 0;
    (void)fprintf(stderr, "%s:%d expected=%" PRIu64 " produced=%" PRIu64 "\n",
                  file, line, expected, produced);
    return 1;
}

static void print_hex(const uint8_t *bytes, size_t length)
{
    size_t i;
    for (i = 0U; i < length; ++i) (void)fprintf(stderr, "%02x", bytes[i]);
}

int lxp_test_assert_bytes_eq(const void *expected, const void *produced,
                             size_t length, const char *file, int line)
{
    if (length == 0U || (expected != NULL && produced != NULL &&
                         memcmp(expected, produced, length) == 0)) return 0;
    (void)fprintf(stderr, "%s:%d expected=", file, line);
    if (expected == NULL) (void)fprintf(stderr, "<null>");
    else print_hex((const uint8_t *)expected, length);
    (void)fprintf(stderr, " produced=");
    if (produced == NULL) (void)fprintf(stderr, "<null>");
    else print_hex((const uint8_t *)produced, length);
    (void)fprintf(stderr, "\n");
    return 1;
}

int lxp_test_assert_result(lxp_result expected, lxp_result produced,
                           const char *file, int line)
{
    if (expected == produced) return 0;
    (void)fprintf(stderr, "%s:%d expected=%s(%d) produced=%s(%d)\n",
                  file, line, lxp_result_name(expected), expected,
                  lxp_result_name(produced), produced);
    return 1;
}
