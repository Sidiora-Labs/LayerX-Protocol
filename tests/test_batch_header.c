#include "layerx/lxp_batch.h"
#include "layerx/lxp_hash.h"

#include <stdint.h>
#include <string.h>

static void fixture(lxp_batch_header *header)
{
    uint8_t *roots[8];
    size_t i;
    (void)memset(header, 0, sizeof(*header));
    header->protocol_version = 1U;
    header->network_id = 2U;
    header->epoch = 3U;
    header->batch_number = 4U;
    header->first_sequence = 5U;
    header->last_sequence = 6U;
    header->timestamp_ms = 7U;
    roots[0] = header->previous_state_root;
    roots[1] = header->resulting_state_root;
    roots[2] = header->activity_merkle_root;
    roots[3] = header->receipt_merkle_root;
    roots[4] = header->event_merkle_root;
    roots[5] = header->data_availability_root;
    roots[6] = header->oracle_root;
    roots[7] = header->sequencer_id;
    for (i = 0U; i < 8U; ++i) {
        roots[i][0] = (uint8_t)(11U + i);
        roots[i][31] = (uint8_t)(75U + i);
    }
}

static int golden_encoding(void)
{
    static const uint8_t golden[LXP_BATCH_HEADER_ENCODED_SIZE] = {
        [1] = 1U, [2] = 0x17U, [3] = 1U, [4] = 15U,
        [5] = 1U, [7] = 1U,
        [8] = 2U, [12] = 2U,
        [13] = 3U, [21] = 3U,
        [22] = 4U, [30] = 4U,
        [31] = 5U, [39] = 5U,
        [40] = 6U, [48] = 6U,
        [49] = 7U, [53] = 32U, [54] = 11U, [85] = 75U,
        [86] = 8U, [90] = 32U, [91] = 12U, [122] = 76U,
        [123] = 9U, [127] = 32U, [128] = 13U, [159] = 77U,
        [160] = 10U, [164] = 32U, [165] = 14U, [196] = 78U,
        [197] = 11U, [201] = 32U, [202] = 15U, [233] = 79U,
        [234] = 12U, [238] = 32U, [239] = 16U, [270] = 80U,
        [271] = 13U, [275] = 32U, [276] = 17U, [307] = 81U,
        [308] = 14U, [316] = 7U,
        [317] = 15U, [321] = 32U, [322] = 18U, [353] = 82U
    };
    uint8_t storage[1024];
    lxp_arena arena;
    lxp_batch_header header;
    lxp_byte_span encoded;
    fixture(&header);
    if (lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_batch_header_encode(&header, &arena, &encoded) != LXP_OK ||
        encoded.length != sizeof(golden) ||
        memcmp(encoded.bytes, golden, sizeof(golden)) != 0) return 1;
    return 0;
}

static int roundtrip_and_hash(void)
{
    uint8_t storage[2048];
    uint8_t digest[32];
    uint8_t direct[32];
    uint8_t other[32];
    lxp_arena arena;
    lxp_batch_header header;
    lxp_batch_header decoded;
    lxp_byte_span first;
    lxp_byte_span second;
    fixture(&header);
    if (lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_batch_header_encode(&header, &arena, &first) != LXP_OK ||
        lxp_batch_header_decode(first.bytes, first.length, &decoded) != LXP_OK ||
        memcmp(&header, &decoded, sizeof(header)) != 0 ||
        lxp_batch_header_encode(&decoded, &arena, &second) != LXP_OK ||
        first.length != second.length ||
        memcmp(first.bytes, second.bytes, first.length) != 0 ||
        lxp_hash_domain(LXP_DOMAIN_BATCH_HEADER, first.bytes, first.length,
                        direct) != LXP_OK ||
        lxp_hash_domain(LXP_DOMAIN_ACTIVITY_ID, first.bytes, first.length,
                        other) != LXP_OK) return 1;
    if (lxp_arena_reset(&arena, 0U) != LXP_OK) return 1;
    if (lxp_batch_header_hash(&header, &arena, digest) != LXP_OK ||
        memcmp(digest, direct, sizeof(digest)) != 0 ||
        memcmp(digest, other, sizeof(digest)) == 0) return 1;
    return 0;
}

static int adversarial(void)
{
    uint8_t storage[512];
    uint8_t altered[LXP_BATCH_HEADER_ENCODED_SIZE + 1U];
    lxp_arena arena;
    lxp_batch_header header;
    lxp_batch_header decoded;
    lxp_byte_span encoded;
    fixture(&header);
    if (lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_batch_header_encode(&header, &arena, &encoded) != LXP_OK) return 1;
    (void)memcpy(altered, encoded.bytes, encoded.length);
    altered[encoded.length] = 0U;
    if (lxp_batch_header_decode(altered, encoded.length + 1U, &decoded) !=
        LXP_ERR_TRAILING_BYTES) return 1;
    altered[4] = 14U;
    if (lxp_batch_header_decode(altered, encoded.length, &decoded) !=
        LXP_ERR_NON_CANONICAL) return 1;
    (void)memcpy(altered, encoded.bytes, encoded.length);
    altered[5] = 16U;
    if (lxp_batch_header_decode(altered, encoded.length, &decoded) !=
        LXP_ERR_UNKNOWN_FIELD) return 1;
    if (lxp_batch_header_decode(encoded.bytes, encoded.length - 1U,
                                &decoded) != LXP_ERR_TRUNCATED) return 1;
    return 0;
}

int main(void)
{
    return golden_encoding() != 0 || roundtrip_and_hash() != 0 ||
           adversarial() != 0;
}
