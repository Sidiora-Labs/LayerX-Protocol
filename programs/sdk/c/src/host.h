#ifndef LAYERX_PROGRAM_HOST_H
#define LAYERX_PROGRAM_HOST_H

#include <stdint.h>

/*
 * The complete version-one host surface. These seven declarations are the only
 * imports a LayerX program may carry; the deterministic runtime refuses every
 * other module or name at validation time, so a clock, a socket, a thread or a
 * source of randomness cannot be reached even by accident.
 */
#define LXP_PROGRAM_IMPORT(function_name) \
    __attribute__((import_module("layerx_v1"), import_name(function_name)))

LXP_PROGRAM_IMPORT("storage_read")
extern int32_t lxp_program_host_storage_read(int32_t key_pointer,
                                             int32_t key_length,
                                             int32_t output_pointer,
                                             int32_t output_capacity);

LXP_PROGRAM_IMPORT("storage_write")
extern int32_t lxp_program_host_storage_write(int32_t key_pointer,
                                              int32_t key_length,
                                              int32_t value_pointer,
                                              int32_t value_length);

LXP_PROGRAM_IMPORT("storage_delete")
extern int32_t lxp_program_host_storage_delete(int32_t key_pointer,
                                               int32_t key_length);

LXP_PROGRAM_IMPORT("event_emit")
extern int32_t lxp_program_host_event_emit(int32_t topic_pointer,
                                           int32_t topic_length,
                                           int32_t data_pointer,
                                           int32_t data_length);

LXP_PROGRAM_IMPORT("program_call")
extern int32_t lxp_program_host_program_call(int32_t program_pointer,
                                             int32_t program_length,
                                             int32_t input_pointer,
                                             int32_t input_length,
                                             int32_t capabilities_pointer,
                                             int32_t capabilities_length);

LXP_PROGRAM_IMPORT("transfer_402")
extern int32_t lxp_program_host_transfer_402(int64_t amount_high,
                                             int64_t amount_low,
                                             int32_t asset_pointer,
                                             int32_t asset_length,
                                             int32_t recipient_pointer,
                                             int32_t recipient_length);

LXP_PROGRAM_IMPORT("receipt_read")
extern int32_t lxp_program_host_receipt_read(int32_t digest_pointer,
                                             int32_t digest_length,
                                             int32_t output_pointer,
                                             int32_t output_capacity);

#endif
