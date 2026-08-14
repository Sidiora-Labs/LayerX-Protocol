#include "layerx/lx_oracle.h"
#include "layerx/lx_service.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static int encode_signed(lx_oracle_observation *observation,
                         const uint8_t seed[32], uint8_t payload[72],
                         size_t *payload_length)
{
    if (lx_oracle_observation_sign(observation, seed) != LXP_OK ||
        lx_oracle_observation_encode(observation, payload, 72U,
                                     payload_length) != LXP_OK)
        return 1;
    return 0;
}

int main(void)
{
    static const uint8_t seed[32] = { 15U };
    lx_oracle_market_store markets;
    lx_oracle_market *market;
    lx_oracle_store store;
    lx_oracle_observation observation;
    lx_oracle_push_request request;
    lx_oracle_accepted accepted;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint8_t payload[72];
    size_t payload_length;
    uint64_t parameters = 1U;

    (void)memset(&markets, 0, sizeof(markets));
    (void)memset(&store, 0, sizeof(store));
    (void)memset(&observation, 0, sizeof(observation));
    markets.count = 1U;
    market = &markets.markets[0];
    market->market_id[0] = 1U;
    market->maximum_staleness = 100U;
    market->minimum_price = (lxp_u128){ 0U, 50U };
    market->maximum_price = (lxp_u128){ 0U, 200U };
    market->maximum_deviation_basis_points = 1000U;
    observation.market_id[0] = 1U;
    observation.observation_sequence = 1U;
    observation.price = (lxp_u128){ 0U, 100U };
    observation.observed_at = 900U;
    observation.source_identifier = 2U;
    if (encode_signed(&observation, seed, payload, &payload_length) != 0)
        return 1;
    market->permitted_key_count = 1U;
    (void)memcpy(market->permitted_keys[0],
                 observation.oracle_public_key, 32U);
    if (lx_oracle_store_put(&store, &observation, payload, payload_length,
                            1U) != LXP_OK ||
        lx_oracle_fail_closed_eval(market, &store, 1001U) !=
            LXP_ERR_MARKET_HALTED || !lx_oracle_market_halted(market))
        return 1;
    if (lx_oracle_market_action_check(market, LX_ORACLE_ACTION_ORDER_PLACE) !=
            LXP_ERR_MARKET_HALTED ||
        lx_oracle_market_action_check(market,
            LX_ORACLE_ACTION_POSITION_INCREASE) != LXP_ERR_MARKET_HALTED ||
        lx_oracle_market_action_check(market, LX_ORACLE_ACTION_LIQUIDATE) !=
            LXP_ERR_MARKET_HALTED ||
        lx_oracle_market_action_check(market, LX_ORACLE_ACTION_FUNDING_TICK) !=
            LXP_ERR_MARKET_HALTED ||
        lx_oracle_market_action_check(market, LX_ORACLE_ACTION_ORDER_CANCEL) !=
            LXP_OK ||
        lx_oracle_market_action_check(market, LX_ORACLE_ACTION_MARGIN_ADD) !=
            LXP_OK)
        return 1;
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_service_module_iface()) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 1001U, 0U, 2U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    observation.observation_sequence = 2U;
    observation.observed_at = 1001U;
    observation.price = (lxp_u128){ 0U, 105U };
    if (encode_signed(&observation, seed, payload, &payload_length) != 0)
        return 1;
    (void)memset(&request, 0, sizeof(request));
    request.store = &store;
    request.markets = &markets;
    request.payload = payload;
    request.payload_length = payload_length;
    request.oracle_public_key = observation.oracle_public_key;
    request.signature = observation.signature;
    if (lx_oracle_push_execute(&ctx, &request, &accepted) != LXP_OK ||
        lx_oracle_market_halted(market) ||
        lx_oracle_market_action_check(market,
            LX_ORACLE_ACTION_ORDER_PLACE) != LXP_OK ||
        accepted.observation.observation_sequence != 2U)
        return 1;
    observation.observation_sequence = 3U;
    observation.observed_at = 800U;
    if (encode_signed(&observation, seed, payload, &payload_length) != 0)
        return 1;
    request.payload = payload;
    request.signature = observation.signature;
    if (lx_oracle_push_execute(&ctx, &request, &accepted) !=
            LXP_ERR_ORACLE_STALE || !lx_oracle_market_halted(market) ||
        store.count != 2U ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
