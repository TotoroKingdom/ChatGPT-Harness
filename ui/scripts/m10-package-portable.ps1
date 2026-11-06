[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$stagingDirectory = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target-exe\distribution"))
$portableRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\release\bundle\portable"))
$appVersion = ([regex]::Match(
        (Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")),
        '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"'
    )).Groups[1].Value
$destination = [IO.Path]::GetFullPath((Join-Path $portableRoot "ChatGPT-Harness_${appVersion}_x64-portable"))

function Assert-Under([string]$Path, [string]$Parent, [string]$Description) {
    $resolved = [IO.Path]::GetFullPath($Path)
    $prefix = [IO.Path]::GetFullPath($Parent).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "$Description must remain under $Parent."
    }
    return $resolved
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
}

function Get-Sha256Text([string]$Text) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text)))).Replace('-', '').ToUpperInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Get-GitProvenance([string]$Root) {
    $gitArgs = @('-C', $Root, '-c', 'core.autocrlf=false', '-c', 'core.safecrlf=false', '-c', 'core.trustctime=false')
    $buildSha = (& git @gitArgs rev-parse HEAD 2>$null).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($buildSha)) {
        throw "Could not determine the build SHA."
    }
    $statusRaw = (& git @gitArgs status --porcelain 2>$null) -join "`n"
    $diff = (& git @gitArgs diff --binary HEAD 2>$null) -join "`n"
    $cachedDiff = (& git @gitArgs diff --cached --binary HEAD 2>$null) -join "`n"
    $worktreeDiff = (& git @gitArgs diff --quiet HEAD 2>$null)
    $hasWorktreeDiff = $LASTEXITCODE -ne 0
    $cachedDiffCheck = (& git @gitArgs diff --cached --quiet 2>$null)
    $hasCachedDiff = $LASTEXITCODE -ne 0
    $status = $statusRaw
    if (-not $hasWorktreeDiff -and -not $hasCachedDiff) {
        $diff = ""
        $cachedDiff = ""
        $status = (($statusRaw -split "`n" | Where-Object { $_.StartsWith("??") }) -join "`n")
    }
    [pscustomobject]@{
        BuildSha = $buildSha
        SourceStatus = $status
        SourceDiff = $diff
        SourceCachedDiff = $cachedDiff
        SourceDirty = -not [string]::IsNullOrWhiteSpace($status)
        SourceStateSha = Get-Sha256Text ($buildSha + "`n" + $status + "`n" + $diff + "`n" + $cachedDiff)
    }
}

$stagingPath = Assert-Under $stagingDirectory $repositoryRoot "Distribution staging directory"
$portablePath = Assert-Under $portableRoot $repositoryRoot "Portable root"
$destinationPath = Assert-Under $destination $portablePath "Portable destination"
if ([IO.Path]::GetFileName($destinationPath) -ne "ChatGPT-Harness_${appVersion}_x64-portable") {
    throw "Portable destination name is unexpected."
}
if (-not (Test-Path -LiteralPath (Join-Path $stagingPath "distribution.json") -PathType Leaf)) {
    throw "Staged distribution manifest is missing."
}
if (-not (Test-Path -LiteralPath (Join-Path $stagingPath "chatgpt-harness-ui.exe") -PathType Leaf)) {
    throw "Staged UI executable is missing."
}

$manifestPath = Join-Path $stagingPath "distribution.json"
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$manifest.ui_sha256 = Get-Sha256 (Join-Path $stagingPath "chatgpt-harness-ui.exe")
$provenance = Get-GitProvenance $repositoryRoot
$manifest.build_sha = $provenance.BuildSha
$manifest.source_dirty = $provenance.SourceDirty
$manifest.source_state_sha256 = $provenance.SourceStateSha
[IO.File]::WriteAllText(
    $manifestPath,
    ($manifest | ConvertTo-Json -Depth 8) + [Environment]::NewLine,
    [Text.UTF8Encoding]::new($false)
)

New-Item -ItemType Directory -Force -Path $portablePath | Out-Null
$temporaryPath = Join-Path $portablePath (".m10-portable-staging-" + [guid]::NewGuid().ToString("N"))
try {
    Copy-Item -LiteralPath $stagingPath -Destination $temporaryPath -Recurse -Force
    if (Test-Path -LiteralPath $destinationPath) {
        Remove-Item -LiteralPath $destinationPath -Recurse -Force
    }
    Move-Item -LiteralPath $temporaryPath -Destination $destinationPath
    $temporaryPath = $null
} finally {
    if ($null -ne $temporaryPath -and (Test-Path -LiteralPath $temporaryPath)) {
        Remove-Item -LiteralPath $temporaryPath -Recurse -Force
    }
}

& (Join-Path $scriptDirectory "verify-portable.ps1") -PortableDirectory $destinationPath
if ($LASTEXITCODE -ne 0) {
    throw "Portable verification failed."
}

Write-Output "Built and verified M10 Portable distribution: $destinationPath"
Write-Output "UI SHA-256: $($manifest.ui_sha256)"
Write-Output "Runtime SHA-256: $($manifest.runtime_sha256)"
Write-Output "Tunnel SHA-256: $($manifest.tunnel_client_sha256)"
