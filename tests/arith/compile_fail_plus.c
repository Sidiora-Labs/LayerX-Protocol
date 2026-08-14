#include "layerx/lxp_u128.h"

int main(void)
{
    lxp_u128 left = { 0U, 1U };
    lxp_u128 right = { 0U, 2U };
    lxp_u128 result = left + right;
    return result.lo == 3U;
}
