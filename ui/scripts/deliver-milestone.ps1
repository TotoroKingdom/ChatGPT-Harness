[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^M[0-9]{2}-[a-z0-9]+(?:-[a-z0-9]+)*$')]
    [string]$Milestone,

    [switch]$ReuseInstallerPayload,

    [switch]$IncludeInstaller,

    [string]$InstallerPath = "",

    [switch]$AllowM09DevelopmentIncomplete
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if (-not (Test-Path -LiteralPath (Join-Path $repositoryRoot "docs\archive\v4-2026-11\v4\milestones\$Milestone") -PathType Container)) {
    throw "Milestone plans do not exist: $Milestone"
}
if ($AllowM09DevelopmentIncomplete) {
    if ($Milestone -ne "M09-file-tool-admission-qualified-expansion") {
        throw "-AllowM09DevelopmentIncomplete is valid only for M09-file-tool-admission-qualified-expansion."
    }
    & (Join-Path $PSScriptRoot "check-milestone-layout.ps1") -Milestone $Milestone -AllowM09DevelopmentIncomplete
} else {
    & (Join-Path $PSScriptRoot "check-milestone-layout.ps1") -Milestone $Milestone
}
if ($ReuseInstallerPayload) {
    & (Join-Path $PSScriptRoot "build-portable.ps1") -ReuseInstallerPayload -InstallerPath $InstallerPath -Milestone $Milestone -AllowM09DevelopmentIncomplete:$AllowM09DevelopmentIncomplete
} else {
    & (Join-Path $PSScriptRoot "build-portable.ps1") -Milestone $Milestone -AllowM09DevelopmentIncomplete:$AllowM09DevelopmentIncomplete
}
if ($LASTEXITCODE -ne 0) { throw "Portable build failed." }
$package = Get-Content -LiteralPath (Join-Path $repositoryRoot "ui\package.json") -Raw | ConvertFrom-Json
$portableName = "ChatGPT-Harness_$($package.version)_x64-portable"
if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
    $InstallerPath = Join-Path $repositoryRoot ("target\release\bundle\nsis\ChatGPT-Harness_{0}_x64-setup.exe" -f $package.version)
}
$source = Join-Path $repositoryRoot "target\release\bundle\portable\$portableName"
$manifest = Get-Content -LiteralPath (Join-Path $source "distribution.json") -Raw | ConvertFrom-Json
$candidate = (Get-Date -Format "yyyyMMdd-HHmmss") + "-" + ([guid]::NewGuid().ToString("N").Substring(0, 8))
$delivery = Join-Path $repositoryRoot "artifacts\V4-$Milestone\$candidate"
New-Item -ItemType Directory -Path $delivery -ErrorAction Stop | Out-Null
$destination = Join-Path $delivery $portableName
Copy-Item -LiteralPath $source -Destination $destination -Recurse
foreach ($file in Get-ChildItem -LiteralPath $source -Recurse -File) {
    $relative = $file.FullName.Substring($source.Length + 1)
    if ((Get-FileHash -LiteralPath $file.FullName).Hash -ne (Get-FileHash -LiteralPath (Join-Path $destination $relative)).Hash) {
        throw "Copied file hash mismatch: $relative"
    }
}
$verification = & (Join-Path $PSScriptRoot "verify-portable.ps1") -PortableDirectory $destination
$verification | Set-Content -LiteralPath (Join-Path $delivery "portable-verification.txt") -Encoding UTF8
$installerRecord = $null
if ($IncludeInstaller) {
    if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
        throw "NSIS installer is missing: $InstallerPath"
    }
    & (Join-Path $PSScriptRoot "verify-nsis-bundle.ps1")
    if ($LASTEXITCODE -ne 0) { throw "NSIS verification failed." }
    $installerName = Split-Path -Leaf $InstallerPath
    $installerDestination = Join-Path $delivery $installerName
    Copy-Item -LiteralPath $InstallerPath -Destination $installerDestination
    $installerHash = (Get-FileHash -LiteralPath $installerDestination -Algorithm SHA256).Hash
    if ($installerHash -ne (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash) {
        throw "Copied NSIS installer hash mismatch."
    }
    $installerRecord = [ordered]@{
        file = $installerName
        sha256 = $installerHash
        verification = "PASS"
    }
}
$hashes = @(Get-ChildItem -LiteralPath $destination -Recurse -File | ForEach-Object {
    [ordered]@{ path = $_.FullName.Substring($destination.Length + 1); sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
})
$record = [ordered]@{
    milestone = $Milestone
    candidate = $candidate
    build_sha = $manifest.build_sha
    source_dirty = $manifest.source_dirty
    source_state_sha256 = $manifest.source_state_sha256
    portable_verification = "PASS"
    installer = $installerRecord
    external_e2e = "NOT_VERIFIED_BY_DELIVERY_SCRIPT"
    manual_acceptance = "NOT_EXECUTED"
    development_only = [bool]$AllowM09DevelopmentIncomplete
    validation_documents = if ($AllowM09DevelopmentIncomplete) { "MISSING_M09_FORMAL_VALIDATION_PLANS" } else { "PRESENT" }
    acceptance_status = "CANDIDATE_ONLY_MANUAL_ACCEPTANCE_NOT_EXECUTED"
    acceptance_plan = "docs/archive/v4-2026-11/v4/milestones/$Milestone/MANUAL_ACCEPTANCE_CHECKLIST.md"
    entry = "$portableName/chatgpt-harness-ui.exe"
    files = $hashes
}
$record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $delivery "delivery.json") -Encoding UTF8
Write-Output "Milestone candidate delivered: $delivery"
$verification | Write-Output
