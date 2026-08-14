#include "layerx/lxp_daemon.h"

#include <string.h>

lxp_result lxp_daemon_main(int argc, char **argv)
{
    lxp_daemon_configuration configuration;
    if (argc != 3 || argv == NULL ||
        strcmp(argv[1], "--check-config") != 0)
        return LXP_ERR_NON_CANONICAL;
    return lxp_daemon_config_load(argv[2], &configuration);
}
