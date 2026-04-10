#include <ntddk.h>
#include <wdf.h>

#include "wd_file_context_template.h"

EVT_WDF_DEVICE_FILE_CREATE WdEvtFileCreate;
EVT_WDF_FILE_CLEANUP WdEvtFileCleanup;
EVT_WDF_FILE_CLOSE WdEvtFileClose;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL WdEvtIoDeviceControl;

NTSTATUS
WdConfigureDefaultQueue(_In_ WDFDEVICE Device);

NTSTATUS
WdEvtDriverDeviceAdd(
    _In_ WDFDRIVER Driver,
    _Inout_ PWDFDEVICE_INIT DeviceInit
)
{
    WDF_FILEOBJECT_CONFIG fileConfig;
    WDF_OBJECT_ATTRIBUTES fileAttributes;
    WDFDEVICE device;
    DECLARE_CONST_UNICODE_STRING(deviceName, L"\\Device\\WdRust");
    DECLARE_CONST_UNICODE_STRING(symbolicLinkName, L"\\DosDevices\\WdRust");
    NTSTATUS status;

    UNREFERENCED_PARAMETER(Driver);

    WdfDeviceInitSetIoType(DeviceInit, WdfDeviceIoBuffered);

    WDF_FILEOBJECT_CONFIG_INIT(
        &fileConfig,
        WdEvtFileCreate,
        WdEvtFileClose,
        WdEvtFileCleanup
    );

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&fileAttributes, WD_FILE_CONTEXT);
    WdfDeviceInitSetFileObjectConfig(DeviceInit, &fileConfig, &fileAttributes);

    status = WdfDeviceInitAssignName(DeviceInit, &deviceName);
    if (!NT_SUCCESS(status)) {
        return status;
    }

    status = WdfDeviceCreate(&DeviceInit, WDF_NO_OBJECT_ATTRIBUTES, &device);
    if (!NT_SUCCESS(status)) {
        return status;
    }

    status = WdfDeviceCreateSymbolicLink(device, &symbolicLinkName);
    if (!NT_SUCCESS(status)) {
        return status;
    }

    return WdConfigureDefaultQueue(device);
}
