Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$buildScript = Join-Path $scriptRoot "build_host_smoke.ps1"
$sourceFile = Join-Path $scriptRoot "wd_runtime_host_smoke.c"

if (-not (Test-Path -LiteralPath $buildScript)) {
    throw "Missing host smoke build script: $buildScript"
}

if (-not (Test-Path -LiteralPath $sourceFile)) {
    throw "Missing host smoke source file: $sourceFile"
}

& $buildScript
