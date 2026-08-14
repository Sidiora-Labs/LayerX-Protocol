#include "layerx/lxp_activity.h"
#include "layerx/lxp_replay_fixture.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const uint32_t expected_activity_types[] = {
    UINT32_C(0x00010001), UINT32_C(0x00010002), UINT32_C(0x00010003),
    UINT32_C(0x00010004), UINT32_C(0x00010005), UINT32_C(0x00010006),
    UINT32_C(0x00010007), UINT32_C(0x00010008),
    UINT32_C(0x00020001), UINT32_C(0x00020002), UINT32_C(0x00020003),
    UINT32_C(0x00020004), UINT32_C(0x00020005), UINT32_C(0x00020006),
    UINT32_C(0x00020007),
    UINT32_C(0x00030001), UINT32_C(0x00030002), UINT32_C(0x00030003),
    UINT32_C(0x00030004), UINT32_C(0x00030005), UINT32_C(0x00030006),
    UINT32_C(0x00030007),
    UINT32_C(0x00040001), UINT32_C(0x00040002), UINT32_C(0x00040003),
    UINT32_C(0x00040004), UINT32_C(0x00040005), UINT32_C(0x00040006),
    UINT32_C(0x00040007),
    UINT32_C(0x00050001), UINT32_C(0x00050002), UINT32_C(0x00050003),
    UINT32_C(0x00050004), UINT32_C(0x00050005), UINT32_C(0x00050006),
    UINT32_C(0x00050007), UINT32_C(0x00050008), UINT32_C(0x00050009),
    UINT32_C(0x0005000a), UINT32_C(0x0005000b), UINT32_C(0x0005000c),
    UINT32_C(0x0005000d),
    UINT32_C(0x00060001), UINT32_C(0x00060002), UINT32_C(0x00060003),
    UINT32_C(0x00060004), UINT32_C(0x00060005), UINT32_C(0x00060006),
    UINT32_C(0x00060007), UINT32_C(0x00060008), UINT32_C(0x00060009),
    UINT32_C(0x0006000a), UINT32_C(0x0006000b)
};

static void print_digest(const uint8_t digest[32])
{
    static const char hex[] = "0123456789abcdef";
    size_t i;
    for (i = 0U; i < 32U; ++i) {
        (void)putchar(hex[digest[i] >> 4U]);
        (void)putchar(hex[digest[i] & 15U]);
    }
    (void)putchar('\n');
}

int main(void)
{
    uint8_t *storage = malloc(2U * 1024U * 1024U);
    lxp_arena arena;
    lxp_replay_fixture fixture;
    uint8_t digest[32];
    uint8_t terminal[32];
    uint64_t divergent = 0U;
    size_t i;
    lxp_result status;
    if (storage == NULL ||
        lxp_arena_init(&arena, storage, 2U * 1024U * 1024U) != LXP_OK ||
        lxp_replay_fixture_load("tests/vectors/replay_corpus.lxb", &arena,
                                &fixture) != LXP_OK ||
        fixture.record_count != sizeof(expected_activity_types) /
                                sizeof(expected_activity_types[0])) {
        free(storage);
        return 1;
    }
    for (i = 0U; i < fixture.record_count; ++i) {
        lxp_activity activity;
        if (lxp_activity_decode(fixture.records[i].canonical_activity.bytes,
                                fixture.records[i].canonical_activity.length,
                                &activity) != LXP_OK ||
            activity.activity_type != expected_activity_types[i]) {
            free(storage);
            return 1;
        }
    }
    status = lxp_replay_digest(&fixture, digest, terminal, &divergent);
    if (status != LXP_OK || divergent != 0U ||
        memcmp(digest, fixture.expected_digest, 32U) != 0 ||
        memcmp(terminal, fixture.expected_terminal_root, 32U) != 0) {
        (void)fprintf(stderr, "first divergent global sequence: %llu\n",
                      (unsigned long long)divergent);
        free(storage);
        return 1;
    }
    ((uint8_t *)fixture.records[17].expected_receipt.bytes)[0] ^= 1U;
    status = lxp_replay_digest(&fixture, digest, terminal, &divergent);
    ((uint8_t *)fixture.records[17].expected_receipt.bytes)[0] ^= 1U;
    if (status != LXP_FATAL_REPLAY_DIVERGENCE || divergent != 18U ||
        lxp_replay_crossarch_case("tests/vectors/replay_corpus.lxb", &arena,
                                  digest, &divergent) != LXP_OK ||
        divergent != 0U) {
        (void)fprintf(stderr, "first divergent global sequence: %llu\n",
                      (unsigned long long)divergent);
        free(storage);
        return 1;
    }
    print_digest(digest);
    free(storage);
    return 0;
}
