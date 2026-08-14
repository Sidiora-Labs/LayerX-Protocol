#include "layerx/lx_escrow.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static size_t transfer_calls;
static uint16_t last_reason;

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    ++transfer_calls;
    last_reason = set->legs[0].reason;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

int main(void)
{
    lx_account_registry accounts;
    lx_account *escrow_account;
    lx_account *beneficiary;
    lx_asset_record asset;
    lxp_transfer_asset_state asset_state;
    lx_escrow_store store;
    lx_escrow_record record;
    lx_escrow_capture_request request;
    lxp_authority_resolved authority;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_receipt first_receipt;
    lxp_receipt replay_receipt;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    const char *escrow_name = "agent:did:key:owner:escrow:hold-1";
    const char *beneficiary_name = "agent:did:key:provider:main";
    uint8_t escrow_id[32];
    uint8_t beneficiary_id[32];
    lxp_u128 remaining;

    (void)memset(&asset, 0, sizeof(asset));
    asset.asset_id[0] = 1U;
    (void)memset(&store, 0, sizeof(store));
    (void)memset(&record, 0, sizeof(record));
    (void)memset(&authority, 0, sizeof(authority));
    (void)memset(&first_receipt, 0, sizeof(first_receipt));
    (void)memset(&replay_receipt, 0xff, sizeof(replay_receipt));
    if (lx_account_registry_init(&accounts) != LXP_OK ||
        lx_account_id_from_string((const uint8_t *)escrow_name,
                                  strlen(escrow_name), escrow_id) != LXP_OK ||
        lx_account_id_from_string((const uint8_t *)beneficiary_name,
                                  strlen(beneficiary_name), beneficiary_id) != LXP_OK ||
        lx_account_open(&accounts, (const uint8_t *)escrow_name,
                        strlen(escrow_name), escrow_id, 1U,
                        LX_ACCOUNT_OPEN_CREDIT, NULL, &escrow_account) != LXP_OK ||
        lx_account_open(&accounts, (const uint8_t *)beneficiary_name,
                        strlen(beneficiary_name), beneficiary_id, 2U,
                        LX_ACCOUNT_OPEN_CREDIT, NULL, &beneficiary) != LXP_OK ||
        lxp_ledger_bootstrap_balance(escrow_account, asset.asset_id,
                                     (lxp_u128){ 0U, 100U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(beneficiary, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lx_asset_transfer_state(&asset, &asset_state) != LXP_OK ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_escrow_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_ESCROW, 10U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK)
        return 1;

    record.escrow_id[0] = 3U;
    record.owner[0] = 4U;
    (void)memcpy(record.escrow_account, escrow_account->id, 32U);
    (void)memcpy(record.beneficiary, beneficiary->id, 32U);
    (void)memcpy(record.asset_id, asset.asset_id, 32U);
    record.locked_amount = (lxp_u128){ 0U, 100U };
    record.state = LX_ESCROW_STATE_OPEN;
    if (lx_escrow_state_put(&store, &record) != LXP_OK) return 1;

    authority.kind = LXP_AUTHORITY_OWNER;
    (void)memcpy(authority.principal, beneficiary->id, 32U);
    (void)memset(&request, 0, sizeof(request));
    request.store = &store;
    request.escrow_id = record.escrow_id;
    request.escrow_account = escrow_account;
    request.beneficiary_account = beneficiary;
    request.asset = &asset;
    request.amount = (lxp_u128){ 0U, 30U };
    request.authority = &authority;
    request.idempotency_key[0] = 1U;
    request.context.assets = &asset_state;
    request.context.asset_count = 1U;
    request.context.sequence_account = escrow_account;
    (void)memcpy(request.context.authorized_from, escrow_account->id, 32U);
    if (lx_escrow_partial_capture_execute(&ctx, &request, &first_receipt) != LXP_OK ||
        transfer_calls != 1U || last_reason != LXP_REASON_ESCROW_CAPTURE ||
        escrow_account->balance.lo != 70U || beneficiary->balance.lo != 30U ||
        store.records[0].captured_amount.lo != 30U ||
        store.records[0].state != LX_ESCROW_STATE_PARTIALLY_CAPTURED ||
        lx_escrow_remaining(&store.records[0], escrow_account,
                            &remaining) != LXP_OK || remaining.lo != 70U)
        return 1;

    request.amount = (lxp_u128){ 0U, 10U };
    if (lx_escrow_partial_capture_execute(&ctx, &request, &replay_receipt) != LXP_OK ||
        transfer_calls != 1U || memcmp(&first_receipt, &replay_receipt,
                                       sizeof(first_receipt)) != 0 ||
        escrow_account->balance.lo != 70U || beneficiary->balance.lo != 30U)
        return 1;

    request.idempotency_key[0] = 2U;
    request.amount = (lxp_u128){ 0U, 71U };
    if (lx_escrow_partial_capture_execute(&ctx, &request, &replay_receipt) !=
            LXP_ERR_CAPTURE_EXCEEDS_HOLD || transfer_calls != 1U)
        return 1;
    authority.principal[0] ^= 0xffU;
    request.amount = (lxp_u128){ 0U, 1U };
    if (lx_escrow_partial_capture_execute(&ctx, &request, &replay_receipt) !=
            LXP_ERR_UNAUTHORIZED_CAPTURE || transfer_calls != 1U)
        return 1;
    (void)memcpy(authority.principal, record.owner, 32U);
    authority.kind = LXP_AUTHORITY_DELEGATED_CAPABILITY;
    request.amount = (lxp_u128){ 0U, 0U };
    request.context.actor_sequence = 1U;
    if (lx_escrow_capture_execute(&ctx, &request, &replay_receipt) != LXP_OK ||
        transfer_calls != 2U || escrow_account->balance.lo != 0U ||
        beneficiary->balance.lo != 100U ||
        store.records[0].captured_amount.lo != 100U ||
        store.records[0].state != LX_ESCROW_STATE_CAPTURED ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
