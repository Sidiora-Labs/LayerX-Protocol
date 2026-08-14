#include "layerx/lx_service.h"
#include "layerx/lxp_hash.h"
#include "layerx/lxp_kernel.h"

#include <openssl/evp.h>
#include <string.h>

static int sign_execution(lx_service_execution *execution,
                          lx_service_attestor_grant *grant,
                          const uint8_t seed[32])
{
    uint8_t message[384];
    uint8_t digest[32];
    size_t message_length;
    size_t public_length = 32U;
    size_t signature_length = 64U;
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL,
                                                  seed, 32U);
    EVP_MD_CTX *context = EVP_MD_CTX_new();
    int failed = key == NULL || context == NULL ||
        EVP_PKEY_get_raw_public_key(key, execution->public_key,
                                    &public_length) != 1;
    if (!failed) {
        (void)memcpy(grant->public_key, execution->public_key, 32U);
        failed = lx_service_attestation_bytes(execution, message,
                                               sizeof(message),
                                               &message_length) != LXP_OK ||
            lxp_hash_domain(LXP_DOMAIN_SIGNATURE_PREIMAGE, message,
                            message_length, digest) != LXP_OK ||
            EVP_DigestSignInit(context, NULL, NULL, NULL, key) != 1 ||
            EVP_DigestSign(context, execution->signature, &signature_length,
                           digest, sizeof(digest)) != 1 ||
            signature_length != 64U;
    }
    EVP_MD_CTX_free(context);
    EVP_PKEY_free(key);
    return failed;
}

int main(void)
{
    static const uint8_t seed[32] = { 7U };
    lx_service_store store;
    lx_service_attestor_grant grant;
    lx_service_attest_request request;
    lx_service_execution accepted;
    lx_service_execution decoded;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint8_t encoded[416];
    uint8_t reencoded[416];
    size_t encoded_length;
    size_t reencoded_length;
    uint64_t parameters = 1U;

    (void)memset(&store, 0, sizeof(store));
    (void)memset(&grant, 0, sizeof(grant));
    (void)memset(&request, 0, sizeof(request));
    store.commitment_count = 1U;
    store.commitments[0].commitment_id[0] = 1U;
    store.commitments[0].agreement_id[0] = 2U;
    store.commitments[0].provider[0] = 3U;
    grant.principal[0] = 3U;
    grant.module_id = LXP_MODULE_SERVICE;
    grant.activity_type = LX_SERVICE_TOOL_EXEC_ATTEST;
    grant.not_before = 50U;
    grant.not_after = 150U;
    request.store = &store;
    request.grant = &grant;
    request.execution.attestation_id[0] = 4U;
    request.execution.activity_id[0] = 5U;
    request.execution.agreement_id[0] = 2U;
    request.execution.commitment_id[0] = 1U;
    request.execution.tool_id[0] = 6U;
    request.execution.input_commitment_hash[0] = 7U;
    request.execution.output_commitment_hash[0] = 8U;
    request.execution.execution_start = 60U;
    request.execution.execution_end = 90U;
    request.execution.resource_units = 1000U;
    request.execution.attestor_identity[0] = 3U;
    request.execution.availability_reference[0] = 9U;
    if (sign_execution(&request.execution, &grant, seed) != 0 ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_service_module_iface()) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_SERVICE, 100U, 0U, 77U,
                            1000U, &arena, true) != LXP_OK)
        return 1;

    grant.revoked = true;
    if (lx_service_tool_exec_attest_execute(&ctx, &request, &accepted) !=
            LXP_ERR_INVALID_ATTESTATION) return 1;
    grant.revoked = false;
    request.execution.output_commitment_hash[0] = 0U;
    if (lx_service_tool_exec_attest_execute(&ctx, &request, &accepted) !=
            LXP_ERR_INVALID_ATTESTATION) return 1;
    request.execution.output_commitment_hash[0] = 8U;
    grant.not_after = 99U;
    if (lx_service_tool_exec_attest_execute(&ctx, &request, &accepted) !=
            LXP_ERR_INVALID_ATTESTATION) return 1;
    grant.not_after = 150U;
    request.execution.signature[0] ^= 1U;
    if (lx_service_tool_exec_attest_execute(&ctx, &request, &accepted) !=
            LXP_ERR_INVALID_ATTESTATION) return 1;
    request.execution.signature[0] ^= 1U;
    if (lx_service_tool_exec_attest_execute(&ctx, &request, &accepted) !=
            LXP_OK || accepted.global_sequence != 77U ||
        accepted.canonical_payload_length == 0U || store.execution_count != 1U ||
        lx_service_tool_exec_attest_execute(&ctx, &request, &decoded) !=
            LXP_ERR_SEQUENCE_REUSED)
        return 1;
    if (lx_service_execution_encode(&accepted, encoded, sizeof(encoded),
                                    &encoded_length) != LXP_OK ||
        lx_service_execution_decode(encoded, encoded_length, &decoded) !=
            LXP_OK ||
        lx_service_execution_encode(&decoded, reencoded, sizeof(reencoded),
                                    &reencoded_length) != LXP_OK ||
        encoded_length != reencoded_length ||
        memcmp(encoded, reencoded, encoded_length) != 0 ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
