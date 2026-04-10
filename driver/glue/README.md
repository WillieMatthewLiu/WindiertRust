# Driver Glue Placeholders

This directory hosts packaging placeholders for Windows host validation.

## Files

- `wd-rust-x64.inf`: x64 sample INF scaffold.
- `wd-rust-x86.inf`: x86 sample INF scaffold.
- `wd_kmdf_bridge.h`: C ABI bridge header for exported `wd_runtime_glue_*` symbols.
- `wd_kmdf_bridge.c`: minimal KMDF-style dispatch template that forwards raw request buffers into the Rust ABI.
- `wd_kmdf_evtio_template.c`: fuller `EvtIoDeviceControl` skeleton with recommended buffer retrieval and request completion flow.
- `wd_driver_entry_template.c`: `DriverEntry` and driver-level cleanup skeleton.
- `wd_device_add_template.c`: `EvtDriverDeviceAdd` template for device naming, file-object config, and queue wiring.
- `wd_file_context_template.h`: per-file runtime-handle ownership template.
- `wd_queue_template.c`: create/cleanup/close plus `EvtIoDeviceControl` bridge skeleton.
- `wd_ntstatus_mapping.h`: reusable `WD_GLUE_IO_STATUS -> NTSTATUS` mapping helper.
- `wd_runtime_host_smoke.c`: pure C smoke harness that exercises the exported bridge ABI.
- `build_host_smoke.ps1`: builds and runs the host smoke path.
- `verify_host_smoke_build.ps1`: verification wrapper around the host smoke build.
- `host-smoke/`: Cargo binary plus pinned `Cargo.lock` that compiles the C smoke file through `build.rs` and links it with the Rust bridge implementation.
- `verify_kmdf_skeleton_assets.ps1`: file-level verification for the KMDF solution skeleton.
- `kmdf-skeleton/`: Visual Studio solution plus compile-only KMDF project skeleton that validates the C side against local WDK headers.
- `KMDF-bridge-notes.md`: recommended `GlueIoStatus -> NTSTATUS` mapping plus `EvtIoDeviceControl` integration notes.
- `build.ps1`: copy helper to stage files under `driver/glue/out`.

## Notes

- These files are intentionally minimal handoff templates, not a production-ready signed driver package.
- The new template set is meant to be copied into a real KMDF project in this order:
  - start from `wd_driver_entry_template.c`
  - wire `wd_device_add_template.c`
  - adopt `wd_file_context_template.h`
  - finish request routing with `wd_queue_template.c` and `wd_ntstatus_mapping.h`
- Future Windows device-control glue should forward runtime `open/recv/send` semantics into
  `driver/wd-kmdf` Rust APIs instead of reimplementing protocol logic in C, PowerShell, or INF glue.
- Current handoff points:
  - `wd_kmdf::RuntimeGlueApi` for glue-facing status codes plus `bytes_written`
    - `device_control_raw(...)` is the thinnest raw-pointer entry point for future `extern "C"` or KMDF buffer bridges
  - exported C ABI bridge symbols:
    - `wd_runtime_glue_create`
    - `wd_runtime_glue_destroy`
    - `wd_runtime_glue_device_control`
    - `wd_runtime_glue_queue_network_event`
  - `wd_kmdf::RuntimeIoctlDispatcher` for `IOCTL_OPEN` / `IOCTL_RECV` / `IOCTL_SEND` byte-buffer dispatch
  - `wd_kmdf::RuntimeDevice` for open/queue/recv/send/shutdown lifecycle behind the dispatcher
  - `wd_kmdf::NetworkRuntime` for raw network event and reinjection request contract validation

## Template Boundaries

- The templates assume a buffered I/O KMDF queue and one runtime handle per file object.
- They intentionally keep IOCTL constants and device names visible in C so the handoff is auditable.
- They do not include WPP tracing, INF signing, service installation, or a live WDF project file.

## Minimal Compilable Path

This repository now has two minimal compilable paths.

### 1. Host ABI Smoke

This is the smallest end-to-end path in-tree for the exported Rust C ABI.

Files involved:

- `wd_runtime_host_smoke.c`: pure C smoke program that includes `wd_kmdf_bridge.h`
- `build_host_smoke.ps1`: builds `wd-kmdf` as a Rust `staticlib`, then compiles and links the smoke harness
- `verify_host_smoke_build.ps1`: verification wrapper for the build

What it proves:

- the exported `wd_runtime_glue_*` symbols are linkable from external C code
- the bridge header is C-consumable
- the ABI returns stable `WD_GLUE_IO_STATUS` values across the language boundary

What it does not prove:

- KMDF callback registration
- WDF request-buffer retrieval
- driver signing or installation
- live kernel attachment

### 2. KMDF Skeleton Project

This is the smallest real Visual Studio / KMDF-facing project skeleton in-tree.

Files involved:

- `kmdf-skeleton/wd_kmdf_skeleton.sln`
- `kmdf-skeleton/wd_kmdf_skeleton.vcxproj`
- `kmdf-skeleton/build_kmdf_skeleton.ps1`
- `kmdf-skeleton/verify_kmdf_skeleton_build.ps1`

What it proves:

- `MSBuild` can load the solution
- the KMDF-facing C sources compile against local WDK headers
- the project emits a compile-only `wd_kmdf_skeleton.lib`

What it does not prove:

- final `.sys` link
- live device attachment
- Rust kernel linkage

Current hard limitation:

- `driver/wd-kmdf-core` now carries the `no_std` bridge types plus fixed-capacity reinjection and byte-ring containers.
- `driver/wd-kmdf` still uses `std` in runtime frame ownership (`Vec<u8>`), IOCTL byte-buffer return paths, and filter-compile diagnostics.
