#ifndef LAYERX_LXP_SEQUENCER_H
#define LAYERX_LXP_SEQUENCER_H

#include "layerx/lxp_admission.h"
#include "layerx/lxp_batch.h"

#include <stddef.h>
#include <stdint.h>

typedef lxp_result (*lxp_seq_persist_fn)(void *context, uint64_t watermark);

typedef struct lxp_seq_allocator {
    uint64_t next_sequence;
    lxp_seq_persist_fn persist;
    void *persist_context;
} lxp_seq_allocator;
#define lxp_seq_allocator lxp_seq_allocator

typedef struct lxp_admission_ticket {
    uint64_t admission_order;
    const lxp_activity *activity;
    lxp_admission_result result;
} lxp_admission_ticket;

typedef struct lxp_admission_queue {
    lxp_admission_ticket *entries;
    size_t capacity;
    size_t head;
    size_t count;
    uint64_t next_admission_order;
} lxp_admission_queue;
#define lxp_admission_queue lxp_admission_queue

typedef lxp_result (*lxp_sequencer_snapshot_load_fn)(
    void *context, uint64_t durable_head, uint64_t *snapshot_sequence,
    uint8_t state_root[32]);
typedef lxp_result (*lxp_sequencer_replay_record_fn)(
    void *context, const lxp_log_record_header *header, const uint8_t *body,
    uint8_t recomputed_root[32], uint8_t committed_root[32], bool *compare_root);
typedef lxp_result (*lxp_sequencer_projection_rebuild_fn)(
    void *context, lxp_log *log, uint64_t durable_head);

typedef struct lxp_sequencer_recovery_ops {
    lxp_sequencer_snapshot_load_fn load_snapshot;
    lxp_sequencer_replay_record_fn replay_record;
    lxp_sequencer_projection_rebuild_fn rebuild_projections;
} lxp_sequencer_recovery_ops;

typedef struct lxp_sequencer_recovery_result {
    uint64_t durable_head;
    uint64_t next_sequence;
    uint64_t snapshot_sequence;
    uint8_t resulting_state_root[32];
    bool halted;
} lxp_sequencer_recovery_result;

enum { LXP_MAX_SEALED_BATCH_HEADERS = 256 };
typedef struct lxp_sealed_header_record {
    lxp_batch_header header;
    uint8_t header_hash[32];
    uint8_t signature[64];
} lxp_sealed_header_record;

typedef struct lxp_sequencer_equivocation_evidence {
    lxp_sealed_header_record first;
    lxp_sealed_header_record second;
} lxp_sequencer_equivocation_evidence;

typedef lxp_result (*lxp_equivocation_publish_fn)(
    void *context, const lxp_sequencer_equivocation_evidence *evidence);

typedef struct lxp_sequencer_header_registry {
    lxp_sealed_header_record records[LXP_MAX_SEALED_BATCH_HEADERS];
    size_t count;
    bool checkpoint_halted;
    lxp_equivocation_publish_fn publish_evidence;
    void *publish_context;
} lxp_sequencer_header_registry;

typedef struct lxp_sequencer_liveness {
    bool accepting_activities;
    bool handover_required;
    uint8_t authorised_sequencer_id[32];
    uint64_t first_authorised_batch;
} lxp_sequencer_liveness;

lxp_result lxp_seq_allocator_init(lxp_seq_allocator *allocator,
                                  uint64_t next_sequence,
                                  lxp_seq_persist_fn persist,
                                  void *persist_context);
lxp_result lxp_seq_assign(lxp_seq_allocator *allocator,
                          lxp_admission_result admission,
                          uint64_t presented_account_sequence,
                          uint64_t *next_account_sequence,
                          uint64_t *global_sequence);
lxp_result lxp_admission_queue_init(lxp_admission_queue *queue,
                                    lxp_admission_ticket *storage,
                                    size_t capacity,
                                    uint64_t next_admission_order);
lxp_result lxp_admission_queue_push(lxp_admission_queue *queue,
                                    const lxp_activity *activity,
                                    lxp_admission_result result,
                                    uint64_t *admission_order);
lxp_result lxp_admission_queue_pop(lxp_admission_queue *queue,
                                   lxp_admission_ticket *ticket);
lxp_result lxp_batch_range_check(const lxp_batch_header *previous,
                                 const lxp_batch_header *candidate);
lxp_result lxp_sequencer_recover(
    lxp_log *log, const lxp_sequencer_recovery_ops *operations,
    void *context, lxp_sequencer_recovery_result *result);
lxp_result lxp_sequencer_header_registry_init(
    lxp_sequencer_header_registry *registry,
    lxp_equivocation_publish_fn publish_evidence, void *publish_context);
lxp_result lxp_sequencer_equivocation_detect(
    lxp_sequencer_header_registry *registry,
    const lxp_batch_header *header, const uint8_t signature[64],
    lxp_arena *arena, lxp_sequencer_equivocation_evidence *evidence);
lxp_result lxp_sequencer_loss(lxp_sequencer_liveness *liveness);
lxp_result lxp_sequencer_handover_authorize(
    lxp_sequencer_liveness *liveness, const uint8_t sequencer_id[32],
    uint64_t first_batch_number);
lxp_result lxp_sequencer_can_seal(const lxp_sequencer_liveness *liveness,
                                  const lxp_batch_header *header);

#endif
