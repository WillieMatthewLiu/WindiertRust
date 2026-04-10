#include <ntddk.h>
#include <wdf.h>

#include "wd_file_context_template.h"

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD WdEvtDriverDeviceAdd;
EVT_WDF_OBJECT_CONTEXT_CLEANUP WdEvtDriverContextCleanup;

VOID
WdEvtDriverContextCleanup(_In_ WDFOBJECT DriverObject)
{
    UNREFERENCED_PARAMETER(DriverObject);
}

NTSTATUS
DriverEntry(
    _In_ PDRIVER_OBJECT DriverObject,
    _In_ PUNICODE_STRING RegistryPath
)
{
    WDF_DRIVER_CONFIG config;
    WDF_OBJECT_ATTRIBUTES attributes;

    WDF_OBJECT_ATTRIBUTES_INIT(&attributes);
    attributes.EvtCleanupCallback = WdEvtDriverContextCleanup;

    WDF_DRIVER_CONFIG_INIT(&config, WdEvtDriverDeviceAdd);
    return WdfDriverCreate(DriverObject, RegistryPath, &attributes, &config, WDF_NO_HANDLE);
}
