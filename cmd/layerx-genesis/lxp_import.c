#include "layerx/lxp_genesis.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

static bool balance_section(lxp_import_section_kind kind)
{
    return kind >= LXP_IMPORT_USDX_BALANCES &&
        kind <= LXP_IMPORT_INSURANCE_POOLS;
}

static lxp_result append_account(
    lxp_genesis_manifest *manifest, const lxp_import_item *item,
    bool locked, uint16_t subaccount_kind)
{
    lxp_genesis_account *account;
    if (manifest->account_count == LXP_GENESIS_MAX_ACCOUNTS)
        return LXP_ERR_LENGTH_LIMIT;
    if (lxp_ct_is_zero(item->account_id, 32U) ||
        lxp_ct_is_zero(item->asset_id, 32U) ||
        lxp_u128_is_zero(item->amount) || item->can_authorize_balance)
        return LXP_ERR_BALANCE_BYPASS;
    if (locked && lxp_ct_is_zero(item->parent_account_id, 32U))
        return LXP_ERR_NON_CANONICAL;
    account = &manifest->accounts[manifest->account_count++];
    (void)memset(account, 0, sizeof(*account));
    (void)memcpy(account->account_id, item->account_id, 32U);
    (void)memcpy(account->asset_id, item->asset_id, 32U);
    account->balance = item->amount;
    account->locked = locked;
    account->subaccount_kind = subaccount_kind;
    if (locked) (void)memcpy(
        account->parent_account_id, item->parent_account_id, 32U);
    return LXP_OK;
}

static lxp_result append_module_value(
    lxp_genesis_manifest *manifest, uint16_t module_id,
    const lxp_import_item *item)
{
    lxp_genesis_module_value *value;
    if (manifest->module_value_count == LXP_GENESIS_MAX_MODULE_VALUES)
        return LXP_ERR_LENGTH_LIMIT;
    if (lxp_ct_is_zero(item->item_id, 32U) ||
        lxp_ct_is_zero(item->payload_hash, 32U) ||
        !lxp_u128_is_zero(item->amount) || item->can_authorize_balance)
        return LXP_ERR_BALANCE_BYPASS;
    value = &manifest->module_values[manifest->module_value_count++];
    (void)memset(value, 0, sizeof(*value));
    value->module_id = module_id;
    (void)memcpy(value->key, item->item_id, 32U);
    (void)memcpy(value->value, item->payload_hash, 32U);
    value->value_length = 32U;
    return LXP_OK;
}

lxp_result lxp_import_balances(
    const lxp_import_section *section, lxp_genesis_manifest *manifest)
{
    size_t original_count;
    size_t i;
    lxp_result status = LXP_OK;
    if (section == NULL || manifest == NULL ||
        !balance_section(section->kind) || section->item_count == 0U ||
        section->item_count > LXP_IMPORT_MAX_ITEMS)
        return LXP_ERR_NON_CANONICAL;
    original_count = manifest->account_count;
    for (i = 0U; i < section->item_count && status == LXP_OK; ++i)
        status = append_account(
            manifest, &section->items[i],
            section->kind != LXP_IMPORT_USDX_BALANCES,
            (uint16_t)section->kind);
    if (status != LXP_OK) manifest->account_count = original_count;
    return status;
}

lxp_result lxp_import_positions(
    const lxp_import_section *section, lxp_genesis_manifest *manifest)
{
    size_t original_accounts;
    size_t original_modules;
    size_t i;
    lxp_result status = LXP_OK;
    if (section == NULL || manifest == NULL || section->item_count == 0U ||
        section->item_count > LXP_IMPORT_MAX_ITEMS ||
        (section->kind != LXP_IMPORT_PERPS_POSITIONS &&
         section->kind != LXP_IMPORT_PENDING_ORDERS &&
         section->kind != LXP_IMPORT_FUNDING_STATE))
        return LXP_ERR_NON_CANONICAL;
    original_accounts = manifest->account_count;
    original_modules = manifest->module_value_count;
    for (i = 0U; i < section->item_count && status == LXP_OK; ++i) {
        if (section->kind == LXP_IMPORT_PERPS_POSITIONS)
            status = append_account(
                manifest, &section->items[i], true,
                (uint16_t)LXP_IMPORT_PERPS_POSITIONS);
        else
            status = append_module_value(
                manifest, (uint16_t)section->kind, &section->items[i]);
    }
    if (status != LXP_OK) {
        manifest->account_count = original_accounts;
        manifest->module_value_count = original_modules;
    }
    return status;
}

lxp_result lxp_import_bindings(
    const lxp_import_section *section, lxp_genesis_manifest *manifest)
{
    size_t original_count;
    size_t i;
    lxp_result status = LXP_OK;
    if (section == NULL || manifest == NULL ||
        section->kind != LXP_IMPORT_DID_EVM_BINDINGS ||
        section->item_count == 0U ||
        section->item_count > LXP_IMPORT_MAX_ITEMS)
        return LXP_ERR_NON_CANONICAL;
    original_count = manifest->module_value_count;
    for (i = 0U; i < section->item_count && status == LXP_OK; ++i)
        status = append_module_value(
            manifest, (uint16_t)LXP_IMPORT_DID_EVM_BINDINGS,
            &section->items[i]);
    if (status != LXP_OK) manifest->module_value_count = original_count;
    return status;
}

lxp_result lxp_import_history(
    const lxp_import_section *section, lxp_genesis_manifest *manifest)
{
    size_t original_count;
    size_t i;
    lxp_result status = LXP_OK;
    if (section == NULL || manifest == NULL ||
        section->kind != LXP_IMPORT_HISTORICAL_COMMITMENTS ||
        section->item_count == 0U ||
        section->item_count > LXP_IMPORT_MAX_ITEMS)
        return LXP_ERR_NON_CANONICAL;
    original_count = manifest->module_value_count;
    for (i = 0U; i < section->item_count && status == LXP_OK; ++i) {
        if (!section->items[i].immutable ||
            section->items[i].re_executable)
            status = LXP_ERR_AUTH_SCOPE;
        else
            status = append_module_value(
                manifest,
                (uint16_t)LXP_IMPORT_HISTORICAL_COMMITMENTS,
                &section->items[i]);
    }
    if (status != LXP_OK) manifest->module_value_count = original_count;
    return status;
}

static lxp_result total_add(
    lxp_import_asset_total *totals, size_t *count,
    const uint8_t asset_id[32], lxp_u128 amount)
{
    size_t i;
    for (i = 0U; i < *count; ++i) {
        if (memcmp(totals[i].asset_id, asset_id, 32U) == 0)
            return lxp_u128_add(totals[i].amount, amount, &totals[i].amount);
    }
    if (*count == LXP_IMPORT_MAX_ASSET_TOTALS)
        return LXP_ERR_LENGTH_LIMIT;
    (void)memcpy(totals[*count].asset_id, asset_id, 32U);
    totals[*count].amount = amount;
    ++*count;
    return LXP_OK;
}

lxp_result lxp_import_totals(
    const lxp_import_section *sections, size_t section_count,
    lxp_import_totals_report *report)
{
    bool seen[LXP_IMPORT_SECTION_COUNT] = {false};
    size_t i;
    if (sections == NULL || report == NULL ||
        section_count != LXP_IMPORT_SECTION_COUNT)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(report, 0, sizeof(*report));
    for (i = 0U; i < section_count; ++i) {
        size_t index;
        size_t j;
        lxp_result status = LXP_OK;
        if (sections[i].kind < LXP_IMPORT_USDX_BALANCES ||
            sections[i].kind > LXP_IMPORT_HISTORICAL_COMMITMENTS)
            return LXP_ERR_NON_CANONICAL;
        index = (size_t)sections[i].kind - 1U;
        if (seen[index] || sections[i].item_count == 0U ||
            sections[i].item_count > LXP_IMPORT_MAX_ITEMS)
            return LXP_ERR_NON_CANONICAL;
        seen[index] = true;
        report->item_counts[index] = sections[i].item_count;
        for (j = 0U; j < sections[i].item_count && status == LXP_OK; ++j) {
            if (!lxp_u128_is_zero(sections[i].items[j].amount))
                status = total_add(
                    report->asset_totals[index],
                    &report->asset_total_counts[index],
                    sections[i].items[j].asset_id,
                    sections[i].items[j].amount);
        }
        if (status != LXP_OK) return status;
    }
    report->section_count = section_count;
    return LXP_OK;
}
