# KMDF Skeleton

This directory contains a real Visual Studio solution and project skeleton for
the future KMDF glue layer.

## What This Builds Today

Today this project is intentionally a compile-only skeleton:

- `MSBuild` loads `wd_kmdf_skeleton.sln`
- `wd_kmdf_skeleton.vcxproj` runs `build_kmdf_skeleton.ps1`
- the build script compiles `Driver.c`, `Device.c`, and `Queue.c`
- the build script archives the objects into `build\x64\Debug\wd_kmdf_skeleton.lib`

This proves the KMDF-facing C sources are syntactically valid against local
WDK headers and can be built as a coherent project.

## Why It Stops At A Static Library

The Rust bridge implementation in `driver/wd-kmdf` currently uses `std` and is
not yet a kernel-linkable `no_std` runtime component. Because of that, this
project does not yet attempt to link a final `.sys` image against the Rust
implementation.

That limitation is explicit and intentional. The repository now has a
`driver/wd-kmdf-core` crate for `no_std` bridge types plus fixed-capacity
runtime containers, and this skeleton is the handoff point before the remaining
byte-ownership and diagnostic pieces are moved off `std`.

## Files

- `Driver.c`: `DriverEntry` and driver cleanup
- `Device.c`: `EvtDriverDeviceAdd` and file-object wiring
- `Queue.c`: file lifecycle plus `EvtIoDeviceControl`
- `FileContext.h`: per-file runtime-handle contract
- `Trace.h`: placeholder trace macros
- `build_kmdf_skeleton.ps1`: compile-only build script
- `verify_kmdf_skeleton_build.ps1`: `MSBuild` verification wrapper

## Build

```powershell
powershell -ExecutionPolicy Bypass -File .\build_kmdf_skeleton.ps1
```

or:

```powershell
powershell -ExecutionPolicy Bypass -File .\verify_kmdf_skeleton_build.ps1
```
