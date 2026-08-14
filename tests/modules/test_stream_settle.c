#include "layerx/lx_stream.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

static void configure_record(lx_stream_record *record,
                             const lx_account *payer,
                             const lx_account *stream_account,
                             const lx_account *recipient,
                             const lx_asset_record *asset,
                             uint8_t stream_marker)
{
    (void)memset(record, 0, sizeof(*record));
    record->stream_id[0] = stream_marker;
    (void)memcpy(record->payer, payer->id, 32U);
    (void)memcpy(record->stream_account, stream_account->id, 32U);
    (void)memcpy(record->recipient, recipient->id, 32U);
    (void)memcpy(record->asset_id, asset->asset_id, 32U);
    record->mode = LX_STREAM_MODE_TIME;
    record->rate = (lxp_u128){ 0U, 10U };
    record->rate_unit = 1000U;
    record->start_timestamp = 100U;
    record->last_accrual_timestamp = 100U;
    record->total_cap = (lxp_u128){ 0U, 100U };
}

static void configure_settle(lx_stream_settle_request *request,
                             lx_stream_store *store,
                             const lx_stream_record *record,
                             lx_account *stream_account,
                             lx_account *recipient,
                             const lx_asset_record *asset,
                             const lxp_transfer_asset_state *asset_state,
                             uint8_t key_marker)
{
    (void)memset(request, 0, sizeof(*request));
    request->store = store;
    request->stream_id = record->stream_id;
    request->stream_account = stream_account;
    request->recipient = recipient;
    request->asset = asset;
    request->idempotency_key[0] = key_marker;
    request->context.assets = asset_state;
    request->context.asset_count = 1U;
    request->context.sequence_account = stream_account;
    (void)memcpy(request->context.authorized_from, stream_account->id, 32U);
}

int main(void)
{
    lx_account payer;
    lx_account exact_stream;
    lx_account exact_recipient;
    lx_account short_stream;
    lx_account short_recipient;
    lx_asset_record asset;
    lxp_transfer_asset_state asset_state;
    lx_stream_store store;
    lx_stream_record record;
    lx_stream_record *stored;
    lx_stream_settle_request settle;
    lx_stream_fund_request top_up;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    lxp_receipt receipt;
    lxp_receipt replayed;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;

    (void)memset(&payer, 0, sizeof(payer));
    (void)memset(&exact_stream, 0, sizeof(exact_stream));
    (void)memset(&exact_recipient, 0, sizeof(exact_recipient));
    (void)memset(&short_stream, 0, sizeof(short_stream));
    (void)memset(&short_recipient, 0, sizeof(short_recipient));
    (void)memset(&asset, 0, sizeof(asset));
    (void)memset(&store, 0, sizeof(store));
    (void)memset(&receipt, 0, sizeof(receipt));
    payer.id[0] = 1U; payer.kind = LX_ACCOUNT_AGENT_MAIN;
    exact_stream.id[0] = 2U; exact_stream.kind = LX_ACCOUNT_AGENT_STREAM;
    exact_recipient.id[0] = 3U; exact_recipient.kind = LX_ACCOUNT_AGENT_MAIN;
    short_stream.id[0] = 4U; short_stream.kind = LX_ACCOUNT_AGENT_STREAM;
    short_recipient.id[0] = 5U; short_recipient.kind = LX_ACCOUNT_AGENT_MAIN;
    asset.asset_id[0] = 6U;
    if (lxp_ledger_bootstrap_balance(&payer, asset.asset_id,
                                     (lxp_u128){ 0U, 20U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&exact_stream, asset.asset_id,
                                     (lxp_u128){ 0U, 10U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&exact_recipient, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&short_stream, asset.asset_id,
                                     (lxp_u128){ 0U, 5U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&short_recipient, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lx_asset_transfer_state(&asset, &asset_state) != LXP_OK ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_stream_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK)
        return 1;

    configure_record(&record, &payer, &exact_stream, &exact_recipient,
                     &asset, 7U);
    if (lx_stream_state_put(&store, &record) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_STREAM, 1100U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    configure_settle(&settle, &store, &record, &exact_stream,
                     &exact_recipient, &asset, &asset_state, 8U);
    if (lx_stream_settle_execute(&ctx, &settle, &receipt) != LXP_OK ||
        exact_stream.balance.lo != 0U || exact_recipient.balance.lo != 10U ||
        lx_stream_lookup(&store, record.stream_id, &stored) != LXP_OK ||
        stored->accrued_total.lo != 10U || stored->settled_total.lo != 10U ||
        stored->underfunded)
        return 1;
    replayed = receipt;
    (void)memset(&receipt, 0, sizeof(receipt));
    if (lx_stream_settle_execute(&ctx, &settle, &receipt) != LXP_OK ||
        memcmp(&receipt, &replayed, sizeof(receipt)) != 0 ||
        exact_recipient.balance.lo != 10U)
        return 1;

    configure_record(&record, &payer, &short_stream, &short_recipient,
                     &asset, 9U);
    if (lx_stream_state_put(&store, &record) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_STREAM, 1100U, 0U, 2U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    configure_settle(&settle, &store, &record, &short_stream,
                     &short_recipient, &asset, &asset_state, 10U);
    if (lx_stream_settle_execute(&ctx, &settle, &receipt) != LXP_OK ||
        short_stream.balance.lo != 0U || short_recipient.balance.lo != 5U ||
        lx_stream_lookup(&store, record.stream_id, &stored) != LXP_OK ||
        !stored->underfunded || stored->settled_total.lo != 5U ||
        stored->accrued_total.lo != 5U)
        return 1;

    (void)memset(&top_up, 0, sizeof(top_up));
    top_up.store = &store;
    top_up.payer = &payer;
    top_up.stream_account = &short_stream;
    top_up.asset = &asset;
    top_up.amount = (lxp_u128){ 0U, 15U };
    top_up.context.assets = &asset_state;
    top_up.context.asset_count = 1U;
    top_up.context.sequence_account = &payer;
    (void)memcpy(top_up.context.authorized_from, payer.id, 32U);
    top_up.record = *stored;
    if (lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_STREAM, 2000U, 0U, 3U,
                            1000U, &arena, true) != LXP_OK ||
        lx_stream_top_up_execute(&ctx, &top_up, &receipt) != LXP_OK ||
        stored->underfunded || stored->last_accrual_timestamp != 2000U ||
        short_stream.balance.lo != 15U)
        return 1;
    configure_settle(&settle, &store, stored, &short_stream,
                     &short_recipient, &asset, &asset_state, 11U);
    settle.context.actor_sequence = short_stream.next_sequence;
    if (lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_STREAM, 3000U, 0U, 4U,
                            1000U, &arena, true) != LXP_OK ||
        lx_stream_settle_execute(&ctx, &settle, &receipt) != LXP_OK ||
        short_stream.balance.lo != 5U || short_recipient.balance.lo != 15U ||
        stored->settled_total.lo != 15U || stored->accrued_total.lo != 15U ||
        stored->underfunded)
        return 1;

    stored->closed = true;
    settle.idempotency_key[0] = 12U;
    settle.context.actor_sequence = short_stream.next_sequence;
    if (lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_STREAM, 4000U, 0U, 5U,
                            1000U, &arena, true) != LXP_OK ||
        lx_stream_settle_execute(&ctx, &settle, &receipt) !=
            LXP_ERR_STREAM_CLOSED ||
        short_stream.balance.lo != 5U || short_recipient.balance.lo != 15U ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
