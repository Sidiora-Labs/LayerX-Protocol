#include "layerx/lxp_authority.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"
#include "layerx/lxp_identity.h"
#include "layerx/lxp_protocol.h"

#include <stdlib.h>
#include <string.h>

static lxp_result write_amount(lxp_codec_writer *writer, lxp_u128 amount)
{
    return lxp_codec_write_u128(writer, amount);
}

static lxp_result validate_grant(const lxp_authority_grant *grant)
{
    if (grant->kind < LXP_AUTHORITY_OWNER ||
        grant->kind > LXP_AUTHORITY_PROTOCOL_MODULE)
        return LXP_ERR_UNKNOWN_AUTHORITY_KIND;
    if (grant->not_after == 0U || grant->not_after <= grant->not_before ||
        lxp_ct_is_zero(grant->grantee, 32U) || lxp_ct_is_zero(grant->key, 32U) ||
        grant->scope.module_mask == 0U ||
        grant->scope.activity_ordinal_min > grant->scope.activity_ordinal_max)
        return LXP_ERR_MALFORMED_GRANT;
    if (grant->kind == LXP_AUTHORITY_SESSION_KEY &&
        grant->grantor_revocation_sequence == 0U)
        return LXP_ERR_MALFORMED_GRANT;
    if (grant->kind == LXP_AUTHORITY_DELEGATED_CAPABILITY ||
        grant->kind == LXP_AUTHORITY_BUDGET_ALLOWANCE) {
        if (lxp_ct_is_zero(grant->scope.asset_id, 32U) ||
            lxp_u128_is_zero(grant->scope.maximum_per_activity) ||
            lxp_ct_is_zero(grant->scope.purpose_hash, 32U) ||
            grant->grantor_revocation_sequence == 0U ||
            (lxp_u128_is_zero(grant->scope.maximum_total) &&
             (grant->scope.period_length == 0U ||
              lxp_u128_is_zero(grant->scope.maximum_per_period))))
            return LXP_ERR_MALFORMED_GRANT;
    }
    return LXP_OK;
}

lxp_result lxp_grant_encode(const lxp_authority_grant *grant,
                            lxp_arena *arena, lxp_byte_span *encoded)
{
    lxp_codec_writer writer;
    lxp_result status;
    if (grant == NULL || arena == NULL || encoded == NULL)
        return LXP_ERR_MALFORMED_GRANT;
    status = validate_grant(grant);
    if (status != LXP_OK) return status;
    status = lxp_codec_writer_init(&writer, arena, 1024U);
    if (status != LXP_OK) return status;
#define WRITE(expression) do { status = (expression); if (status != LXP_OK) return status; } while (0)
    WRITE(lxp_codec_write_struct_header(&writer, 0x2001U));
    WRITE(lxp_codec_write_u8(&writer, 1U));
    WRITE(lxp_codec_write_bytes(&writer, grant->grantor, 32U, 32U));
    WRITE(lxp_codec_write_bytes(&writer, grant->grantee, 32U, 32U));
    WRITE(lxp_codec_write_u8(&writer, (uint8_t)grant->kind));
    WRITE(lxp_codec_write_bytes(&writer, grant->key, 32U, 32U));
    WRITE(lxp_codec_write_u64(&writer, grant->scope.module_mask));
    WRITE(lxp_codec_write_u16(&writer, grant->scope.activity_ordinal_min));
    WRITE(lxp_codec_write_u16(&writer, grant->scope.activity_ordinal_max));
    WRITE(lxp_codec_write_bytes(&writer, grant->scope.asset_id, 32U, 32U));
    WRITE(write_amount(&writer, grant->scope.maximum_per_activity));
    WRITE(write_amount(&writer, grant->scope.maximum_total));
    WRITE(write_amount(&writer, grant->scope.spent_total));
    WRITE(lxp_codec_write_u64(&writer, grant->scope.period_length));
    WRITE(write_amount(&writer, grant->scope.maximum_per_period));
    WRITE(write_amount(&writer, grant->scope.spent_this_period));
    WRITE(lxp_codec_write_u64(&writer, grant->scope.period_start));
    WRITE(lxp_codec_write_bytes(&writer, grant->scope.purpose_hash, 32U, 32U));
    WRITE(lxp_codec_write_u64(&writer, grant->not_before));
    WRITE(lxp_codec_write_u64(&writer, grant->not_after));
    WRITE(lxp_codec_write_u64(&writer, grant->grantor_revocation_sequence));
    WRITE(lxp_codec_write_u8(&writer, grant->revoked ? 1U : 0U));
    WRITE(lxp_codec_write_u64(&writer, grant->revoked_at_sequence));
    WRITE(lxp_codec_write_bytes(&writer, grant->grantor_signature, 64U, 64U));
#undef WRITE
    encoded->bytes = writer.bytes;
    encoded->length = writer.length;
    return LXP_OK;
}

lxp_result lxp_grant_decode(const uint8_t *bytes, size_t length,
                            lxp_authority_grant *grant)
{
    lxp_codec_reader reader;
    lxp_byte_span span;
    uint8_t version;
    uint8_t kind;
    uint8_t revoked;
    lxp_authority_grant decoded;
    lxp_result status;
#define READ(expression) do { status = (expression); if (status != LXP_OK) return status; } while (0)
#define READ_EXACT(target, count) do { \
    READ(lxp_codec_read_bytes(&reader, &span, count)); \
    if (span.length != count) return LXP_ERR_NON_CANONICAL; \
    (void)memcpy(target, span.bytes, count); \
} while (0)
    if (bytes == NULL || grant == NULL || length == 0U || length > 1024U)
        return LXP_ERR_MALFORMED_GRANT;
    (void)memset(&decoded, 0, sizeof(decoded));
    READ(lxp_codec_reader_init(&reader, bytes, length));
    READ(lxp_codec_read_struct_header(&reader, 0x2001U));
    READ(lxp_codec_read_u8(&reader, &version));
    if (version != 1U) return LXP_ERR_VERSION_UNSUPPORTED;
    READ_EXACT(decoded.grantor, 32U);
    READ_EXACT(decoded.grantee, 32U);
    READ(lxp_codec_read_u8(&reader, &kind));
    decoded.kind = (lxp_authority_kind)kind;
    READ_EXACT(decoded.key, 32U);
    READ(lxp_codec_read_u64(&reader, &decoded.scope.module_mask));
    READ(lxp_codec_read_u16(&reader, &decoded.scope.activity_ordinal_min));
    READ(lxp_codec_read_u16(&reader, &decoded.scope.activity_ordinal_max));
    READ_EXACT(decoded.scope.asset_id, 32U);
    READ(lxp_codec_read_u128(&reader, &decoded.scope.maximum_per_activity));
    READ(lxp_codec_read_u128(&reader, &decoded.scope.maximum_total));
    READ(lxp_codec_read_u128(&reader, &decoded.scope.spent_total));
    READ(lxp_codec_read_u64(&reader, &decoded.scope.period_length));
    READ(lxp_codec_read_u128(&reader, &decoded.scope.maximum_per_period));
    READ(lxp_codec_read_u128(&reader, &decoded.scope.spent_this_period));
    READ(lxp_codec_read_u64(&reader, &decoded.scope.period_start));
    READ_EXACT(decoded.scope.purpose_hash, 32U);
    READ(lxp_codec_read_u64(&reader, &decoded.not_before));
    READ(lxp_codec_read_u64(&reader, &decoded.not_after));
    READ(lxp_codec_read_u64(&reader, &decoded.grantor_revocation_sequence));
    READ(lxp_codec_read_u8(&reader, &revoked));
    if (revoked > 1U) return LXP_ERR_NON_CANONICAL;
    decoded.revoked = revoked != 0U;
    READ(lxp_codec_read_u64(&reader, &decoded.revoked_at_sequence));
    READ_EXACT(decoded.grantor_signature, 64U);
    READ(lxp_codec_finish(&reader));
    if (decoded.kind != LXP_AUTHORITY_SESSION_KEY || decoded.revoked ||
        decoded.revoked_at_sequence != 0U ||
        lxp_ct_memcmp(decoded.grantor, decoded.grantee, 32U) != 0)
        return LXP_ERR_MALFORMED_GRANT;
    status = validate_grant(&decoded);
    if (status == LXP_OK)
        status = lxp_grant_id_compute(&decoded, decoded.grant_id);
    if (status == LXP_OK) *grant = decoded;
    return status;
#undef READ_EXACT
#undef READ
}

lxp_result lxp_grant_id_compute(const lxp_authority_grant *grant,
                                uint8_t grant_id[32])
{
    uint8_t *storage;
    lxp_arena arena;
    lxp_byte_span encoded;
    lxp_result status;
    if (grant_id == NULL) return LXP_ERR_MALFORMED_GRANT;
    storage = malloc(1024U);
    if (storage == NULL) return LXP_ERR_IO;
    status = lxp_arena_init(&arena, storage, 1024U);
    if (status == LXP_OK) status = lxp_grant_encode(grant, &arena, &encoded);
    if (status == LXP_OK)
        status = lxp_hash_authority(encoded.bytes, encoded.length, grant_id);
    lxp_secure_zero(storage, 1024U);
    free(storage);
    return status;
}

lxp_result lxp_session_key_bind(lxp_authority_grant *grant,
                                const uint8_t grantor[32],
                                const uint8_t session_key[32],
                                uint64_t module_mask,
                                uint16_t ordinal_min,
                                uint16_t ordinal_max,
                                uint64_t not_before, uint64_t not_after,
                                uint64_t revocation_sequence)
{
    if (grant == NULL || grantor == NULL || session_key == NULL ||
        module_mask == 0U || ordinal_min > ordinal_max || not_after == 0U ||
        not_after <= not_before) return LXP_ERR_MALFORMED_GRANT;
    (void)memset(grant, 0, sizeof(*grant));
    (void)memcpy(grant->grantor, grantor, 32U);
    (void)memcpy(grant->grantee, grantor, 32U);
    (void)memcpy(grant->key, session_key, 32U);
    grant->kind = LXP_AUTHORITY_SESSION_KEY;
    grant->scope.module_mask = module_mask;
    grant->scope.activity_ordinal_min = ordinal_min;
    grant->scope.activity_ordinal_max = ordinal_max;
    grant->not_before = not_before;
    grant->not_after = not_after;
    grant->grantor_revocation_sequence = revocation_sequence;
    return lxp_grant_id_compute(grant, grant->grant_id);
}

lxp_result lxp_authority_hash(lxp_authority_kind kind,
                              const uint8_t grant_id[32],
                              const uint8_t verified_key[32],
                              uint8_t authority_hash[32])
{
    uint8_t preimage[65];
    if (kind < LXP_AUTHORITY_OWNER || kind > LXP_AUTHORITY_PROTOCOL_MODULE ||
        grant_id == NULL || verified_key == NULL || authority_hash == NULL)
        return LXP_ERR_UNKNOWN_AUTHORITY_KIND;
    preimage[0] = (uint8_t)kind;
    (void)memcpy(preimage + 1U, grant_id, 32U);
    (void)memcpy(preimage + 33U, verified_key, 32U);
    return lxp_hash_authority(preimage, sizeof(preimage), authority_hash);
}

lxp_result lxp_authority_check_scope(const lxp_authority_scope *scope,
                                     uint32_t activity_type,
                                     uint64_t declared_module_mask,
                                     uint16_t declared_ordinal_min,
                                     uint16_t declared_ordinal_max)
{
    uint16_t module = (uint16_t)(activity_type >> 16U);
    uint16_t ordinal = (uint16_t)activity_type;
    uint64_t module_bit;
    if (scope == NULL || module >= 64U) return LXP_ERR_AUTH_SCOPE;
    module_bit = UINT64_C(1) << module;
    if ((scope->module_mask & module_bit) == 0U ||
        ordinal < scope->activity_ordinal_min ||
        ordinal > scope->activity_ordinal_max ||
        (scope->module_mask & ~declared_module_mask) != 0U ||
        scope->activity_ordinal_min < declared_ordinal_min ||
        scope->activity_ordinal_max > declared_ordinal_max)
        return LXP_ERR_AUTH_SCOPE;
    return LXP_OK;
}

lxp_result lxp_authority_resolve(const lxp_authority_grant *grant,
                                 const uint8_t actor[32],
                                 uint32_t activity_type,
                                 uint64_t declared_module_mask,
                                 uint16_t declared_ordinal_min,
                                 uint16_t declared_ordinal_max,
                                 bool signature_valid,
                                 lxp_authority_resolved *resolved)
{
    lxp_result status;
    if (grant == NULL || actor == NULL || resolved == NULL)
        return LXP_ERR_MALFORMED_GRANT;
    if (grant->kind < LXP_AUTHORITY_OWNER ||
        grant->kind > LXP_AUTHORITY_PROTOCOL_MODULE)
        return LXP_ERR_UNKNOWN_AUTHORITY_KIND;
    if (!signature_valid) return LXP_ERR_BAD_SIGNATURE;
    if (grant->revoked) return LXP_ERR_AUTH_REVOKED;
    if (lxp_ct_memcmp(actor, grant->grantee, 32U) != 0)
        return LXP_ERR_UNAUTHORIZED_DEBIT;
    status = lxp_authority_check_scope(&grant->scope, activity_type,
                                       declared_module_mask,
                                       declared_ordinal_min,
                                       declared_ordinal_max);
    if (status != LXP_OK) return status;
    (void)memcpy(resolved->actor, actor, 32U);
    (void)memcpy(resolved->principal, grant->grantor, 32U);
    (void)memcpy(resolved->verified_key, grant->key, 32U);
    resolved->kind = grant->kind;
    resolved->scope = &grant->scope;
    return lxp_authority_hash(grant->kind, grant->grant_id, grant->key,
                              resolved->authority_hash);
}

lxp_result lxp_authority_period_roll(lxp_authority_scope *scope,
                                     uint64_t batch_timestamp)
{
    uint64_t elapsed;
    uint64_t periods;
    if (scope == NULL) return LXP_ERR_NON_CANONICAL;
    if (scope->period_length == 0U) return LXP_OK;
    if (batch_timestamp < scope->period_start) return LXP_ERR_NOT_YET_VALID;
    elapsed = batch_timestamp - scope->period_start;
    periods = elapsed / scope->period_length;
    if (periods != 0U) {
        scope->period_start += periods * scope->period_length;
        scope->spent_this_period = (lxp_u128){ 0U, 0U };
    }
    return LXP_OK;
}

lxp_result lxp_authority_spend_check(const lxp_authority_scope *scope,
                                     lxp_u128 amount)
{
    lxp_u128 total;
    lxp_u128 period;
    lxp_result status;
    if (scope == NULL) return LXP_ERR_NON_CANONICAL;
    if (lxp_u128_cmp(amount, scope->maximum_per_activity) > 0)
        return LXP_ERR_GRANT_EXHAUSTED;
    status = lxp_u128_add(scope->spent_total, amount, &total);
    if (status != LXP_OK) return status;
    if (!lxp_u128_is_zero(scope->maximum_total) &&
        lxp_u128_cmp(total, scope->maximum_total) > 0)
        return LXP_ERR_GRANT_EXHAUSTED;
    status = lxp_u128_add(scope->spent_this_period, amount, &period);
    if (status != LXP_OK) return status;
    if (scope->period_length != 0U &&
        lxp_u128_cmp(period, scope->maximum_per_period) > 0)
        return LXP_ERR_GRANT_EXHAUSTED;
    return LXP_OK;
}

lxp_result lxp_authority_charge_allowance(lxp_authority_scope *scope,
                                          lxp_u128 amount,
                                          uint64_t batch_timestamp)
{
    lxp_authority_scope updated;
    lxp_result status;
    if (scope == NULL) return LXP_ERR_NON_CANONICAL;
    updated = *scope;
    status = lxp_authority_period_roll(&updated, batch_timestamp);
    if (status != LXP_OK) return status;
    status = lxp_authority_spend_check(&updated, amount);
    if (status != LXP_OK) return status;
    status = lxp_u128_add(updated.spent_total, amount, &updated.spent_total);
    if (status != LXP_OK) return status;
    status = lxp_u128_add(updated.spent_this_period, amount,
                          &updated.spent_this_period);
    if (status != LXP_OK) return status;
    *scope = updated;
    return LXP_OK;
}

lxp_result lxp_authority_revoke(lxp_authority_grant *grant,
                                uint64_t revocation_sequence,
                                uint64_t global_sequence)
{
    if (grant == NULL) return LXP_ERR_MALFORMED_GRANT;
    if (revocation_sequence <= grant->grantor_revocation_sequence)
        return LXP_ERR_STALE_REVOCATION;
    grant->grantor_revocation_sequence = revocation_sequence;
    grant->revoked = true;
    grant->revoked_at_sequence = global_sequence;
    return LXP_OK;
}

static int cap_narrows(lxp_u128 old_cap, lxp_u128 new_cap)
{
    if (lxp_u128_is_zero(old_cap)) return 1;
    return !lxp_u128_is_zero(new_cap) && lxp_u128_cmp(new_cap, old_cap) <= 0;
}

lxp_result lxp_authority_amend(lxp_authority_grant *grant,
                               const lxp_authority_grant *narrower)
{
    if (grant == NULL || narrower == NULL) return LXP_ERR_MALFORMED_GRANT;
    if (narrower->grantor_revocation_sequence <=
        grant->grantor_revocation_sequence) return LXP_ERR_STALE_REVOCATION;
    if (narrower->kind != grant->kind ||
        lxp_ct_memcmp(narrower->grantor, grant->grantor, 32U) != 0 ||
        lxp_ct_memcmp(narrower->grantee, grant->grantee, 32U) != 0 ||
        lxp_ct_memcmp(narrower->key, grant->key, 32U) != 0 ||
        lxp_ct_memcmp(narrower->scope.asset_id, grant->scope.asset_id, 32U) != 0 ||
        lxp_ct_memcmp(narrower->scope.purpose_hash,
                      grant->scope.purpose_hash, 32U) != 0 ||
        (narrower->scope.module_mask & ~grant->scope.module_mask) != 0U ||
        narrower->scope.activity_ordinal_min <
            grant->scope.activity_ordinal_min ||
        narrower->scope.activity_ordinal_max >
            grant->scope.activity_ordinal_max ||
        narrower->scope.activity_ordinal_min >
            narrower->scope.activity_ordinal_max ||
        narrower->not_before < grant->not_before ||
        narrower->not_after > grant->not_after ||
        narrower->not_after <= narrower->not_before ||
        !cap_narrows(grant->scope.maximum_per_activity,
                     narrower->scope.maximum_per_activity) ||
        !cap_narrows(grant->scope.maximum_total,
                     narrower->scope.maximum_total) ||
        !cap_narrows(grant->scope.maximum_per_period,
                     narrower->scope.maximum_per_period) ||
        lxp_u128_cmp(narrower->scope.spent_total,
                     grant->scope.spent_total) != 0 ||
        lxp_u128_cmp(narrower->scope.spent_this_period,
                     grant->scope.spent_this_period) != 0 ||
        narrower->scope.period_length != grant->scope.period_length ||
        narrower->scope.period_start != grant->scope.period_start)
        return LXP_ERR_AUTH_SCOPE;
    *grant = *narrower;
    return lxp_grant_id_compute(grant, grant->grant_id);
}

lxp_result lxp_authority_is_live(const lxp_authority_grant *grant,
                                 uint64_t identity_revocation_sequence,
                                 uint64_t batch_timestamp,
                                 uint64_t global_sequence)
{
    if (grant == NULL) return LXP_ERR_MALFORMED_GRANT;
    if (batch_timestamp < grant->not_before) return LXP_ERR_NOT_YET_VALID;
    if (batch_timestamp >= grant->not_after) return LXP_ERR_AUTH_EXPIRED;
    if (grant->grantor_revocation_sequence != identity_revocation_sequence)
        return LXP_ERR_AUTH_REVOKED;
    if (grant->revoked && global_sequence >= grant->revoked_at_sequence)
        return LXP_ERR_AUTH_REVOKED;
    return LXP_OK;
}

lxp_result lxp_identity_bump_revocation_sequence(lxp_identity *identity,
                                                 uint64_t new_sequence)
{
    if (identity == NULL) return LXP_ERR_UNKNOWN_DID;
    if (new_sequence <= identity->revocation_sequence)
        return LXP_ERR_STALE_REVOCATION;
    identity->revocation_sequence = new_sequence;
    return LXP_OK;
}
