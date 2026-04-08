Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $scriptRoot "out"

if (-not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}

Copy-Item -LiteralPath (Join-Path $scriptRoot "wd-rust-x64.inf") -Destination $outDir -Force
Copy-Item -LiteralPath (Join-Path $scriptRoot "wd-rust-x86.inf") -Destination $outDir -Force

Write-Host "Driver glue artifacts staged in: $outDir"
