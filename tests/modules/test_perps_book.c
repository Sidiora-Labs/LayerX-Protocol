#include "layerx/lx_perps.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static lx_perps_market market_make(void)
{
    lx_perps_market market;
    (void)memset(&market, 0, sizeof(market));
    market.market_id[0] = 1U;
    market.quote_asset[0] = 2U;
    market.contract_size = (lxp_u128){ 0U, 1U };
    market.tick_size = (lxp_u128){ 0U, 1U };
    market.lot_size = (lxp_u128){ 0U, 1U };
    market.initial_margin_ratio_bps = 1000U;
    market.maintenance_margin_ratio_bps = 500U;
    market.funding_interval_ms = 100U;
    market.maximum_oracle_staleness_ms = 100U;
    market.minimum_price = (lxp_u128){ 0U, 1U };
    market.maximum_price = (lxp_u128){ 0U, 1000U };
    market.permitted_oracle_key_count = 1U;
    market.permitted_oracle_keys[0][0] = 1U;
    market.parameter_version = 1U;
    return market;
}

static lx_perps_order order_make(uint8_t id, uint8_t owner,
                                 lx_perps_side side, uint64_t price,
                                 uint64_t quantity)
{
    lx_perps_order order;
    (void)memset(&order, 0, sizeof(order));
    order.order_id[0] = id;
    order.market_id[0] = 1U;
    order.owner_account_id[0] = owner;
    order.side = side;
    order.price = (lxp_u128){ 0U, price };
    order.quantity = (lxp_u128){ 0U, quantity };
    return order;
}

static lxp_result run(lxp_module_ctx *ctx, lx_perps_book *book,
                      lx_perps_fill fills[4], size_t *total_fills)
{
    lx_perps_market market = market_make();
    lx_perps_order orders[3];
    size_t i;
    size_t fill_count;
    size_t transfer_count;
    orders[0] = order_make(1U, 1U, LX_PERPS_SIDE_SELL, 101U, 4U);
    orders[1] = order_make(2U, 2U, LX_PERPS_SIDE_SELL, 100U, 3U);
    orders[2] = order_make(3U, 3U, LX_PERPS_SIDE_BUY, 102U, 6U);
    *total_fills = 0U;
    for (i = 0U; i < 3U; ++i) {
        ctx->global_sequence = i + 1U;
        if (lx_perps_order_place_execute(
                ctx, book, &market, &orders[i], (lxp_u128){ 0U, 1000U },
                fills + *total_fills, 4U - *total_fills, &fill_count,
                &transfer_count) != LXP_OK || transfer_count != 0U)
            return LXP_FATAL_INVARIANT;
        *total_fills += fill_count;
    }
    return LXP_OK;
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
    lx_perps_book first;
    lx_perps_book second;
    lx_perps_fill first_fills[4];
    lx_perps_fill second_fills[4];
    lx_perps_market market = market_make();
    lx_perps_order rejected = order_make(9U, 9U, LX_PERPS_SIDE_BUY,
                                         100U, 100U);
    size_t first_count;
    size_t second_count;
    size_t fill_count;
    size_t transfer_count;

    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_perps_module_iface()) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PERPS, 100U, 0U, 1U,
                            10000U, &arena, true) != LXP_OK ||
        lx_perps_book_init(&first) != LXP_OK ||
        lx_perps_book_init(&second) != LXP_OK ||
        run(&ctx, &first, first_fills, &first_count) != LXP_OK ||
        run(&ctx, &second, second_fills, &second_count) != LXP_OK ||
        first_count != 2U || second_count != first_count ||
        memcmp(&first, &second, sizeof(first)) != 0 ||
        memcmp(first_fills, second_fills,
               first_count * sizeof(first_fills[0])) != 0 ||
        first_fills[0].price.lo != 100U ||
        first_fills[1].price.lo != 101U)
        return 1;
    if (lx_perps_order_place_execute(
            &ctx, &first, &market, &rejected, (lxp_u128){ 0U, 999U },
            first_fills, 4U, &fill_count, &transfer_count) !=
            LXP_ERR_MARGIN_INSUFFICIENT || first.count != 1U)
        return 1;
    market.halted = true;
    if (lx_perps_order_place_execute(
            &ctx, &first, &market, &rejected, (lxp_u128){ 0U, 10000U },
            first_fills, 4U, &fill_count, &transfer_count) !=
            LXP_ERR_MARKET_HALTED ||
        lx_perps_order_cancel_execute(&ctx, &first,
                                      first.orders[0].order_id,
                                      first.orders[0].owner_account_id,
                                      &transfer_count) != LXP_OK ||
        transfer_count != 0U || first.count != 0U ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
