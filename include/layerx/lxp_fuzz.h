#ifndef LAYERX_LXP_FUZZ_H
#define LAYERX_LXP_FUZZ_H

#include "layerx/lxp_activity.h"

#include <stddef.h>
#include <stdint.h>

lxp_result lxp_fuzz_activity_decode(const uint8_t *data, size_t size,
                                    lxp_result *decode_result);
lxp_result lxp_fuzz_signature_mutate(const lxp_activity *signed_activity);
lxp_result lxp_fuzz_transfer_set(const uint8_t *data, size_t size);
lxp_result lxp_fuzz_corpus_seed(const char *qualification_corpus,
                                const char *seed_directory);

#endif
