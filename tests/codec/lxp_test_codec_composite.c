#include "layerx/lxp_codec.h"
#include "layerx/lxp_protocol.h"

#include <string.h>

int main(void)
{
    uint8_t storage[256];
    lxp_arena arena;
    lxp_codec_writer writer;
    lxp_codec_reader reader;
    lxp_byte_span span;
    const uint8_t a[] = {0x01U};
    const uint8_t b[] = {0x01U, 0x00U};
    const uint8_t c[] = {0x02U};
    lxp_byte_span sorted[] = {{a, sizeof(a)}, {b, sizeof(b)}, {c, sizeof(c)}};
    lxp_byte_span duplicate[] = {{a, sizeof(a)}, {a, sizeof(a)}};
    const char canonical[] = "LayerX";
    const char decomposed[] = "e\xcc\x81";
    uint8_t tag = 0U;

    if (lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_codec_writer_init(&writer, &arena, sizeof(storage)) != LXP_OK)
        return 1;
    if (lxp_codec_write_bytes(&writer, a, sizeof(a), LXP_MAX_PAYLOAD_BYTES) != LXP_OK ||
        lxp_codec_write_text(&writer, canonical, strlen(canonical),
                             LXP_MAX_DID_LENGTH) != LXP_OK ||
        lxp_codec_write_seq(&writer, 3U, LXP_MAX_EFFECTS) != LXP_OK ||
        lxp_codec_write_tag(&writer, 2U, 3U) != LXP_OK) return 1;
    if (lxp_codec_write_text(&writer, decomposed, sizeof(decomposed) - 1U,
                             LXP_MAX_DID_LENGTH) != LXP_ERR_NON_CANONICAL)
        return 1;
    if (lxp_codec_seq_check_sorted(sorted, 3U) != LXP_OK ||
        lxp_codec_seq_check_sorted(duplicate, 2U) != LXP_ERR_UNSORTED_SEQUENCE)
        return 1;
    if (lxp_codec_reader_init(&reader, writer.bytes, writer.length) != LXP_OK ||
        lxp_codec_read_bytes(&reader, &span, LXP_MAX_PAYLOAD_BYTES) != LXP_OK ||
        span.length != sizeof(a) || memcmp(span.bytes, a, sizeof(a)) != 0)
        return 1;
    if (lxp_codec_read_bytes(&reader, &span, LXP_MAX_DID_LENGTH) != LXP_OK ||
        span.length != strlen(canonical)) return 1;
    {
        uint32_t count = 0U;
        if (lxp_codec_read_u32(&reader, &count) != LXP_OK || count != 3U)
            return 1;
    }
    if (lxp_codec_read_tag(&reader, 3U, &tag) != LXP_OK || tag != 2U)
        return 1;
    return lxp_codec_write_tag(&writer, 4U, 3U) == LXP_ERR_INVALID_TAG ? 0 : 1;
}
