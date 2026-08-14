#include "layerx/lxp_authority.h"

#include <stdint.h>
#include <string.h>

static void initialize(lxp_authority_grant *grant)
{
    (void)memset(grant, 0, sizeof(*grant));
    grant->kind = LXP_AUTHORITY_DELEGATED_CAPABILITY;
    grant->grantor[0] = 1U;
    grant->grantee[0] = 2U;
    grant->key[0] = 3U;
    grant->grant_id[0] = 4U;
    grant->scope.module_mask = UINT64_C(1) << 5U;
    grant->scope.activity_ordinal_min = 2U;
    grant->scope.activity_ordinal_max = 4U;
}

int main(void)
{
    lxp_authority_grant grant;
    lxp_authority_resolved first;
    lxp_authority_resolved second;
    uint8_t actor[32] = { 2U };
    initialize(&grant);
    if (lxp_authority_resolve(&grant, actor, UINT32_C(0x00050003),
                              UINT64_C(1) << 5U, 1U, 5U, true, &first) !=
            LXP_OK ||
        lxp_authority_resolve(&grant, actor, UINT32_C(0x00050003),
                              UINT64_C(1) << 5U, 1U, 5U, true, &second) !=
            LXP_OK || memcmp(first.authority_hash, second.authority_hash, 32U) != 0 ||
        memcmp(first.principal, grant.grantor, 32U) != 0) return 1;
    if (lxp_authority_resolve(&grant, actor, UINT32_C(0x00050006),
                              UINT64_C(1) << 5U, 1U, 6U, true, &first) !=
        LXP_ERR_AUTH_SCOPE) return 1;
    grant.scope.module_mask |= UINT64_C(1) << 6U;
    if (lxp_authority_resolve(&grant, actor, UINT32_C(0x00050003),
                              UINT64_C(1) << 5U, 1U, 5U, true, &first) !=
        LXP_ERR_AUTH_SCOPE) return 1;
    grant.kind = (lxp_authority_kind)7;
    if (lxp_authority_resolve(&grant, actor, UINT32_C(0x00050003),
                              UINT64_MAX, 0U, UINT16_MAX, true, &first) !=
        LXP_ERR_UNKNOWN_AUTHORITY_KIND) return 1;
    return 0;
}
