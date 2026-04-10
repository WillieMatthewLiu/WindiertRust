# KMDF Bridge Notes

This note documents how a future Windows KMDF bridge should call the exported
Rust C ABI without reimplementing runtime protocol logic in C.

## Bridge Shape

Recommended layering:

1. KMDF request callback retrieves raw request buffers.
2. Glue code forwards them into `wd_runtime_glue_device_control(...)`.
3. Glue code maps `WD_GLUE_IO_STATUS` to local `NTSTATUS`.
4. Glue code completes the request with `bytes_written`.

Recommended ownership:

- Keep one `wd_runtime_glue_api_handle*` per file-object or per runtime handle.
- Create it on open/create path.
- Destroy it on cleanup/close path.
- Do not share one handle globally across unrelated callers unless the bridge
  also serializes access and consciously shares queue/state.

## Recommended Status Mapping

These are recommended defaults for a phase-one bridge. A production driver may
choose a stricter policy, but the mapping should remain stable and explicit.

| `WD_GLUE_IO_STATUS` | Recommended `NTSTATUS` | Why |
| --- | --- | --- |
| `SUCCESS` | `STATUS_SUCCESS` | Operation completed normally |
| `UNSUPPORTED_IOCTL` | `STATUS_INVALID_DEVICE_REQUEST` | Unknown control code |
| `DECODE_OPEN` | `STATUS_INVALID_PARAMETER` | Open request buffer shape/version invalid |
| `OUTPUT_TOO_SMALL` | `STATUS_BUFFER_TOO_SMALL` | Caller-provided output buffer insufficient |
| `QUEUE_EMPTY` | `STATUS_NO_MORE_ENTRIES` | No queued runtime event available |
| `RECV_DISABLED` | `STATUS_INVALID_DEVICE_STATE` | Receive path intentionally shut down |
| `SEND_DISABLED` | `STATUS_INVALID_DEVICE_STATE` | Send path intentionally shut down |
| `INVALID_STATE` | `STATUS_INVALID_DEVICE_STATE` | Runtime handle lifecycle order violated |
| `NETWORK_RUNTIME` | `STATUS_DATA_ERROR` | Runtime packet/token/layer validation failed |
| `INVALID_POINTER` | `STATUS_INVALID_USER_BUFFER` | Null or invalid raw buffer pointer |
| `INVALID_HANDLE` | `STATUS_INVALID_HANDLE` | Bridge handle pointer invalid |
| `INVALID_LAYER` | `STATUS_INVALID_PARAMETER` | Unsupported layer byte |

## Buffer Retrieval Policy

Suggested KMDF policy per IOCTL:

- `IOCTL_OPEN`
  - retrieve input buffer
  - retrieve output buffer
  - call `wd_runtime_glue_device_control`
- `IOCTL_RECV`
  - no input buffer required
  - retrieve output buffer
  - call `wd_runtime_glue_device_control`
- `IOCTL_SEND`
  - retrieve input buffer
  - output buffer may be omitted or zero-length
  - call `wd_runtime_glue_device_control`

If an IOCTL does not require one side of the buffer pair:

- pass `NULL + 0` for the unused pointer/length
- do not fabricate temporary heap buffers just to satisfy the ABI

## Suggested `EvtIoDeviceControl` Flow

```c
VOID WdEvtIoDeviceControl(
    WDFQUEUE Queue,
    WDFREQUEST Request,
    size_t OutputBufferLength,
    size_t InputBufferLength,
    ULONG IoControlCode
) {
    WD_FILE_CONTEXT* file_ctx = GetWdFileContext(WdfRequestGetFileObject(Request));
    void* input_ptr = NULL;
    void* output_ptr = NULL;
    WD_GLUE_IO_RESULT result;
    NTSTATUS status;

    // 1. Retrieve input/output buffers only when that IOCTL needs them.
    // 2. Forward pointers directly into wd_runtime_glue_device_control(...).
    // 3. Map result.status to NTSTATUS using the policy table above.
    // 4. Complete with WdfRequestCompleteWithInformation(Request, status, result.bytes_written).
}
```

## Handle Lifecycle

Recommended flow:

1. Create `wd_runtime_glue_api_handle*` during file/create path.
2. Store it in file context.
3. Route `IOCTL_OPEN`, `IOCTL_RECV`, `IOCTL_SEND` through that same handle.
4. On cleanup/close:
   - call `wd_runtime_glue_destroy(handle)`
   - null the stored pointer before releasing file context

This preserves the queue, reinjection table, and shutdown state per runtime
handle instead of silently mixing state across callers.

## What The Glue Must Not Do

- Do not parse `OpenRequest`, runtime event payloads, or send payloads in C.
- Do not duplicate reinjection token bookkeeping in C.
- Do not rewrite `bytes_written` semantics independently of the Rust bridge.
- Do not collapse all non-success statuses into a single generic `NTSTATUS`
  without at least logging the underlying `WD_GLUE_IO_STATUS`.

## Current Limitation

This repository now exposes a stable C ABI and a KMDF-oriented bridge template,
but it still does not contain a real WDF project with headers, callback
registration, or signed packaging. These notes are the handoff contract for
that future glue layer.
