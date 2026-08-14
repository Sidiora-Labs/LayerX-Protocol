#include "layerx/lxp_qualification.h"

#include <inttypes.h>
#include <stdio.h>

int main(void)
{
    uint64_t u128_cases = 0U;
    uint64_t u256_cases = 0U;
    uint64_t rounding_cases = 0U;
    lxp_result status = lxp_u128_proof_harness(&u128_cases);
    if (status == LXP_OK) status = lxp_u256_boundary_case(&u256_cases);
    if (status == LXP_OK)
        status = lxp_rounding_direction_check(&rounding_cases);
    if (status != LXP_OK) {
        (void)fprintf(stderr, "arithmetic qualification failed: %d\n",
                      (int)status);
        return 1;
    }
    (void)printf("u128_boundary_cases=%" PRIu64 "\n", u128_cases);
    (void)printf("u256_boundary_cases=%" PRIu64 "\n", u256_cases);
    (void)printf("rounding_cases=%" PRIu64 "\n", rounding_cases);
    return 0;
}
