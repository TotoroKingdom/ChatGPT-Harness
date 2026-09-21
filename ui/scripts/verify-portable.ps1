[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PortableDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$portableRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\release\bundle\portable"))
$portablePath = [IO.Path]::GetFullPath($PortableDirectory)
if (-not $portablePath.StartsWith($portableRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Portable verification target must remain under target/release/bundle/portable."
}

function Assert-File([string]$Path, [string]$Description) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "$Description is missing: $Path" }
    if ((Get-Item -LiteralPath $Path).Length -le 0) { throw "$Description is empty: $Path" }
}

$manifestPath = Join-Path $portablePath "distribution.json"
$uiExecutable = Join-Path $portablePath "chatgpt-harness-ui.exe"
$runtimeExecutable = Join-Path $portablePath "runtime\chatgpt-harness-runtime.exe"
$tunnelExecutable = Join-Path $portablePath "tunnel\tunnel-client.exe"
$cloudflaredExecutable = Join-Path $portablePath "tunnel\cloudflared.exe"
Assert-File $manifestPath "Distribution manifest"
Assert-File $uiExecutable "Portable UI executable"
Assert-File $runtimeExecutable "Portable Runtime executable"
Assert-File $tunnelExecutable "Portable Tunnel client"
Assert-File $cloudflaredExecutable "Portable cloudflared"

$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$expectedVersion = [regex]::Match(
    (Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")),
    '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"'
).Groups[1].Value
if ($manifest.app_version -ne $expectedVersion -or $manifest.runtime_version -ne $expectedVersion) {
    throw "Portable manifest versions do not match $expectedVersion."
}
if ($manifest.runtime.relative_path -ne "runtime/chatgpt-harness-runtime.exe" -or
    $manifest.tunnel.relative_path -ne "tunnel/tunnel-client.exe" -or
    $manifest.tunnel.cloudflared_relative_path -ne "tunnel/cloudflared.exe") {
    throw "Portable manifest paths are invalid."
}

$iconPath = Join-Path $repositoryRoot "ui\src-tauri\icons\portable\generated\icon.ico"
Assert-File $iconPath "Portable multi-size icon"
$requiredIconSizes = @(16, 32, 48, 64, 128, 256)
foreach ($size in $requiredIconSizes) {
    Assert-File (Join-Path $repositoryRoot "ui\src-tauri\icons\portable\generated\${size}x${size}.png") "Portable ${size}px icon"
}
$bytes = [IO.File]::ReadAllBytes($iconPath)
$entryCount = [BitConverter]::ToUInt16($bytes, 4)
if ($entryCount -lt 6) { throw "Portable icon must contain at least six image entries; found $entryCount." }
$icoSizes = @()
for ($index = 0; $index -lt $entryCount; $index++) {
    $width = [int]$bytes[6 + ($index * 16)]
    if ($width -eq 0) { $width = 256 }
    $icoSizes += $width
}
foreach ($size in $requiredIconSizes) {
    if ($size -notin $icoSizes) { throw "Portable icon is missing the ${size}px ICO entry." }
}
Add-Type -AssemblyName System.Drawing
$associatedIcon = [System.Drawing.Icon]::ExtractAssociatedIcon($uiExecutable)
if ($null -eq $associatedIcon -or $associatedIcon.Width -le 1 -or $associatedIcon.Height -le 1) {
    throw "Portable executable does not expose a usable associated icon."
}
$associatedIcon.Dispose()

$requests = @(
    (@{id=1;method='initialize';params=@{clientInfo=@{name='portable-verifier';title='Portable Verifier';version=$expectedVersion};protocolVersion=1;capabilities=@{activityReplay=$true};runtimeConfig=@{configRevision='1';workspace=@{rootPath=$repositoryRoot};security=@{mode='normal';developerTestMode=$false};runtimeLimits=@{maxConcurrentRequests=1;maxActivityEvents=100;maxReadBytes=1048576}}}} | ConvertTo-Json -Depth 8 -Compress),
    (@{method='initialized';params=@{}} | ConvertTo-Json -Depth 4 -Compress),
    (@{id=2;method='runtime.status';params=@{}} | ConvertTo-Json -Depth 4 -Compress),
    (@{id=3;method='runtime.stop';params=@{}} | ConvertTo-Json -Depth 4 -Compress)
)
$runtimeOutput = $requests | & $runtimeExecutable 2>$null
if ($LASTEXITCODE -ne 0) { throw "Portable Runtime smoke test exited with $LASTEXITCODE." }
$messages = @($runtimeOutput | ForEach-Object { $_ | ConvertFrom-Json })
if (-not ($messages | Where-Object { $_.id -eq 2 -and $_.result.lifecycle -eq 'ready' })) {
    throw "Portable Runtime did not report ready."
}
if (-not ($messages | Where-Object { $_.id -eq 3 -and $_.result.accepted -eq $true })) {
    throw "Portable Runtime did not stop cleanly."
}

$archives = @(Get-ChildItem -LiteralPath $portableRoot -File -Filter "*.zip" -ErrorAction SilentlyContinue)
if ($archives.Count -gt 0) { throw "Portable build must not create ZIP archives." }

Write-Output "Verified Portable distribution: $portablePath"
Write-Output "Verified version: $expectedVersion"
Write-Output "Verified icon entries: $entryCount"
Write-Output "Verified Runtime smoke test: PASS"
