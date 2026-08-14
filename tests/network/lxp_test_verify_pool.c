#include "layerx/lxp_crypto.h"
#include "layerx/lxp_verify_pool.h"

#include <openssl/evp.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef struct signature_job {
    uint8_t public_key[32];
    uint8_t signature[64];
    const uint8_t *message;
    size_t message_length;
    lxp_domain_tag_id domain;
} signature_job;

static bool verify_signature(const void *opaque)
{
    const signature_job *job = (const signature_job *)opaque;
    return lxp_ed25519_verify(job->public_key, job->signature, job->domain,
                              job->message, job->message_length) == LXP_OK;
}

static int run(size_t workers, signature_job *jobs, size_t count,
               bool *results)
{
    lxp_verify_pool pool;
    size_t result_count;
    size_t i;
    if (lxp_verify_pool_create(&pool, workers, count) != LXP_OK) return 0;
    for (i = 0U; i < count; ++i)
        if (lxp_verify_pool_submit(&pool, verify_signature, &jobs[i]) != LXP_OK)
            return 0;
    return lxp_verify_pool_join(&pool, results, count, &result_count) == LXP_OK &&
           result_count == count;
}

int main(void)
{
    enum { COUNT = 64 };
    static const uint8_t message[] = { 'L', 'X', 'P' };
    uint8_t seed[32] = { 1U };
    uint8_t digest[32];
    size_t public_length = 32U;
    size_t signature_length = 64U;
    EVP_PKEY *private_key;
    EVP_MD_CTX *signing;
    signature_job jobs[COUNT];
    bool serial[COUNT];
    bool parallel[COUNT];
    size_t i;
    private_key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL, seed,
                                               sizeof(seed));
    if (private_key == NULL ||
        EVP_PKEY_get_raw_public_key(private_key, jobs[0].public_key,
                                    &public_length) != 1 ||
        lxp_hash_domain(LXP_DOMAIN_SIGNATURE_PREIMAGE, message,
                        sizeof(message), digest) != LXP_OK) return 1;
    signing = EVP_MD_CTX_new();
    if (signing == NULL ||
        EVP_DigestSignInit(signing, NULL, NULL, NULL, private_key) != 1 ||
        EVP_DigestSign(signing, jobs[0].signature, &signature_length, digest,
                       sizeof(digest)) != 1) return 1;
    EVP_MD_CTX_free(signing);
    EVP_PKEY_free(private_key);
    jobs[0].message = message;
    jobs[0].message_length = sizeof(message);
    jobs[0].domain = LXP_DOMAIN_SIGNATURE_PREIMAGE;
    for (i = 1U; i < COUNT; ++i) {
        jobs[i] = jobs[0];
        if ((i & 1U) != 0U) jobs[i].domain = LXP_DOMAIN_ACTIVITY_ID;
    }
    if (!run(0U, jobs, COUNT, serial) || !run(8U, jobs, COUNT, parallel) ||
        memcmp(serial, parallel, sizeof(serial)) != 0) return 1;
    for (i = 0U; i < COUNT; ++i)
        if (serial[i] != ((i & 1U) == 0U)) return 1;
    return 0;
}
