param(
    [string]$CliPath = ".\target\debug\wd-cli.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "[open_close] validating CLI path: $CliPath"
if (-not (Test-Path -LiteralPath $CliPath)) {
    throw "CLI not found at '$CliPath'. Build wd-cli first."
}

Write-Host "[open_close] smoke run: reflectctl"
& $CliPath reflectctl | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "reflectctl command failed with exit code $LASTEXITCODE"
}

Write-Host "[open_close] PASS"
