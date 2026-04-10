param(
    [string]$CliPath = ".\target\debug\wd-cli.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-Contains {
    param(
        [string]$Text,
        [string]$Expected
    )

    if (-not $Text.Contains($Expected)) {
        throw "Expected output to contain '$Expected' but got: $Text"
    }
}

Write-Host "[network_reinject] validating CLI path: $CliPath"
if (-not (Test-Path -LiteralPath $CliPath)) {
    throw "CLI not found at '$CliPath'. Build wd-cli first."
}

Write-Host "[network_reinject] validating netfilter output contract"
$previousErrorActionPreference = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$output = (& $CliPath netfilter --filter "tcp and inbound" --mode reinject 2>&1 | Out-String).Trim()
$exitCode = $LASTEXITCODE
$ErrorActionPreference = $previousErrorActionPreference
if ($exitCode -eq 0) {
    Assert-Contains -Text $output -Expected "NETFILTER OK"
    Assert-Contains -Text $output -Expected "mode=reinject"
}
elseif ($exitCode -eq 3) {
    Assert-Contains -Text $output -Expected "device_unavailable"
}
elseif ($exitCode -eq 6) {
    Assert-Contains -Text $output -Expected "category=io_failure"
}
else {
    throw "netfilter returned unexpected exit code $exitCode"
}

Write-Host "[network_reinject] PASS"
