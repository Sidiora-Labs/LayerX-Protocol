#include "layerx/lxp_kernel.h"

#include <stdio.h>
#include <stdint.h>
#include <string.h>

enum { GOLDEN_RECORDS = 3 };

static lxp_result genesis(lxp_module_ctx *ctx, const uint8_t *bytes, size_t n)
{ (void)ctx; (void)bytes; (void)n; return LXP_OK; }
static lxp_result decode(lxp_module_ctx *ctx, uint16_t ordinal,
                         const uint8_t *bytes, size_t n, void **decoded)
{ (void)ctx; (void)ordinal; (void)bytes; (void)n; *decoded = NULL; return LXP_OK; }
static lxp_result validate(lxp_module_ctx *ctx, const lxp_activity *activity,
                           const lxp_authority_resolved *authority,
                           const void *decoded)
{ (void)ctx; (void)activity; (void)authority; (void)decoded; return LXP_OK; }
static lxp_result execute(lxp_module_ctx *ctx, const lxp_activity *activity,
                          const lxp_authority_resolved *authority,
                          const void *decoded, lxp_effect_buffer *effects)
{ (void)ctx; (void)activity; (void)authority; (void)decoded; (void)effects;
  return LXP_OK; }
static lxp_result epoch(lxp_module_ctx *ctx, uint64_t number, uint64_t timestamp)
{ (void)ctx; (void)number; (void)timestamp; return LXP_OK; }
static lxp_result module_root(lxp_module_ctx *ctx, uint8_t root[32])
{ (void)ctx; (void)memset(root, 0, 32U); return LXP_OK; }

static int hex_nibble(char value)
{
    if (value >= '0' && value <= '9') return value - '0';
    if (value >= 'a' && value <= 'f') return value - 'a' + 10;
    if (value >= 'A' && value <= 'F') return value - 'A' + 10;
    return -1;
}

static int hex_decode(const char *text, uint8_t *bytes, size_t length)
{
    size_t i;
    for (i = 0U; i < length; ++i) {
        int high = hex_nibble(text[i * 2U]);
        int low = hex_nibble(text[i * 2U + 1U]);
        if (high < 0 || low < 0) return 1;
        bytes[i] = (uint8_t)((unsigned int)high * 16U + (unsigned int)low);
    }
    return text[length * 2U] == '\0' ? 0 : 1;
}

static void hex_print(const uint8_t bytes[32])
{
    static const char digits[] = "0123456789abcdef";
    size_t i;
    for (i = 0U; i < 32U; ++i) {
        (void)putchar(digits[bytes[i] >> 4U]);
        (void)putchar(digits[bytes[i] & 15U]);
    }
}

static int load(lxp_replay_record records[GOLDEN_RECORDS],
                uint8_t roots[GOLDEN_RECORDS][32])
{
    FILE *file = fopen("tests/replay/golden/history.lxl", "rb");
    char header[32];
    size_t i;
    if (file == NULL || fgets(header, sizeof(header), file) == NULL ||
        strcmp(header, "LXP-GOLDEN-1\n") != 0) {
        if (file != NULL) (void)fclose(file);
        return 1;
    }
    for (i = 0U; i < GOLDEN_RECORDS; ++i) {
        unsigned int module;
        char key[3];
        char value[3];
        char root[65];
        if (fscanf(file, "%u %2s %2s %64s", &module, key, value, root) != 4 ||
            module > UINT16_MAX || hex_decode(key, records[i].key, 1U) != 0 ||
            hex_decode(value, records[i].value, 1U) != 0 ||
            hex_decode(root, roots[i], 32U) != 0) {
            (void)fclose(file);
            return 1;
        }
        records[i].module_id = (uint16_t)module;
        records[i].key_length = 1U;
        records[i].value_length = 1U;
    }
    if (fclose(file) != 0) return 1;
    return 0;
}

static int setup(lxp_kernel *kernel, lxp_state_store *store,
                 lxp_state_journal *journal)
{
    static const uint32_t types[] = { UINT32_C(0x00010001) };
    static const lxp_module_iface iface = { 1U, 1U, "asset", types, 1U,
        genesis, decode, validate, execute, epoch, epoch, module_root, NULL };
    static uint64_t parameters = 1U;
    return lxp_state_store_init(store, 0U) != LXP_OK ||
           lxp_kernel_create(kernel, store, journal, &parameters, 0U) != LXP_OK ||
           lxp_kernel_register_module(kernel, &iface) != LXP_OK;
}

int main(int argc, char **argv)
{
    static lxp_kernel first_kernel;
    static lxp_kernel second_kernel;
    static lxp_state_store first_store;
    static lxp_state_store second_store;
    static lxp_state_journal first_journal;
    static lxp_state_journal second_journal;
    lxp_replay_record records[GOLDEN_RECORDS] = { 0 };
    uint8_t expected[GOLDEN_RECORDS][32];
    uint8_t produced[GOLDEN_RECORDS][32];
    uint8_t terminal_first[32];
    uint8_t terminal_second[32];
    uint8_t digest_first[32];
    uint8_t digest_second[32];
    size_t i;
    bool record = argc == 2 && strcmp(argv[1], "--record") == 0;
    if (load(records, expected) != 0 ||
        setup(&first_kernel, &first_store, &first_journal) != 0) return 1;
    for (i = 0U; i < GOLDEN_RECORDS; ++i) {
        if (lxp_kernel_replay(&first_kernel, &records[i], NULL, 1U, 0U,
                              produced[i]) != LXP_OK) return 1;
    }
    if (record) {
        for (i = 0U; i < GOLDEN_RECORDS; ++i) {
            (void)printf("%u %02x %02x ", records[i].module_id,
                         records[i].key[0], records[i].value[0]);
            hex_print(produced[i]);
            (void)putchar('\n');
        }
        return lxp_state_store_destroy(&first_store) == LXP_OK ? 0 : 1;
    }
    for (i = 0U; i < GOLDEN_RECORDS; ++i)
        if (lxp_replay_compare_roots(expected[i], produced[i]) != LXP_OK)
            return 1;
    if (setup(&second_kernel, &second_store, &second_journal) != 0 ||
        lxp_kernel_replay(&second_kernel, records,
                          (const uint8_t (*)[32])expected, GOLDEN_RECORDS, 8U,
                          terminal_second) != LXP_OK) return 1;
    (void)memcpy(terminal_first, produced[GOLDEN_RECORDS - 1U], 32U);
    if (lxp_replay_golden_run(records, GOLDEN_RECORDS,
                              (const uint8_t (*)[32])produced, 0U,
                              digest_first) != LXP_OK ||
        lxp_replay_golden_run(records, GOLDEN_RECORDS,
                              (const uint8_t (*)[32])expected, 8U,
                              digest_second) != LXP_OK ||
        memcmp(terminal_first, terminal_second, 32U) != 0 ||
        memcmp(digest_first, digest_second, 32U) != 0 ||
        lxp_determinism_guard_check() != LXP_OK ||
        lxp_determinism_guard_trip("clock_gettime") != LXP_FATAL_INVARIANT ||
        lxp_determinism_guard_check() != LXP_FATAL_INVARIANT) return 1;
    lxp_determinism_guard_reset();
    if (lxp_determinism_guard_check() != LXP_OK ||
        lxp_state_store_destroy(&first_store) != LXP_OK ||
        lxp_state_store_destroy(&second_store) != LXP_OK) return 1;
    return 0;
}
