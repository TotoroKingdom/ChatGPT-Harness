[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AppDataRoot,
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
$appDataPath = [IO.Path]::GetFullPath($AppDataRoot)
if (-not $appDataPath.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "M07 QA AppData must be a disposable directory under the system temporary directory."
}
$identifier = "com.chatgpt.harness.m07.qa." + [guid]::NewGuid().ToString("N")
& (Join-Path $PSScriptRoot "m03-e2e-build.ps1") -AppDataRoot $appDataPath -CandidateDirectory $CandidateDirectory -AppIdentifier $identifier
if ($LASTEXITCODE -ne 0) { throw "M07 isolated QA build failed." }
