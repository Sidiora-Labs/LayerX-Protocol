#include <stddef.h>
#include <stdint.h>

/*
 * A freestanding guest links no C library. The compiler may still lower an
 * aggregate copy or an array initialiser onto these four names, so the SDK
 * supplies them itself. Every one is a plain integer loop over linear memory:
 * no allocator, no locale, no errno and no ambient state of any kind.
 */

void *memcpy(void *destination, const void *source, size_t length);
void *memmove(void *destination, const void *source, size_t length);
void *memset(void *destination, int value, size_t length);
int memcmp(const void *left, const void *right, size_t length);

void *memcpy(void *destination, const void *source, size_t length)
{
    unsigned char *out = (unsigned char *)destination;
    const unsigned char *in = (const unsigned char *)source;
    size_t index;
    for (index = 0U; index < length; ++index) out[index] = in[index];
    return destination;
}

void *memmove(void *destination, const void *source, size_t length)
{
    unsigned char *out = (unsigned char *)destination;
    const unsigned char *in = (const unsigned char *)source;
    size_t index;
    if (out == in || length == 0U) return destination;
    if (out < in) {
        for (index = 0U; index < length; ++index) out[index] = in[index];
        return destination;
    }
    index = length;
    while (index > 0U) {
        --index;
        out[index] = in[index];
    }
    return destination;
}

void *memset(void *destination, int value, size_t length)
{
    unsigned char *out = (unsigned char *)destination;
    unsigned char byte = (unsigned char)((unsigned int)value & 0xFFU);
    size_t index;
    for (index = 0U; index < length; ++index) out[index] = byte;
    return destination;
}

int memcmp(const void *left, const void *right, size_t length)
{
    const unsigned char *first = (const unsigned char *)left;
    const unsigned char *second = (const unsigned char *)right;
    size_t index;
    for (index = 0U; index < length; ++index) {
        if (first[index] != second[index])
            return first[index] < second[index] ? -1 : 1;
    }
    return 0;
}
