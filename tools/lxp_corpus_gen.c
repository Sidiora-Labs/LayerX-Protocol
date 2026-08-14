#include "layerx/lxp_qualification.h"

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int parse_u64(const char *text, uint64_t *value)
{
    char *end = NULL;
    unsigned long long parsed;
    errno = 0;
    parsed = strtoull(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0') return -1;
    *value = (uint64_t)parsed;
    return (unsigned long long)*value == parsed ? 0 : -1;
}

static int parse_u32(const char *text, uint32_t *value)
{
    uint64_t parsed;
    if (parse_u64(text, &parsed) != 0 || parsed > UINT32_MAX) return -1;
    *value = (uint32_t)parsed;
    return 0;
}

int main(int argc, char **argv)
{
    uint64_t activity_count = LXP_QUAL_MIN_ACTIVITY_COUNT;
    uint32_t batch_size = UINT32_C(10000);
    lxp_result status;
    if (argc != 3 && argc != 5) {
        (void)fprintf(stderr,
            "usage: %s CORPUS ROOT_LEDGER [ACTIVITY_COUNT BATCH_SIZE]\n",
            argv[0]);
        return 2;
    }
    if (argc == 5 &&
        (parse_u64(argv[3], &activity_count) != 0 ||
         parse_u32(argv[4], &batch_size) != 0)) {
        (void)fprintf(stderr, "invalid corpus dimensions\n");
        return 2;
    }
    status = lxp_qual_corpus_generate(argv[1], argv[2], activity_count,
                                      batch_size);
    if (status != LXP_OK) {
        (void)fprintf(stderr, "corpus generation failed: %d\n", status);
        return 1;
    }
    (void)printf("generated=%llu batch_size=%u\n",
                 (unsigned long long)activity_count, batch_size);
    return 0;
}
