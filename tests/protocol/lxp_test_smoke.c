#include "lxp_test_harness.h"

#include <string.h>

static int no_op_suite(void)
{
    static const unsigned char expected[] = { 0x4cU, 0x58U, 0x50U };
    unsigned char produced[sizeof(expected)];
    (void)memcpy(produced, expected, sizeof(expected));
    return LXP_ASSERT_BYTES(expected, produced, sizeof(expected)) |
           LXP_ASSERT_U64(UINT64_C(1), UINT64_C(1)) |
           LXP_ASSERT_RESULT(LXP_OK, LXP_OK);
}

int main(int argc, char **argv)
{
    int list_only = argc == 2 && strcmp(argv[1], "--list") == 0;
    if (lxp_test_register("protocol.no-op", no_op_suite) != 0) return 1;
    return lxp_test_run_all(list_only);
}
