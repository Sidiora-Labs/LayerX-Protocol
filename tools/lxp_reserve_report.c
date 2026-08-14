#include <stdint.h>
#include <stdio.h>
#include <string.h>

enum { REPORT_HEADER_SIZE = 274, REPORT_MAX_SIZE = 65536 };

static uint64_t load_u64(const uint8_t *bytes)
{
    uint64_t value = 0U;
    size_t i;
    for (i = 0U; i < 8U; ++i) value = (value << 8U) | bytes[i];
    return value;
}

static void print_hex(const uint8_t *bytes, size_t length)
{
    size_t i;
    for (i = 0U; i < length; ++i) (void)printf("%02x", bytes[i]);
}

int main(int argc, char **argv)
{
    static const char *classes[15] = {
        "agent_main", "escrow", "budget", "stream", "margin",
        "liquidity", "insurance", "fees", "withdrawals",
        "other_system", "reserve_mirror", "raw_total", "circulating",
        "effective_total", "expected_backing"
    };
    uint8_t bytes[REPORT_MAX_SIZE];
    size_t length;
    size_t i;
    uint16_t escrow_count;
    if (argc == 2 && strcmp(argv[1], "--classes") == 0) {
        for (i = 0U; i < 11U; ++i) (void)puts(classes[i]);
        return 0;
    }
    if (argc != 1) {
        (void)fprintf(stderr,
            "usage: lxp-reserve-report [--classes] < encoded-report\n");
        return 2;
    }
    length = fread(bytes, 1U, sizeof(bytes), stdin);
    if (length < REPORT_HEADER_SIZE || ferror(stdin) != 0) {
        (void)fprintf(stderr, "invalid reserve report\n");
        return 1;
    }
    escrow_count = (uint16_t)((uint16_t)bytes[272] << 8U) | bytes[273];
    if (length != REPORT_HEADER_SIZE + (size_t)escrow_count * 48U) {
        (void)fprintf(stderr, "non-canonical reserve report length\n");
        return 1;
    }
    (void)printf("{\"asset_id\":\"");
    print_hex(bytes, 32U);
    (void)printf("\",\"classes\":{");
    for (i = 0U; i < 15U; ++i) {
        const uint8_t *value = bytes + 32U + i * 16U;
        (void)printf("%s\"%s\":\"%llu:%llu\"",
            i == 0U ? "" : ",", classes[i],
            (unsigned long long)load_u64(value),
            (unsigned long long)load_u64(value + 8U));
    }
    (void)printf("},\"escrow_lines\":%u}\n", (unsigned)escrow_count);
    return 0;
}
