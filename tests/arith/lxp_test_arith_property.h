#ifndef LXP_TEST_ARITH_PROPERTY_H
#define LXP_TEST_ARITH_PROPERTY_H

#include "layerx/lxp_u128.h"

#include <stdint.h>

int lxp_test_arith_property(lxp_u128 left, lxp_u128 right,
                            uint32_t basis_points);

#endif
