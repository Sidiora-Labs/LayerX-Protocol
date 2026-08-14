#define _POSIX_C_SOURCE 200809L
#include "layerx/lxp_crypto.h"

#include <stdint.h>
#include <time.h>

static uint64_t elapsed_for_difference(size_t position)
{
    uint8_t left[256] = {0};
    uint8_t right[256] = {0};
    struct timespec start, end;
    volatile int sink = 0;
    size_t i;
    right[position] = 1U;
    if (clock_gettime(CLOCK_MONOTONIC, &start) != 0) return UINT64_MAX;
    for (i = 0U; i < 200000U; ++i)
        sink |= lxp_ct_memcmp(left, right, sizeof(left));
    if (clock_gettime(CLOCK_MONOTONIC, &end) != 0 || sink == 0) return UINT64_MAX;
    {
        int64_t seconds = (int64_t)end.tv_sec - (int64_t)start.tv_sec;
        int64_t nanoseconds = (int64_t)end.tv_nsec - (int64_t)start.tv_nsec;
        if (nanoseconds < 0) { --seconds; nanoseconds += INT64_C(1000000000); }
        if (seconds < 0) return UINT64_MAX;
        return (uint64_t)seconds * UINT64_C(1000000000) +
               (uint64_t)nanoseconds;
    }
}

int main(void)
{
    uint8_t left[32] = {0};
    uint8_t right[32] = {0};
    uint8_t secret[32];
    uint64_t first;
    uint64_t last;
    size_t i;
    if (lxp_ct_memcmp(left,right,sizeof(left))!=0 || !lxp_ct_is_zero(left,sizeof(left))) return 1;
    right[31]=1U;
    if (lxp_ct_memcmp(left,right,sizeof(left))==0 || lxp_ct_is_zero(right,sizeof(right))) return 1;
    for(i=0U;i<sizeof(secret);++i) secret[i]=(uint8_t)(i+1U);
    lxp_secure_zero(secret,sizeof(secret));
    if(!lxp_ct_is_zero(secret,sizeof(secret))) return 1;
    first=elapsed_for_difference(0U); last=elapsed_for_difference(255U);
    if(first==UINT64_MAX || last==UINT64_MAX) return 1;
    return first > last*8U || last > first*8U ? 1 : 0;
}
