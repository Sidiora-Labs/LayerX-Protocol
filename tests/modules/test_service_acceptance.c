#include "layerx/lx_service.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static int replay_default(bool delayed,
                          lx_service_agreement result[2])
{
    lx_service_store store;
    lx_service_runtime runtime;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    volatile uint64_t elapsed = 0U;
    uint64_t i;

    (void)memset(&store, 0, sizeof(store));
    store.agreement_count = 2U;
    store.agreements[0].agreement_id[0] = 1U;
    store.agreements[0].state = LX_SERVICE_AGREEMENT_DELIVERED;
    store.agreements[0].default_outcome = LX_SERVICE_DEFAULT_ACCEPT;
    store.agreements[0].acceptance_window_end = 400U;
    store.agreements[1].agreement_id[0] = 2U;
    store.agreements[1].state = LX_SERVICE_AGREEMENT_DELIVERED;
    store.agreements[1].default_outcome = LX_SERVICE_DEFAULT_REJECT;
    store.agreements[1].acceptance_window_end = 400U;
    runtime.store = &store;
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_service_module_iface()) != LXP_OK ||
        lxp_kernel_bind_module_runtime(&kernel, LXP_MODULE_SERVICE,
                                       &runtime) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 500U, 3U, 90U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    if (delayed)
        for (i = 0U; i < UINT64_C(1000000); ++i) elapsed += i;
    if (lx_service_module_iface()->epoch_begin(&ctx, 3U, 500U) != LXP_OK ||
        store.agreements[0].state != LX_SERVICE_AGREEMENT_ACCEPTED ||
        store.agreements[1].state != LX_SERVICE_AGREEMENT_REJECTED ||
        !store.agreements[0].default_applied ||
        !store.agreements[1].default_applied ||
        store.agreements[0].outcome_sequence != 90U ||
        store.agreements[0].outcome_timestamp != 500U)
        return 1;
    result[0] = store.agreements[0];
    result[1] = store.agreements[1];
    (void)elapsed;
    return lxp_state_store_destroy(&state) == LXP_OK ? 0 : 1;
}

static int explicit_outcomes(void)
{
    lx_service_store store;
    lx_service_outcome_request request;
    lxp_authority_resolved buyer;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;

    (void)memset(&store, 0, sizeof(store));
    (void)memset(&buyer, 0, sizeof(buyer));
    buyer.principal[0] = 10U;
    store.agreement_count = 2U;
    store.agreements[0].agreement_id[0] = 11U;
    (void)memcpy(store.agreements[0].buyer, buyer.principal, 32U);
    store.agreements[0].state = LX_SERVICE_AGREEMENT_DELIVERED;
    store.agreements[0].acceptance_window_end = 1000U;
    store.agreements[1] = store.agreements[0];
    store.agreements[1].agreement_id[0] = 12U;
    store.delivery_count = 1U;
    (void)memcpy(store.deliveries[0].agreement_id,
                 store.agreements[1].agreement_id, 32U);
    store.deliveries[0].deliverable_count = 1U;
    store.deliveries[0].deliverables[0].hash[0] = 13U;
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_service_module_iface()) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 500U, 0U, 91U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    (void)memset(&request, 0, sizeof(request));
    request.store = &store;
    request.authority = &buyer;
    request.agreement_id = store.agreements[0].agreement_id;
    if (lx_service_accept_execute(&ctx, &request) != LXP_OK ||
        store.agreements[0].state != LX_SERVICE_AGREEMENT_ACCEPTED ||
        store.agreements[0].outcome_sequence != 91U)
        return 1;
    request.agreement_id = store.agreements[1].agreement_id;
    request.rejection_reason = 7U;
    request.contested_hash_count = 1U;
    request.contested_hashes[0][0] = 13U;
    if (lx_service_reject_execute(&ctx, &request) != LXP_OK ||
        store.agreements[1].state != LX_SERVICE_AGREEMENT_REJECTED ||
        store.agreements[1].rejection_reason != 7U ||
        store.agreements[1].contested_hash_count != 1U ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}

int main(void)
{
    lx_service_agreement immediate[2];
    lx_service_agreement delayed[2];
    if (replay_default(false, immediate) != 0 ||
        replay_default(true, delayed) != 0 ||
        memcmp(immediate, delayed, sizeof(immediate)) != 0 ||
        explicit_outcomes() != 0)
        return 1;
    return 0;
}
