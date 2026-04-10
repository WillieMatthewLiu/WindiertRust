#include <ntddk.h>
#include <wdf.h>

#include "FileContext.h"
#include "Trace.h"
#include "..\wd_ntstatus_mapping.h"

#define WD_IOCTL_OPEN 0x80002000u
#define WD_IOCTL_RECV 0x80002004u

EVT_WDF_DEVICE_FILE_CREATE WdEvtFileCreate;
EVT_WDF_FILE_CLEANUP WdEvtFileCleanup;
EVT_WDF_FILE_CLOSE WdEvtFileClose;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL WdEvtIoDeviceControl;

NTSTATUS
WdConfigureDefaultQueue(_In_ WDFDEVICE Device)
{
    WDF_IO_QUEUE_CONFIG queueConfig;
    WDFQUEUE queue;

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchSequential);
    queueConfig.EvtIoDeviceControl = WdEvtIoDeviceControl;
    return WdfIoQueueCreate(Device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, &queue);
}

VOID
WdEvtFileCreate(
    _In_ WDFDEVICE Device,
    _In_ WDFREQUEST Request,
    _In_ WDFFILEOBJECT FileObject
)
{
    WD_FILE_CONTEXT* file_ctx = GetWdFileContext(FileObject);

    UNREFERENCED_PARAMETER(Device);

    WdInitializeFileContext(file_ctx);
    file_ctx->runtime = wd_runtime_glue_create(file_ctx->queue_capacity);
    if (file_ctx->runtime == NULL) {
        WdfRequestComplete(Request, STATUS_INSUFFICIENT_RESOURCES);
        return;
    }

    WdfRequestComplete(Request, STATUS_SUCCESS);
}

VOID
WdEvtFileCleanup(_In_ WDFFILEOBJECT FileObject)
{
    WD_FILE_CONTEXT* file_ctx = GetWdFileContext(FileObject);

    if (file_ctx->runtime != NULL) {
        wd_runtime_glue_destroy(file_ctx->runtime);
        WdResetFileContext(file_ctx);
    }
}

VOID
WdEvtFileClose(_In_ WDFFILEOBJECT FileObject)
{
    UNREFERENCED_PARAMETER(FileObject);
}

VOID
WdEvtIoDeviceControl(
    _In_ WDFQUEUE Queue,
    _In_ WDFREQUEST Request,
    _In_ size_t OutputBufferLength,
    _In_ size_t InputBufferLength,
    _In_ ULONG IoControlCode
)
{
    WD_FILE_CONTEXT* file_ctx;
    WDFFILEOBJECT fileObject;
    void* input_ptr = NULL;
    void* output_ptr = NULL;
    size_t output_len = 0;
    WD_GLUE_IO_RESULT result;
    NTSTATUS status;

    UNREFERENCED_PARAMETER(Queue);

    fileObject = WdfRequestGetFileObject(Request);
    if (fileObject == NULL) {
        WdfRequestComplete(Request, STATUS_INVALID_HANDLE);
        return;
    }

    file_ctx = GetWdFileContext(fileObject);
    if (file_ctx->runtime == NULL) {
        WdfRequestComplete(Request, STATUS_INVALID_DEVICE_STATE);
        return;
    }

    if (InputBufferLength > 0) {
        status = WdfRequestRetrieveInputBuffer(Request, InputBufferLength, &input_ptr, NULL);
        if (!NT_SUCCESS(status)) {
            WdfRequestComplete(Request, status);
            return;
        }
    }

    if (OutputBufferLength > 0 || IoControlCode == WD_IOCTL_OPEN || IoControlCode == WD_IOCTL_RECV) {
        status = WdfRequestRetrieveOutputBuffer(Request, OutputBufferLength, &output_ptr, &output_len);
        if (!NT_SUCCESS(status)) {
            WdfRequestComplete(Request, status);
            return;
        }
    }

    result = wd_runtime_glue_device_control(
        file_ctx->runtime,
        IoControlCode,
        (const uint8_t*)input_ptr,
        InputBufferLength,
        (uint8_t*)output_ptr,
        output_len
    );

    status = WdMapGlueStatusToNtStatus(result.status);
    WdfRequestCompleteWithInformation(Request, status, result.bytes_written);
}
