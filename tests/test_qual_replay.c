#include "layerx/lxp_qualification.h"

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static void print_hex(const char *name, const uint8_t digest[32])
{
    static const char hex[] = "0123456789abcdef";
    size_t i;
    (void)printf("%s=", name);
    for (i = 0U; i < 32U; ++i) {
        (void)putchar(hex[digest[i] >> 4U]);
        (void)putchar(hex[digest[i] & 15U]);
    }
    (void)putchar('\n');
}

int main(int argc, char **argv)
{
    lxp_qual_replay_result result;
    lxp_result status;
    int allow_small = argc == 4 && strcmp(argv[3], "--allow-small") == 0;
    if (argc != 3 && !allow_small) {
        (void)fprintf(stderr,
                      "usage: %s CORPUS ROOT_LEDGER [--allow-small]\n",
                      argv[0]);
        return 2;
    }
    status = lxp_qual_replay_matrix(argv[1], argv[2], &result);
    if (status != LXP_OK) {
        (void)fprintf(stderr,
            "replay qualification failed: status=%d sequence=%llu\n",
            status, (unsigned long long)result.first_divergent_sequence);
        return 1;
    }
    if (!allow_small && result.activity_count < LXP_QUAL_MIN_ACTIVITY_COUNT) {
        (void)fprintf(stderr, "qualification corpus is below ten million activities\n");
        return 1;
    }
    (void)printf("activities=%llu\n",
                 (unsigned long long)result.activity_count);
    (void)printf("batches=%llu\n", (unsigned long long)result.batch_count);
    print_hex("activity", result.activity_digest);
    print_hex("receipt", result.receipt_digest);
    print_hex("event", result.event_digest);
    print_hex("batch", result.batch_digest);
    print_hex("ledger", result.root_ledger_digest);
    print_hex("terminal", result.terminal_root);
    print_hex("corpus", result.corpus_digest);
    return 0;
}
