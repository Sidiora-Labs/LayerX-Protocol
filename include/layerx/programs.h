#ifndef LAYERX_PROGRAMS_H
#define LAYERX_PROGRAMS_H

#include "layerx/lxp_module.h"

#include <stdint.h>

enum {
    LX_PROGRAMS_DEPLOY = 0x00090001,
    LX_PROGRAMS_UPGRADE = 0x00090002,
    LX_PROGRAMS_CALL = 0x00090003,
    LX_PROGRAMS_REGISTRY = 0x00090004,
    LX_PROGRAMS_ABI_VERSION = 1,
    LX_PROGRAMS_EVENT_DEPLOYED = 1,
    LX_PROGRAMS_EVENT_UPGRADED = 2,
    LX_PROGRAMS_EVENT_CALLED = 3,
    LX_PROGRAMS_EVENT_REGISTRY_READ = 4
};

const lxp_module_iface *programs_module_registration(void);
const lxp_module_iface *lx_programs_module_iface(void);

#endif
