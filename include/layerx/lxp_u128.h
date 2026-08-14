#ifndef LAYERX_LXP_U128_H
#define LAYERX_LXP_U128_H

#include "layerx/lxp_result.h"

#include <stdbool.h>
#include <stdint.h>

typedef struct lxp_u128 {
    uint64_t hi;
    uint64_t lo;
} lxp_u128;
#define lxp_u128 lxp_u128

/*
 * Every amount operation is checked and failure leaves output unchanged.
 * Ignoring a returned lxp_result is a protocol conformance failure, not a
 * style issue. Bare arithmetic on amount structs is intentionally impossible.
 */
lxp_result lxp_u128_add(lxp_u128 left, lxp_u128 right, lxp_u128 *out);
lxp_result lxp_u128_sub(lxp_u128 left, lxp_u128 right, lxp_u128 *out);
int lxp_u128_cmp(lxp_u128 left, lxp_u128 right);
bool lxp_u128_is_zero(lxp_u128 value);
lxp_result lxp_u128_from_be(const uint8_t bytes[16], lxp_u128 *out);
lxp_result lxp_u128_to_be(lxp_u128 value, uint8_t bytes[16]);

/* Wider intermediates use four least-significant-first words. */
typedef struct lxp_u256 {
    uint64_t words[4];
} lxp_u256;
#define lxp_u256 lxp_u256

/* Exact widening multiplication; no input bits are discarded. */
lxp_result lxp_u128_mul(lxp_u128 left, lxp_u128 right, lxp_u256 *out);
/* Checked addition; failure leaves out unchanged. */
lxp_result lxp_u256_add(lxp_u256 left, lxp_u256 right, lxp_u256 *out);
/* Floor division. Both the quotient and explicit residue are returned. */
lxp_result lxp_u256_div_floor(lxp_u256 dividend, lxp_u128 divisor,
                              lxp_u128 *quotient, lxp_u128 *remainder);

typedef struct lxp_i128 {
    bool negative;
    lxp_u128 magnitude;
} lxp_i128;
#define lxp_i128 lxp_i128

/* Signed-magnitude operations always canonicalize zero to non-negative. */
lxp_result lxp_i128_add(lxp_i128 left, lxp_i128 right, lxp_i128 *out);
lxp_result lxp_i128_sub(lxp_i128 left, lxp_i128 right, lxp_i128 *out);

#define LXP_BASIS_POINTS_ONE UINT32_C(10000)

/* Multiply first at full width, then floor-divide and expose the residue. */
lxp_result lxp_u128_mul_div_floor(lxp_u128 value, lxp_u128 multiplier,
                                  lxp_u128 divisor, lxp_u128 *quotient,
                                  lxp_u128 *remainder);
/* Payout ratios round down; fee ratios round up. */
lxp_result lxp_u128_mul_bps_floor(lxp_u128 value, uint32_t basis_points,
                                  lxp_u128 *out);
lxp_result lxp_u128_mul_bps_ceil(lxp_u128 value, uint32_t basis_points,
                                 lxp_u128 *out);

#endif
