Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$skeletonRoot = Join-Path $scriptRoot "kmdf-skeleton"

$requiredFiles = @(
    "wd_kmdf_skeleton.sln",
    "wd_kmdf_skeleton.vcxproj",
    "wd_kmdf_skeleton.vcxproj.filters",
    "Driver.c",
    "Device.c",
    "Queue.c",
    "FileContext.h",
    "Trace.h",
    "README.md",
    "build_kmdf_skeleton.ps1",
    "verify_kmdf_skeleton_build.ps1"
)

foreach ($name in $requiredFiles) {
    $path = Join-Path $skeletonRoot $name
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing KMDF skeleton asset: $name"
    }
}

Write-Host "Verified KMDF skeleton assets in: $skeletonRoot"
