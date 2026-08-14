#include "layerx/lxp_result.h"

#include <stddef.h>
#include <stdio.h>
#include <string.h>

typedef struct result_case {
    const char *name;
    lxp_result value;
} result_case;

#define LXP_RESULT_TEST_CASE(name, value) { #name, name },
static const result_case cases[] = {
    LXP_RESULT_CODE_LIST(LXP_RESULT_TEST_CASE)
};
#undef LXP_RESULT_TEST_CASE

static int domain_matches_value(lxp_result value, lxp_result_domain_id domain)
{
    if (value == 0) return domain == LXP_RESULT_DOMAIN_SUCCESS;
    if (value <= -1000) return domain == LXP_RESULT_DOMAIN_FATAL;
    if (value <= -900) return domain == LXP_RESULT_DOMAIN_STORAGE;
    if (value <= -800) return domain == LXP_RESULT_DOMAIN_BATCH;
    if (value <= -700) return domain == LXP_RESULT_DOMAIN_MODULE;
    if (value <= -600) return domain == LXP_RESULT_DOMAIN_METERING;
    if (value <= -500) return domain == LXP_RESULT_DOMAIN_ARITHMETIC;
    if (value <= -400) return domain == LXP_RESULT_DOMAIN_LEDGER;
    if (value <= -300) return domain == LXP_RESULT_DOMAIN_SEQUENCING;
    if (value <= -200) return domain == LXP_RESULT_DOMAIN_AUTHORITY;
    if (value <= -100) return domain == LXP_RESULT_DOMAIN_ENVELOPE;
    return domain == LXP_RESULT_DOMAIN_CODEC;
}

int main(void)
{
    size_t i;
    size_t j;

    for (i = 0U; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        if (strcmp(lxp_result_name(cases[i].value), cases[i].name) != 0) {
            fprintf(stderr, "missing result name for %d\n", cases[i].value);
            return 1;
        }
        if (!domain_matches_value(cases[i].value,
                                  lxp_result_domain(cases[i].value))) {
            fprintf(stderr, "wrong result domain for %s\n", cases[i].name);
            return 1;
        }
        if ((cases[i].value <= -1000) != lxp_result_is_fatal(cases[i].value)) {
            fprintf(stderr, "wrong fatal classification for %s\n", cases[i].name);
            return 1;
        }
        for (j = i + 1U; j < sizeof(cases) / sizeof(cases[0]); ++j) {
            if (cases[i].value == cases[j].value) {
                fprintf(stderr, "duplicate result values: %s and %s\n",
                        cases[i].name, cases[j].name);
                return 1;
            }
        }
    }
    if (strcmp(lxp_result_name(1), "LXP_ERR_UNKNOWN") != 0 ||
        lxp_result_domain(1) != LXP_RESULT_DOMAIN_UNKNOWN) {
        return 1;
    }
    return 0;
}
