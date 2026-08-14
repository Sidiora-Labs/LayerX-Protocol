#include "layerx/lxp_authority.h"

#include <stdint.h>
#include <string.h>

int main(void)
{
    lxp_authority_scope scope;
    lxp_authority_scope unchanged;
    (void)memset(&scope, 0, sizeof(scope));
    scope.maximum_per_activity = (lxp_u128){ 0U, 40U };
    scope.maximum_total = (lxp_u128){ 0U, 100U };
    scope.period_length = 10U;
    scope.maximum_per_period = (lxp_u128){ 0U, 60U };
    scope.period_start = 100U;
    if (lxp_authority_charge_allowance(&scope, (lxp_u128){ 0U, 40U }, 100U) !=
            LXP_OK ||
        lxp_authority_charge_allowance(&scope, (lxp_u128){ 0U, 20U }, 109U) !=
            LXP_OK || scope.spent_total.lo != 60U ||
        scope.spent_this_period.lo != 60U) return 1;
    unchanged = scope;
    if (lxp_authority_charge_allowance(&scope, (lxp_u128){ 0U, 1U }, 109U) !=
            LXP_ERR_GRANT_EXHAUSTED ||
        memcmp(&scope, &unchanged, sizeof(scope)) != 0) return 1;
    if (lxp_authority_charge_allowance(&scope, (lxp_u128){ 0U, 40U }, 130U) !=
            LXP_OK || scope.period_start != 130U ||
        scope.spent_this_period.lo != 40U || scope.spent_total.lo != 100U)
        return 1;
    unchanged = scope;
    if (lxp_authority_charge_allowance(&scope, (lxp_u128){ 0U, 1U }, 130U) !=
            LXP_ERR_GRANT_EXHAUSTED ||
        memcmp(&scope, &unchanged, sizeof(scope)) != 0) return 1;
    scope.spent_total = (lxp_u128){ UINT64_MAX, UINT64_MAX };
    scope.maximum_total = (lxp_u128){ 0U, 0U };
    if (lxp_authority_charge_allowance(&scope, (lxp_u128){ 0U, 1U }, 130U) !=
        LXP_ERR_OVERFLOW) return 1;
    return 0;
}
