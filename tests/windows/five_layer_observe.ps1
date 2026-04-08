param(
    [string]$CliPath = ".\target\debug\wd-cli.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "[five_layer_observe] validating CLI path: $CliPath"
if (-not (Test-Path -LiteralPath $CliPath)) {
    throw "CLI not found at '$CliPath'. Build wd-cli first."
}

Write-Host "[five_layer_observe] placeholder flow: netdump + flowtrack + socketdump"
& $CliPath netdump | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "netdump command failed with exit code $LASTEXITCODE"
}

& $CliPath flowtrack | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "flowtrack command failed with exit code $LASTEXITCODE"
}

& $CliPath socketdump | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "socketdump command failed with exit code $LASTEXITCODE"
}

Write-Host "[five_layer_observe] PASS"
