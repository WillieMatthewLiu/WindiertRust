param(
    [string]$Configuration = "Debug",
    [string]$Platform = "x64"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$llvmRoot = "D:\0DProgramFiles\LLVM\bin"
$clang = Join-Path $llvmRoot "clang-cl.exe"
$llvmLib = Join-Path $llvmRoot "llvm-lib.exe"
$buildRoot = Join-Path $scriptRoot "build\$Platform\$Configuration"
$sdkRoot = "C:\Program Files (x86)\Windows Kits\10"
$sdkVersion = Get-ChildItem (Join-Path $sdkRoot "build") -Directory |
    Sort-Object { [version]$_.Name } -Descending |
    Select-Object -First 1 -ExpandProperty Name
$kmdfVersion = Get-ChildItem (Join-Path $sdkRoot "Include\wdf\kmdf") -Directory |
    Sort-Object { [version]$_.Name } -Descending |
    Select-Object -First 1 -ExpandProperty Name

if (-not (Test-Path -LiteralPath $buildRoot)) {
    New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null
}

$commonArgs = @(
    "/nologo",
    "/c",
    "/TC",
    "/W3",
    "/D_AMD64_=1",
    "/DWIN64=1",
    "/D_WIN64=1",
    "/DDBG=1",
    "/DNTDDI_VERSION=0x0A00000C",
    "/D_WIN32_WINNT=0x0A00",
    "/clang:-Wno-ignored-pragma-intrinsic",
    "/clang:-Wno-pragma-pack",
    "/clang:-Wno-visibility",
    "/clang:-Wno-deprecated-declarations",
    "/clang:-Wno-nonportable-include-path",
    "/clang:-Wno-microsoft-anon-tag",
    "/clang:-Wno-microsoft-enum-forward-reference",
    "/clang:-Wno-switch",
    "/clang:-Wno-unknown-pragmas",
    "/clang:-Wno-unused-but-set-variable",
    "/I$scriptRoot",
    "/I$($scriptRoot | Split-Path -Parent)",
    "/I$(Join-Path $sdkRoot "Include\$sdkVersion\km")",
    "/I$(Join-Path $sdkRoot "Include\$sdkVersion\shared")",
    "/I$(Join-Path $sdkRoot "Include\$sdkVersion\ucrt")",
    "/I$(Join-Path $sdkRoot "Include\wdf\kmdf\$kmdfVersion")"
)

$sources = @("Driver.c", "Device.c", "Queue.c")
$objects = @()

foreach ($source in $sources) {
    $sourcePath = Join-Path $scriptRoot $source
    $objectPath = Join-Path $buildRoot ([System.IO.Path]::GetFileNameWithoutExtension($source) + ".obj")
    & $clang @commonArgs "/Fo$objectPath" $sourcePath
    if ($LASTEXITCODE -ne 0) {
        throw "clang-cl failed for $source"
    }
    $objects += $objectPath
}

$libPath = Join-Path $buildRoot "wd_kmdf_skeleton.lib"
& $llvmLib /nologo "/out:$libPath" @objects
if ($LASTEXITCODE -ne 0) {
    throw "llvm-lib failed for KMDF skeleton"
}

Write-Host "Built KMDF skeleton library: $libPath"
Write-Host "Using SDK version: $sdkVersion"
Write-Host "Using KMDF version: $kmdfVersion"
