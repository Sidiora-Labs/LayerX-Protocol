#ifndef LAYERX_LXP_BUILD_H
#define LAYERX_LXP_BUILD_H

typedef struct lxp_build_metadata {
    const char *compiler_id;
    const char *target_triple;
    const char *optimisation_level;
    const char *revision;
} lxp_build_metadata;

const lxp_build_metadata *lxp_build_info(void);
const char *lxp_build_target_triple(void);
const char *lxp_build_compiler_id(void);

#endif
