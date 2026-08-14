#include "layerx/lxp_crypto.h"

#include <string.h>

static const uint32_t round_constants[64] = {
    0x428a2f98U,0x71374491U,0xb5c0fbcfU,0xe9b5dba5U,0x3956c25bU,0x59f111f1U,0x923f82a4U,0xab1c5ed5U,
    0xd807aa98U,0x12835b01U,0x243185beU,0x550c7dc3U,0x72be5d74U,0x80deb1feU,0x9bdc06a7U,0xc19bf174U,
    0xe49b69c1U,0xefbe4786U,0x0fc19dc6U,0x240ca1ccU,0x2de92c6fU,0x4a7484aaU,0x5cb0a9dcU,0x76f988daU,
    0x983e5152U,0xa831c66dU,0xb00327c8U,0xbf597fc7U,0xc6e00bf3U,0xd5a79147U,0x06ca6351U,0x14292967U,
    0x27b70a85U,0x2e1b2138U,0x4d2c6dfcU,0x53380d13U,0x650a7354U,0x766a0abbU,0x81c2c92eU,0x92722c85U,
    0xa2bfe8a1U,0xa81a664bU,0xc24b8b70U,0xc76c51a3U,0xd192e819U,0xd6990624U,0xf40e3585U,0x106aa070U,
    0x19a4c116U,0x1e376c08U,0x2748774cU,0x34b0bcb5U,0x391c0cb3U,0x4ed8aa4aU,0x5b9cca4fU,0x682e6ff3U,
    0x748f82eeU,0x78a5636fU,0x84c87814U,0x8cc70208U,0x90befffaU,0xa4506cebU,0xbef9a3f7U,0xc67178f2U
};

static uint32_t rotate_right(uint32_t value, unsigned shift)
{
    return (value >> shift) | (value << (32U - shift));
}

static void transform(lxp_hash_context *context, const uint8_t block[64])
{
    uint32_t words[64];
    uint32_t a,b,c,d,e,f,g,h;
    size_t i;
    for (i = 0U; i < 16U; ++i) {
        words[i] = ((uint32_t)block[i*4U] << 24U) |
                   ((uint32_t)block[i*4U+1U] << 16U) |
                   ((uint32_t)block[i*4U+2U] << 8U) |
                   (uint32_t)block[i*4U+3U];
    }
    for (i = 16U; i < 64U; ++i) {
        uint32_t s0 = rotate_right(words[i-15U],7U) ^ rotate_right(words[i-15U],18U) ^ (words[i-15U] >> 3U);
        uint32_t s1 = rotate_right(words[i-2U],17U) ^ rotate_right(words[i-2U],19U) ^ (words[i-2U] >> 10U);
        words[i] = words[i-16U] + s0 + words[i-7U] + s1;
    }
    a=context->state[0]; b=context->state[1]; c=context->state[2]; d=context->state[3];
    e=context->state[4]; f=context->state[5]; g=context->state[6]; h=context->state[7];
    for (i = 0U; i < 64U; ++i) {
        uint32_t s1 = rotate_right(e,6U) ^ rotate_right(e,11U) ^ rotate_right(e,25U);
        uint32_t choice = (e & f) ^ ((~e) & g);
        uint32_t temp1 = h + s1 + choice + round_constants[i] + words[i];
        uint32_t s0 = rotate_right(a,2U) ^ rotate_right(a,13U) ^ rotate_right(a,22U);
        uint32_t majority = (a & b) ^ (a & c) ^ (b & c);
        uint32_t temp2 = s0 + majority;
        h=g; g=f; f=e; e=d+temp1; d=c; c=b; b=a; a=temp1+temp2;
    }
    context->state[0]+=a; context->state[1]+=b; context->state[2]+=c; context->state[3]+=d;
    context->state[4]+=e; context->state[5]+=f; context->state[6]+=g; context->state[7]+=h;
}

void lxp_hash_init(lxp_hash_context *context)
{
    static const uint32_t initial[8] = {0x6a09e667U,0xbb67ae85U,0x3c6ef372U,0xa54ff53aU,
                                         0x510e527fU,0x9b05688cU,0x1f83d9abU,0x5be0cd19U};
    if (context == NULL) return;
    (void)memcpy(context->state, initial, sizeof(initial));
    context->total_length = 0U;
    context->block_length = 0U;
    (void)memset(context->block, 0, sizeof(context->block));
}

lxp_result lxp_hash_update(lxp_hash_context *context, const void *data, size_t length)
{
    const uint8_t *bytes = (const uint8_t *)data;
    if (context == NULL || (data == NULL && length != 0U) ||
        length > UINT64_MAX - context->total_length) return LXP_ERR_LENGTH_LIMIT;
    context->total_length += length;
    while (length != 0U) {
        size_t take = sizeof(context->block) - context->block_length;
        if (take > length) take = length;
        (void)memcpy(context->block + context->block_length, bytes, take);
        context->block_length += take; bytes += take; length -= take;
        if (context->block_length == sizeof(context->block)) {
            transform(context, context->block);
            context->block_length = 0U;
        }
    }
    return LXP_OK;
}

lxp_result lxp_hash_final(lxp_hash_context *context, uint8_t digest[32])
{
    uint64_t bit_length;
    size_t i;
    if (context == NULL || digest == NULL || context->total_length > UINT64_MAX / 8U)
        return LXP_ERR_LENGTH_LIMIT;
    bit_length = context->total_length * 8U;
    context->block[context->block_length++] = 0x80U;
    if (context->block_length > 56U) {
        (void)memset(context->block + context->block_length, 0,
                     sizeof(context->block) - context->block_length);
        transform(context, context->block);
        context->block_length = 0U;
    }
    (void)memset(context->block + context->block_length, 0, 56U - context->block_length);
    for (i = 0U; i < 8U; ++i)
        context->block[63U-i] = (uint8_t)(bit_length >> (i*8U));
    transform(context, context->block);
    for (i = 0U; i < 8U; ++i) {
        digest[i*4U] = (uint8_t)(context->state[i] >> 24U);
        digest[i*4U+1U] = (uint8_t)(context->state[i] >> 16U);
        digest[i*4U+2U] = (uint8_t)(context->state[i] >> 8U);
        digest[i*4U+3U] = (uint8_t)context->state[i];
    }
    lxp_secure_zero(context, sizeof(*context));
    return LXP_OK;
}

lxp_result lxp_hash_sha256(const void *data, size_t length, uint8_t digest[32])
{
    lxp_hash_context context;
    lxp_result result;
    lxp_hash_init(&context);
    result = lxp_hash_update(&context, data, length);
    return result == LXP_OK ? lxp_hash_final(&context, digest) : result;
}

lxp_result lxp_hash_domain(lxp_domain_tag_id domain, const void *data,
                           size_t length, uint8_t digest[32])
{
    lxp_hash_context context;
    size_t tag_length = 0U;
    const uint8_t *tag = lxp_domain_tag(domain, &tag_length);
    lxp_result result;
    if (tag == NULL) return LXP_ERR_INVALID_TAG;
    lxp_hash_init(&context);
    result = lxp_hash_update(&context, tag, tag_length);
    if (result == LXP_OK) result = lxp_hash_update(&context, data, length);
    return result == LXP_OK ? lxp_hash_final(&context, digest) : result;
}

#define LXP_PURPOSE_HELPER(name, domain) \
lxp_result name(const void *data, size_t length, uint8_t out[32]) \
{ return lxp_hash_domain(domain, data, length, out); }
LXP_PURPOSE_HELPER(lxp_hash_activity_id, LXP_DOMAIN_ACTIVITY_ID)
LXP_PURPOSE_HELPER(lxp_hash_payload, LXP_DOMAIN_PAYLOAD_HASH)
LXP_PURPOSE_HELPER(lxp_hash_signature_preimage, LXP_DOMAIN_SIGNATURE_PREIMAGE)
LXP_PURPOSE_HELPER(lxp_hash_authority, LXP_DOMAIN_AUTHORITY_HASH)
LXP_PURPOSE_HELPER(lxp_hash_context_value, LXP_DOMAIN_CONTEXT_HASH)
LXP_PURPOSE_HELPER(lxp_hash_account_id, LXP_DOMAIN_ACCOUNT_ID)
#undef LXP_PURPOSE_HELPER
