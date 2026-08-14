#include "layerx/lx_perps.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

typedef struct visit_state {
    uint8_t previous[32];
    size_t count;
} visit_state;

static lxp_result visit(const lx_perps_market *market, void *user)
{
    visit_state *state = (visit_state *)user;
    if (state->count != 0U && memcmp(state->previous, market->market_id, 32U) >= 0)
        return LXP_ERR_UNSORTED_SEQUENCE;
    (void)memcpy(state->previous, market->market_id, 32U);
    ++state->count;
    return LXP_OK;
}

static lx_perps_market make_market(uint8_t id)
{
    lx_perps_market market;
    (void)memset(&market, 0, sizeof(market));
    market.market_id[0] = id;
    market.quote_asset[0] = 9U;
    market.contract_size = (lxp_u128){ 0U, 100U };
    market.tick_size = (lxp_u128){ 0U, 5U };
    market.lot_size = (lxp_u128){ 0U, 2U };
    market.initial_margin_ratio_bps = 1000U;
    market.maintenance_margin_ratio_bps = 500U;
    market.funding_interval_ms = 3600000U;
    market.maximum_oracle_staleness_ms = 30000U;
    market.minimum_price = (lxp_u128){ 0U, 10U };
    market.maximum_price = (lxp_u128){ 0U, 1000000U };
    market.permitted_oracle_key_count = 2U;
    market.permitted_oracle_keys[0][0] = 1U;
    market.permitted_oracle_keys[1][0] = 2U;
    market.parameter_version = 1U;
    return market;
}

int main(void)
{
    uint8_t arena_bytes[4096];
    lxp_arena arena;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    uint64_t parameters = 1U;
    lx_perps_market first = make_market(2U);
    lx_perps_market second = make_market(1U);
    lx_perps_market decoded;
    lx_perps_market invalid;
    uint8_t encoded[LX_PERPS_MARKET_BYTES];
    const lxp_module_iface *iface = lx_perps_module_iface();
    const lxp_module_registration *registration;
    visit_state visits;

    (void)memset(&visits, 0, sizeof(visits));
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        iface == NULL || iface->module_id != LXP_MODULE_PERPS ||
        iface->activity_type_count != 11U ||
        lxp_kernel_register_module(&kernel, iface) != LXP_OK ||
        lxp_kernel_module_for_activity(&kernel, LX_PERPS_ADL, 0U,
                                       &registration) != LXP_OK ||
        registration->activity_type_count != 11U ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PERPS, 100U, 0U, 1U,
                            10000U, &arena, true) != LXP_OK)
        return 1;
    if (lx_perps_market_encode(&first, encoded) != LXP_OK ||
        lx_perps_market_decode(encoded, sizeof(encoded), &decoded) != LXP_OK ||
        memcmp(&first, &decoded, sizeof(first)) != 0 ||
        lx_perps_market_create_execute(&ctx, &first) != LXP_OK ||
        lx_perps_market_create_execute(&ctx, &first) !=
            LXP_ERR_MARKET_ALREADY_EXISTS ||
        lx_perps_market_create_execute(&ctx, &second) != LXP_OK ||
        lx_perps_market_lookup(&ctx, first.market_id, &decoded) != LXP_OK ||
        decoded.funding_interval_ms != first.funding_interval_ms ||
        lx_perps_market_iter(&ctx, visit, &visits) != LXP_OK ||
        visits.count != 2U)
        return 1;
    invalid = first;
    invalid.market_id[0] = 3U;
    invalid.initial_margin_ratio_bps = 10001U;
    if (lx_perps_market_create_execute(&ctx, &invalid) !=
        LXP_ERR_PARAMETER_BOUNDS)
        return 1;
    if (lxp_module_ctx_commit(&ctx) != LXP_OK ||
        lx_perps_market_lookup(&ctx, second.market_id, &decoded) != LXP_OK ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
