#include "layerx/lxp_build.h"

#ifndef LXP_BUILD_TARGET_TRIPLE
#define LXP_BUILD_TARGET_TRIPLE "unknown"
#endif

#ifndef LXP_BUILD_OPTIMISATION
#define LXP_BUILD_OPTIMISATION "unknown"
#endif

#ifndef LXP_BUILD_REVISION
#define LXP_BUILD_REVISION "unknown"
#endif

static const lxp_build_metadata build_info = {
    .compiler_id = __VERSION__,
    .target_triple = LXP_BUILD_TARGET_TRIPLE,
    .optimisation_level = LXP_BUILD_OPTIMISATION,
    .revision = LXP_BUILD_REVISION,
};

const lxp_build_metadata *lxp_build_info(void)
{
    return &build_info;
}

const char *lxp_build_target_triple(void)
{
    return build_info.target_triple;
}

const char *lxp_build_compiler_id(void)
{
    return build_info.compiler_id;
}
