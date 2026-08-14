#include "layerx/lxp_reserve.h"

#include <string.h>

static const lxp_outstanding_claims *claim_for(
    const lxp_outstanding_claims *claims,
    size_t claim_count,
    const uint8_t asset_id[32])
{
    size_t i;
    for (i = 0U; i < claim_count; ++i)
        if (memcmp(claims[i].asset_id, asset_id, 32U) == 0)
            return &claims[i];
    return NULL;
}

static uint32_t class_mask(const lx_asset_reserve_report_record *report)
{
    uint32_t mask = LXP_RESERVE_ALL_CLASSES;
    (void)report;
    return mask;
}

lxp_result lxp_reserve_report(
    const lx_account_registry *accounts,
    const lx_asset_custody_attestation *attestation,
    const lxp_outstanding_claims *claims,
    lxp_reserve_report_view *report)
{
    lxp_result status;
    if (accounts == NULL || attestation == NULL || claims == NULL ||
        report == NULL ||
        memcmp(attestation->asset_id, claims->asset_id, 32U) != 0)
        return LXP_FATAL_SUPPLY_MISMATCH;
    (void)memset(report, 0, sizeof(*report));
    status = lx_asset_reserve_report(
        accounts, attestation, &report->accounts);
    if (status != LXP_OK) return LXP_FATAL_SUPPLY_MISMATCH;
    report->outstanding_claims = claims->amount;
    status = lxp_u128_sub(attestation->custody_amount,
                          attestation->settled_out,
                          &report->available_custody);
    if (status == LXP_OK)
        status = lxp_u128_sub(report->accounts.circulating,
                              report->accounts.withdrawals,
                              &report->required_backing);
    if (status == LXP_OK)
        status = lxp_u128_add(report->required_backing,
                              claims->amount,
                              &report->required_backing);
    if (status == LXP_OK)
        status = lxp_u128_sub(report->available_custody,
                              report->required_backing,
                              &report->excess_backing);
    if (status != LXP_OK) return LXP_FATAL_SUPPLY_MISMATCH;
    report->contributing_class_mask = class_mask(&report->accounts);
    report->zero_tolerance_match =
        lxp_u128_cmp(claims->amount, report->accounts.withdrawals) == 0 &&
        lxp_u128_cmp(report->accounts.effective_total,
                     report->accounts.expected_backing) == 0;
    return LXP_OK;
}

lxp_result lxp_reserve_reconcile(
    const lx_account_registry *accounts,
    const lx_asset_custody_attestation *attestation,
    const lxp_outstanding_claims *claims,
    lxp_reserve_report_view *report)
{
    if (lxp_reserve_report(accounts, attestation, claims, report) != LXP_OK ||
        !report->zero_tolerance_match ||
        lxp_u128_cmp(report->required_backing,
                     report->available_custody) > 0 ||
        lx_asset_reserve_reconcile(
            accounts, attestation, &report->accounts) != LXP_OK)
        return LXP_FATAL_SUPPLY_MISMATCH;
    return LXP_OK;
}

lxp_result lxp_supply_invariant_check(
    const lx_asset_registry *assets,
    const lx_account_registry *accounts,
    const lx_asset_custody_attestation *attestations,
    size_t attestation_count,
    const lxp_outstanding_claims *claims,
    size_t claim_count)
{
    size_t i;
    if (assets == NULL || accounts == NULL || attestations == NULL ||
        claims == NULL || attestation_count == 0U || claim_count == 0U ||
        lx_asset_supply_check(
            assets, accounts, attestations, attestation_count) != LXP_OK)
        return LXP_FATAL_SUPPLY_MISMATCH;
    for (i = 0U; i < attestation_count; ++i) {
        const lxp_outstanding_claims *asset_claims = claim_for(
            claims, claim_count, attestations[i].asset_id);
        lxp_reserve_report_view report;
        if (asset_claims == NULL ||
            lxp_reserve_reconcile(
                accounts, &attestations[i], asset_claims, &report) != LXP_OK)
            return LXP_FATAL_SUPPLY_MISMATCH;
    }
    return LXP_OK;
}
