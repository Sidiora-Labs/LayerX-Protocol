#include "layerx/lxp_crypto.h"

int lxp_ct_memcmp(const void *left, const void *right, size_t length)
{
    const volatile uint8_t *a = (const volatile uint8_t *)left;
    const volatile uint8_t *b = (const volatile uint8_t *)right;
    uint8_t difference = 0U;
    size_t i;
    if ((left == NULL || right == NULL) && length != 0U) return 1;
    for (i = 0U; i < length; ++i) difference |= a[i] ^ b[i];
    return (int)difference;
}

bool lxp_ct_is_zero(const void *bytes, size_t length)
{
    static const uint8_t zero[256] = {0};
    const uint8_t *cursor = (const uint8_t *)bytes;
    uint8_t difference = 0U;
    if (bytes == NULL && length != 0U) return false;
    while (length != 0U) {
        size_t take = length > sizeof(zero) ? sizeof(zero) : length;
        difference |= (uint8_t)lxp_ct_memcmp(cursor, zero, take);
        cursor += take;
        length -= take;
    }
    return difference == 0U;
}

void lxp_secure_zero(void *bytes, size_t length)
{
    volatile uint8_t *cursor = (volatile uint8_t *)bytes;
    if (bytes == NULL) return;
    while (length-- != 0U) *cursor++ = 0U;
}
