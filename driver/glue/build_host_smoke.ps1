Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $scriptRoot)
$outDir = Join-Path $scriptRoot "out"
$targetDir = Join-Path $scriptRoot "target\\host-smoke"
$kmdfManifest = Join-Path $repoRoot "driver\\wd-kmdf\\Cargo.toml"
$smokeManifest = Join-Path $scriptRoot "host-smoke\\Cargo.toml"
$staticLib = Join-Path $targetDir "debug\\wd_kmdf.lib"
$smokeExe = Join-Path $targetDir "debug\\wd_glue_host_smoke.exe"

if (-not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}

cargo build --manifest-path $kmdfManifest --offline --target-dir $targetDir
if (-not (Test-Path -LiteralPath $staticLib)) {
    throw "Expected Rust staticlib was not produced: $staticLib"
}

cargo run --manifest-path $smokeManifest --offline --target-dir $targetDir
if (-not (Test-Path -LiteralPath $smokeExe)) {
    throw "Expected host smoke executable was not produced: $smokeExe"
}

Copy-Item -LiteralPath $staticLib -Destination (Join-Path $outDir "wd_kmdf.lib") -Force
Copy-Item -LiteralPath $smokeExe -Destination (Join-Path $outDir "wd_glue_host_smoke.exe") -Force

Write-Host "Host smoke artifacts built in: $targetDir"
