Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $scriptRoot "out"

if (-not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}
if (-not (Test-Path -LiteralPath (Join-Path $outDir "host-smoke"))) {
    New-Item -ItemType Directory -Path (Join-Path $outDir "host-smoke") | Out-Null
}
if (-not (Test-Path -LiteralPath (Join-Path $outDir "host-smoke\\src"))) {
    New-Item -ItemType Directory -Path (Join-Path $outDir "host-smoke\\src") | Out-Null
}
if (-not (Test-Path -LiteralPath (Join-Path $outDir "kmdf-skeleton"))) {
    New-Item -ItemType Directory -Path (Join-Path $outDir "kmdf-skeleton") | Out-Null
}

Copy-Item -LiteralPath (Join-Path $scriptRoot "wd-rust-x64.inf") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd-rust-x86.inf") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_kmdf_bridge.h") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_kmdf_bridge.c") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_kmdf_evtio_template.c") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_driver_entry_template.c") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_device_add_template.c") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_file_context_template.h") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_queue_template.c") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_ntstatus_mapping.h") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd_runtime_host_smoke.c") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "build_host_smoke.ps1") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "verify_host_smoke_build.ps1") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "host-smoke\\Cargo.toml") -Destination (Join-Path $outDir "host-smoke") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "host-smoke\\Cargo.lock") -Destination (Join-Path $outDir "host-smoke") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "host-smoke\\build.rs") -Destination (Join-Path $outDir "host-smoke") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "host-smoke\\src\\main.rs") -Destination (Join-Path $outDir "host-smoke\\src") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "verify_kmdf_skeleton_assets.ps1") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\FileContext.h") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\Trace.h") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\Driver.c") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\Device.c") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\Queue.c") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\README.md") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\build_kmdf_skeleton.ps1") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\verify_kmdf_skeleton_build.ps1") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\wd_kmdf_skeleton.sln") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\wd_kmdf_skeleton.vcxproj") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "kmdf-skeleton\\wd_kmdf_skeleton.vcxproj.filters") -Destination (Join-Path $outDir "kmdf-skeleton") -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "KMDF-bridge-notes.md") -Destination $outDir -Force

Write-Host "Driver glue artifacts staged in: $outDir"
