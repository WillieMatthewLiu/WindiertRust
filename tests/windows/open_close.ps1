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

function Invoke-ReflectAction {
    param(
        [string]$CliPath,
        [string]$Action,
        [string]$ExpectedState
    )

    Write-Host "[open_close] reflectctl --action $Action"
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = (& $CliPath reflectctl --action $Action 2>&1 | Out-String).Trim()
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorActionPreference

    if ($exitCode -eq 0) {
        Assert-Contains -Text $output -Expected "REFLECTCTL OK"
        Assert-Contains -Text $output -Expected "state=$ExpectedState"
        return
    }

    if ($exitCode -ne 3) {
        throw "reflectctl --action $Action returned unexpected exit code $exitCode"
    }
    Assert-Contains -Text $output -Expected "REFLECTCTL ERROR"
    Assert-Contains -Text $output -Expected "category=device_unavailable"
}

Write-Host "[open_close] validating CLI path: $CliPath"
if (-not (Test-Path -LiteralPath $CliPath)) {
    throw "CLI not found at '$CliPath'. Build wd-cli first."
}

Write-Host "[open_close] validating reflectctl open/close runtime contract"
Invoke-ReflectAction -CliPath $CliPath -Action "open" -ExpectedState "Open"
Invoke-ReflectAction -CliPath $CliPath -Action "close" -ExpectedState "CloseAttempted"

Write-Host "[open_close] PASS"
