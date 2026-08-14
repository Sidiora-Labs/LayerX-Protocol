#ifndef LXP_ARITH_REFERENCE_H
#define LXP_ARITH_REFERENCE_H

#include "layerx/lxp_result.h"

#include <stddef.h>

typedef enum lxp_arith_reference_op {
    LXP_REF_ADD,
    LXP_REF_SUB,
    LXP_REF_MUL,
    LXP_REF_DIV_FLOOR
} lxp_arith_reference_op;

lxp_result lxp_arith_reference_apply(lxp_arith_reference_op operation,
                                     const char *left, const char *right,
                                     char *result, size_t result_capacity,
                                     char *remainder,
                                     size_t remainder_capacity);

#endif
