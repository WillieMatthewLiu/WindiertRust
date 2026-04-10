#include "wd_kmdf_bridge.h"

int wd_host_smoke_run(void)
{
    wd_runtime_glue_api_handle* handle;
    WD_GLUE_IO_RESULT control_result;
    WD_GLUE_IO_RESULT queue_result;

    handle = wd_runtime_glue_create(8);
    if (handle == 0) {
        return 10;
    }

    wd_runtime_glue_destroy(handle);

    control_result = wd_runtime_glue_device_control(0, 0, 0, 0, 0, 0);
    if (control_result.status != WD_GLUE_IO_STATUS_INVALID_HANDLE) {
        return 11;
    }
    if (control_result.bytes_written != 0) {
        return 12;
    }

    queue_result = wd_runtime_glue_queue_network_event(0, 0, 1, 0, 0);
    if (queue_result.status != WD_GLUE_IO_STATUS_INVALID_HANDLE) {
        return 13;
    }
    if (queue_result.bytes_written != 0) {
        return 14;
    }

    return 0;
}
