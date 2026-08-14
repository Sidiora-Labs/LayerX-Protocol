#include "layerx/lx_oracle.h"

lxp_result lx_oracle_market_halt(lx_oracle_market *market)
{
    if (market == NULL) return LXP_ERR_NON_CANONICAL;
    market->halted = true;
    return LXP_OK;
}

bool lx_oracle_market_halted(const lx_oracle_market *market)
{
    return market == NULL || market->halted;
}

lxp_result lx_oracle_market_action_check(
    const lx_oracle_market *market, lx_oracle_market_action action)
{
    if (market == NULL || action < LX_ORACLE_ACTION_ORDER_PLACE ||
        action > LX_ORACLE_ACTION_MARGIN_ADD)
        return LXP_ERR_NON_CANONICAL;
    if (!lx_oracle_market_halted(market)) return LXP_OK;
    return action == LX_ORACLE_ACTION_ORDER_CANCEL ||
           action == LX_ORACLE_ACTION_MARGIN_ADD ?
        LXP_OK : LXP_ERR_MARKET_HALTED;
}

lxp_result lx_oracle_fail_closed_eval(lx_oracle_market *market,
                                      const lx_oracle_store *store,
                                      uint64_t batch_timestamp)
{
    const lx_oracle_accepted *latest = NULL;
    lxp_result status;
    if (market == NULL || store == NULL) return LXP_ERR_NON_CANONICAL;
    status = lx_oracle_store_latest(store, market->market_id, &latest);
    if (status != LXP_OK || latest == NULL ||
        lx_oracle_staleness_check(market, &latest->observation,
                                  batch_timestamp) != LXP_OK) {
        (void)lx_oracle_market_halt(market);
        return LXP_ERR_MARKET_HALTED;
    }
    return market->halted ? LXP_ERR_MARKET_HALTED : LXP_OK;
}
