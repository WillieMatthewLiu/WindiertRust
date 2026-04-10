#ifndef WD_KMDF_BRIDGE_H
#define WD_KMDF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct wd_runtime_glue_api_handle wd_runtime_glue_api_handle;

typedef enum WD_GLUE_IO_STATUS {
    WD_GLUE_IO_STATUS_SUCCESS = 0,
    WD_GLUE_IO_STATUS_UNSUPPORTED_IOCTL = 1,
    WD_GLUE_IO_STATUS_DECODE_OPEN = 2,
    WD_GLUE_IO_STATUS_OUTPUT_TOO_SMALL = 3,
    WD_GLUE_IO_STATUS_QUEUE_EMPTY = 4,
    WD_GLUE_IO_STATUS_RECV_DISABLED = 5,
    WD_GLUE_IO_STATUS_SEND_DISABLED = 6,
    WD_GLUE_IO_STATUS_INVALID_STATE = 7,
    WD_GLUE_IO_STATUS_NETWORK_RUNTIME = 8,
    WD_GLUE_IO_STATUS_INVALID_POINTER = 9,
    WD_GLUE_IO_STATUS_INVALID_HANDLE = 10,
    WD_GLUE_IO_STATUS_INVALID_LAYER = 11
} WD_GLUE_IO_STATUS;

typedef struct WD_GLUE_IO_RESULT {
    uint32_t status;
    uint32_t bytes_written;
} WD_GLUE_IO_RESULT;

wd_runtime_glue_api_handle* wd_runtime_glue_create(size_t queue_capacity);
void wd_runtime_glue_destroy(wd_runtime_glue_api_handle* handle);

WD_GLUE_IO_RESULT wd_runtime_glue_device_control(
    wd_runtime_glue_api_handle* handle,
    uint32_t ioctl,
    const uint8_t* input_ptr,
    size_t input_len,
    uint8_t* output_ptr,
    size_t output_len
);

WD_GLUE_IO_RESULT wd_runtime_glue_queue_network_event(
    wd_runtime_glue_api_handle* handle,
    uint8_t layer_wire,
    uint64_t packet_id,
    const uint8_t* packet_ptr,
    size_t packet_len
);

/*
 * Template helper for a future KMDF EvtIoDeviceControl bridge:
 *
 * 1. Retrieve input/output request buffers from WDFREQUEST.
 * 2. Call wd_runtime_glue_device_control(...).
 * 3. Translate result.status to an NTSTATUS policy local to the glue layer.
 * 4. Complete the request with result.bytes_written.
 *
 * This header intentionally keeps the ABI thin and C-friendly.
 */

#ifdef __cplusplus
}
#endif

#endif /* WD_KMDF_BRIDGE_H */
