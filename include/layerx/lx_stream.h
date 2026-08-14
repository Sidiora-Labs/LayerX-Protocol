#ifndef LAYERX_LX_STREAM_H
#define LAYERX_LX_STREAM_H

#include "layerx/lx_asset.h"
#include "layerx/lxp_module.h"
#include "layerx/lxp_receipt.h"
#include "layerx/lxp_transfer.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
    LX_STREAM_STORE_CAPACITY = 128,
    LX_STREAM_MAX_METER_AUTHORITIES = 8,
    LX_STREAM_IDEMPOTENCY_CAPACITY = 16,
    LX_STREAM_OPEN = 0x00040001,
    LX_STREAM_TOP_UP = 0x00040002,
    LX_STREAM_METER = 0x00040003,
    LX_STREAM_SETTLE = 0x00040004,
    LX_STREAM_PAUSE = 0x00040005,
    LX_STREAM_RESUME = 0x00040006,
    LX_STREAM_CLOSE = 0x00040007
};

typedef enum lx_stream_mode {
    LX_STREAM_MODE_TIME = 1,
    LX_STREAM_MODE_METERED = 2
} lx_stream_mode;

typedef struct lx_stream_record {
    uint8_t stream_id[32];
    uint8_t payer[32];
    uint8_t stream_account[32];
    uint8_t recipient[32];
    uint8_t asset_id[32];
    lx_stream_mode mode;
    lxp_u128 rate;
    uint64_t rate_unit;
    uint64_t start_timestamp;
    uint64_t last_accrual_timestamp;
    uint64_t end_timestamp;
    lxp_u128 total_cap;
    lxp_u128 accrued_total;
    lxp_u128 settled_total;
    lxp_u128 remainder_carry;
    uint64_t cumulative_meter;
    uint8_t meter_authorities[LX_STREAM_MAX_METER_AUTHORITIES][32];
    size_t meter_authority_count;
    bool underfunded;
    bool paused;
    bool closed;
} lx_stream_record;

typedef struct lx_stream_store {
    lx_stream_record records[LX_STREAM_STORE_CAPACITY];
    size_t count;
    struct {
        uint8_t key[32];
        lxp_receipt receipt;
    } economic_results[LX_STREAM_IDEMPOTENCY_CAPACITY];
    size_t economic_result_count;
} lx_stream_store;

typedef struct lx_stream_fund_request {
    lx_stream_store *store;
    lx_account *payer;
    lx_account *stream_account;
    const lx_asset_record *asset;
    lxp_u128 amount;
    lxp_transfer_context context;
    lx_stream_record record;
} lx_stream_fund_request;

typedef struct lx_stream_meter_attestation {
    uint8_t stream_id[32];
    uint64_t cumulative_reading;
    uint8_t authority_key[32];
    uint8_t signature[64];
} lx_stream_meter_attestation;

typedef struct lx_stream_settle_request {
    lx_stream_store *store;
    const uint8_t *stream_id;
    lx_account *stream_account;
    lx_account *recipient;
    const lx_asset_record *asset;
    uint8_t idempotency_key[32];
    lxp_transfer_context context;
} lx_stream_settle_request;

typedef struct lx_stream_lifecycle_request {
    lx_stream_store *store;
    const uint8_t *stream_id;
    lx_account *stream_account;
    lx_account *payer;
    lx_account *recipient;
    const lx_asset_record *asset;
    const lxp_authority_resolved *authority;
    uint8_t idempotency_key[32];
    lxp_transfer_context context;
} lx_stream_lifecycle_request;

const lxp_module_iface *lx_stream_module_iface(void);
lxp_result lx_stream_lookup(lx_stream_store *store,
                            const uint8_t stream_id[32],
                            lx_stream_record **record);
lxp_result lx_stream_state_put(lx_stream_store *store,
                               const lx_stream_record *record);
lxp_result lx_stream_open_execute(lxp_module_ctx *ctx,
                                  const lx_stream_fund_request *request,
                                  lxp_receipt *receipt);
lxp_result lx_stream_top_up_execute(lxp_module_ctx *ctx,
                                    const lx_stream_fund_request *request,
                                    lxp_receipt *receipt);
lxp_result lx_stream_elapsed_ms(const lx_stream_record *record,
                                uint64_t batch_timestamp,
                                uint64_t *elapsed_ms);
lxp_result lx_stream_carry_apply(lx_stream_record *record,
                                 lxp_u128 remainder);
lxp_result lx_stream_accrue(lx_stream_record *record,
                            uint64_t batch_timestamp,
                            lxp_u128 *newly_accrued);
lxp_result lx_stream_meter_attestation_bytes(
    const lx_stream_meter_attestation *attestation,
    uint8_t *bytes, size_t capacity, size_t *length);
lxp_result lx_stream_meter_authority_check(
    const lx_stream_record *record,
    const lx_stream_meter_attestation *attestation);
lxp_result lx_stream_metered_accrue(lx_stream_record *record,
                                    uint64_t cumulative_reading,
                                    lxp_u128 *newly_accrued);
lxp_result lx_stream_meter_execute(
    lx_stream_record *record,
    const lx_stream_meter_attestation *attestation,
    lxp_u128 *newly_accrued);
lxp_result lx_stream_settle_amount(const lx_stream_record *record,
                                   lxp_u128 *amount);
lxp_result lx_stream_mark_underfunded(lx_stream_record *record,
                                      uint64_t batch_timestamp,
                                      lxp_u128 settled_amount);
lxp_result lx_stream_settle_execute(lxp_module_ctx *ctx,
                                    const lx_stream_settle_request *request,
                                    lxp_receipt *receipt);
lxp_result lx_stream_receipt_replay(const lx_stream_store *store,
                                    const uint8_t key[32],
                                    lxp_receipt *receipt, bool *found);
lxp_result lx_stream_receipt_record(lx_stream_store *store,
                                    const uint8_t key[32],
                                    const lxp_receipt *receipt);
lxp_result lx_stream_pause_execute(
    lxp_module_ctx *ctx, const lx_stream_lifecycle_request *request);
lxp_result lx_stream_resume_execute(
    lxp_module_ctx *ctx, const lx_stream_lifecycle_request *request);
lxp_result lx_stream_close_execute(
    lxp_module_ctx *ctx, const lx_stream_lifecycle_request *request,
    lxp_receipt *receipt);
lxp_result lx_stream_authority_check(const lx_account *account,
                                     lxp_authorization_kind authority_kind,
                                     uint16_t origin_module_id,
                                     uint16_t reason);

#endif
