#ifndef LAYERX_LXP_QUALIFICATION_H
#define LAYERX_LXP_QUALIFICATION_H

#include "layerx/lxp_result.h"
#include "layerx/lxp_fault.h"
#include "layerx/lxp_u128.h"

#include <stdint.h>

enum {
    LXP_QUAL_ACTIVITY_TYPE_COUNT = 53,
    LXP_QUAL_BATCH_HEADER_BYTES = 72,
    LXP_QUAL_RECEIPT_BYTES = 106,
    LXP_QUAL_EVENT_BYTES = 36
};

#define LXP_QUAL_MIN_ACTIVITY_COUNT UINT64_C(10000000)

typedef struct lxp_qual_replay_result {
    uint64_t activity_count;
    uint64_t batch_count;
    uint64_t first_divergent_sequence;
    uint8_t activity_digest[32];
    uint8_t receipt_digest[32];
    uint8_t event_digest[32];
    uint8_t batch_digest[32];
    uint8_t root_ledger_digest[32];
    uint8_t terminal_root[32];
    uint8_t corpus_digest[32];
} lxp_qual_replay_result;

lxp_result lxp_qual_corpus_generate(const char *corpus_path,
                                    const char *root_ledger_path,
                                    uint64_t activity_count,
                                    uint32_t batch_size);
lxp_result lxp_qual_root_ledger(const char *root_ledger_path,
                               uint64_t expected_batch_count,
                               const uint8_t expected_corpus_digest[32],
                               const uint8_t expected_digest[32]);
lxp_result lxp_qual_replay_matrix(const char *corpus_path,
                                  const char *root_ledger_path,
                                  lxp_qual_replay_result *result);
lxp_result lxp_partition_sim(void);
lxp_result lxp_sequencer_loss_sim(void);
lxp_result lxp_qual_fault_boundaries(void);
lxp_result lxp_u128_proof_harness(uint64_t *case_count);
lxp_result lxp_u256_boundary_case(uint64_t *case_count);
lxp_result lxp_rounding_direction_check(uint64_t *case_count);

#endif
