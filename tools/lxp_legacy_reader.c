#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_legacy.h"
#include "layerx/lxp_protocol.h"

#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static lxp_result read_exact(
    int descriptor, uint8_t *bytes, size_t length, bool *eof)
{
    size_t offset = 0U;
    *eof = false;
    while (offset < length) {
        ssize_t count = read(descriptor, bytes + offset, length - offset);
        if (count == 0) {
            if (offset == 0U) *eof = true;
            return offset == 0U ? LXP_OK : LXP_ERR_TRUNCATED;
        }
        if (count < 0) {
            if (errno == EINTR) continue;
            return LXP_ERR_IO;
        }
        offset += (size_t)count;
    }
    return LXP_OK;
}

lxp_result lxp_legacy_stream_open(
    const char *path, lxp_legacy_reader *reader)
{
    struct stat metadata;
    int descriptor;
    if (path == NULL || reader == NULL ||
        strstr(path, "://") != NULL || strstr(path, ".sql") != NULL)
        return LXP_ERR_NON_CANONICAL;
    descriptor = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    if (descriptor < 0 || fstat(descriptor, &metadata) != 0 ||
        !S_ISREG(metadata.st_mode) ||
        (fcntl(descriptor, F_GETFL) & O_ACCMODE) != O_RDONLY) {
        if (descriptor >= 0) (void)close(descriptor);
        return LXP_ERR_IO;
    }
    (void)memset(reader, 0, sizeof(*reader));
    reader->descriptor = descriptor;
    reader->read_only = true;
    return LXP_OK;
}

lxp_result lxp_legacy_stream_next(
    lxp_legacy_reader *reader, lxp_arena *arena,
    lxp_byte_span *canonical_activity, bool *end_of_stream)
{
    uint8_t length_bytes[4];
    uint32_t length;
    void *memory;
    bool eof;
    lxp_result status;
    if (reader == NULL || arena == NULL || canonical_activity == NULL ||
        end_of_stream == NULL || !reader->read_only || reader->halted)
        return LXP_FATAL_INVARIANT;
    status = read_exact(
        reader->descriptor, length_bytes, sizeof(length_bytes), &eof);
    if (status != LXP_OK) return status;
    if (eof) {
        *end_of_stream = true;
        *canonical_activity = (lxp_byte_span){NULL, 0U};
        return LXP_OK;
    }
    length = ((uint32_t)length_bytes[0] << 24U) |
        ((uint32_t)length_bytes[1] << 16U) |
        ((uint32_t)length_bytes[2] << 8U) |
        (uint32_t)length_bytes[3];
    if (length == 0U || length > LXP_MAX_ACTIVITY_BYTES)
        return LXP_ERR_LENGTH_LIMIT;
    status = lxp_arena_alloc(arena, length, 1U, &memory);
    if (status == LXP_OK)
        status = read_exact(
            reader->descriptor, (uint8_t *)memory, length, &eof);
    if (status != LXP_OK || eof)
        return status == LXP_OK ? LXP_ERR_TRUNCATED : status;
    *canonical_activity = (lxp_byte_span){memory, length};
    *end_of_stream = false;
    ++reader->records_read;
    return LXP_OK;
}

lxp_result lxp_legacy_readonly_guard(
    lxp_legacy_reader *reader, lxp_legacy_write_kind attempted_write)
{
    if (reader == NULL || attempted_write < LXP_LEGACY_WRITE_STATE ||
        attempted_write > LXP_LEGACY_WRITE_EVENT)
        return LXP_ERR_NON_CANONICAL;
    reader->halted = true;
    return LXP_FATAL_INVARIANT;
}

lxp_result lxp_legacy_boundaries_check(
    const lxp_legacy_boundaries *boundaries)
{
    if (boundaries == NULL || boundaries->postgres_defines_protocol ||
        boundaries->http_is_canonical_wire ||
        boundaries->memory_challenge_is_authority ||
        boundaries->execution_reads_crossverse ||
        boundaries->development_settlement_fallback ||
        boundaries->process_local_events_authoritative)
        return LXP_FATAL_INVARIANT;
    return LXP_OK;
}

lxp_result lxp_legacy_stream_close(lxp_legacy_reader *reader)
{
    if (reader == NULL || reader->descriptor < 0) return LXP_ERR_IO;
    if (close(reader->descriptor) != 0) return LXP_ERR_IO;
    reader->descriptor = -1;
    reader->read_only = false;
    return LXP_OK;
}
