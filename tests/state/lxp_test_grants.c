#include "layerx/lxp_authority.h"

#include <stdint.h>
#include <string.h>

int main(void)
{
    uint8_t storage[1024];
    uint8_t second_storage[1024];
    uint8_t grantor[32] = { 1U };
    uint8_t session_key[32] = { 2U };
    lxp_authority_grant grant;
    lxp_arena arena;
    lxp_arena second_arena;
    lxp_byte_span first;
    lxp_byte_span second;
    uint8_t first_id[32];
    uint8_t amended_id[32];
    if (lxp_session_key_bind(&grant, grantor, session_key, UINT64_C(3), 1U,
                             9U, 10U, 20U, 0U) != LXP_OK ||
        lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_grant_encode(&grant, &arena, &first) != LXP_OK ||
        lxp_grant_id_compute(&grant, first_id) != LXP_OK ||
        memcmp(first_id, grant.grant_id, 32U) != 0) return 1;
    if (lxp_arena_init(&second_arena, second_storage, sizeof(second_storage)) !=
        LXP_OK || lxp_grant_encode(&grant, &second_arena, &second) != LXP_OK ||
        first.length != second.length ||
        memcmp(first.bytes, second.bytes, first.length) != 0) return 1;
    grant.not_after = 19U;
    if (lxp_grant_id_compute(&grant, amended_id) != LXP_OK ||
        memcmp(first_id, amended_id, 32U) == 0) return 1;
    if (lxp_session_key_bind(&grant, grantor, session_key, 0U, 1U, 9U,
                             10U, 20U, 0U) != LXP_ERR_MALFORMED_GRANT ||
        lxp_session_key_bind(&grant, grantor, session_key, 1U, 9U, 1U,
                             10U, 20U, 0U) != LXP_ERR_MALFORMED_GRANT ||
        lxp_session_key_bind(&grant, grantor, session_key, 1U, 1U, 9U,
                             10U, 0U, 0U) != LXP_ERR_MALFORMED_GRANT)
        return 1;
    (void)memset(&grant, 0, sizeof(grant));
    grant.kind = LXP_AUTHORITY_DELEGATED_CAPABILITY;
    grant.not_after = 20U;
    if (lxp_arena_reset(&arena, 0U) != LXP_OK ||
        lxp_grant_encode(&grant, &arena, &first) != LXP_ERR_MALFORMED_GRANT)
        return 1;
    return 0;
}
