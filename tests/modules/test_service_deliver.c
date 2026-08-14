#include "layerx/lx_service.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static void request_init(lx_service_delivery_request *request,
                         lx_service_store *store,
                         const lxp_authority_resolved *provider,
                         const lx_service_agreement *agreement)
{
    (void)memset(request, 0, sizeof(*request));
    request->store = store;
    request->authority = provider;
    request->delivery.delivery_id[0] = 10U;
    request->delivery.activity_id[0] = 11U;
    (void)memcpy(request->delivery.agreement_id,
                 agreement->agreement_id, 32U);
    (void)memcpy(request->delivery.provider, provider->principal, 32U);
    request->delivery.deliverable_count = 2U;
    request->delivery.deliverables[0].hash[0] = 9U;
    request->delivery.deliverables[0].artifact_size = 100U;
    request->delivery.deliverables[0].availability_reference[0] = 2U;
    request->delivery.deliverables[1].hash[0] = 9U;
    request->delivery.deliverables[1].artifact_size = 200U;
    request->delivery.deliverables[1].availability_reference[0] = 1U;
}

int main(void)
{
    lx_service_store store;
    lx_service_agreement *agreement;
    lx_service_delivery_request request;
    lx_service_delivery result;
    lxp_authority_resolved provider;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;

    (void)memset(&store, 0, sizeof(store));
    (void)memset(&provider, 0, sizeof(provider));
    provider.principal[0] = 1U;
    store.offer_count = 1U;
    store.offers[0].offer_id[0] = 2U;
    store.offers[0].deliverable_specification_hash[0] = 9U;
    store.agreement_count = 1U;
    agreement = &store.agreements[0];
    agreement->agreement_id[0] = 3U;
    agreement->offer_id[0] = 2U;
    (void)memcpy(agreement->provider, provider.principal, 32U);
    agreement->state = LX_SERVICE_AGREEMENT_COMMITTED;
    agreement->delivery_deadline = 1000U;
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_service_module_iface()) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 500U, 0U, 80U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    request_init(&request, &store, &provider, agreement);
    request.delivery.deliverables[0].hash[0] = 8U;
    if (lx_service_deliver_execute(&ctx, &request, &result) !=
            LXP_ERR_DELIVERABLE_MISMATCH || store.delivery_count != 0U)
        return 1;
    request.delivery.deliverables[0].hash[0] = 9U;
    request.delivery.deliverables[0].availability_reference[0] = 0U;
    if (lx_service_deliver_execute(&ctx, &request, &result) !=
            LXP_ERR_DA_MISSING || store.delivery_count != 0U)
        return 1;
    request.delivery.deliverables[0].availability_reference[0] = 2U;
    if (lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 1001U, 0U, 81U,
                            1000U, &arena, true) != LXP_OK ||
        lx_service_deliver_execute(&ctx, &request, &result) !=
            LXP_ERR_DELIVERY_DEADLINE_PASSED || store.delivery_count != 0U ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 500U, 0U, 82U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    if (lx_service_deliver_execute(&ctx, &request, &result) != LXP_OK ||
        result.global_sequence != 82U || store.delivery_count != 1U ||
        result.deliverables[0].availability_reference[0] != 1U ||
        result.deliverables[1].availability_reference[0] != 2U ||
        agreement->state != LX_SERVICE_AGREEMENT_DELIVERED)
        return 1;
    agreement->state = LX_SERVICE_AGREEMENT_REJECTED;
    request.delivery.delivery_id[0] = 12U;
    request.delivery.activity_id[0] = 13U;
    if (lx_service_deliver_execute(&ctx, &request, &result) != LXP_OK ||
        store.delivery_count != 2U ||
        agreement->state != LX_SERVICE_AGREEMENT_DELIVERED ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
