#include "layerx/program.h"

#include "host.h"
#include "internal.h"

lxp_program_status lxp_program_event_emit(const uint8_t *topic,
                                          size_t topic_length,
                                          const uint8_t *data,
                                          size_t data_length)
{
    if (topic == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (topic_length == 0U) return LXP_PROGRAM_ERR_EMPTY_TOPIC;
    if (topic_length > (size_t)LXP_PROGRAM_MAX_EVENT_TOPIC_BYTES)
        return LXP_PROGRAM_ERR_TOPIC_TOO_LARGE;
    if (data_length > 0U && data == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (data_length > (size_t)LXP_PROGRAM_MAX_EVENT_DATA_BYTES)
        return LXP_PROGRAM_ERR_DATA_TOO_LARGE;
    return lxp_program_host_event_emit(
        lxp_program_pointer(topic), lxp_program_length(topic_length),
        lxp_program_pointer(data), lxp_program_length(data_length));
}
