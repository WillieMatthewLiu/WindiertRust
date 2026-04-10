#ifndef WD_NTSTATUS_MAPPING_H
#define WD_NTSTATUS_MAPPING_H

/*
 * Include ntddk.h / wdf.h before this header so NTSTATUS symbols are defined.
 */

#include "wd_kmdf_bridge.h"

static inline NTSTATUS
WdMapGlueStatusToNtStatus(uint32_t glue_status)
{
    switch (glue_status) {
    case WD_GLUE_IO_STATUS_SUCCESS:
        return STATUS_SUCCESS;
    case WD_GLUE_IO_STATUS_UNSUPPORTED_IOCTL:
        return STATUS_INVALID_DEVICE_REQUEST;
    case WD_GLUE_IO_STATUS_DECODE_OPEN:
        return STATUS_INVALID_PARAMETER;
    case WD_GLUE_IO_STATUS_OUTPUT_TOO_SMALL:
        return STATUS_BUFFER_TOO_SMALL;
    case WD_GLUE_IO_STATUS_QUEUE_EMPTY:
        return STATUS_NO_MORE_ENTRIES;
    case WD_GLUE_IO_STATUS_RECV_DISABLED:
    case WD_GLUE_IO_STATUS_SEND_DISABLED:
    case WD_GLUE_IO_STATUS_INVALID_STATE:
        return STATUS_INVALID_DEVICE_STATE;
    case WD_GLUE_IO_STATUS_NETWORK_RUNTIME:
        return STATUS_DATA_ERROR;
    case WD_GLUE_IO_STATUS_INVALID_POINTER:
        return STATUS_INVALID_USER_BUFFER;
    case WD_GLUE_IO_STATUS_INVALID_HANDLE:
        return STATUS_INVALID_HANDLE;
    case WD_GLUE_IO_STATUS_INVALID_LAYER:
        return STATUS_INVALID_PARAMETER;
    default:
        return STATUS_UNSUCCESSFUL;
    }
}

#endif /* WD_NTSTATUS_MAPPING_H */
