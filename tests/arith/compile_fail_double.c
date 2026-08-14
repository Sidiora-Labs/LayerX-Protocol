#include "layerx/lxp_u128.h"

int main(void)
{
    lxp_u128 amount = 1.0;
    return amount.lo == 0U;
}
