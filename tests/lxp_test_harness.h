#ifndef LAYERX_LXP_TEST_HARNESS_H
#define LAYERX_LXP_TEST_HARNESS_H

#include "layerx/lxp_result.h"

#include <stddef.h>
#include <stdint.h>

typedef int (*lxp_test_function)(void);

int lxp_test_register(const char *name, lxp_test_function function);
int lxp_test_run_all(int list_only);
int lxp_test_assert_u64_eq(uint64_t expected, uint64_t produced,
                           const char *file, int line);
int lxp_test_assert_bytes_eq(const void *expected, const void *produced,
                             size_t length, const char *file, int line);
int lxp_test_assert_result(lxp_result expected, lxp_result produced,
                           const char *file, int line);

#define LXP_ASSERT_U64(expected, produced) \
    lxp_test_assert_u64_eq((expected), (produced), __FILE__, __LINE__)
#define LXP_ASSERT_BYTES(expected, produced, length) \
    lxp_test_assert_bytes_eq((expected), (produced), (length), __FILE__, __LINE__)
#define LXP_ASSERT_RESULT(expected, produced) \
    lxp_test_assert_result((expected), (produced), __FILE__, __LINE__)

#endif
