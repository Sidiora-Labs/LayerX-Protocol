#ifndef LAYERX_LXP_VERIFY_POOL_H
#define LAYERX_LXP_VERIFY_POOL_H

#include "layerx/lxp_result.h"

#include <stdbool.h>
#include <stddef.h>

typedef bool (*lxp_verify_job_fn)(const void *context);

typedef struct lxp_verify_pool {
    void *implementation;
} lxp_verify_pool;
#define lxp_verify_pool lxp_verify_pool

lxp_result lxp_verify_pool_create(lxp_verify_pool *pool, size_t worker_count,
                                  size_t capacity);
lxp_result lxp_verify_pool_submit(lxp_verify_pool *pool,
                                  lxp_verify_job_fn verify,
                                  const void *context);
lxp_result lxp_verify_pool_join(lxp_verify_pool *pool, bool *results,
                                size_t result_capacity,
                                size_t *result_count);

#endif
