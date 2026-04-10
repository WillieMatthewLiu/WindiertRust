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

function Invoke-CliCapture {
    param(
        [string]$CliPath,
        [string[]]$Arguments
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = (& $CliPath @Arguments 2>&1 | Out-String).Trim()
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorActionPreference

    return @{
        Output = $output
        ExitCode = $exitCode
    }
}

Write-Host "[five_layer_observe] validating CLI path: $CliPath"
if (-not (Test-Path -LiteralPath $CliPath)) {
    throw "CLI not found at '$CliPath'. Build wd-cli first."
}

Write-Host "[five_layer_observe] validating netdump output contract"
$netdump = Invoke-CliCapture -CliPath $CliPath -Arguments @("netdump")
if ($netdump.ExitCode -eq 0) {
    Assert-Contains -Text $netdump.Output -Expected "NETDUMP OK"
    Assert-Contains -Text $netdump.Output -Expected "layer=NETWORK"
}
elseif ($netdump.ExitCode -eq 3) {
    Assert-Contains -Text $netdump.Output -Expected "device_unavailable"
}
else {
    throw "netdump returned unexpected exit code $($netdump.ExitCode)"
}

Write-Host "[five_layer_observe] validating flowtrack output contract"
$flowtrack = Invoke-CliCapture -CliPath $CliPath -Arguments @("flowtrack", "--process-id", "42")
if ($flowtrack.ExitCode -eq 0) {
    Assert-Contains -Text $flowtrack.Output -Expected "FLOWTRACK OK"
}
elseif ($flowtrack.ExitCode -eq 3) {
    Assert-Contains -Text $flowtrack.Output -Expected "device_unavailable"
}
else {
    throw "flowtrack returned unexpected exit code $($flowtrack.ExitCode)"
}

Write-Host "[five_layer_observe] validating socketdump output contract"
$socketdump = Invoke-CliCapture -CliPath $CliPath -Arguments @("socketdump", "--filter", "event == CONNECT and processId == 7")
if ($socketdump.ExitCode -eq 0) {
    Assert-Contains -Text $socketdump.Output -Expected "SOCKETDUMP OK"
}
elseif ($socketdump.ExitCode -eq 3) {
    Assert-Contains -Text $socketdump.Output -Expected "device_unavailable"
}
else {
    throw "socketdump returned unexpected exit code $($socketdump.ExitCode)"
}

Write-Host "[five_layer_observe] PASS"
