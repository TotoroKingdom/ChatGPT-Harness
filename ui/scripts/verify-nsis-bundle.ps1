[CmdletBinding()]
param(
    [string]$BundleDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$stagingDirectory = Join-Path $repositoryRoot "target-exe\distribution"
if ([string]::IsNullOrWhiteSpace($BundleDirectory)) {
    $BundleDirectory = Join-Path $repositoryRoot "target\release\bundle\nsis"
}
$bundlePath = [IO.Path]::GetFullPath($BundleDirectory)

function Get-CargoPackageVersion([string]$ManifestPath) {
    $content = Get-Content -Raw -LiteralPath $ManifestPath
    $match = [regex]::Match($content, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) { throw "Could not read package version from $ManifestPath." }
    return $match.Groups[1].Value
}

function Assert-File([string]$Path, [string]$Description) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Description is missing: $Path"
    }
    if ((Get-Item -LiteralPath $Path).Length -le 0) {
        throw "$Description is empty: $Path"
    }
}

$uiExecutable = Join-Path $stagingDirectory "chatgpt-harness-ui.exe"
$runtimeExecutable = Join-Path $stagingDirectory "runtime\chatgpt-harness-runtime.exe"
$tunnelClientExecutable = Join-Path $stagingDirectory "tunnel\tunnel-client.exe"
$cloudflaredExecutable = Join-Path $stagingDirectory "tunnel\cloudflared.exe"
$manifestPath = Join-Path $stagingDirectory "distribution.json"
Assert-File $uiExecutable "Staged UI executable"
Assert-File $runtimeExecutable "Staged Runtime executable"
Assert-File $tunnelClientExecutable "Staged Tunnel client executable"
Assert-File $cloudflaredExecutable "Staged cloudflared executable"
Assert-File $manifestPath "Staged distribution manifest"

$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$appVersion = Get-CargoPackageVersion (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")
$runtimeVersion = Get-CargoPackageVersion (Join-Path $repositoryRoot "runtime\Cargo.toml")
if ($manifest.app_version -ne $appVersion -or $manifest.runtime_version -ne $runtimeVersion) {
    throw "Staged distribution manifest versions do not match Cargo package versions."
}
if ($manifest.distribution_schema_version -ne 2 -or
    $manifest.tunnel.version -ne "0.0.13" -or
    $manifest.tunnel.relative_path -ne "tunnel/tunnel-client.exe" -or
    $manifest.tunnel.cloudflared_relative_path -ne "tunnel/cloudflared.exe") {
    throw "Staged distribution manifest does not describe the complete Tunnel payload."
}
if ($manifest.runtime.relative_path -ne "runtime/chatgpt-harness-runtime.exe") {
    throw "Staged distribution manifest does not use the M9-B Runtime path."
}

$config = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\src-tauri\tauri.conf.json") | ConvertFrom-Json
$targets = @($config.bundle.targets)
if (-not $config.bundle.active -or $targets.Count -ne 1 -or $targets[0] -ne "nsis") {
    throw "Tauri bundle must have only the NSIS target enabled."
}
if ($config.bundle.windows.nsis.installMode -ne "currentUser") {
    throw "NSIS install mode must be currentUser."
}
$installers = @(Get-ChildItem -LiteralPath $bundlePath -File -Filter "*-setup.exe")
if ($installers.Count -ne 1) {
    throw "Expected exactly one NSIS installer, found $($installers.Count)."
}
Assert-File $installers[0].FullName "NSIS installer"
if ($installers[0].BaseName -notmatch [regex]::Escape($appVersion)) {
    throw "NSIS installer name does not contain version $appVersion."
}
$unexpected = Get-ChildItem -LiteralPath $bundlePath -File | Where-Object { $_.Extension -in @(".msi", ".appimage", ".deb", ".rpm") }
if ($unexpected) {
    throw "Unexpected non-NSIS bundle artifacts were generated: $($unexpected.Name -join ', ')"
}

Write-Output "Verified NSIS installer: $bundlePath"
Write-Output "Verified payload source: $stagingDirectory"
Write-Output "Verified payload layout: chatgpt-harness-ui.exe, runtime/chatgpt-harness-runtime.exe, tunnel/tunnel-client.exe, tunnel/cloudflared.exe, distribution.json"
Write-Output "Verified version: $appVersion"

