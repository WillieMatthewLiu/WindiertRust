#include "wd_kmdf_bridge.h"

/*
 * KMDF EvtIoDeviceControl template.
 *
 * This file is intentionally not buildable in the current repository because
 * there is no WDF project, no ntddk headers, and no signed driver pipeline
 * here yet. The goal is to make the intended bridge shape concrete enough that
 * a future KMDF project can wire the existing Rust C ABI in with minimal guesswork.
 */

/*
 * Suggested local policy:
 *
 *   WD_GLUE_IO_STATUS_SUCCESS              -> STATUS_SUCCESS
 *   WD_GLUE_IO_STATUS_UNSUPPORTED_IOCTL    -> STATUS_INVALID_DEVICE_REQUEST
 *   WD_GLUE_IO_STATUS_DECODE_OPEN          -> STATUS_INVALID_PARAMETER
 *   WD_GLUE_IO_STATUS_OUTPUT_TOO_SMALL     -> STATUS_BUFFER_TOO_SMALL
 *   WD_GLUE_IO_STATUS_QUEUE_EMPTY          -> STATUS_NO_MORE_ENTRIES
 *   WD_GLUE_IO_STATUS_RECV_DISABLED        -> STATUS_INVALID_DEVICE_STATE
 *   WD_GLUE_IO_STATUS_SEND_DISABLED        -> STATUS_INVALID_DEVICE_STATE
 *   WD_GLUE_IO_STATUS_INVALID_STATE        -> STATUS_INVALID_DEVICE_STATE
 *   WD_GLUE_IO_STATUS_NETWORK_RUNTIME      -> STATUS_DATA_ERROR
 *   WD_GLUE_IO_STATUS_INVALID_POINTER      -> STATUS_INVALID_USER_BUFFER
 *   WD_GLUE_IO_STATUS_INVALID_HANDLE       -> STATUS_INVALID_HANDLE
 *   WD_GLUE_IO_STATUS_INVALID_LAYER        -> STATUS_INVALID_PARAMETER
 */

/*
 * Example placeholders only:
 *
 * typedef struct _WD_FILE_CONTEXT {
 *     wd_runtime_glue_api_handle* runtime;
 * } WD_FILE_CONTEXT, *PWD_FILE_CONTEXT;
 *
 * WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(WD_FILE_CONTEXT, GetWdFileContext);
 */

/*
 * static NTSTATUS
 * WdMapGlueStatusToNtStatus(uint32_t glue_status)
 * {
 *     switch (glue_status) {
 *     case WD_GLUE_IO_STATUS_SUCCESS:
 *         return STATUS_SUCCESS;
 *     case WD_GLUE_IO_STATUS_UNSUPPORTED_IOCTL:
 *         return STATUS_INVALID_DEVICE_REQUEST;
 *     case WD_GLUE_IO_STATUS_DECODE_OPEN:
 *         return STATUS_INVALID_PARAMETER;
 *     case WD_GLUE_IO_STATUS_OUTPUT_TOO_SMALL:
 *         return STATUS_BUFFER_TOO_SMALL;
 *     case WD_GLUE_IO_STATUS_QUEUE_EMPTY:
 *         return STATUS_NO_MORE_ENTRIES;
 *     case WD_GLUE_IO_STATUS_RECV_DISABLED:
 *     case WD_GLUE_IO_STATUS_SEND_DISABLED:
 *     case WD_GLUE_IO_STATUS_INVALID_STATE:
 *         return STATUS_INVALID_DEVICE_STATE;
 *     case WD_GLUE_IO_STATUS_NETWORK_RUNTIME:
 *         return STATUS_DATA_ERROR;
 *     case WD_GLUE_IO_STATUS_INVALID_POINTER:
 *         return STATUS_INVALID_USER_BUFFER;
 *     case WD_GLUE_IO_STATUS_INVALID_HANDLE:
 *         return STATUS_INVALID_HANDLE;
 *     case WD_GLUE_IO_STATUS_INVALID_LAYER:
 *         return STATUS_INVALID_PARAMETER;
 *     default:
 *         return STATUS_UNSUCCESSFUL;
 *     }
 * }
 */

/*
 * VOID
 * WdEvtIoDeviceControl(
 *     WDFQUEUE Queue,
 *     WDFREQUEST Request,
 *     size_t OutputBufferLength,
 *     size_t InputBufferLength,
 *     ULONG IoControlCode
 * )
 * {
 *     WD_FILE_CONTEXT* file_ctx;
 *     void* input_ptr = NULL;
 *     void* output_ptr = NULL;
 *     size_t output_len = 0;
 *     WD_GLUE_IO_RESULT result;
 *     NTSTATUS status;
 *
 *     UNREFERENCED_PARAMETER(Queue);
 *
 *     file_ctx = GetWdFileContext(WdfRequestGetFileObject(Request));
 *
 *     if (InputBufferLength > 0) {
 *         status = WdfRequestRetrieveInputBuffer(Request, InputBufferLength, &input_ptr, NULL);
 *         if (!NT_SUCCESS(status)) {
 *             WdfRequestComplete(Request, status);
 *             return;
 *         }
 *     }
 *
 *     if (OutputBufferLength > 0 || IoControlCode == IOCTL_OPEN || IoControlCode == IOCTL_RECV) {
 *         status = WdfRequestRetrieveOutputBuffer(Request, OutputBufferLength, &output_ptr, &output_len);
 *         if (!NT_SUCCESS(status)) {
 *             WdfRequestComplete(Request, status);
 *             return;
 *         }
 *     }
 *
 *     result = wd_runtime_glue_device_control(
 *         file_ctx->runtime,
 *         IoControlCode,
 *         (const uint8_t*)input_ptr,
 *         InputBufferLength,
 *         (uint8_t*)output_ptr,
 *         output_len
 *     );
 *
 *     status = WdMapGlueStatusToNtStatus(result.status);
 *     WdfRequestCompleteWithInformation(Request, status, result.bytes_written);
 * }
 */

/*
 * Recommended adjacent lifecycle callbacks:
 *
 *  - On file create/open:
 *      file_ctx->runtime = wd_runtime_glue_create(queue_capacity);
 *
 *  - On cleanup/close:
 *      wd_runtime_glue_destroy(file_ctx->runtime);
 *      file_ctx->runtime = NULL;
 *
 *  - On packet/event source path:
 *      wd_runtime_glue_queue_network_event(...);
 */
