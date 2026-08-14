#include "layerx/lxp_verify_pool.h"

#include <pthread.h>
#include <stdlib.h>
#include <string.h>

typedef struct verify_job {
    lxp_verify_job_fn verify;
    const void *context;
    size_t admission_index;
} verify_job;

typedef struct verify_pool_impl {
    pthread_t *workers;
    size_t worker_count;
    verify_job *queue;
    bool *results;
    size_t capacity;
    size_t submitted;
    size_t completed;
    size_t queue_head;
    size_t queue_count;
    int stop;
    pthread_mutex_t mutex;
    pthread_cond_t available;
    pthread_cond_t finished;
} verify_pool_impl;

static void *worker_main(void *opaque)
{
    verify_pool_impl *pool = (verify_pool_impl *)opaque;
    for (;;) {
        verify_job job;
        bool valid;
        (void)pthread_mutex_lock(&pool->mutex);
        while (pool->queue_count == 0U && pool->stop == 0)
            (void)pthread_cond_wait(&pool->available, &pool->mutex);
        if (pool->queue_count == 0U && pool->stop != 0) {
            (void)pthread_mutex_unlock(&pool->mutex);
            return NULL;
        }
        job = pool->queue[pool->queue_head];
        pool->queue_head = (pool->queue_head + 1U) % pool->capacity;
        --pool->queue_count;
        (void)pthread_mutex_unlock(&pool->mutex);
        valid = job.verify(job.context);
        (void)pthread_mutex_lock(&pool->mutex);
        pool->results[job.admission_index] = valid;
        ++pool->completed;
        (void)pthread_cond_broadcast(&pool->finished);
        (void)pthread_mutex_unlock(&pool->mutex);
    }
}

lxp_result lxp_verify_pool_create(lxp_verify_pool *pool, size_t worker_count,
                                  size_t capacity)
{
    verify_pool_impl *implementation;
    size_t i;
    if (pool == NULL || capacity == 0U) return LXP_ERR_NON_CANONICAL;
    implementation = calloc(1U, sizeof(*implementation));
    if (implementation == NULL) return LXP_ERR_IO;
    implementation->queue = calloc(capacity, sizeof(*implementation->queue));
    implementation->results = calloc(capacity, sizeof(*implementation->results));
    implementation->workers = worker_count == 0U ? NULL :
                              calloc(worker_count, sizeof(*implementation->workers));
    if (implementation->queue == NULL || implementation->results == NULL ||
        (worker_count != 0U && implementation->workers == NULL) ||
        pthread_mutex_init(&implementation->mutex, NULL) != 0 ||
        pthread_cond_init(&implementation->available, NULL) != 0 ||
        pthread_cond_init(&implementation->finished, NULL) != 0) {
        free(implementation->workers);
        free(implementation->results);
        free(implementation->queue);
        free(implementation);
        return LXP_ERR_IO;
    }
    implementation->worker_count = worker_count;
    implementation->capacity = capacity;
    for (i = 0U; i < worker_count; ++i) {
        if (pthread_create(&implementation->workers[i], NULL, worker_main,
                           implementation) != 0) {
            implementation->stop = 1;
            (void)pthread_cond_broadcast(&implementation->available);
            while (i-- > 0U) (void)pthread_join(implementation->workers[i], NULL);
            (void)pthread_cond_destroy(&implementation->finished);
            (void)pthread_cond_destroy(&implementation->available);
            (void)pthread_mutex_destroy(&implementation->mutex);
            free(implementation->workers);
            free(implementation->results);
            free(implementation->queue);
            free(implementation);
            return LXP_ERR_IO;
        }
    }
    pool->implementation = implementation;
    return LXP_OK;
}

lxp_result lxp_verify_pool_submit(lxp_verify_pool *pool,
                                  lxp_verify_job_fn verify,
                                  const void *context)
{
    verify_pool_impl *implementation;
    size_t tail;
    if (pool == NULL || pool->implementation == NULL || verify == NULL)
        return LXP_ERR_NON_CANONICAL;
    implementation = (verify_pool_impl *)pool->implementation;
    if (implementation->submitted == implementation->capacity)
        return LXP_ERR_LENGTH_LIMIT;
    if (implementation->worker_count == 0U) {
        implementation->results[implementation->submitted] = verify(context);
        ++implementation->submitted;
        ++implementation->completed;
        return LXP_OK;
    }
    if (pthread_mutex_lock(&implementation->mutex) != 0) return LXP_ERR_IO;
    tail = (implementation->queue_head + implementation->queue_count) %
           implementation->capacity;
    implementation->queue[tail] = (verify_job){ verify, context,
                                                implementation->submitted };
    ++implementation->submitted;
    ++implementation->queue_count;
    (void)pthread_cond_signal(&implementation->available);
    (void)pthread_mutex_unlock(&implementation->mutex);
    return LXP_OK;
}

lxp_result lxp_verify_pool_join(lxp_verify_pool *pool, bool *results,
                                size_t result_capacity,
                                size_t *result_count)
{
    verify_pool_impl *implementation;
    size_t count;
    size_t i;
    if (pool == NULL || pool->implementation == NULL || results == NULL ||
        result_count == NULL) return LXP_ERR_NON_CANONICAL;
    implementation = (verify_pool_impl *)pool->implementation;
    if (result_capacity < implementation->submitted) return LXP_ERR_LENGTH_LIMIT;
    if (implementation->worker_count != 0U) {
        if (pthread_mutex_lock(&implementation->mutex) != 0) return LXP_ERR_IO;
        while (implementation->completed != implementation->submitted)
            (void)pthread_cond_wait(&implementation->finished,
                                    &implementation->mutex);
        implementation->stop = 1;
        (void)pthread_cond_broadcast(&implementation->available);
        (void)pthread_mutex_unlock(&implementation->mutex);
        for (i = 0U; i < implementation->worker_count; ++i)
            if (pthread_join(implementation->workers[i], NULL) != 0)
                return LXP_ERR_IO;
    }
    count = implementation->submitted;
    (void)memcpy(results, implementation->results, count * sizeof(*results));
    *result_count = count;
    (void)pthread_cond_destroy(&implementation->finished);
    (void)pthread_cond_destroy(&implementation->available);
    (void)pthread_mutex_destroy(&implementation->mutex);
    free(implementation->workers);
    free(implementation->results);
    free(implementation->queue);
    free(implementation);
    pool->implementation = NULL;
    return LXP_OK;
}
