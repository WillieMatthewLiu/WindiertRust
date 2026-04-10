Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$solutionPath = Join-Path $scriptRoot "wd_kmdf_skeleton.sln"
$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"

if (-not (Test-Path -LiteralPath $vswhere)) {
    throw "vswhere.exe not found at $vswhere"
}

$msbuild = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find MSBuild\**\Bin\MSBuild.exe |
    Select-Object -First 1
if (-not $msbuild) {
    throw "MSBuild.exe could not be located via vswhere"
}

& $msbuild $solutionPath /t:Build /p:Configuration=Debug /p:Platform=x64 /m /v:minimal
if ($LASTEXITCODE -ne 0) {
    throw "MSBuild failed for KMDF skeleton"
}

$expected = Join-Path $scriptRoot "build\x64\Debug\wd_kmdf_skeleton.lib"
if (-not (Test-Path -LiteralPath $expected)) {
    throw "Expected KMDF skeleton output was not produced: $expected"
}

Write-Host "Verified KMDF skeleton build output: $expected"
