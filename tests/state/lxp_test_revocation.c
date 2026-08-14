#include "layerx/lxp_authority.h"
#include "layerx/lxp_identity.h"

#include <stdint.h>
#include <string.h>

static void initialize(lxp_authority_grant *grant)
{
    (void)memset(grant, 0, sizeof(*grant));
    grant->kind = LXP_AUTHORITY_DELEGATED_CAPABILITY;
    grant->grantor[0] = 1U;
    grant->grantee[0] = 2U;
    grant->key[0] = 3U;
    grant->scope.module_mask = 1U;
    grant->scope.activity_ordinal_max = 10U;
    grant->scope.asset_id[0] = 4U;
    grant->scope.maximum_per_activity = (lxp_u128){ 0U, 100U };
    grant->scope.maximum_total = (lxp_u128){ 0U, 1000U };
    grant->scope.maximum_per_period = (lxp_u128){ 0U, 100U };
    grant->scope.period_length = 10U;
    grant->scope.purpose_hash[0] = 5U;
    grant->not_before = 10U;
    grant->not_after = 100U;
    grant->grantor_revocation_sequence = 1U;
}

int main(void)
{
    lxp_authority_grant grant;
    lxp_authority_grant narrower;
    lxp_authority_grant wider;
    lxp_identity identity;
    initialize(&grant);
    if (lxp_authority_is_live(&grant, 1U, 50U, 8U) != LXP_OK ||
        lxp_authority_is_live(&grant, 1U, 100U, 8U) != LXP_ERR_AUTH_EXPIRED ||
        lxp_authority_revoke(&grant, 2U, 10U) != LXP_OK ||
        lxp_authority_is_live(&grant, 2U, 50U, 9U) != LXP_OK ||
        lxp_authority_is_live(&grant, 2U, 50U, 10U) != LXP_ERR_AUTH_REVOKED ||
        lxp_authority_revoke(&grant, 2U, 11U) != LXP_ERR_STALE_REVOCATION)
        return 1;
    initialize(&grant);
    narrower = grant;
    narrower.scope.maximum_total = (lxp_u128){ 0U, 900U };
    narrower.scope.module_mask = 1U;
    narrower.not_after = 90U;
    narrower.grantor_revocation_sequence = 2U;
    if (lxp_authority_amend(&grant, &narrower) != LXP_OK ||
        grant.scope.maximum_total.lo != 900U) return 1;
    wider = grant;
    wider.scope.maximum_total = (lxp_u128){ 0U, 901U };
    wider.grantor_revocation_sequence = 3U;
    if (lxp_authority_amend(&grant, &wider) != LXP_ERR_AUTH_SCOPE) return 1;
    (void)memset(&identity, 0, sizeof(identity));
    identity.revocation_sequence = 4U;
    if (lxp_identity_bump_revocation_sequence(&identity, 5U) != LXP_OK ||
        lxp_identity_bump_revocation_sequence(&identity, 5U) !=
            LXP_ERR_STALE_REVOCATION ||
        lxp_authority_is_live(&grant, identity.revocation_sequence, 50U, 1U) !=
            LXP_ERR_AUTH_REVOKED) return 1;
    return 0;
}
