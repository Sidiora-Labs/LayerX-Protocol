#ifndef LAYERX_LX_PERPS_H
#define LAYERX_LX_PERPS_H

#include "layerx/lxp_module.h"
#include "layerx/lxp_transfer.h"
#include "layerx/lxp_u128.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
    LX_PERPS_MARKET_CREATE = 0x00060001,
    LX_PERPS_MARKET_HALT = 0x00060002,
    LX_PERPS_ORACLE_PUSH = 0x00060003,
    LX_PERPS_ORDER_PLACE = 0x00060004,
    LX_PERPS_ORDER_CANCEL = 0x00060005,
    LX_PERPS_POSITION_OPEN = 0x00060006,
    LX_PERPS_POSITION_INCREASE = 0x00060007,
    LX_PERPS_POSITION_CLOSE = 0x00060008,
    LX_PERPS_FUNDING_TICK = 0x00060009,
    LX_PERPS_LIQUIDATE = 0x0006000a,
    LX_PERPS_ADL = 0x0006000b,
    LX_PERPS_MAX_ORACLE_KEYS = 8,
    LX_PERPS_MARKET_KEY_BYTES = 39,
    LX_PERPS_MARKET_BYTES = 430,
    LX_PERPS_BOOK_CAPACITY = 256,
    LX_PERPS_FILL_CAPACITY = 256,
    LX_PERPS_POSITION_CAPACITY = 128,
    LX_PERPS_DEFICIT_CAPACITY = 128,
    LX_PERPS_ADL_CAPACITY = 128,
    LX_PERPS_MARGIN_RATIO_MAX_BPS = 10000
};

typedef enum lx_perps_side {
    LX_PERPS_SIDE_BUY = 1,
    LX_PERPS_SIDE_SELL = 2
} lx_perps_side;

typedef struct lx_perps_order {
    uint8_t order_id[32];
    uint8_t market_id[32];
    uint8_t owner_account_id[32];
    lx_perps_side side;
    lxp_u128 price;
    lxp_u128 quantity;
    lxp_u128 remaining;
    lxp_u128 initial_margin_required;
    uint64_t global_sequence;
    bool active;
} lx_perps_order;

typedef struct lx_perps_fill {
    uint8_t maker_order_id[32];
    uint8_t taker_order_id[32];
    lxp_u128 price;
    lxp_u128 quantity;
    uint64_t maker_sequence;
    uint64_t taker_sequence;
} lx_perps_fill;

typedef struct lx_perps_book {
    lx_perps_order orders[LX_PERPS_BOOK_CAPACITY];
    size_t count;
} lx_perps_book;

typedef lxp_result (*lx_perps_order_visit_fn)(const lx_perps_order *order,
                                              void *user);

typedef struct lx_perps_position {
    uint8_t position_id[32];
    uint8_t market_id[32];
    uint8_t owner_main_account_id[32];
    uint8_t margin_account_id[32];
    uint8_t asset_id[32];
    lx_perps_side side;
    lxp_u128 size;
    lxp_u128 entry_notional;
    lxp_i128 funding_index_at_entry;
    bool open;
} lx_perps_position;

typedef struct lx_perps_position_store {
    lx_perps_position positions[LX_PERPS_POSITION_CAPACITY];
    size_t count;
} lx_perps_position_store;

typedef struct lx_perps_position_request {
    lx_perps_position_store *store;
    lx_account *owner_main;
    lx_account *margin_account;
    const lxp_transfer_asset_state *asset;
    lx_perps_position position;
    lxp_u128 margin_amount;
    lxp_u128 size_delta;
    lxp_u128 notional_delta;
    lxp_transfer_context context;
} lx_perps_position_request;

typedef struct lx_perps_market {
    uint8_t market_id[32];
    uint8_t quote_asset[32];
    lxp_u128 contract_size;
    lxp_u128 tick_size;
    lxp_u128 lot_size;
    uint32_t initial_margin_ratio_bps;
    uint32_t maintenance_margin_ratio_bps;
    uint64_t funding_interval_ms;
    uint64_t maximum_oracle_staleness_ms;
    lxp_u128 minimum_price;
    lxp_u128 maximum_price;
    uint8_t permitted_oracle_keys[LX_PERPS_MAX_ORACLE_KEYS][32];
    uint8_t permitted_oracle_key_count;
    uint32_t parameter_version;
    bool halted;
} lx_perps_market;

typedef struct lx_perps_funding_tick_request {
    const lx_perps_market *market;
    lx_account *long_funding_account;
    lx_account *short_funding_account;
    const lxp_transfer_asset_state *asset;
    lxp_i128 funding_rate_bps;
    lxp_u128 open_notional;
    uint64_t *last_funding_timestamp_ms;
    lxp_i128 *funding_index;
    lxp_transfer_context context;
} lx_perps_funding_tick_request;

typedef struct lx_perps_liquidation_request {
    lx_perps_position *position;
    const lx_perps_market *market;
    lx_account *margin_account;
    lx_account *market_liquidity_account;
    lx_account *liquidator_main_account;
    lx_account *insurance_account;
    lx_account *owner_main_account;
    const lxp_transfer_asset_state *asset;
    lxp_u128 mark_price;
    lxp_u128 price_scale;
    lxp_u128 trading_loss;
    uint32_t liquidation_fee_bps;
    uint32_t liquidator_share_bps;
    lxp_transfer_context context;
} lx_perps_liquidation_request;

typedef struct lx_perps_deficit {
    uint8_t market_id[32];
    uint8_t insurance_account_id[32];
    lxp_u128 amount;
    uint64_t recorded_at_sequence;
} lx_perps_deficit;

typedef struct lx_perps_deficit_store {
    lx_perps_deficit deficits[LX_PERPS_DEFICIT_CAPACITY];
    size_t count;
} lx_perps_deficit_store;

typedef struct lx_perps_adl_candidate {
    lx_perps_position *position;
    lx_account *margin_account;
    lxp_u128 maximum_contribution;
} lx_perps_adl_candidate;

typedef lxp_result (*lx_perps_market_visit_fn)(
    const lx_perps_market *market, void *user);

const lxp_module_iface *lx_perps_module_iface(void);
lxp_result lx_perps_market_encode(const lx_perps_market *market,
                                  uint8_t bytes[LX_PERPS_MARKET_BYTES]);
lxp_result lx_perps_market_decode(
    const uint8_t bytes[LX_PERPS_MARKET_BYTES], size_t length,
    lx_perps_market *market);
lxp_result lx_perps_market_put(lxp_module_ctx *ctx,
                               const lx_perps_market *market);
lxp_result lx_perps_market_lookup(lxp_module_ctx *ctx,
                                  const uint8_t market_id[32],
                                  lx_perps_market *market);
lxp_result lx_perps_market_iter(lxp_module_ctx *ctx,
                                lx_perps_market_visit_fn visit, void *user);
lxp_result lx_perps_market_create_execute(lxp_module_ctx *ctx,
                                          const lx_perps_market *market);
lxp_result lx_perps_book_init(lx_perps_book *book);
lxp_result lx_perps_book_iter(const lx_perps_book *book,
                              lx_perps_order_visit_fn visit, void *user);
lxp_result lx_perps_book_match(lx_perps_book *book,
                               lx_perps_order *incoming,
                               lx_perps_fill *fills, size_t fill_capacity,
                               size_t *fill_count);
lxp_result lx_perps_order_place_execute(
    lxp_module_ctx *ctx, lx_perps_book *book, const lx_perps_market *market,
    const lx_perps_order *order, lxp_u128 available_margin,
    lx_perps_fill *fills, size_t fill_capacity, size_t *fill_count,
    size_t *transfer_leg_count);
lxp_result lx_perps_order_cancel_execute(
    lxp_module_ctx *ctx, lx_perps_book *book, const uint8_t order_id[32],
    const uint8_t owner_account_id[32], size_t *transfer_leg_count);
lxp_result lx_perps_position_lookup(lx_perps_position_store *store,
                                    const uint8_t position_id[32],
                                    lx_perps_position **position);
lxp_result lx_perps_margin_post(lxp_module_ctx *ctx,
                                lx_account *owner_main,
                                lx_account *margin_account,
                                const lxp_transfer_asset_state *asset,
                                lxp_u128 amount,
                                lxp_transfer_context context,
                                lxp_receipt *receipt);
lxp_result lx_perps_margin_release(lxp_module_ctx *ctx,
                                   lx_account *margin_account,
                                   lx_account *owner_main,
                                   const lxp_transfer_asset_state *asset,
                                   lxp_u128 amount,
                                   lxp_transfer_context context,
                                   lxp_receipt *receipt);
lxp_result lx_perps_position_open_execute(
    lxp_module_ctx *ctx, const lx_perps_position_request *request,
    lxp_receipt *receipt);
lxp_result lx_perps_position_increase_execute(
    lxp_module_ctx *ctx, const lx_perps_position_request *request,
    lxp_receipt *receipt);
lxp_result lx_perps_position_close_execute(
    lxp_module_ctx *ctx, lx_perps_position_store *store,
    const uint8_t position_id[32], lx_account *margin_account,
    lx_account *owner_main, const lxp_transfer_asset_state *asset,
    lxp_transfer_context context, lxp_receipt *receipt);
lxp_result lx_perps_authority_check(const lx_account *account,
                                    lxp_authorization_kind kind,
                                    uint16_t origin_module_id,
                                    uint16_t reason);
lxp_result lx_perps_pnl_compute(lx_perps_side side, lxp_u128 entry_price,
                                lxp_u128 mark_price, lxp_u128 size,
                                lxp_u128 price_scale, lxp_i128 *pnl);
lxp_result lx_perps_funding_rate(const lx_perps_market *market,
                                 lxp_u128 oracle_price,
                                 lxp_u128 reference_price,
                                 uint32_t maximum_rate_bps,
                                 lxp_i128 *rate_bps);
lxp_result lx_perps_funding_index_update(lxp_i128 current,
                                         lxp_i128 rate_bps,
                                         uint64_t elapsed_intervals,
                                         lxp_i128 *updated);
lxp_result lx_perps_funding_tick_execute(
    lxp_module_ctx *ctx, const lx_perps_funding_tick_request *request,
    lxp_receipt *receipt);
lxp_result lx_perps_maintenance_check(const lx_perps_market *market,
                                      const lx_perps_position *position,
                                      lxp_u128 mark_price,
                                      lxp_u128 price_scale,
                                      lxp_u128 margin_balance,
                                      bool *liquidatable);
lxp_result lx_perps_liquidation_fee_split(lxp_u128 total_fee,
                                          uint32_t liquidator_share_bps,
                                          lxp_u128 *liquidator_fee,
                                          lxp_u128 *insurance_fee);
lxp_result lx_perps_liquidation_legs_build(
    const lx_perps_liquidation_request *request, lxp_transfer_set *set);
lxp_result lx_perps_liquidate_execute(
    lxp_module_ctx *ctx, const lx_perps_liquidation_request *request,
    lxp_receipt *receipt);
lxp_result lx_perps_insurance_cover(
    lxp_module_ctx *ctx, lx_account *insurance_account,
    lx_account *liquidity_account, const lxp_transfer_asset_state *asset,
    lxp_u128 deficit, lxp_transfer_context context, lxp_receipt *receipt);
lxp_result lx_perps_deficit_record(
    lx_perps_deficit_store *store, const uint8_t market_id[32],
    const uint8_t insurance_account_id[32], lxp_u128 amount,
    uint64_t global_sequence);
lxp_result lx_perps_adl_execute(
    lxp_module_ctx *ctx, lx_perps_adl_candidate *candidates,
    size_t candidate_count, lx_account *liquidity_account,
    const lxp_transfer_asset_state *asset, lxp_u128 deficit,
    lxp_transfer_context context, lxp_receipt *receipt,
    lxp_u128 *remaining_deficit);

#endif
