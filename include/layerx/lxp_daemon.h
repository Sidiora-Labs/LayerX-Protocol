#ifndef LAYERX_LXP_DAEMON_H
#define LAYERX_LXP_DAEMON_H

#include "layerx/lxp_result.h"

#include <pthread.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
    LXP_DAEMON_MAX_WORKERS = 16,
    LXP_DAEMON_QUEUE_CAPACITY = 4096,
    LXP_DAEMON_ACTIVITY_BYTES = 256
};

typedef enum lxp_daemon_role_kind {
    LXP_DAEMON_SEQUENCER = 1,
    LXP_DAEMON_REPLICA = 2,
    LXP_DAEMON_GUARANTOR = 3
} lxp_daemon_role_kind;

typedef struct lxp_daemon_configuration {
    lxp_daemon_role_kind role;
    uint32_t network_id;
    uint64_t start_sequence;
    size_t verify_workers;
    size_t network_workers;
    size_t projection_workers;
    size_t checkpoint_workers;
    bool serial_execution;
} lxp_daemon_configuration;

typedef struct lxp_daemon_activity {
    uint8_t bytes[LXP_DAEMON_ACTIVITY_BYTES];
    size_t length;
} lxp_daemon_activity;

typedef lxp_result (*lxp_daemon_apply_fn)(
    void *context, uint64_t global_sequence,
    const uint8_t *activity, size_t activity_length);

typedef struct lxp_daemon {
    lxp_daemon_configuration config;
    lxp_daemon_apply_fn apply;
    void *apply_context;
    pthread_t executor_thread;
    pthread_t workers[LXP_DAEMON_MAX_WORKERS * 4U];
    size_t worker_count;
    pthread_mutex_t mutex;
    pthread_cond_t queue_changed;
    lxp_daemon_activity queue[LXP_DAEMON_QUEUE_CAPACITY];
    size_t queue_head;
    size_t queue_count;
    uint64_t next_sequence;
    lxp_result failure;
    bool accepting;
    bool stop_requested;
    bool executor_started;
    bool primitives_initialized;
} lxp_daemon;

lxp_result lxp_daemon_config_load(
    const char *path, lxp_daemon_configuration *config);
lxp_result lxp_daemon_config(
    const char *path, lxp_daemon_configuration *config);
lxp_result lxp_daemon_role(
    const lxp_daemon_configuration *config,
    lxp_daemon_role_kind *role);
lxp_result lxp_daemon_start(
    lxp_daemon *daemon, const lxp_daemon_configuration *config,
    lxp_daemon_apply_fn apply, void *apply_context);
lxp_result lxp_daemon_submit(
    lxp_daemon *daemon, const uint8_t *activity, size_t activity_length);
lxp_result lxp_daemon_shutdown(lxp_daemon *daemon);
lxp_result lxp_daemon_main(int argc, char **argv);

#endif
