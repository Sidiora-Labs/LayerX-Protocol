#include "layerx/lxp_batch.h"
#include "layerx/lxp_crypto.h"

#include <string.h>

enum { LXP_BATCH_BODY_STRUCTURE_TAG = 0x1702,
       LXP_BATCH_BODY_FIELD_COUNT = 8 };

static lxp_result body_size(const lxp_batch_body *body, size_t *size)
{
    const lxp_byte_span *sections[6];
    size_t total = 5U + LXP_BATCH_BODY_FIELD_COUNT +
                   (LXP_BATCH_BODY_FIELD_COUNT * 4U) +
                   LXP_BATCH_HEADER_ENCODED_SIZE + 64U;
    size_t i;
    if (body == NULL || size == NULL) return LXP_ERR_NON_CANONICAL;
    sections[0] = &body->activities;
    sections[1] = &body->receipts;
    sections[2] = &body->events;
    sections[3] = &body->oracle_inputs;
    sections[4] = &body->state_diff;
    sections[5] = &body->recovery_metadata;
    for (i = 0U; i < 6U; ++i) {
        if ((sections[i]->bytes == NULL && sections[i]->length != 0U) ||
            sections[i]->length > LXP_MAX_BATCH_BODY_BYTES - total)
            return LXP_ERR_LENGTH_LIMIT;
        total += sections[i]->length;
    }
    if (total > LXP_MAX_BATCH_BODY_BYTES) return LXP_ERR_LENGTH_LIMIT;
    *size = total;
    return LXP_OK;
}

lxp_result lxp_batch_body_encode(const lxp_batch_body *body, lxp_arena *arena,
                                 lxp_byte_span *encoded)
{
    lxp_codec_writer writer;
    lxp_byte_span header;
    size_t size;
    lxp_result status;
    if (arena == NULL || encoded == NULL) return LXP_ERR_NON_CANONICAL;
    status = body_size(body, &size);
    if (status != LXP_OK) return status;
    status = lxp_codec_writer_init(&writer, arena, size);
    if (status != LXP_OK) return status;
    status = lxp_codec_write_struct_header(&writer,
                                           LXP_BATCH_BODY_STRUCTURE_TAG);
    if (status == LXP_OK)
        status = lxp_codec_write_u8(&writer, LXP_BATCH_BODY_FIELD_COUNT);
#define BODY_FIELD(id, value, length, maximum) do { \
    if (status == LXP_OK) status = lxp_codec_write_tag( \
        &writer, (id), LXP_BATCH_BODY_FIELD_COUNT); \
    if (status == LXP_OK) status = lxp_codec_write_bytes( \
        &writer, (value), (length), (maximum)); \
} while (0)
    if (status == LXP_OK) {
        uint8_t header_storage[LXP_BATCH_HEADER_ENCODED_SIZE];
        lxp_arena header_arena;
        status = lxp_arena_init(&header_arena, header_storage,
                                sizeof(header_storage));
        if (status == LXP_OK)
            status = lxp_batch_header_encode(&body->header, &header_arena,
                                             &header);
        if (status == LXP_OK)
            BODY_FIELD(1U, header.bytes, header.length,
                       LXP_BATCH_HEADER_ENCODED_SIZE);
    }
    BODY_FIELD(2U, body->sequencer_signature, 64U, 64U);
    BODY_FIELD(3U, body->activities.bytes, body->activities.length,
               LXP_MAX_BATCH_BODY_BYTES);
    BODY_FIELD(4U, body->receipts.bytes, body->receipts.length,
               LXP_MAX_BATCH_BODY_BYTES);
    BODY_FIELD(5U, body->events.bytes, body->events.length,
               LXP_MAX_BATCH_BODY_BYTES);
    BODY_FIELD(6U, body->oracle_inputs.bytes, body->oracle_inputs.length,
               LXP_MAX_BATCH_BODY_BYTES);
    BODY_FIELD(7U, body->state_diff.bytes, body->state_diff.length,
               LXP_MAX_BATCH_BODY_BYTES);
    BODY_FIELD(8U, body->recovery_metadata.bytes,
               body->recovery_metadata.length, LXP_MAX_BATCH_BODY_BYTES);
#undef BODY_FIELD
    if (status != LXP_OK) return status;
    if (writer.length != size) return LXP_FATAL_INVARIANT;
    encoded->bytes = writer.bytes;
    encoded->length = writer.length;
    return LXP_OK;
}

static lxp_result next_span(lxp_codec_reader *reader, uint8_t expected,
                            lxp_byte_span *span, uint32_t maximum)
{
    uint8_t tag;
    lxp_result status = lxp_codec_read_tag(reader,
                                           LXP_BATCH_BODY_FIELD_COUNT, &tag);
    if (status != LXP_OK || tag != expected) return LXP_ERR_UNKNOWN_FIELD;
    return lxp_codec_read_bytes(reader, span, maximum);
}

lxp_result lxp_batch_body_decode(const uint8_t *bytes, size_t length,
                                 lxp_batch_body *body)
{
    lxp_codec_reader reader;
    lxp_byte_span span;
    uint8_t count;
    lxp_result status;
    if (body == NULL || (bytes == NULL && length != 0U) ||
        length > LXP_MAX_BATCH_BODY_BYTES) return LXP_ERR_LENGTH_LIMIT;
    (void)memset(body, 0, sizeof(*body));
    status = lxp_codec_reader_init(&reader, bytes, length);
    if (status == LXP_OK)
        status = lxp_codec_read_struct_header(&reader,
                                              LXP_BATCH_BODY_STRUCTURE_TAG);
    if (status == LXP_OK) status = lxp_codec_read_u8(&reader, &count);
    if (status != LXP_OK) return status;
    if (count != LXP_BATCH_BODY_FIELD_COUNT) return LXP_ERR_NON_CANONICAL;
    status = next_span(&reader, 1U, &span, LXP_BATCH_HEADER_ENCODED_SIZE);
    if (status == LXP_OK && span.length != LXP_BATCH_HEADER_ENCODED_SIZE)
        status = LXP_ERR_NON_CANONICAL;
    if (status == LXP_OK)
        status = lxp_batch_header_decode(span.bytes, span.length,
                                         &body->header);
    if (status == LXP_OK) status = next_span(&reader, 2U, &span, 64U);
    if (status == LXP_OK && span.length != 64U)
        status = LXP_ERR_NON_CANONICAL;
    if (status == LXP_OK)
        (void)memcpy(body->sequencer_signature, span.bytes, 64U);
    if (status == LXP_OK)
        status = next_span(&reader, 3U, &body->activities,
                           LXP_MAX_BATCH_BODY_BYTES);
    if (status == LXP_OK)
        status = next_span(&reader, 4U, &body->receipts,
                           LXP_MAX_BATCH_BODY_BYTES);
    if (status == LXP_OK)
        status = next_span(&reader, 5U, &body->events,
                           LXP_MAX_BATCH_BODY_BYTES);
    if (status == LXP_OK)
        status = next_span(&reader, 6U, &body->oracle_inputs,
                           LXP_MAX_BATCH_BODY_BYTES);
    if (status == LXP_OK)
        status = next_span(&reader, 7U, &body->state_diff,
                           LXP_MAX_BATCH_BODY_BYTES);
    if (status == LXP_OK)
        status = next_span(&reader, 8U, &body->recovery_metadata,
                           LXP_MAX_BATCH_BODY_BYTES);
    return status == LXP_OK ? lxp_codec_finish(&reader) : status;
}

lxp_result lxp_batch_publish(const lxp_batch_body *body,
                             lxp_batch_replica_target *replicas,
                             size_t replica_count, size_t chunk_size,
                             lxp_batch_chunk_send_fn send_chunk,
                             void *send_context, lxp_arena *arena)
{
    lxp_byte_span encoded;
    size_t mark;
    size_t i;
    lxp_result status;
    if (body == NULL || replicas == NULL || replica_count == 0U ||
        replica_count > LXP_MAX_BATCH_REPLICAS || chunk_size == 0U ||
        chunk_size > LXP_MAX_BATCH_CHUNK_BYTES || send_chunk == NULL ||
        arena == NULL || lxp_ct_is_zero(body->sequencer_signature, 64U))
        return LXP_ERR_NON_CANONICAL;
    mark = lxp_arena_mark(arena);
    status = lxp_batch_body_encode(body, arena, &encoded);
    for (i = 0U; status == LXP_OK && i < replica_count; ++i) {
        if (replicas[i].resume_offset > encoded.length) {
            status = LXP_ERR_NON_CANONICAL;
            break;
        }
        while (replicas[i].resume_offset < encoded.length) {
            size_t offset = (size_t)replicas[i].resume_offset;
            size_t remaining = encoded.length - offset;
            size_t count = remaining < chunk_size ? remaining : chunk_size;
            status = send_chunk(send_context, replicas[i].replica_id,
                                body->header.batch_number,
                                replicas[i].resume_offset,
                                encoded.bytes + offset, count,
                                (uint64_t)encoded.length);
            if (status != LXP_OK) break;
            replicas[i].resume_offset += count;
        }
        if (status == LXP_OK) replicas[i].complete = 1U;
    }
    (void)lxp_arena_reset(arena, mark);
    return status;
}
