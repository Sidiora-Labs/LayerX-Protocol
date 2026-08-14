#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_fuzz.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_protocol.h"
#include "layerx/lxp_qualification.h"

#include <errno.h>
#include <openssl/evp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct activity_seed {
    uint8_t *bytes;
    size_t length;
} activity_seed;

static uint64_t random_next(uint64_t *state)
{
    *state ^= *state << 13U;
    *state ^= *state >> 7U;
    *state ^= *state << 17U;
    return *state;
}

static lxp_result load_seed(const char *directory, size_t index,
                            activity_seed *seed)
{
    char path[4096];
    FILE *file;
    long length;
    int path_length;
    if (directory == NULL || seed == NULL) return LXP_ERR_NON_CANONICAL;
    path_length = snprintf(path, sizeof(path), "%s/activity-%02zu.bin",
                           directory, index);
    if (path_length < 0 || (size_t)path_length >= sizeof(path))
        return LXP_ERR_LENGTH_LIMIT;
    file = fopen(path, "rb");
    if (file == NULL || fseek(file, 0L, SEEK_END) != 0) {
        if (file != NULL) (void)fclose(file);
        return LXP_ERR_IO;
    }
    length = ftell(file);
    if (length <= 0L || (uint64_t)length > LXP_MAX_ACTIVITY_BYTES ||
        fseek(file, 0L, SEEK_SET) != 0) {
        (void)fclose(file);
        return LXP_ERR_NON_CANONICAL;
    }
    seed->bytes = malloc((size_t)length);
    seed->length = (size_t)length;
    if (seed->bytes == NULL ||
        fread(seed->bytes, 1U, seed->length, file) != seed->length ||
        fclose(file) != 0) {
        free(seed->bytes);
        seed->bytes = NULL;
        seed->length = 0U;
        return LXP_ERR_IO;
    }
    return LXP_OK;
}

static lxp_result sign_activity(lxp_activity *activity,
                                uint8_t public_key[32],
                                uint8_t signature[64])
{
    static const uint8_t private_bytes[32] = {
        0x9dU,0x61U,0xb1U,0x9dU,0xefU,0xfdU,0x5aU,0x60U,
        0xbaU,0x84U,0x4aU,0xf4U,0x92U,0xecU,0x2cU,0xc4U,
        0x44U,0x49U,0xc5U,0x69U,0x7bU,0x32U,0x69U,0x19U,
        0x70U,0x3bU,0xacU,0x03U,0x1cU,0xaeU,0x7fU,0x60U
    };
    uint8_t preimage[32];
    size_t public_length = 32U;
    size_t signature_length = 64U;
    EVP_PKEY *key;
    EVP_MD_CTX *signing;
    lxp_result status;
    int signed_ok;
    key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL,
                                       private_bytes, sizeof(private_bytes));
    signing = key == NULL ? NULL : EVP_MD_CTX_new();
    signed_ok = signing != NULL &&
        EVP_PKEY_get_raw_public_key(key, public_key, &public_length) == 1 &&
        public_length == 32U;
    status = signed_ok ? lxp_activity_signing_preimage(activity, preimage) :
             LXP_ERR_BAD_SIGNATURE;
    signed_ok = status == LXP_OK &&
        EVP_DigestSignInit(signing, NULL, NULL, NULL, key) == 1 &&
        EVP_DigestSign(signing, signature, &signature_length, preimage,
                       sizeof(preimage)) == 1 && signature_length == 64U;
    EVP_MD_CTX_free(signing);
    EVP_PKEY_free(key);
    lxp_secure_zero(preimage, sizeof(preimage));
    return signed_ok ? LXP_OK : status != LXP_OK ? status :
           LXP_ERR_BAD_SIGNATURE;
}

static lxp_result signature_suite(void)
{
    uint8_t public_key[32];
    uint8_t signature[64];
    uint8_t payload[] = { 0x10U, 0x20U, 0x30U, 0x40U };
    uint8_t actor[] = "did:lxp:fuzz-signer";
    lxp_activity activity;
    lxp_result status;
    (void)memset(&activity, 0, sizeof(activity));
    activity.protocol_version = LXP_PROTOCOL_VERSION;
    activity.network_id = 77U;
    activity.activity_type = UINT32_C(0x00010001);
    activity.actor_did = (lxp_byte_span){ actor, sizeof(actor) - 1U };
    activity.account_sequence = 1U;
    activity.timestamp_bound = (lxp_timestamp_bound){ 100U, 200U };
    activity.idempotency_key[0] = 0x51U;
    activity.fee_limit = (lxp_u128){ 0U, 100U };
    activity.payload = (lxp_byte_span){ payload, sizeof(payload) };
    status = lxp_hash_payload(payload, sizeof(payload), activity.payload_hash);
    activity.authority = (lxp_byte_span){ public_key, sizeof(public_key) };
    activity.signature = (lxp_byte_span){ signature, sizeof(signature) };
    if (status == LXP_OK) status = sign_activity(&activity, public_key,
                                                 signature);
    if (status == LXP_OK) status = lxp_activity_verify_signature(&activity);
    if (status == LXP_OK) status = lxp_fuzz_signature_mutate(&activity);
    lxp_secure_zero(signature, sizeof(signature));
    return status;
}

static lxp_result activity_exhaustive(const activity_seed *seeds,
                                      uint64_t *executions)
{
    uint8_t *mutated = malloc(LXP_MAX_ACTIVITY_BYTES + 32U);
    uint8_t *nested = malloc(LXP_MAX_ACTIVITY_BYTES);
    size_t seed_index;
    lxp_result status = LXP_OK;
    if (mutated == NULL || nested == NULL) {
        free(mutated);
        free(nested);
        return LXP_ERR_IO;
    }
    for (seed_index = 0U; status == LXP_OK &&
         seed_index < LXP_QUAL_ACTIVITY_TYPE_COUNT; ++seed_index) {
        const activity_seed *seed = &seeds[seed_index];
        size_t length;
        size_t byte;
        lxp_result decoded;
        status = lxp_fuzz_activity_decode(seed->bytes, seed->length, &decoded);
        *executions += 1U;
        if (status == LXP_OK && decoded != LXP_OK)
            status = LXP_FATAL_REPLAY_DIVERGENCE;
        for (length = 0U; status == LXP_OK && length < seed->length; ++length) {
            status = lxp_fuzz_activity_decode(seed->bytes, length, &decoded);
            *executions += 1U;
            if (status == LXP_OK && decoded == LXP_OK)
                status = LXP_FATAL_REPLAY_DIVERGENCE;
        }
        for (byte = 0U; status == LXP_OK && byte < seed->length; ++byte) {
            (void)memcpy(mutated, seed->bytes, seed->length);
            mutated[byte] ^= (uint8_t)(1U << (byte & 7U));
            status = lxp_fuzz_activity_decode(mutated, seed->length, &decoded);
            *executions += 1U;
        }
        for (length = 1U; status == LXP_OK && length <= 32U; ++length) {
            (void)memcpy(mutated, seed->bytes, seed->length);
            (void)memset(mutated + seed->length, 0xa5, length);
            status = lxp_fuzz_activity_decode(mutated, seed->length + length,
                                              &decoded);
            *executions += 1U;
            if (status == LXP_OK && decoded == LXP_OK)
                status = LXP_FATAL_REPLAY_DIVERGENCE;
        }
    }
    (void)memset(nested, 0x10, LXP_MAX_ACTIVITY_BYTES);
    if (status == LXP_OK) {
        size_t length;
        for (length = 1U; length <= LXP_MAX_ACTIVITY_BYTES; length *= 2U) {
            lxp_result decoded;
            status = lxp_fuzz_activity_decode(nested, length, &decoded);
            *executions += 1U;
            if (status != LXP_OK) break;
            if (decoded == LXP_OK) {
                status = LXP_FATAL_REPLAY_DIVERGENCE;
                break;
            }
            if (length > LXP_MAX_ACTIVITY_BYTES / 2U) break;
        }
    }
    lxp_secure_zero(mutated, LXP_MAX_ACTIVITY_BYTES + 32U);
    lxp_secure_zero(nested, LXP_MAX_ACTIVITY_BYTES);
    free(mutated);
    free(nested);
    return status;
}

static lxp_result random_suite(const activity_seed *seeds, uint64_t iterations,
                               uint64_t *activity_executions,
                               uint64_t *transfer_executions)
{
    uint8_t *activity = malloc(LXP_MAX_ACTIVITY_BYTES + 32U);
    uint8_t transfer[2304];
    uint64_t random = UINT64_C(0x4c6179657258467a);
    uint64_t iteration;
    lxp_result status = LXP_OK;
    if (activity == NULL) return LXP_ERR_IO;
    for (iteration = 0U; status == LXP_OK && iteration < iterations;
         ++iteration) {
        size_t seed_index = (size_t)(random_next(&random) %
                                     LXP_QUAL_ACTIVITY_TYPE_COUNT);
        size_t length = seeds[seed_index].length;
        size_t mutations = 1U + (size_t)(random_next(&random) % 8U);
        size_t i;
        lxp_result decoded;
        (void)memcpy(activity, seeds[seed_index].bytes, length);
        for (i = 0U; i < mutations; ++i) {
            size_t offset = (size_t)(random_next(&random) % length);
            activity[offset] ^= (uint8_t)random_next(&random);
        }
        if ((random_next(&random) & 7U) == 0U)
            length = (size_t)(random_next(&random) % (length + 1U));
        else if ((random_next(&random) & 7U) == 1U) {
            size_t extra = 1U + (size_t)(random_next(&random) % 32U);
            (void)memset(activity + length, (int)(random & 0xffU), extra);
            length += extra;
        }
        status = lxp_fuzz_activity_decode(activity, length, &decoded);
        *activity_executions += 1U;
        if (status != LXP_OK) break;
        for (i = 0U; i < sizeof(transfer); ++i)
            transfer[i] = (uint8_t)random_next(&random);
        status = lxp_fuzz_transfer_set(transfer,
                 1U + (size_t)(random_next(&random) % sizeof(transfer)));
        *transfer_executions += 1U;
    }
    lxp_secure_zero(activity, LXP_MAX_ACTIVITY_BYTES + 32U);
    free(activity);
    return status;
}

static lxp_result explicit_transfer_suite(uint64_t *executions)
{
    uint8_t bytes[2304];
    size_t count;
    for (count = 0U; count <= LXP_MAX_TRANSFER_SET_LEGS + 2U; ++count) {
        size_t i;
        (void)memset(bytes, 0, sizeof(bytes));
        bytes[0] = (uint8_t)(count >> 8U);
        bytes[1] = (uint8_t)count;
        bytes[2] = (uint8_t)count;
        for (i = 3U; i < sizeof(bytes); ++i)
            bytes[i] = (uint8_t)(i * 17U + count);
        if (lxp_fuzz_transfer_set(bytes, sizeof(bytes)) != LXP_OK)
            return LXP_FATAL_REPLAY_DIVERGENCE;
        *executions += 1U;
    }
    return LXP_OK;
}

int main(int argc, char **argv)
{
    activity_seed seeds[LXP_QUAL_ACTIVITY_TYPE_COUNT];
    uint64_t iterations = UINT64_C(100000);
    uint64_t activity_executions = 0U;
    uint64_t transfer_executions = 0U;
    size_t loaded = 0U;
    lxp_result status;
    if (argc < 3 || argc > 4) return 2;
    if (argc == 4) {
        char *end = NULL;
        errno = 0;
        unsigned long long parsed = strtoull(argv[3], &end, 10);
        if (errno != 0 || end == argv[3] || *end != '\0' || parsed == 0U ||
            parsed > UINT64_MAX) return 2;
        iterations = (uint64_t)parsed;
    }
    (void)memset(seeds, 0, sizeof(seeds));
    status = lxp_fuzz_corpus_seed(argv[1], argv[2]);
    for (loaded = 0U; status == LXP_OK &&
         loaded < LXP_QUAL_ACTIVITY_TYPE_COUNT; ++loaded)
        status = load_seed(argv[2], loaded, &seeds[loaded]);
    if (status == LXP_OK)
        status = activity_exhaustive(seeds, &activity_executions);
    if (status == LXP_OK) status = signature_suite();
    if (status == LXP_OK)
        status = explicit_transfer_suite(&transfer_executions);
    if (status == LXP_OK)
        status = random_suite(seeds, iterations, &activity_executions,
                              &transfer_executions);
    while (loaded > 0U) {
        --loaded;
        if (seeds[loaded].bytes != NULL) {
            lxp_secure_zero(seeds[loaded].bytes, seeds[loaded].length);
            free(seeds[loaded].bytes);
        }
    }
    if (status != LXP_OK) {
        (void)fprintf(stderr, "fuzz qualification failed: %d\n", (int)status);
        return 1;
    }
    (void)printf("activity_cases=%llu\ntransfer_cases=%llu\nsignature_mutations=522\n",
                 (unsigned long long)activity_executions,
                 (unsigned long long)transfer_executions);
    return 0;
}
