#include "layerx/lxp_governance.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_identity.h"
#include "layerx/lxp_kernel.h"
#include "layerx/lxp_ledger.h"

#include <string.h>

enum { GRANT_RECORD_MAX = 1U + 4U + 1024U + 1U + 8U };

typedef struct governance_session_activity {
    uint16_t ordinal;
    union {
        lxp_governance_session_grant grant;
        lxp_governance_session_revoke revoke;
    } value;
} governance_session_activity;

static const uint32_t activity_types[] = {
    LX_GOVERNANCE_SESSION_GRANT, LX_GOVERNANCE_SESSION_REVOKE
};

static lxp_result read_exact(lxp_codec_reader *reader, uint8_t *output,
                             size_t length)
{
    lxp_byte_span span;
    lxp_result status = lxp_codec_read_bytes(reader, &span, (uint32_t)length);
    if (status != LXP_OK) return status;
    if (span.length != length) return LXP_ERR_NON_CANONICAL;
    (void)memcpy(output, span.bytes, length);
    return LXP_OK;
}

static lxp_result session_decode(lxp_module_ctx *ctx, uint16_t ordinal,
                                 const uint8_t *payload, size_t length,
                                 void **decoded)
{
    governance_session_activity *value;
    lxp_codec_reader reader;
    lxp_byte_span grant_bytes;
    uint16_t field_count;
    void *allocation;
    lxp_result status;
    if (ctx == NULL || decoded == NULL || payload == NULL ||
        (ordinal != 5U && ordinal != 6U)) return LXP_ERR_UNKNOWN_ACTIVITY;
    status = lxp_ctx_arena_alloc(ctx, sizeof(*value),
                                 _Alignof(governance_session_activity),
                                 &allocation);
    if (status != LXP_OK) return status;
    value = (governance_session_activity *)allocation;
    (void)memset(value, 0, sizeof(*value));
    value->ordinal = ordinal;
    status = lxp_codec_reader_init(&reader, payload, length);
    if (status != LXP_OK) return status;
    status = lxp_codec_read_struct_header(
        &reader, ordinal == 5U ? 0x7105U : 0x7106U);
    if (status != LXP_OK) return status;
    status = lxp_codec_read_u16(&reader, &field_count);
    if (status != LXP_OK || field_count != (ordinal == 5U ? 1U : 3U))
        return status != LXP_OK ? status : LXP_ERR_NON_CANONICAL;
    if (ordinal == 5U) {
        status = lxp_codec_read_bytes(
            &reader, &grant_bytes, LX_GOVERNANCE_SESSION_GRANT_MAX_BYTES);
        if (status == LXP_OK && grant_bytes.length == 0U)
            status = LXP_ERR_MALFORMED_GRANT;
        if (status == LXP_OK)
            status = lxp_grant_decode(grant_bytes.bytes, grant_bytes.length,
                                      &value->value.grant.grant);
        if (status == LXP_OK)
            value->value.grant.canonical_grant = grant_bytes;
    } else {
        status = read_exact(&reader, value->value.revoke.grant_id, 32U);
        if (status == LXP_OK)
            status = lxp_codec_read_u8(&reader, &value->value.revoke.reason);
        if (status == LXP_OK)
            status = lxp_codec_read_u64(
                &reader, &value->value.revoke.effective_sequence);
    }
    if (status == LXP_OK) status = lxp_codec_finish(&reader);
    if (status == LXP_OK) *decoded = value;
    return status;
}

static lxp_result session_validate(lxp_module_ctx *ctx,
                                   const lxp_activity *activity,
                                   const lxp_authority_resolved *authority,
                                   const void *decoded)
{
    const governance_session_activity *value = decoded;
    uint8_t actor[32];
    lxp_result status;
    if (ctx == NULL || activity == NULL || authority == NULL || value == NULL ||
        authority->kind != LXP_AUTHORITY_OWNER)
        return LXP_ERR_AUTH_SCOPE;
    status = lxp_did_id_derive(activity->actor_did.bytes,
                               activity->actor_did.length, actor);
    if (status != LXP_OK ||
        lxp_ct_memcmp(actor, authority->principal, 32U) != 0)
        return LXP_ERR_AUTH_SCOPE;
    if (value->ordinal == 5U) {
        const lxp_authority_grant *grant = &value->value.grant.grant;
        if (lxp_ct_memcmp(grant->grantor, authority->principal, 32U) != 0 ||
            grant->not_before > lxp_ctx_batch_timestamp_ms(ctx) ||
            grant->not_after <= lxp_ctx_batch_timestamp_ms(ctx))
            return LXP_ERR_AUTH_SCOPE;
        return lxp_ctx_charge_gas(
            ctx, value->value.grant.canonical_grant.length + 33U);
    }
    if (value->value.revoke.reason < 1U ||
        value->value.revoke.reason > 5U ||
        value->value.revoke.effective_sequence !=
            lxp_ctx_global_sequence(ctx) ||
        lxp_ct_is_zero(value->value.revoke.grant_id, 32U))
        return LXP_ERR_STALE_REVOCATION;
    return lxp_ctx_charge_gas(ctx, 42U);
}

static lxp_result encode_record(const lxp_byte_span *grant, uint8_t state,
                                uint8_t reason, uint64_t sequence,
                                uint8_t *record, size_t *record_length)
{
    size_t index;
    if (grant == NULL || record == NULL || record_length == NULL ||
        grant->length == 0U || grant->length > 1024U)
        return LXP_ERR_LENGTH_LIMIT;
    record[0] = state;
    record[1] = (uint8_t)(grant->length >> 24U);
    record[2] = (uint8_t)(grant->length >> 16U);
    record[3] = (uint8_t)(grant->length >> 8U);
    record[4] = (uint8_t)grant->length;
    (void)memcpy(record + 5U, grant->bytes, grant->length);
    record[5U + grant->length] = reason;
    for (index = 0U; index < 8U; ++index)
        record[6U + grant->length + index] =
            (uint8_t)(sequence >> (56U - index * 8U));
    *record_length = 14U + grant->length;
    return LXP_OK;
}

static lxp_result session_execute(lxp_module_ctx *ctx,
                                  const lxp_activity *activity,
                                  const lxp_authority_resolved *authority,
                                  const void *decoded,
                                  lxp_effect_buffer *effects)
{
    const governance_session_activity *value = decoded;
    lxp_grant_store *store = lxp_ctx_module_runtime(ctx);
    uint8_t key[33];
    uint8_t record[GRANT_RECORD_MAX];
    const uint8_t *existing;
    size_t existing_length;
    size_t record_length;
    size_t grant_length;
    lxp_byte_span grant_bytes;
    lxp_authority_grant grant;
    lxp_result status;
    (void)activity;
    (void)effects;
    if (ctx == NULL || authority == NULL || value == NULL || store == NULL)
        return LXP_ERR_NON_CANONICAL;
    key[0] = (uint8_t)'s';
    if (value->ordinal == 5U) {
        (void)memcpy(key + 1U, value->value.grant.grant.grant_id, 32U);
        status = lxp_ctx_kv_get(ctx, key, sizeof(key), &existing,
                                &existing_length);
        if (status == LXP_OK) return LXP_ERR_SEQUENCE_REUSED;
        if (status != LXP_ERR_UNKNOWN_FIELD) return status;
        status = encode_record(&value->value.grant.canonical_grant, 1U, 0U,
                               0U, record, &record_length);
        if (status == LXP_OK)
            status = lxp_ctx_kv_put(ctx, key, sizeof(key), record,
                                    record_length);
        if (status == LXP_OK)
            status = lxp_session_grant_store_put(store,
                                                  &value->value.grant.grant);
    } else {
        (void)memcpy(key + 1U, value->value.revoke.grant_id, 32U);
        status = lxp_ctx_kv_get(ctx, key, sizeof(key), &existing,
                                &existing_length);
        if (status != LXP_OK) return status;
        if (existing_length < 14U || existing[0] != 1U)
            return LXP_ERR_AUTH_REVOKED;
        grant_length = ((size_t)existing[1] << 24U) |
                       ((size_t)existing[2] << 16U) |
                       ((size_t)existing[3] << 8U) | (size_t)existing[4];
        if (grant_length == 0U || grant_length > 1024U ||
            existing_length != grant_length + 14U)
            return LXP_ERR_NON_CANONICAL;
        status = lxp_grant_decode(existing + 5U, grant_length, &grant);
        if (status != LXP_OK ||
            lxp_ct_memcmp(grant.grantor, authority->principal, 32U) != 0)
            return status != LXP_OK ? status : LXP_ERR_AUTH_SCOPE;
        grant_bytes = (lxp_byte_span){existing + 5U, grant_length};
        status = encode_record(&grant_bytes, 2U, value->value.revoke.reason,
                               value->value.revoke.effective_sequence, record,
                               &record_length);
        if (status == LXP_OK)
            status = lxp_ctx_kv_put(ctx, key, sizeof(key), record,
                                    record_length);
        if (status == LXP_OK)
            status = lxp_session_grant_store_put(store, &grant);
        if (status == LXP_OK)
            status = lxp_session_grant_store_revoke(
                store, value->value.revoke.grant_id,
                value->value.revoke.reason,
                value->value.revoke.effective_sequence);
    }
    if (status != LXP_OK) return status;
    status = lxp_ctx_emit_event(ctx, value->ordinal, key + 1U, 32U);
    if (status != LXP_OK) return status;
    return lxp_ctx_charge_gas(ctx, record_length + sizeof(key));
}

static lxp_result session_genesis(lxp_module_ctx *ctx,
                                  const uint8_t *manifest, size_t length)
{
    if (ctx == NULL || (manifest == NULL && length != 0U))
        return LXP_ERR_NON_CANONICAL;
    return lxp_ctx_charge_gas(ctx, length);
}

static lxp_result session_epoch(lxp_module_ctx *ctx, uint64_t epoch,
                                uint64_t timestamp)
{
    (void)epoch;
    (void)timestamp;
    return ctx == NULL ? LXP_ERR_NON_CANONICAL : LXP_OK;
}

static lxp_result session_state_root(lxp_module_ctx *ctx, uint8_t root[32])
{
    if (ctx == NULL || root == NULL) return LXP_ERR_NON_CANONICAL;
    return lxp_state_subtree_root(ctx->kernel, LXP_MODULE_GOVERNANCE, root);
}

const lxp_module_iface *lxp_governance_module_iface(void)
{
    static const lxp_module_iface iface = {
        LXP_MODULE_GOVERNANCE, 1U, "governance", activity_types,
        sizeof(activity_types) / sizeof(activity_types[0]), session_genesis,
        session_decode, session_validate, session_execute, session_epoch,
        session_epoch, session_state_root, NULL
    };
    return &iface;
}
