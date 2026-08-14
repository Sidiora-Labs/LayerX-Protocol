#include "layerx/lx_perps.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static const uint8_t market_prefix[] = "market:";

static const uint32_t activity_types[] = {
    LX_PERPS_MARKET_CREATE, LX_PERPS_MARKET_HALT, LX_PERPS_ORACLE_PUSH,
    LX_PERPS_ORDER_PLACE, LX_PERPS_ORDER_CANCEL, LX_PERPS_POSITION_OPEN,
    LX_PERPS_POSITION_INCREASE, LX_PERPS_POSITION_CLOSE,
    LX_PERPS_FUNDING_TICK, LX_PERPS_LIQUIDATE, LX_PERPS_ADL
};

typedef struct perps_decoded {
    uint16_t ordinal;
    const uint8_t *payload;
    size_t payload_length;
} perps_decoded;

typedef struct market_iter_adapter {
    lx_perps_market_visit_fn visit;
    void *user;
} market_iter_adapter;

static void put_u32(uint8_t bytes[4], uint32_t value)
{
    bytes[0] = (uint8_t)(value >> 24U);
    bytes[1] = (uint8_t)(value >> 16U);
    bytes[2] = (uint8_t)(value >> 8U);
    bytes[3] = (uint8_t)value;
}

static uint32_t get_u32(const uint8_t bytes[4])
{
    return ((uint32_t)bytes[0] << 24U) |
           ((uint32_t)bytes[1] << 16U) |
           ((uint32_t)bytes[2] << 8U) | (uint32_t)bytes[3];
}

static void put_u64(uint8_t bytes[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        bytes[i] = (uint8_t)(value >> (56U - 8U * i));
}

static uint64_t get_u64(const uint8_t bytes[8])
{
    uint64_t value = 0U;
    size_t i;
    for (i = 0U; i < 8U; ++i) value = (value << 8U) | bytes[i];
    return value;
}

static bool keys_canonical(const lx_perps_market *market)
{
    size_t i;
    if (market->permitted_oracle_key_count == 0U ||
        market->permitted_oracle_key_count > LX_PERPS_MAX_ORACLE_KEYS)
        return false;
    for (i = 0U; i < market->permitted_oracle_key_count; ++i) {
        if (lxp_ct_is_zero(market->permitted_oracle_keys[i], 32U))
            return false;
        if (i != 0U && memcmp(market->permitted_oracle_keys[i - 1U],
                              market->permitted_oracle_keys[i], 32U) >= 0)
            return false;
    }
    return true;
}

static lxp_result market_validate(const lx_perps_market *market)
{
    if (market == NULL || lxp_ct_is_zero(market->market_id, 32U) ||
        lxp_ct_is_zero(market->quote_asset, 32U) ||
        lxp_u128_is_zero(market->contract_size) ||
        lxp_u128_is_zero(market->tick_size) ||
        lxp_u128_is_zero(market->lot_size) ||
        market->maintenance_margin_ratio_bps == 0U ||
        market->initial_margin_ratio_bps <=
            market->maintenance_margin_ratio_bps ||
        market->initial_margin_ratio_bps > LX_PERPS_MARGIN_RATIO_MAX_BPS ||
        market->funding_interval_ms == 0U ||
        market->maximum_oracle_staleness_ms == 0U ||
        lxp_u128_is_zero(market->minimum_price) ||
        lxp_u128_cmp(market->minimum_price, market->maximum_price) >= 0 ||
        market->parameter_version == 0U || !keys_canonical(market))
        return LXP_ERR_PARAMETER_BOUNDS;
    return LXP_OK;
}

static void market_key(const uint8_t market_id[32],
                       uint8_t key[LX_PERPS_MARKET_KEY_BYTES])
{
    (void)memcpy(key, market_prefix, sizeof(market_prefix) - 1U);
    (void)memcpy(key + sizeof(market_prefix) - 1U, market_id, 32U);
}

lxp_result lx_perps_market_encode(const lx_perps_market *market,
                                  uint8_t bytes[LX_PERPS_MARKET_BYTES])
{
    size_t offset = 0U;
    size_t key_bytes;
    lxp_result status = market_validate(market);
    if (status != LXP_OK || bytes == NULL)
        return status != LXP_OK ? status : LXP_ERR_NON_CANONICAL;
    (void)memset(bytes, 0, LX_PERPS_MARKET_BYTES);
    (void)memcpy(bytes + offset, market->market_id, 32U); offset += 32U;
    (void)memcpy(bytes + offset, market->quote_asset, 32U); offset += 32U;
    (void)lxp_u128_to_be(market->contract_size, bytes + offset); offset += 16U;
    (void)lxp_u128_to_be(market->tick_size, bytes + offset); offset += 16U;
    (void)lxp_u128_to_be(market->lot_size, bytes + offset); offset += 16U;
    put_u32(bytes + offset, market->initial_margin_ratio_bps); offset += 4U;
    put_u32(bytes + offset, market->maintenance_margin_ratio_bps); offset += 4U;
    put_u64(bytes + offset, market->funding_interval_ms); offset += 8U;
    put_u64(bytes + offset, market->maximum_oracle_staleness_ms); offset += 8U;
    (void)lxp_u128_to_be(market->minimum_price, bytes + offset); offset += 16U;
    (void)lxp_u128_to_be(market->maximum_price, bytes + offset); offset += 16U;
    bytes[offset++] = market->permitted_oracle_key_count;
    key_bytes = (size_t)market->permitted_oracle_key_count * 32U;
    (void)memcpy(bytes + offset, market->permitted_oracle_keys, key_bytes);
    offset += LX_PERPS_MAX_ORACLE_KEYS * 32U;
    put_u32(bytes + offset, market->parameter_version); offset += 4U;
    bytes[offset++] = market->halted ? 1U : 0U;
    return offset == LX_PERPS_MARKET_BYTES ? LXP_OK : LXP_FATAL_INVARIANT;
}

lxp_result lx_perps_market_decode(
    const uint8_t bytes[LX_PERPS_MARKET_BYTES], size_t length,
    lx_perps_market *market)
{
    size_t offset = 0U;
    size_t key_bytes;
    lxp_result status;
    if (bytes == NULL || market == NULL || length != LX_PERPS_MARKET_BYTES)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(market, 0, sizeof(*market));
    (void)memcpy(market->market_id, bytes + offset, 32U); offset += 32U;
    (void)memcpy(market->quote_asset, bytes + offset, 32U); offset += 32U;
    status = lxp_u128_from_be(bytes + offset, &market->contract_size);
    if (status != LXP_OK) return status;
    offset += 16U;
    status = lxp_u128_from_be(bytes + offset, &market->tick_size);
    if (status != LXP_OK) return status;
    offset += 16U;
    status = lxp_u128_from_be(bytes + offset, &market->lot_size);
    if (status != LXP_OK) return status;
    offset += 16U;
    market->initial_margin_ratio_bps = get_u32(bytes + offset); offset += 4U;
    market->maintenance_margin_ratio_bps = get_u32(bytes + offset); offset += 4U;
    market->funding_interval_ms = get_u64(bytes + offset); offset += 8U;
    market->maximum_oracle_staleness_ms = get_u64(bytes + offset); offset += 8U;
    status = lxp_u128_from_be(bytes + offset, &market->minimum_price);
    if (status != LXP_OK) return status;
    offset += 16U;
    status = lxp_u128_from_be(bytes + offset, &market->maximum_price);
    if (status != LXP_OK) return status;
    offset += 16U;
    market->permitted_oracle_key_count = bytes[offset++];
    if (market->permitted_oracle_key_count > LX_PERPS_MAX_ORACLE_KEYS)
        return LXP_ERR_NON_CANONICAL;
    key_bytes = (size_t)market->permitted_oracle_key_count * 32U;
    (void)memcpy(market->permitted_oracle_keys, bytes + offset, key_bytes);
    if (key_bytes < LX_PERPS_MAX_ORACLE_KEYS * 32U &&
        !lxp_ct_is_zero(bytes + offset + key_bytes,
                        LX_PERPS_MAX_ORACLE_KEYS * 32U - key_bytes))
        return LXP_ERR_NON_CANONICAL;
    offset += LX_PERPS_MAX_ORACLE_KEYS * 32U;
    market->parameter_version = get_u32(bytes + offset); offset += 4U;
    if (bytes[offset] > 1U) return LXP_ERR_NON_CANONICAL;
    market->halted = bytes[offset++] != 0U;
    return offset == length ? market_validate(market) : LXP_ERR_TRAILING_BYTES;
}

lxp_result lx_perps_market_put(lxp_module_ctx *ctx,
                               const lx_perps_market *market)
{
    uint8_t key[LX_PERPS_MARKET_KEY_BYTES];
    uint8_t bytes[LX_PERPS_MARKET_BYTES];
    lxp_result status;
    if (ctx == NULL || ctx->module_id != LXP_MODULE_PERPS)
        return LXP_ERR_NON_CANONICAL;
    status = lx_perps_market_encode(market, bytes);
    if (status != LXP_OK) return status;
    market_key(market->market_id, key);
    return lxp_ctx_kv_put(ctx, key, sizeof(key), bytes, sizeof(bytes));
}

lxp_result lx_perps_market_lookup(lxp_module_ctx *ctx,
                                  const uint8_t market_id[32],
                                  lx_perps_market *market)
{
    uint8_t key[LX_PERPS_MARKET_KEY_BYTES];
    const uint8_t *bytes;
    size_t length;
    lxp_result status;
    if (ctx == NULL || ctx->module_id != LXP_MODULE_PERPS ||
        market_id == NULL || market == NULL)
        return LXP_ERR_NON_CANONICAL;
    market_key(market_id, key);
    status = lxp_ctx_kv_get(ctx, key, sizeof(key), &bytes, &length);
    if (status != LXP_OK) return status;
    return lx_perps_market_decode(bytes, length, market);
}

static lxp_result visit_market(const uint8_t *key, size_t key_length,
                               const uint8_t *value, size_t value_length,
                               void *user)
{
    market_iter_adapter *adapter = (market_iter_adapter *)user;
    lx_perps_market market;
    lxp_result status;
    if (key == NULL || key_length != LX_PERPS_MARKET_KEY_BYTES ||
        memcmp(key, market_prefix, sizeof(market_prefix) - 1U) != 0)
        return LXP_ERR_NON_CANONICAL;
    status = lx_perps_market_decode(value, value_length, &market);
    if (status != LXP_OK) return status;
    return adapter->visit(&market, adapter->user);
}

lxp_result lx_perps_market_iter(lxp_module_ctx *ctx,
                                lx_perps_market_visit_fn visit, void *user)
{
    market_iter_adapter adapter;
    if (ctx == NULL || ctx->module_id != LXP_MODULE_PERPS || visit == NULL)
        return LXP_ERR_NON_CANONICAL;
    adapter.visit = visit;
    adapter.user = user;
    return lxp_ctx_kv_iter(ctx, market_prefix, sizeof(market_prefix) - 1U,
                           visit_market, &adapter);
}

lxp_result lx_perps_market_create_execute(lxp_module_ctx *ctx,
                                          const lx_perps_market *market)
{
    lx_perps_market existing;
    lxp_result status;
    if (ctx == NULL || market == NULL) return LXP_ERR_NON_CANONICAL;
    status = market_validate(market);
    if (status != LXP_OK) return status;
    status = lx_perps_market_lookup(ctx, market->market_id, &existing);
    if (status == LXP_OK) return LXP_ERR_MARKET_ALREADY_EXISTS;
    if (status != LXP_ERR_UNKNOWN_FIELD) return status;
    return lx_perps_market_put(ctx, market);
}

static lxp_result module_genesis(lxp_module_ctx *ctx, const uint8_t *manifest,
                                 size_t length)
{
    if (ctx == NULL || (manifest == NULL && length != 0U))
        return LXP_ERR_NON_CANONICAL;
    return lxp_ctx_charge_gas(ctx, length);
}

static lxp_result module_decode(lxp_module_ctx *ctx, uint16_t ordinal,
                                const uint8_t *payload, size_t length,
                                void **decoded)
{
    perps_decoded *value;
    void *memory;
    lxp_result status;
    if (ctx == NULL || decoded == NULL || ordinal == 0U || ordinal > 11U ||
        (payload == NULL && length != 0U)) return LXP_ERR_UNKNOWN_ACTIVITY;
    status = lxp_ctx_arena_alloc(ctx, sizeof(*value), _Alignof(perps_decoded),
                                 &memory);
    if (status != LXP_OK) return status;
    value = (perps_decoded *)memory;
    value->ordinal = ordinal;
    value->payload = payload;
    value->payload_length = length;
    *decoded = value;
    return LXP_OK;
}

static lxp_result module_validate(lxp_module_ctx *ctx,
                                  const lxp_activity *activity,
                                  const lxp_authority_resolved *authority,
                                  const void *decoded)
{
    const perps_decoded *value = (const perps_decoded *)decoded;
    lx_perps_market market;
    lxp_result status;
    if (ctx == NULL || activity == NULL || authority == NULL || value == NULL)
        return LXP_ERR_UNKNOWN_ACTIVITY;
    status = lxp_ctx_charge_gas(ctx, value->payload_length + 1U);
    if (status != LXP_OK) return status;
    if (value->ordinal != 1U) return LXP_OK;
    return lx_perps_market_decode(value->payload, value->payload_length,
                                  &market);
}

static lxp_result module_execute(lxp_module_ctx *ctx,
                                 const lxp_activity *activity,
                                 const lxp_authority_resolved *authority,
                                 const void *decoded,
                                 lxp_effect_buffer *effects)
{
    const perps_decoded *value = (const perps_decoded *)decoded;
    lx_perps_market market;
    lxp_result status;
    (void)activity;
    (void)authority;
    (void)effects;
    if (ctx == NULL || value == NULL) return LXP_ERR_UNKNOWN_ACTIVITY;
    if (value->ordinal != 1U) return LXP_ERR_UNKNOWN_ACTIVITY;
    status = lx_perps_market_decode(value->payload, value->payload_length,
                                    &market);
    return status == LXP_OK ? lx_perps_market_create_execute(ctx, &market) :
                              status;
}

static lxp_result module_epoch(lxp_module_ctx *ctx, uint64_t epoch,
                               uint64_t timestamp)
{
    (void)epoch;
    (void)timestamp;
    return ctx == NULL ? LXP_ERR_NON_CANONICAL : LXP_OK;
}

static lxp_result module_state_root(lxp_module_ctx *ctx, uint8_t root[32])
{
    if (ctx == NULL || root == NULL) return LXP_ERR_NON_CANONICAL;
    return lxp_state_subtree_root(ctx->kernel, LXP_MODULE_PERPS, root);
}

const lxp_module_iface *lx_perps_module_iface(void)
{
    static const lxp_module_iface iface = {
        LXP_MODULE_PERPS, 1U, "perps", activity_types,
        sizeof(activity_types) / sizeof(activity_types[0]),
        module_genesis, module_decode, module_validate, module_execute,
        module_epoch, module_epoch, module_state_root, NULL
    };
    return &iface;
}
