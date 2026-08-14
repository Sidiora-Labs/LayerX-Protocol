#ifndef LAYERX_LXP_TOOLS_H
#define LAYERX_LXP_TOOLS_H

#include "layerx/lxp_da.h"
#include "layerx/lxp_guarantor.h"
#include "layerx/lxp_replica.h"

#include <stddef.h>
#include <stdint.h>

enum {
    LXP_CTL_OUTPUT_BYTES = 50,
    LXP_VERIFY_OUTPUT_BYTES = 253,
    LXP_GENESIS_OUTPUT_BYTES = 42
};

typedef enum lxp_ctl_command {
    LXP_CTL_SUBMIT = 1,
    LXP_CTL_READ_STATE = 2
} lxp_ctl_command;

typedef lxp_result (*lxp_ctl_ordered_submit_fn)(
    void *context, const uint8_t *activity, size_t activity_length,
    uint64_t *global_sequence, uint8_t state_root[32]);
typedef lxp_result (*lxp_ctl_state_read_fn)(
    void *context, uint64_t *global_sequence, uint8_t state_root[32]);

typedef struct lxp_ctl_context {
    lxp_ctl_ordered_submit_fn ordered_submit;
    lxp_ctl_state_read_fn read_state;
    void *context;
} lxp_ctl_context;

typedef struct lxp_verify_run {
    const lxp_da_bundle *bundle;
    const lxp_batch_header *header;
    const lxp_guarantor_cert *certificate;
    const lxp_guarantor_key_record *guarantor_keys;
    size_t guarantor_key_count;
    lxp_replay_engine *engine;
    const uint8_t *starting_state_root;
    lxp_arena *arena;
} lxp_verify_run;

typedef enum lxp_genesis_cli_action {
    LXP_GENESIS_BUILD = 1,
    LXP_GENESIS_RECONCILE = 2
} lxp_genesis_cli_action;

typedef lxp_result (*lxp_genesis_cli_action_fn)(
    void *context, lxp_genesis_cli_action action,
    lxp_byte_span canonical_input, uint8_t manifest_root[32]);

lxp_result lxp_ctl_submit_activity(
    const lxp_ctl_context *context,
    const uint8_t *activity, size_t activity_length,
    uint8_t output[LXP_CTL_OUTPUT_BYTES]);
lxp_result lxp_ctl_main(
    lxp_ctl_command command, const lxp_ctl_context *context,
    const uint8_t *input, size_t input_length,
    uint8_t output[LXP_CTL_OUTPUT_BYTES]);
lxp_result lxp_verify_main(
    const lxp_verify_run *run,
    uint8_t output[LXP_VERIFY_OUTPUT_BYTES]);
lxp_result lxp_genesis_cli_main(
    lxp_genesis_cli_action action, lxp_byte_span canonical_input,
    lxp_genesis_cli_action_fn execute, void *context,
    uint8_t output[LXP_GENESIS_OUTPUT_BYTES]);

#endif
