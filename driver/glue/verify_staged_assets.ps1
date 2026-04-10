Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $scriptRoot "out"

& (Join-Path $scriptRoot "build.ps1")

$requiredFiles = @(
    "wd-rust-x64.inf",
    "wd-rust-x86.inf",
    "wd_kmdf_bridge.h",
    "wd_kmdf_bridge.c",
    "wd_kmdf_evtio_template.c",
    "wd_driver_entry_template.c",
    "wd_device_add_template.c",
    "wd_file_context_template.h",
    "wd_queue_template.c",
    "wd_ntstatus_mapping.h",
    "wd_runtime_host_smoke.c",
    "build_host_smoke.ps1",
    "verify_host_smoke_build.ps1",
    "host-smoke\\Cargo.toml",
    "host-smoke\\Cargo.lock",
    "host-smoke\\build.rs",
    "host-smoke\\src\\main.rs",
    "verify_kmdf_skeleton_assets.ps1",
    "kmdf-skeleton\\wd_kmdf_skeleton.sln",
    "kmdf-skeleton\\wd_kmdf_skeleton.vcxproj",
    "kmdf-skeleton\\wd_kmdf_skeleton.vcxproj.filters",
    "kmdf-skeleton\\Driver.c",
    "kmdf-skeleton\\Device.c",
    "kmdf-skeleton\\Queue.c",
    "kmdf-skeleton\\FileContext.h",
    "kmdf-skeleton\\Trace.h",
    "kmdf-skeleton\\README.md",
    "kmdf-skeleton\\build_kmdf_skeleton.ps1",
    "kmdf-skeleton\\verify_kmdf_skeleton_build.ps1",
    "KMDF-bridge-notes.md"
)

foreach ($name in $requiredFiles) {
    $path = Join-Path $outDir $name
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing staged glue asset: $name"
    }
}

Write-Host "Verified staged glue assets in: $outDir"
