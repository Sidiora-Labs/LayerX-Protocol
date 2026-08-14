#include "layerx/lx_oracle.h"
#include "layerx/lx_service.h"
#include "layerx/lxp_kernel.h"
#include "layerx/lxp_ledger.h"

#include <string.h>

int main(void)
{
    static const uint8_t seed[32] = { 13U };
    lx_oracle_observation observation;
    lx_oracle_market_store markets;
    lx_oracle_store store;
    lx_oracle_push_request request;
    lx_oracle_accepted accepted;
    lx_account account;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint8_t payload[LX_ORACLE_OBSERVATION_BYTES];
    uint8_t unknown_key[32] = { 99U };
    size_t payload_length;
    uint64_t parameters = 1U;

    (void)memset(&observation, 0, sizeof(observation));
    (void)memset(&markets, 0, sizeof(markets));
    (void)memset(&store, 0, sizeof(store));
    (void)memset(&account, 0, sizeof(account));
    observation.market_id[0] = 1U;
    observation.observation_sequence = 1U;
    observation.price = (lxp_u128){ 0U, 100U };
    observation.observed_at = 500U;
    observation.source_identifier = 7U;
    markets.count = 1U;
    (void)memcpy(markets.markets[0].market_id,
                 observation.market_id, 32U);
    markets.markets[0].permitted_key_count = 1U;
    markets.markets[0].maximum_staleness = 100U;
    markets.markets[0].minimum_price = (lxp_u128){ 0U, 1U };
    markets.markets[0].maximum_price = (lxp_u128){ 0U, 1000U };
    markets.markets[0].maximum_deviation_basis_points = 1000U;
    account.balance = (lxp_u128){ 0U, 77U };
    if (lx_oracle_observation_sign(&observation, seed) != LXP_OK ||
        lx_oracle_observation_encode(&observation, payload, sizeof(payload),
                                     &payload_length) != LXP_OK)
        return 1;
    (void)memcpy(markets.markets[0].permitted_keys[0],
                 observation.oracle_public_key, 32U);
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK)
        return 1;
    if (lxp_kernel_register_module(&kernel, lx_service_module_iface()) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 500U, 0U, 44U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    (void)memset(&request, 0, sizeof(request));
    request.store = &store;
    request.markets = &markets;
    request.payload = payload;
    request.payload_length = payload_length;
    request.oracle_public_key = unknown_key;
    request.signature = observation.signature;
    if (lx_oracle_push_execute(&ctx, &request, &accepted) !=
            LXP_ERR_UNAUTHORIZED_ORACLE || store.count != 0U ||
        account.balance.lo != 77U)
        return 1;
    request.oracle_public_key = observation.oracle_public_key;
    if (lx_oracle_push_execute(&ctx, &request, &accepted) != LXP_OK ||
        store.count != 1U || accepted.global_sequence != 44U ||
        accepted.payload_length != payload_length ||
        memcmp(accepted.payload, payload, payload_length) != 0 ||
        account.balance.lo != 77U)
        return 1;
    request.attempts_balance_mutation = true;
    if (lx_oracle_push_execute(&ctx, &request, &accepted) !=
            LXP_ERR_MODULE_MAY_NOT_WRITE_BALANCE ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
