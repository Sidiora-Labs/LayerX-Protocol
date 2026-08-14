#ifndef LAYERX_LXP_RESERVE_H
#define LAYERX_LXP_RESERVE_H

#include "layerx/lx_asset.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
    LXP_RESERVE_CLASS_AGENT_MAIN = 1U << 0,
    LXP_RESERVE_CLASS_ESCROW = 1U << 1,
    LXP_RESERVE_CLASS_BUDGET = 1U << 2,
    LXP_RESERVE_CLASS_STREAM = 1U << 3,
    LXP_RESERVE_CLASS_MARGIN = 1U << 4,
    LXP_RESERVE_CLASS_LIQUIDITY = 1U << 5,
    LXP_RESERVE_CLASS_INSURANCE = 1U << 6,
    LXP_RESERVE_CLASS_FEES = 1U << 7,
    LXP_RESERVE_CLASS_WITHDRAWALS = 1U << 8,
    LXP_RESERVE_CLASS_OTHER_SYSTEM = 1U << 9,
    LXP_RESERVE_CLASS_RESERVE_MIRROR = 1U << 10,
    LXP_RESERVE_ALL_CLASSES = (1U << 11) - 1U
};

typedef struct lxp_outstanding_claims {
    uint8_t asset_id[32];
    lxp_u128 amount;
} lxp_outstanding_claims;

typedef struct lxp_reserve_report_view {
    lx_asset_reserve_report_record accounts;
    lxp_u128 outstanding_claims;
    lxp_u128 available_custody;
    lxp_u128 required_backing;
    lxp_u128 excess_backing;
    uint32_t contributing_class_mask;
    bool zero_tolerance_match;
} lxp_reserve_report_view;

lxp_result lxp_reserve_report(
    const lx_account_registry *accounts,
    const lx_asset_custody_attestation *attestation,
    const lxp_outstanding_claims *claims,
    lxp_reserve_report_view *report);
lxp_result lxp_reserve_reconcile(
    const lx_account_registry *accounts,
    const lx_asset_custody_attestation *attestation,
    const lxp_outstanding_claims *claims,
    lxp_reserve_report_view *report);
lxp_result lxp_supply_invariant_check(
    const lx_asset_registry *assets,
    const lx_account_registry *accounts,
    const lx_asset_custody_attestation *attestations,
    size_t attestation_count,
    const lxp_outstanding_claims *claims,
    size_t claim_count);

#endif
