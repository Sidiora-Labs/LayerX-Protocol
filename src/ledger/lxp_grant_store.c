#include "layerx/lxp_ledger.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"

#include <string.h>

static lxp_result append(uint8_t *bytes, size_t capacity, size_t *cursor,
                         const void *value, size_t length)
{
    if (bytes == NULL || cursor == NULL || value == NULL ||
        *cursor > capacity || length > capacity - *cursor)
        return LXP_ERR_LENGTH_LIMIT;
    (void)memcpy(bytes + *cursor, value, length);
    *cursor += length;
    return LXP_OK;
}

static lxp_result append_u64(uint8_t *bytes, size_t capacity, size_t *cursor,
                             uint64_t value)
{
    uint8_t encoded[8];
    size_t i;
    for (i = 0U; i < 8U; ++i) encoded[7U - i] = (uint8_t)(value >> (i * 8U));
    return append(bytes, capacity, cursor, encoded, sizeof(encoded));
}

lxp_result lxp_grant_authorization_message(const lxp_payer_grant *grant,
                                           uint8_t *bytes, size_t capacity,
                                           size_t *length)
{
    static const uint8_t tag[] = "LXP:GRANT:v1";
    uint8_t amount[16];
    size_t cursor = 0U;
    lxp_result status;
    if (grant == NULL || bytes == NULL || length == NULL)
        return LXP_ERR_MALFORMED_GRANT;
    status = append(bytes, capacity, &cursor, tag, sizeof(tag) - 1U);
#define APPEND_GRANT(value, count) do { \
    if (status == LXP_OK) status = append(bytes, capacity, &cursor, value, count); \
} while (0)
    APPEND_GRANT(grant->from, 32U);
    APPEND_GRANT(grant->recipient, 32U);
    APPEND_GRANT(grant->asset, 32U);
    if (status == LXP_OK) status = lxp_u128_to_be(grant->per_draw_maximum, amount);
    APPEND_GRANT(amount, sizeof(amount));
    if (status == LXP_OK) status = lxp_u128_to_be(grant->allowance, amount);
    APPEND_GRANT(amount, sizeof(amount));
    {
        uint8_t recurring = grant->recurring ? 1U : 0U;
        APPEND_GRANT(&recurring, 1U);
    }
    if (status == LXP_OK)
        status = append_u64(bytes, capacity, &cursor, grant->window_length);
    if (status == LXP_OK)
        status = append_u64(bytes, capacity, &cursor, grant->expiration);
    APPEND_GRANT(grant->purpose_hash, 32U);
    {
        uint8_t has_reference = grant->has_reference ? 1U : 0U;
        APPEND_GRANT(&has_reference, 1U);
    }
    APPEND_GRANT(grant->reference_hash, 32U);
    if (status == LXP_OK)
        status = append_u64(bytes, capacity, &cursor,
                            grant->revocation_sequence);
    APPEND_GRANT(grant->public_key, 32U);
#undef APPEND_GRANT
    if (status == LXP_OK) *length = cursor;
    return status;
}

lxp_result lxp_verify_payer_grant(const lxp_payer_grant *grant,
                                  const lx_account *from)
{
    uint8_t message[384];
    uint8_t identifier[32];
    size_t message_length;
    lxp_result status;
    if (grant == NULL || from == NULL || lxp_u128_is_zero(grant->per_draw_maximum) ||
        lxp_u128_is_zero(grant->allowance) || grant->expiration == 0U ||
        (grant->recurring && grant->window_length == 0U) ||
        (!grant->recurring && grant->window_length != 0U) ||
        !from->has_authority_key ||
        memcmp(grant->from, from->id, 32U) != 0 ||
        memcmp(grant->public_key, from->authority_key, 32U) != 0 ||
        lxp_ct_is_zero(grant->recipient, 32U) ||
        lxp_ct_is_zero(grant->asset, 32U) ||
        lxp_ct_is_zero(grant->purpose_hash, 32U))
        return LXP_ERR_MALFORMED_GRANT;
    status = lxp_grant_authorization_message(grant, message, sizeof(message),
                                             &message_length);
    if (status == LXP_OK)
        status = lxp_hash_authority(message, message_length, identifier);
    if (status != LXP_OK || memcmp(identifier, grant->grant_id, 32U) != 0)
        return LXP_ERR_MALFORMED_GRANT;
    status = lxp_ed25519_verify(grant->public_key, grant->signature,
                                LXP_DOMAIN_AUTHORITY_HASH,
                                message, message_length);
    return status == LXP_OK ? LXP_OK : LXP_ERR_BAD_SIGNATURE;
}

lxp_result lxp_grant_store_put(lxp_grant_store *store,
                               const lxp_payer_grant *grant,
                               const lx_account *from)
{
    size_t i;
    lxp_result status;
    if (store == NULL) return LXP_ERR_MALFORMED_GRANT;
    status = lxp_verify_payer_grant(grant, from);
    if (status != LXP_OK) return status;
    for (i = 0U; i < store->count; ++i)
        if (memcmp(store->grants[i].grant.grant_id, grant->grant_id, 32U) == 0)
            return LXP_ERR_SEQUENCE_REUSED;
    if (store->count == LXP_GRANT_STORE_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    (void)memset(&store->grants[store->count], 0,
                 sizeof(store->grants[store->count]));
    store->grants[store->count].grant = *grant;
    ++store->count;
    return LXP_OK;
}

lxp_result lxp_grant_draw_record(lxp_grant_state *state, lxp_u128 amount,
                                 uint64_t batch_timestamp)
{
    lxp_u128 next_total;
    lxp_u128 next_period;
    uint64_t window_start = state != NULL ? state->window_start : 0U;
    lxp_result status;
    if (state == NULL || lxp_u128_is_zero(amount)) return LXP_ERR_ZERO_AMOUNT;
    status = lxp_u128_add(state->drawn_total, amount, &next_total);
    if (status != LXP_OK) return LXP_ERR_GRANT_EXHAUSTED;
    if (lxp_u128_cmp(amount, state->grant.per_draw_maximum) > 0)
        return LXP_ERR_GRANT_SCOPE_VIOLATION;
    if (state->grant.recurring) {
        uint64_t current_window = batch_timestamp -
                                  (batch_timestamp % state->grant.window_length);
        lxp_u128 base = state->drawn_this_period;
        if (current_window != state->window_start) {
            base = (lxp_u128){ 0U, 0U };
            window_start = current_window;
        }
        status = lxp_u128_add(base, amount, &next_period);
        if (status != LXP_OK ||
            lxp_u128_cmp(next_period, state->grant.allowance) > 0)
            return LXP_ERR_GRANT_EXHAUSTED;
    } else {
        next_period = next_total;
        if (lxp_u128_cmp(next_total, state->grant.allowance) > 0)
            return LXP_ERR_GRANT_EXHAUSTED;
    }
    state->drawn_total = next_total;
    state->drawn_this_period = next_period;
    state->window_start = window_start;
    return LXP_OK;
}

lxp_result lxp_grant_revoke(lxp_grant_store *store, const uint8_t grant_id[32],
                            uint64_t global_sequence)
{
    size_t i;
    if (store == NULL || grant_id == NULL) return LXP_ERR_MALFORMED_GRANT;
    for (i = 0U; i < store->count; ++i) {
        if (memcmp(store->grants[i].grant.grant_id, grant_id, 32U) == 0) {
            if (global_sequence < store->grants[i].grant.revocation_sequence)
                return LXP_ERR_STALE_REVOCATION;
            store->grants[i].revoked = true;
            store->grants[i].revoked_at_sequence = global_sequence;
            return LXP_OK;
        }
    }
    return LXP_ERR_NO_PAYER_GRANT;
}
