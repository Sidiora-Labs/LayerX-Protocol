#ifndef LAYERX_LX_ESCROW_H
#define LAYERX_LX_ESCROW_H

#include "layerx/lx_asset.h"
#include "layerx/lxp_module.h"
#include "layerx/lxp_receipt.h"
#include "layerx/lxp_transfer.h"

#include <stddef.h>
#include <stdint.h>

enum {
    LX_ESCROW_STORE_CAPACITY = 128,
    LX_ESCROW_IDEMPOTENCY_CAPACITY = 16,
    LX_ESCROW_OPEN = 0x00020001,
    LX_ESCROW_CAPTURE = 0x00020002,
    LX_ESCROW_PARTIAL_CAPTURE = 0x00020003,
    LX_ESCROW_RELEASE = 0x00020004,
    LX_ESCROW_TIMEOUT = 0x00020005,
    LX_ESCROW_DISPUTE_OPEN = 0x00020006,
    LX_ESCROW_DISPUTE_RESOLVE = 0x00020007
};

typedef enum lx_escrow_status {
    LX_ESCROW_STATE_OPEN = 1,
    LX_ESCROW_STATE_PARTIALLY_CAPTURED,
    LX_ESCROW_STATE_CAPTURED,
    LX_ESCROW_STATE_RELEASED,
    LX_ESCROW_STATE_DISPUTED,
    LX_ESCROW_STATE_RESOLVED,
    LX_ESCROW_STATE_TIMED_OUT
} lx_escrow_status;

typedef struct lx_escrow_record {
    uint8_t escrow_id[32];
    uint8_t owner[32];
    uint8_t escrow_account[32];
    uint8_t beneficiary[32];
    uint8_t arbiter[32];
    uint8_t asset_id[32];
    lxp_u128 locked_amount;
    lxp_u128 captured_amount;
    lx_escrow_status state;
    uint64_t expiry;
    uint64_t dispute_window;
    uint8_t terms_hash[32];
    uint8_t agreement_reference[32];
} lx_escrow_record;

typedef struct lx_escrow_store {
    lx_escrow_record records[LX_ESCROW_STORE_CAPACITY];
    size_t count;
    struct {
        uint8_t key[32];
        lxp_receipt receipt;
    } economic_results[LX_ESCROW_IDEMPOTENCY_CAPACITY];
    size_t economic_result_count;
} lx_escrow_store;

typedef struct lx_escrow_open_request {
    lx_escrow_store *store;
    lx_account *owner;
    lx_account *escrow_account;
    const lx_asset_record *asset;
    lxp_u128 amount;
    lxp_transfer_context context;
    lx_escrow_record record;
} lx_escrow_open_request;

typedef struct lx_escrow_capture_request {
    lx_escrow_store *store;
    const uint8_t *escrow_id;
    lx_account *escrow_account;
    lx_account *beneficiary_account;
    lx_account *owner_account;
    const lx_asset_record *asset;
    lxp_u128 amount;
    const lxp_authority_resolved *authority;
    uint8_t idempotency_key[32];
    lxp_transfer_context context;
} lx_escrow_capture_request;

typedef struct lx_escrow_release_request {
    lx_escrow_store *store;
    const uint8_t *escrow_id;
    lx_account *escrow_account;
    lx_account *owner_account;
    const lx_asset_record *asset;
    const lxp_authority_resolved *authority;
    uint8_t idempotency_key[32];
    lxp_transfer_context context;
} lx_escrow_release_request;

typedef struct lx_escrow_runtime {
    lx_escrow_store *store;
    lx_account_registry *accounts;
    lx_asset_registry *assets;
} lx_escrow_runtime;

typedef struct lx_escrow_dispute_request {
    lx_escrow_store *store;
    const uint8_t *escrow_id;
    lx_account *escrow_account;
    lx_account *beneficiary_account;
    lx_account *owner_account;
    const lx_asset_record *asset;
    const lxp_authority_resolved *authority;
    uint32_t beneficiary_basis_points;
    uint8_t idempotency_key[32];
    lxp_transfer_context context;
} lx_escrow_dispute_request;

const lxp_module_iface *lx_escrow_module_iface(void);
lxp_result lx_escrow_state_put(lx_escrow_store *store,
                               const lx_escrow_record *record);
lxp_result lx_escrow_lookup(lx_escrow_store *store,
                            const uint8_t escrow_id[32],
                            lx_escrow_record **record);
lxp_result lx_escrow_open_execute(lxp_module_ctx *ctx,
                                  const lx_escrow_open_request *request,
                                  lxp_receipt *receipt);
lxp_result lx_escrow_remaining(const lx_escrow_record *record,
                               const lx_account *escrow_account,
                               lxp_u128 *remaining);
lxp_result lx_escrow_capture_execute(lxp_module_ctx *ctx,
                                     const lx_escrow_capture_request *request,
                                     lxp_receipt *receipt);
lxp_result lx_escrow_partial_capture_execute(
    lxp_module_ctx *ctx, const lx_escrow_capture_request *request,
    lxp_receipt *receipt);
lxp_result lx_escrow_release_execute(lxp_module_ctx *ctx,
                                     const lx_escrow_release_request *request,
                                     lxp_receipt *receipt);
lxp_result lx_escrow_timeout_execute(lxp_module_ctx *ctx,
                                     const lx_escrow_release_request *request,
                                     lxp_receipt *receipt);
lxp_result lx_escrow_epoch_begin(lxp_module_ctx *ctx, uint64_t epoch,
                                 uint64_t timestamp);
lxp_result lx_escrow_receipt_replay(const lx_escrow_store *store,
                                    const uint8_t key[32],
                                    lxp_receipt *receipt, bool *found);
lxp_result lx_escrow_receipt_record(lx_escrow_store *store,
                                    const uint8_t key[32],
                                    const lxp_receipt *receipt);
lxp_result lx_escrow_dispute_open_execute(
    lxp_module_ctx *ctx, const lx_escrow_dispute_request *request);
lxp_result lx_escrow_split_bps(lxp_u128 balance,
                               uint32_t beneficiary_basis_points,
                               lxp_u128 *beneficiary, lxp_u128 *owner);
lxp_result lx_escrow_dispute_resolve_execute(
    lxp_module_ctx *ctx, const lx_escrow_dispute_request *request,
    lxp_receipt *receipt);
lxp_result lx_escrow_authority_check(const lx_account *account,
                                     lxp_authorization_kind authority_kind,
                                     uint16_t origin_module_id,
                                     uint16_t reason);
lxp_result lx_escrow_invariant_check(const lx_escrow_record *record,
                                     const lx_account *escrow_account);

#endif
