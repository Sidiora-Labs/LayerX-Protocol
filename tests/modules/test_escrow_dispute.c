#include "layerx/lx_escrow.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static size_t transfer_calls;

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    ++transfer_calls;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

static void prepare_record(lx_escrow_record *record,
                           const lx_account *owner,
                           const lx_account *escrow_account,
                           const lx_account *beneficiary,
                           const lx_asset_record *asset,
                           lx_escrow_status state)
{
    (void)memset(record, 0, sizeof(*record));
    record->escrow_id[0] = 4U;
    (void)memcpy(record->owner, owner->id, 32U);
    (void)memcpy(record->escrow_account, escrow_account->id, 32U);
    (void)memcpy(record->beneficiary, beneficiary->id, 32U);
    record->arbiter[0] = 9U;
    (void)memcpy(record->asset_id, asset->asset_id, 32U);
    record->locked_amount = (lxp_u128){ 0U, 101U };
    record->state = state;
    record->expiry = 1000U;
    record->dispute_window = 600U;
}

int main(void)
{
    lx_account owner;
    lx_account escrow_account;
    lx_account beneficiary;
    lx_asset_record asset;
    lxp_transfer_asset_state asset_state;
    lx_escrow_store store;
    lx_escrow_record record;
    lx_escrow_dispute_request dispute;
    lx_escrow_capture_request capture;
    lx_escrow_release_request release;
    lxp_authority_resolved authority;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    lxp_receipt receipt;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    lxp_u128 beneficiary_share;
    lxp_u128 owner_share;

    (void)memset(&owner, 0, sizeof(owner));
    (void)memset(&escrow_account, 0, sizeof(escrow_account));
    (void)memset(&beneficiary, 0, sizeof(beneficiary));
    (void)memset(&asset, 0, sizeof(asset));
    (void)memset(&store, 0, sizeof(store));
    (void)memset(&authority, 0, sizeof(authority));
    owner.id[0] = 1U;
    owner.kind = LX_ACCOUNT_AGENT_MAIN;
    escrow_account.id[0] = 2U;
    escrow_account.kind = LX_ACCOUNT_AGENT_ESCROW;
    beneficiary.id[0] = 3U;
    beneficiary.kind = LX_ACCOUNT_AGENT_MAIN;
    asset.asset_id[0] = 5U;
    if (lxp_ledger_bootstrap_balance(&owner, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&escrow_account, asset.asset_id,
                                     (lxp_u128){ 0U, 101U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&beneficiary, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lx_asset_transfer_state(&asset, &asset_state) != LXP_OK ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_escrow_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_ESCROW, 500U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    prepare_record(&record, &owner, &escrow_account, &beneficiary, &asset,
                   LX_ESCROW_STATE_OPEN);
    if (lx_escrow_state_put(&store, &record) != LXP_OK) return 1;
    authority.kind = LXP_AUTHORITY_OWNER;
    (void)memcpy(authority.principal, beneficiary.id, 32U);
    (void)memset(&dispute, 0, sizeof(dispute));
    dispute.store = &store;
    dispute.escrow_id = record.escrow_id;
    dispute.escrow_account = &escrow_account;
    dispute.beneficiary_account = &beneficiary;
    dispute.owner_account = &owner;
    dispute.asset = &asset;
    dispute.authority = &authority;
    dispute.beneficiary_basis_points = 3333U;
    dispute.idempotency_key[0] = 1U;
    dispute.context.assets = &asset_state;
    dispute.context.asset_count = 1U;
    dispute.context.sequence_account = &escrow_account;
    (void)memcpy(dispute.context.authorized_from, escrow_account.id, 32U);
    if (lx_escrow_dispute_open_execute(&ctx, &dispute) != LXP_OK ||
        store.records[0].state != LX_ESCROW_STATE_DISPUTED ||
        transfer_calls != 0U)
        return 1;

    (void)memset(&capture, 0, sizeof(capture));
    capture.store = &store;
    capture.escrow_id = record.escrow_id;
    capture.escrow_account = &escrow_account;
    capture.beneficiary_account = &beneficiary;
    capture.owner_account = &owner;
    capture.asset = &asset;
    capture.amount = (lxp_u128){ 0U, 1U };
    capture.authority = &authority;
    capture.idempotency_key[0] = 2U;
    (void)memset(&release, 0, sizeof(release));
    release.store = &store;
    release.escrow_id = record.escrow_id;
    release.escrow_account = &escrow_account;
    release.owner_account = &owner;
    release.asset = &asset;
    release.authority = &authority;
    release.idempotency_key[0] = 3U;
    if (lx_escrow_partial_capture_execute(&ctx, &capture, &receipt) !=
            LXP_ERR_HOLD_DISPUTED ||
        lx_escrow_release_execute(&ctx, &release, &receipt) !=
            LXP_ERR_HOLD_DISPUTED ||
        lx_escrow_timeout_execute(&ctx, &release, &receipt) !=
            LXP_ERR_HOLD_DISPUTED || transfer_calls != 0U)
        return 1;

    (void)memcpy(authority.principal, record.arbiter, 32U);
    if (lx_escrow_split_bps((lxp_u128){ 0U, 101U }, 3333U,
                            &beneficiary_share, &owner_share) != LXP_OK ||
        beneficiary_share.lo != 33U || owner_share.lo != 68U ||
        lx_escrow_dispute_resolve_execute(&ctx, &dispute, &receipt) != LXP_OK ||
        transfer_calls != 1U || escrow_account.balance.lo != 0U ||
        beneficiary.balance.lo != 33U || owner.balance.lo != 68U ||
        store.records[0].state != LX_ESCROW_STATE_RESOLVED)
        return 1;

    (void)memset(&store, 0, sizeof(store));
    if (lxp_ledger_bootstrap_balance(&owner, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&escrow_account, asset.asset_id,
                                     (lxp_u128){ 0U, 101U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&beneficiary, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK)
        return 1;
    prepare_record(&record, &owner, &escrow_account, &beneficiary, &asset,
                   LX_ESCROW_STATE_DISPUTED);
    if (lx_escrow_state_put(&store, &record) != LXP_OK) return 1;
    dispute.store = &store;
    dispute.escrow_id = record.escrow_id;
    dispute.idempotency_key[0] = 4U;
    dispute.context.inject_failure = true;
    dispute.context.failure_after_leg = 0U;
    if (lx_escrow_dispute_resolve_execute(&ctx, &dispute, &receipt) !=
            LXP_ERR_IO || escrow_account.balance.lo != 101U ||
        beneficiary.balance.lo != 0U || owner.balance.lo != 0U ||
        store.records[0].state != LX_ESCROW_STATE_DISPUTED)
        return 1;

    store.records[0].state = LX_ESCROW_STATE_OPEN;
    store.records[0].dispute_window = 499U;
    (void)memcpy(authority.principal, beneficiary.id, 32U);
    dispute.authority = &authority;
    if (lx_escrow_dispute_open_execute(&ctx, &dispute) !=
            LXP_ERR_DISPUTE_WINDOW_CLOSED ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
