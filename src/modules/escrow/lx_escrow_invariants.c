#include "layerx/lx_escrow.h"

#include <stdbool.h>
#include <string.h>

lxp_result lx_escrow_authority_check(const lx_account *account,
                                     lxp_authorization_kind authority_kind,
                                     uint16_t origin_module_id,
                                     uint16_t reason)
{
    if (account == NULL) return LXP_ERR_NON_CANONICAL;
    if (account->kind != LX_ACCOUNT_AGENT_ESCROW) return LXP_OK;
    if (origin_module_id != LXP_MODULE_ESCROW ||
        authority_kind == LXP_AUTH_OWNER ||
        authority_kind == LXP_AUTH_SESSION_KEY)
        return LXP_ERR_UNAUTHORIZED_DEBIT;
    if (reason != LXP_REASON_ESCROW_CAPTURE &&
        reason != LXP_REASON_ESCROW_RELEASE &&
        reason != LXP_REASON_ESCROW_RESOLVE)
        return LXP_ERR_UNAUTHORIZED_ESCROW_SPEND;
    return LXP_OK;
}

static bool terminal(lx_escrow_status state)
{
    return state == LX_ESCROW_STATE_CAPTURED ||
           state == LX_ESCROW_STATE_RELEASED ||
           state == LX_ESCROW_STATE_RESOLVED ||
           state == LX_ESCROW_STATE_TIMED_OUT;
}

lxp_result lx_escrow_invariant_check(const lx_escrow_record *record,
                                     const lx_account *escrow_account)
{
    lxp_u128 balance;
    lxp_result status;
    if (record == NULL || escrow_account == NULL ||
        memcmp(record->escrow_account, escrow_account->id, 32U) != 0)
        return LXP_FATAL_INVARIANT;
    status = lxp_state_balance_get(escrow_account, record->asset_id, &balance);
    if (status != LXP_OK) return LXP_FATAL_INVARIANT;
    if ((terminal(record->state) && !lxp_u128_is_zero(balance)) ||
        lxp_u128_cmp(record->locked_amount, balance) != 0)
        return LXP_FATAL_INVARIANT;
    return LXP_OK;
}
