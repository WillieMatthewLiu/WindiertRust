param(
    [string]$CliPath = ".\target\debug\wd-cli.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "[network_reinject] validating CLI path: $CliPath"
if (-not (Test-Path -LiteralPath $CliPath)) {
    throw "CLI not found at '$CliPath'. Build wd-cli first."
}

Write-Host "[network_reinject] placeholder flow: netfilter"
& $CliPath netfilter | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "netfilter command failed with exit code $LASTEXITCODE"
}

Write-Host "[network_reinject] PASS"
