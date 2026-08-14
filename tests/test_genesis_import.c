#include "layerx/lxp_genesis.h"

#include <string.h>

int main(void)
{
    static lxp_import_section sections[LXP_IMPORT_SECTION_COUNT];
    static lxp_genesis_manifest manifest;
    static lxp_import_totals_report totals;
    lxp_import_section altered;
    size_t i;
    size_t account_count;
    size_t module_count;

    (void)memset(&manifest, 0, sizeof(manifest));
    for (i = 0U; i < LXP_IMPORT_SECTION_COUNT; ++i) {
        lxp_import_item *item;
        sections[i].kind = (lxp_import_section_kind)(i + 1U);
        sections[i].item_count = 1U;
        item = &sections[i].items[0];
        item->item_id[0] = (uint8_t)(i + 1U);
        if (i < 7U) {
            item->asset_id[0] = 1U;
            item->account_id[0] = (uint8_t)(i + 1U);
            item->parent_account_id[0] = 0xf0U;
            item->amount = (lxp_u128){0U, (uint64_t)(i + 1U) * 10U};
        } else {
            item->payload_hash[0] = (uint8_t)(0xa0U + i);
        }
        if (sections[i].kind == LXP_IMPORT_HISTORICAL_COMMITMENTS)
            item->immutable = true;
    }
    for (i = 0U; i < 6U; ++i)
        if (lxp_import_balances(&sections[i], &manifest) != LXP_OK)
            return 1;
    for (i = 6U; i < 9U; ++i)
        if (lxp_import_positions(&sections[i], &manifest) != LXP_OK)
            return 1;
    if (lxp_import_bindings(&sections[9], &manifest) != LXP_OK ||
        lxp_import_history(&sections[10], &manifest) != LXP_OK ||
        manifest.account_count != 7U ||
        manifest.module_value_count != 4U ||
        !manifest.accounts[1].locked ||
        manifest.accounts[6].subaccount_kind !=
            (uint16_t)LXP_IMPORT_PERPS_POSITIONS ||
        manifest.module_values[2].module_id !=
            (uint16_t)LXP_IMPORT_DID_EVM_BINDINGS ||
        lxp_import_totals(
            sections, LXP_IMPORT_SECTION_COUNT, &totals) != LXP_OK ||
        totals.section_count != LXP_IMPORT_SECTION_COUNT)
        return 1;
    for (i = 0U; i < 7U; ++i) {
        if (totals.item_counts[i] != 1U ||
            totals.asset_total_counts[i] != 1U ||
            totals.asset_totals[i][0].amount.lo != (uint64_t)(i + 1U) * 10U)
            return 1;
    }
    for (i = 7U; i < LXP_IMPORT_SECTION_COUNT; ++i)
        if (totals.item_counts[i] != 1U ||
            totals.asset_total_counts[i] != 0U)
            return 1;

    account_count = manifest.account_count;
    altered = sections[0];
    altered.items[0].can_authorize_balance = true;
    if (lxp_import_balances(&altered, &manifest) != LXP_ERR_BALANCE_BYPASS ||
        manifest.account_count != account_count)
        return 1;
    module_count = manifest.module_value_count;
    altered = sections[9];
    altered.items[0].amount.lo = 1U;
    if (lxp_import_bindings(&altered, &manifest) != LXP_ERR_BALANCE_BYPASS ||
        manifest.module_value_count != module_count)
        return 1;
    altered = sections[10];
    altered.items[0].re_executable = true;
    if (lxp_import_history(&altered, &manifest) != LXP_ERR_AUTH_SCOPE ||
        manifest.module_value_count != module_count)
        return 1;
    altered = sections[0];
    altered.kind = LXP_IMPORT_VAULT_RESERVES;
    return lxp_import_totals(
        &altered, 1U, &totals) == LXP_ERR_NON_CANONICAL ? 0 : 1;
}
