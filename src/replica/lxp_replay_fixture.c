#include "layerx/lxp_replay_fixture.h"

#include "layerx/lxp_activity.h"
#include "layerx/lxp_hash.h"
#include "layerx/lxp_protocol.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const uint8_t fixture_magic[8] = {
    'L', 'X', 'P', 'R', 'P', '0', '0', '1'
};

typedef struct fixture_reader {
    const uint8_t *bytes;
    size_t length;
    size_t offset;
} fixture_reader;

static lxp_result take(fixture_reader *reader, size_t length,
                       const uint8_t **bytes)
{
    if (reader == NULL || bytes == NULL || reader->offset > reader->length ||
        length > reader->length - reader->offset) return LXP_ERR_TRUNCATED;
    *bytes = reader->bytes + reader->offset;
    reader->offset += length;
    return LXP_OK;
}

static lxp_result read_u8(fixture_reader *reader, uint8_t *value)
{
    const uint8_t *bytes;
    lxp_result status = take(reader, 1U, &bytes);
    if (status == LXP_OK) *value = bytes[0];
    return status;
}

static lxp_result read_u32(fixture_reader *reader, uint32_t *value)
{
    const uint8_t *bytes;
    lxp_result status = take(reader, 4U, &bytes);
    if (status == LXP_OK)
        *value = ((uint32_t)bytes[0] << 24U) |
                 ((uint32_t)bytes[1] << 16U) |
                 ((uint32_t)bytes[2] << 8U) | bytes[3];
    return status;
}

static lxp_result read_u64(fixture_reader *reader, uint64_t *value)
{
    const uint8_t *bytes;
    size_t i;
    lxp_result status = take(reader, 8U, &bytes);
    if (status != LXP_OK) return status;
    *value = 0U;
    for (i = 0U; i < 8U; ++i) *value = (*value << 8U) | bytes[i];
    return LXP_OK;
}

static lxp_result read_span(fixture_reader *reader, uint32_t maximum,
                            lxp_byte_span *span)
{
    uint32_t length;
    const uint8_t *bytes;
    lxp_result status = read_u32(reader, &length);
    if (status != LXP_OK) return status;
    if (length > maximum) return LXP_ERR_LENGTH_LIMIT;
    status = take(reader, length, &bytes);
    if (status != LXP_OK) return status;
    span->bytes = bytes;
    span->length = length;
    return LXP_OK;
}

lxp_result lxp_replay_fixture_load(const char *path, lxp_arena *arena,
                                   lxp_replay_fixture *fixture)
{
    FILE *file;
    long file_length;
    void *storage = NULL;
    fixture_reader reader;
    const uint8_t *bytes;
    uint32_t version;
    uint32_t count;
    size_t i;
    lxp_result status;
    if (path == NULL || arena == NULL || fixture == NULL)
        return LXP_ERR_NON_CANONICAL;
    file = fopen(path, "rb");
    if (file == NULL || fseek(file, 0L, SEEK_END) != 0) {
        if (file != NULL) (void)fclose(file);
        return LXP_ERR_IO;
    }
    file_length = ftell(file);
    if (file_length <= 0 || fseek(file, 0L, SEEK_SET) != 0) {
        (void)fclose(file);
        return LXP_ERR_IO;
    }
    status = lxp_arena_alloc(arena, (size_t)file_length, 1U, &storage);
    if (status != LXP_OK) {
        (void)fclose(file);
        return status;
    }
    if (fread(storage, 1U, (size_t)file_length, file) != (size_t)file_length ||
        fclose(file) != 0) return LXP_ERR_IO;
    reader.bytes = (const uint8_t *)storage;
    reader.length = (size_t)file_length;
    reader.offset = 0U;
    (void)memset(fixture, 0, sizeof(*fixture));
    status = take(&reader, sizeof(fixture_magic), &bytes);
    if (status == LXP_OK &&
        memcmp(bytes, fixture_magic, sizeof(fixture_magic)) != 0)
        status = LXP_ERR_NON_CANONICAL;
    if (status == LXP_OK) status = read_u32(&reader, &version);
    if (status == LXP_OK && version != 1U) status = LXP_ERR_VERSION_UNSUPPORTED;
    if (status == LXP_OK) status = read_u32(&reader, &count);
    if (status == LXP_OK &&
        (count == 0U || count > LXP_REPLAY_FIXTURE_MAX_RECORDS))
        status = LXP_ERR_LENGTH_LIMIT;
    if (status == LXP_OK)
        status = take(&reader, 32U, &bytes);
    if (status == LXP_OK)
        (void)memcpy(fixture->expected_terminal_root, bytes, 32U);
    if (status == LXP_OK)
        status = take(&reader, 32U, &bytes);
    if (status == LXP_OK)
        (void)memcpy(fixture->expected_digest, bytes, 32U);
    if (status == LXP_OK)
        status = lxp_arena_alloc(arena, (size_t)count * sizeof(*fixture->records),
                                 _Alignof(lxp_replay_fixture_record),
                                 (void **)&fixture->records);
    for (i = 0U; status == LXP_OK && i < count; ++i) {
        lxp_activity activity;
        status = read_u64(&reader, &fixture->records[i].global_sequence);
        if (status == LXP_OK)
            status = read_u8(&reader, &fixture->records[i].batch_boundary);
        if (status == LXP_OK && fixture->records[i].batch_boundary > 1U)
            status = LXP_ERR_NON_CANONICAL;
        if (status == LXP_OK)
            status = read_span(&reader, LXP_MAX_ACTIVITY_BYTES,
                               &fixture->records[i].canonical_activity);
        if (status == LXP_OK)
            status = lxp_activity_decode(
                fixture->records[i].canonical_activity.bytes,
                fixture->records[i].canonical_activity.length, &activity);
        if (status == LXP_OK)
            status = take(&reader, 32U, &bytes);
        if (status == LXP_OK)
            (void)memcpy(fixture->records[i].expected_state_root, bytes, 32U);
        if (status == LXP_OK)
            status = read_span(&reader, LXP_REPLAY_FIXTURE_RECEIPT_BYTES,
                               &fixture->records[i].expected_receipt);
        if (status == LXP_OK && fixture->records[i].expected_receipt.length !=
            LXP_REPLAY_FIXTURE_RECEIPT_BYTES) status = LXP_ERR_NON_CANONICAL;
        if (status == LXP_OK)
            status = read_span(&reader, LXP_REPLAY_FIXTURE_EVENT_BYTES,
                               &fixture->records[i].expected_event);
        if (status == LXP_OK && fixture->records[i].expected_event.length !=
            LXP_REPLAY_FIXTURE_EVENT_BYTES) status = LXP_ERR_NON_CANONICAL;
        if (status == LXP_OK)
            status = take(&reader, 32U, &bytes);
        if (status == LXP_OK)
            (void)memcpy(fixture->records[i].expected_batch_root, bytes, 32U);
        if (status == LXP_OK && fixture->records[i].global_sequence != i + 1U)
            status = LXP_ERR_SEQUENCE_GAP;
    }
    if (status == LXP_OK && reader.offset != reader.length)
        status = LXP_ERR_TRAILING_BYTES;
    if (status == LXP_OK && fixture->records[count - 1U].batch_boundary != 1U)
        status = LXP_ERR_NON_CANONICAL;
    if (status == LXP_OK) fixture->record_count = count;
    return status;
}

static void store_u32(uint8_t out[4], uint32_t value)
{
    out[0] = (uint8_t)(value >> 24U);
    out[1] = (uint8_t)(value >> 16U);
    out[2] = (uint8_t)(value >> 8U);
    out[3] = (uint8_t)value;
}

static void store_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i) out[7U - i] = (uint8_t)(value >> (i * 8U));
}

static lxp_result replay_record(const lxp_replay_fixture_record *record,
                                const uint8_t previous_state[32],
                                const uint8_t previous_batch[32],
                                const uint8_t previous_digest[32],
                                uint8_t state[32], uint8_t receipt[106],
                                uint8_t event[36], uint8_t batch[32],
                                uint8_t digest[32])
{
    lxp_activity activity;
    uint8_t activity_id[32];
    uint8_t state_input[72];
    uint8_t batch_input[72];
    uint8_t digest_input[215];
    size_t digest_length = 0U;
    lxp_result status;
    status = lxp_activity_decode(record->canonical_activity.bytes,
                                 record->canonical_activity.length,
                                 &activity);
    if (status == LXP_OK)
        status = lxp_activity_id(record->canonical_activity.bytes,
                                 record->canonical_activity.length,
                                 activity_id);
    if (status != LXP_OK) return status;
    (void)memcpy(state_input, previous_state, 32U);
    store_u64(state_input + 32U, record->global_sequence);
    (void)memcpy(state_input + 40U, activity_id, 32U);
    status = lxp_hash_domain(LXP_DOMAIN_STATE_ROOT_CHAIN, state_input,
                             sizeof(state_input), state);
    if (status != LXP_OK) return status;
    receipt[0] = 0U;
    receipt[1] = 1U;
    store_u64(receipt + 2U, record->global_sequence);
    (void)memcpy(receipt + 10U, activity_id, 32U);
    (void)memcpy(receipt + 42U, previous_state, 32U);
    (void)memcpy(receipt + 74U, state, 32U);
    store_u32(event, activity.activity_type);
    (void)memcpy(event + 4U, activity_id, 32U);
    (void)memset(batch, 0, 32U);
    if (record->batch_boundary != 0U) {
        (void)memcpy(batch_input, previous_batch, 32U);
        (void)memcpy(batch_input + 32U, state, 32U);
        store_u64(batch_input + 64U, record->global_sequence);
        status = lxp_hash_domain(LXP_DOMAIN_BATCH_HEADER, batch_input,
                                 sizeof(batch_input), batch);
        if (status != LXP_OK) return status;
    }
    (void)memcpy(digest_input + digest_length, previous_digest, 32U);
    digest_length += 32U;
    store_u32(digest_input + digest_length,
              LXP_REPLAY_FIXTURE_RECEIPT_BYTES);
    digest_length += 4U;
    (void)memcpy(digest_input + digest_length, receipt,
                 LXP_REPLAY_FIXTURE_RECEIPT_BYTES);
    digest_length += LXP_REPLAY_FIXTURE_RECEIPT_BYTES;
    store_u32(digest_input + digest_length, LXP_REPLAY_FIXTURE_EVENT_BYTES);
    digest_length += 4U;
    (void)memcpy(digest_input + digest_length, event,
                 LXP_REPLAY_FIXTURE_EVENT_BYTES);
    digest_length += LXP_REPLAY_FIXTURE_EVENT_BYTES;
    digest_input[digest_length++] = record->batch_boundary;
    if (record->batch_boundary != 0U) {
        (void)memcpy(digest_input + digest_length, batch, 32U);
        digest_length += 32U;
    }
    return lxp_hash_domain(LXP_DOMAIN_STATE_ROOT_CHAIN, digest_input,
                           digest_length, digest);
}

lxp_result lxp_replay_digest(const lxp_replay_fixture *fixture,
                             uint8_t digest[32], uint8_t terminal_root[32],
                             uint64_t *first_divergent_sequence)
{
    uint8_t state[32] = {0};
    uint8_t previous_state[32] = {0};
    uint8_t batch[32] = {0};
    uint8_t previous_batch[32] = {0};
    uint8_t current_digest[32] = {0};
    uint8_t previous_digest[32] = {0};
    uint8_t receipt[LXP_REPLAY_FIXTURE_RECEIPT_BYTES];
    uint8_t event[LXP_REPLAY_FIXTURE_EVENT_BYTES];
    size_t i;
    lxp_result status = LXP_OK;
    if (fixture == NULL || digest == NULL || terminal_root == NULL ||
        first_divergent_sequence == NULL || fixture->records == NULL ||
        fixture->record_count == 0U) return LXP_ERR_NON_CANONICAL;
    *first_divergent_sequence = 0U;
    for (i = 0U; i < fixture->record_count; ++i) {
        const lxp_replay_fixture_record *record = &fixture->records[i];
        status = replay_record(record, previous_state, previous_batch,
                               previous_digest, state, receipt, event, batch,
                               current_digest);
        if (status != LXP_OK ||
            memcmp(state, record->expected_state_root, 32U) != 0 ||
            memcmp(receipt, record->expected_receipt.bytes,
                   sizeof(receipt)) != 0 ||
            memcmp(event, record->expected_event.bytes, sizeof(event)) != 0 ||
            memcmp(batch, record->expected_batch_root, 32U) != 0) {
            *first_divergent_sequence = record->global_sequence;
            return status == LXP_OK ? LXP_FATAL_REPLAY_DIVERGENCE : status;
        }
        (void)memcpy(previous_state, state, 32U);
        if (record->batch_boundary != 0U)
            (void)memcpy(previous_batch, batch, 32U);
        (void)memcpy(previous_digest, current_digest, 32U);
    }
    (void)memcpy(digest, current_digest, 32U);
    (void)memcpy(terminal_root, state, 32U);
    return LXP_OK;
}

lxp_result lxp_replay_crossarch_case(const char *path, lxp_arena *arena,
                                     uint8_t digest[32],
                                     uint64_t *first_divergent_sequence)
{
    lxp_replay_fixture fixture;
    uint8_t terminal_root[32];
    lxp_result status = lxp_replay_fixture_load(path, arena, &fixture);
    if (status == LXP_OK)
        status = lxp_replay_digest(&fixture, digest, terminal_root,
                                   first_divergent_sequence);
    if (status == LXP_OK &&
        (memcmp(digest, fixture.expected_digest, 32U) != 0 ||
         memcmp(terminal_root, fixture.expected_terminal_root, 32U) != 0)) {
        *first_divergent_sequence = fixture.records[
            fixture.record_count - 1U].global_sequence;
        status = LXP_FATAL_REPLAY_DIVERGENCE;
    }
    return status;
}
