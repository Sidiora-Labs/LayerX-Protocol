#include "layerx/lxp_fee.h"

#include <limits.h>
#include <stdint.h>
#include <string.h>

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size);

static int test_meter_accumulation(void)
{
    lxp_meter_ctx meter;
    lxp_fee_meter usage;
    uint64_t before;
    lxp_u128 fee_before;
    if (lxp_meter_init(&meter, 100U, 20U, (lxp_u128){0U, 3U},
                       (lxp_u128){0U, 1000U}, 7U, true) != LXP_OK ||
        meter.parameter_version != 7U ||
        lxp_meter_charge_exec(&meter, 100U) != LXP_OK ||
        lxp_meter_fee_usage(&meter, 44U, &usage) != LXP_OK ||
        usage.canonical_encoded_bytes != 44U ||
        usage.execution_units != 100U || usage.storage_units != 0U ||
        lxp_meter_charge_exec(&meter, 1U) != LXP_ERR_GAS_EXHAUSTED ||
        meter.execution_units != 101U ||
        lxp_meter_exhausted(&meter) != LXP_ERR_GAS_EXHAUSTED ||
        lxp_meter_charge_exec(&meter, 0U) != LXP_ERR_GAS_EXHAUSTED)
        return 1;

    if (lxp_meter_init(&meter, UINT64_MAX, 20U, (lxp_u128){0U, 3U},
                       (lxp_u128){0U, 1000U}, 7U, true) != LXP_OK)
        return 1;
    meter.execution_units = UINT64_MAX - 1U;
    before = meter.execution_units;
    if (lxp_meter_charge_exec(&meter, 2U) != LXP_ERR_OVERFLOW ||
        meter.execution_units != before || meter.exhausted)
        return 1;

    if (lxp_meter_charge_storage(&meter, 10) != LXP_OK ||
        meter.net_storage_bytes != 10U || meter.storage_fee.hi != 0U ||
        meter.storage_fee.lo != 30U ||
        lxp_meter_charge_storage(&meter, -4) != LXP_OK ||
        meter.net_storage_bytes != 6U || meter.storage_fee.lo != 18U)
        return 1;
    before = meter.net_storage_bytes;
    fee_before = meter.storage_fee;
    if (lxp_meter_charge_storage(&meter, -7) != LXP_ERR_OVERFLOW ||
        meter.net_storage_bytes != before ||
        lxp_u128_cmp(meter.storage_fee, fee_before) != 0 || meter.exhausted ||
        lxp_meter_charge_storage(&meter, 15) != LXP_ERR_GAS_EXHAUSTED ||
        meter.net_storage_bytes != 21U || meter.storage_fee.lo != 63U ||
        lxp_meter_exhausted(&meter) != LXP_ERR_GAS_EXHAUSTED)
        return 1;
    return 0;
}

static int test_meter_overflow_and_admission(void)
{
    lxp_meter_ctx meter;
    if (lxp_meter_init(&meter, UINT64_MAX, UINT64_MAX,
                       (lxp_u128){UINT64_MAX, UINT64_MAX},
                       (lxp_u128){UINT64_MAX, UINT64_MAX}, 1U, true) != LXP_OK ||
        lxp_meter_charge_storage(&meter, 2) != LXP_ERR_OVERFLOW ||
        meter.net_storage_bytes != 0U ||
        !lxp_u128_is_zero(meter.storage_fee) ||
        lxp_meter_init(&meter, 1U, 1U, (lxp_u128){0U, 1U},
                       (lxp_u128){0U, 1U}, 0U, true) !=
            LXP_ERR_NON_CANONICAL ||
        lxp_meter_init(&meter, 1U, 1U, (lxp_u128){0U, 1U},
                       (lxp_u128){0U, 1U}, 1U, false) !=
            LXP_ERR_NON_CANONICAL)
        return 1;
    if (lxp_meter_admission_check(false, true, (lxp_u128){0U, 4U},
                                  (lxp_u128){0U, 4U}) !=
            LXP_ERR_MALFORMED_ENVELOPE ||
        lxp_meter_admission_check(true, false, (lxp_u128){0U, 4U},
                                  (lxp_u128){0U, 4U}) !=
            LXP_ERR_MALFORMED_ENVELOPE ||
        lxp_meter_admission_check(true, true, (lxp_u128){0U, 4U},
                                  (lxp_u128){0U, 3U}) !=
            LXP_ERR_FEE_UNPAYABLE ||
        lxp_meter_admission_check(true, true, (lxp_u128){0U, 4U},
                                  (lxp_u128){0U, 4U}) != LXP_OK ||
        lxp_fee_limit_check((lxp_u128){0U, 4U}, (lxp_u128){0U, 4U},
                            (lxp_u128){0U, 4U}) != LXP_OK ||
        lxp_fee_limit_check((lxp_u128){0U, 5U}, (lxp_u128){0U, 4U},
                            (lxp_u128){0U, 5U}) != LXP_ERR_FEE_LIMIT)
        return 1;
    return 0;
}

static int test_treasury_overflow(void)
{
    static const uint8_t actor_name[] = "agent:meter:main";
    static const uint8_t treasury_name[] = "system:fees";
    uint8_t actor_id[32];
    uint8_t treasury_id[32];
    uint8_t asset_id[32] = {1U};
    lx_account_registry registry;
    lx_account *actor;
    lx_account *treasury;
    lxp_transfer_asset_state asset;
    lxp_transfer_context context;
    lxp_transfer_result result;
    lxp_receipt receipt;
    if (lx_account_registry_init(&registry) != LXP_OK ||
        lx_account_id_from_string(actor_name, sizeof(actor_name) - 1U,
                                  actor_id) != LXP_OK ||
        lx_account_id_from_string(treasury_name, sizeof(treasury_name) - 1U,
                                  treasury_id) != LXP_OK ||
        lx_account_open(&registry, actor_name, sizeof(actor_name) - 1U,
                        actor_id, 1U, LX_ACCOUNT_OPEN_GENESIS, NULL,
                        &actor) != LXP_OK ||
        lx_account_open(&registry, treasury_name, sizeof(treasury_name) - 1U,
                        treasury_id, 2U, LX_ACCOUNT_OPEN_GENESIS, NULL,
                        &treasury) != LXP_OK ||
        lxp_ledger_bootstrap_balance(actor, asset_id, (lxp_u128){0U, 10U},
                                     3U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(
            treasury, asset_id, (lxp_u128){UINT64_MAX, UINT64_MAX}, 0U) !=
            LXP_OK)
        return 1;
    (void)memset(&asset, 0, sizeof(asset));
    (void)memcpy(asset.asset_id, asset_id, sizeof(asset_id));
    asset.registered = true;
    (void)memset(&context, 0, sizeof(context));
    context.assets = &asset;
    context.asset_count = 1U;
    (void)memcpy(context.authorized_from, actor->id, sizeof(actor->id));
    context.actor_sequence = 3U;
    context.origin_module_id = 1U;
    context.debit_authority_kind = LXP_AUTH_OWNER;
    (void)memset(&receipt, 0, sizeof(receipt));
    receipt.fee_charged = (lxp_u128){0U, 9U};
    if (lxp_fee_charge(actor, treasury, asset_id, (lxp_u128){0U, 1U},
                       (lxp_u128){0U, 1U}, &context, &receipt, &result) !=
            LXP_ERR_OVERFLOW ||
        actor->balance.lo != 10U || actor->next_sequence != 3U ||
        treasury->balance.hi != UINT64_MAX ||
        treasury->balance.lo != UINT64_MAX ||
        receipt.fee_charged.lo != 9U)
        return 1;
    return 0;
}

int main(void)
{
    static const uint8_t corpus[][41] = {
        {0U},
        {0xffU, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU,
         0U, 0U, 0U, 0U, 0U, 0U, 0U, 1U,
         0U, 0U, 0U, 0U, 0U, 0U, 0U, 0U,
         0U, 0U, 0U, 0U, 0U, 0U, 0U, 3U,
         1U, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU, 0xffU}
    };
    size_t i;
    if (test_meter_accumulation() != 0 ||
        test_meter_overflow_and_admission() != 0 ||
        test_treasury_overflow() != 0)
        return 1;
    for (i = 0U; i < sizeof(corpus) / sizeof(corpus[0]); ++i)
        if (LLVMFuzzerTestOneInput(corpus[i], sizeof(corpus[i])) != 0)
            return 1;
    return 0;
}
