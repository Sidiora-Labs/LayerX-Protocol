#include "layerx/lxp_codec.h"

#include <stdint.h>

int main(void)
{
    uint8_t storage[32];
    lxp_arena arena;
    lxp_codec_writer writer;
    lxp_codec_reader reader;
    uint64_t sequence = 9U;
    uint64_t fee = 11U;

    if (lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_codec_writer_init(&writer, &arena, sizeof(storage)) != LXP_OK ||
        lxp_codec_write_struct_header(&writer, 0x1001U) != LXP_OK ||
        lxp_codec_write_u8(&writer, 0xaaU) != LXP_OK) return 1;
    if (lxp_codec_reader_init(&reader, writer.bytes, writer.length) != LXP_OK ||
        lxp_codec_read_struct_header(&reader, 0x1001U) != LXP_OK ||
        lxp_codec_finish(&reader) != LXP_ERR_TRAILING_BYTES) return 1;
    {
        uint8_t value;
        if (lxp_codec_read_u8(&reader, &value) != LXP_OK || value != 0xaaU ||
            lxp_codec_finish(&reader) != LXP_OK) return 1;
    }
    if (lxp_codec_reader_init(&reader, writer.bytes, writer.length) != LXP_OK ||
        lxp_codec_read_struct_header(&reader, 0x1002U) !=
        LXP_ERR_VERSION_UNSUPPORTED) return 1;
    if (lxp_codec_reject_unknown_field(4U, 3U) != LXP_ERR_UNKNOWN_FIELD ||
        lxp_codec_reject_unknown_field(3U, 3U) != LXP_OK) return 1;
    /* Strict codec rejection is pre-admission and cannot mutate these values. */
    return sequence == 9U && fee == 11U ? 0 : 1;
}
