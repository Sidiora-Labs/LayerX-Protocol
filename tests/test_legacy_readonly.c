#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_legacy.h"

#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(void)
{
    static uint8_t arena_bytes[4096];
    uint8_t record[] = {0U, 0U, 0U, 4U, 1U, 2U, 3U, 4U};
    uint8_t unchanged[sizeof(record)];
    char path[] = "/tmp/lxp-legacy-readonly-XXXXXX";
    int descriptor = mkstemp(path);
    lxp_legacy_reader reader;
    lxp_legacy_boundaries boundaries;
    lxp_arena arena;
    lxp_byte_span activity;
    bool eof = false;
    ssize_t count;

    if (descriptor < 0 ||
        write(descriptor, record, sizeof(record)) != (ssize_t)sizeof(record) ||
        close(descriptor) != 0 ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_legacy_stream_open(path, &reader) != LXP_OK ||
        !reader.read_only ||
        (fcntl(reader.descriptor, F_GETFL) & O_ACCMODE) != O_RDONLY ||
        lxp_legacy_stream_next(
            &reader, &arena, &activity, &eof) != LXP_OK || eof ||
        activity.length != 4U ||
        memcmp(activity.bytes, record + 4U, 4U) != 0 ||
        lxp_legacy_stream_next(
            &reader, &arena, &activity, &eof) != LXP_OK || !eof)
        return 1;
    (void)memset(&boundaries, 0, sizeof(boundaries));
    if (lxp_legacy_boundaries_check(&boundaries) != LXP_OK)
        return 1;
    boundaries.development_settlement_fallback = true;
    if (lxp_legacy_boundaries_check(&boundaries) != LXP_FATAL_INVARIANT ||
        lxp_legacy_readonly_guard(
            &reader, LXP_LEGACY_WRITE_CUSTODY) != LXP_FATAL_INVARIANT ||
        !reader.halted ||
        lxp_legacy_stream_next(
            &reader, &arena, &activity, &eof) != LXP_FATAL_INVARIANT ||
        lxp_legacy_stream_close(&reader) != LXP_OK ||
        lxp_legacy_stream_open("http://legacy/activities", &reader) !=
            LXP_ERR_NON_CANONICAL)
        return 1;
    descriptor = open(path, O_RDONLY);
    count = descriptor < 0 ? -1 : read(descriptor, unchanged, sizeof(unchanged));
    if (descriptor >= 0) (void)close(descriptor);
    if (count != (ssize_t)sizeof(record) ||
        memcmp(unchanged, record, sizeof(record)) != 0 ||
        unlink(path) != 0)
        return 1;
    return 0;
}
