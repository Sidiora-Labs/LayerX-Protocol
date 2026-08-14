#include "layerx/lxp_u128.h"

int lxp_arith_nofloat_gate(void)
{
    lxp_u128 result;
    return lxp_u128_add((lxp_u128){ 0U, 1U }, (lxp_u128){ 0U, 1U },
                        &result) == LXP_OK && result.hi == 0U && result.lo == 2U;
}

int lxp_test_arith_compile_fail(void)
{
    return lxp_arith_nofloat_gate();
}

int main(void)
{
    return lxp_test_arith_compile_fail() ? 0 : 1;
}
