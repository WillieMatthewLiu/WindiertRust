#include "wd_kmdf_bridge.h"

/*
 * This file is a bridge template, not a production-ready KMDF implementation.
 * It shows how Windows glue code can forward request buffers into the exported
 * Rust C ABI without reimplementing protocol parsing or runtime state logic.
 */

WD_GLUE_IO_RESULT wd_bridge_dispatch_request(
    wd_runtime_glue_api_handle* handle,
    uint32_t ioctl,
    const uint8_t* input_ptr,
    size_t input_len,
    uint8_t* output_ptr,
    size_t output_len
) {
    return wd_runtime_glue_device_control(
        handle,
        ioctl,
        input_ptr,
        input_len,
        output_ptr,
        output_len
    );
}

WD_GLUE_IO_RESULT wd_bridge_queue_network_event(
    wd_runtime_glue_api_handle* handle,
    uint8_t layer_wire,
    uint64_t packet_id,
    const uint8_t* packet_ptr,
    size_t packet_len
) {
    return wd_runtime_glue_queue_network_event(
        handle,
        layer_wire,
        packet_id,
        packet_ptr,
        packet_len
    );
}

/*
 * Example KMDF shape, intentionally left as comments because this repository
 * does not yet include WDF headers or a live driver project:
 *
 *   VOID WdEvtIoDeviceControl(
 *       WDFQUEUE Queue,
 *       WDFREQUEST Request,
 *       size_t OutputBufferLength,
 *       size_t InputBufferLength,
 *       ULONG IoControlCode
 *   ) {
 *       void* input = NULL;
 *       void* output = NULL;
 *       WD_GLUE_IO_RESULT result;
 *       NTSTATUS status;
 *
 *       // Retrieve request buffers with WdfRequestRetrieveInputBuffer /
 *       // WdfRequestRetrieveOutputBuffer.
 *       // Call wd_bridge_dispatch_request(...)
 *       // Map result.status to an NTSTATUS policy.
 *       // Complete with WdfRequestCompleteWithInformation(Request, status, result.bytes_written).
 *   }
 */
