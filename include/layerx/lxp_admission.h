#ifndef LAYERX_LXP_ADMISSION_H
#define LAYERX_LXP_ADMISSION_H

#include "layerx/lxp_activity.h"

#include <stdbool.h>
#include <stdint.h>

typedef struct lxp_admission_result {
    lxp_result result_code;
    bool assign_global_sequence;
    bool consume_account_sequence;
    bool charge_fee;
} lxp_admission_result;
#define lxp_admission_result lxp_admission_result

typedef struct lxp_admission_context {
    uint32_t network_id;
    uint64_t batch_timestamp;
    uint64_t maximum_timestamp_window;
    uint64_t next_account_sequence;
    bool signature_valid;
    bool idempotency_key_exists;
    bool fee_limit_spendable;
} lxp_admission_context;

lxp_result lxp_activity_check_timestamp_bound(lxp_timestamp_bound bound,
                                              uint64_t batch_timestamp,
                                              uint64_t maximum_window);
/*
 * Normative admission order: envelope, timestamp, signature, account sequence,
 * idempotency key, then fee-limit spendability. Every rejection is pre-order.
 */
lxp_admission_result lxp_admit_activity(const lxp_activity *activity,
                                        const lxp_admission_context *context);

#endif
