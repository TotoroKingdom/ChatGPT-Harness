[CmdletBinding()]
param(
    [switch]$ReuseInstallerPayload,
    [string]$InstallerPath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDirectory = (Resolve-Path (Join-Path $scriptDirectory "..")).Path
$repositoryRoot = (Resolve-Path (Join-Path $uiDirectory "..")).Path
& (Join-Path $scriptDirectory "check-milestone-layout.ps1")
$stagingDirectory = Join-Path $repositoryRoot "target-exe\distribution"
$releaseExecutable = Join-Path $repositoryRoot "target\release\chatgpt-harness-ui.exe"
$portableRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\release\bundle\portable"))

function Get-CargoPackageVersion([string]$ManifestPath) {
    $content = Get-Content -Raw -LiteralPath $ManifestPath
    $match = [regex]::Match($content, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) { throw "Could not read package version from $ManifestPath." }
    return $match.Groups[1].Value
}

function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try {
            $hash = $sha.ComputeHash($stream)
        } finally {
            $sha.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
    return ([BitConverter]::ToString($hash)).Replace("-", "")
}

function Get-Sha256Text([string]$Text) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text))
    } finally {
        $sha.Dispose()
    }
    return ([BitConverter]::ToString($hash)).Replace("-", "")
}

function Get-GitProvenance([string]$Root) {
    $gitArgs = @('-C', $Root, '-c', 'core.autocrlf=false', '-c', 'core.safecrlf=false', '-c', 'core.trustctime=false')
    $buildSha = (& git @gitArgs rev-parse HEAD 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($buildSha)) {
        throw "Could not determine the build SHA."
    }
    $buildSha = $buildSha.Trim()
    $statusRaw = (& git @gitArgs status --porcelain 2>$null) -join "`n"
    $diff = (& git @gitArgs diff --binary HEAD 2>$null) -join "`n"
    $cachedDiff = (& git @gitArgs diff --cached --binary 2>$null) -join "`n"
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

$appVersion = Get-CargoPackageVersion (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")
$portableName = "ChatGPT-Harness_${appVersion}_x64-portable"
$destination = [IO.Path]::GetFullPath((Join-Path $portableRoot $portableName))
if (-not $destination.StartsWith($portableRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Portable destination must remain under target/release/bundle/portable."
}

if ($ReuseInstallerPayload) {
    if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
        $InstallerPath = Join-Path $repositoryRoot ("target\release\bundle\nsis\ChatGPT-Harness_{0}_x64-setup.exe" -f $appVersion)
    }
    if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
        throw "Installer payload source is missing: $InstallerPath"
    }
    $sevenZip = "C:\Program Files\7-Zip\7z.exe"
    if (-not (Test-Path -LiteralPath $sevenZip -PathType Leaf)) {
        throw "7-Zip is required to reuse the NSIS payload: $sevenZip"
    }
    $payloadRoot = Join-Path ([IO.Path]::GetTempPath()) ("chatgpt-harness-installer-payload-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $payloadRoot | Out-Null
    try {
        & $sevenZip x $InstallerPath ("-o" + $payloadRoot) -y | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "7-Zip could not extract the NSIS installer." }
        foreach ($relative in @(
            "chatgpt-harness-ui.exe",
            "distribution.json",
            "runtime\chatgpt-harness-runtime.exe",
            "tunnel\tunnel-client.exe",
            "tunnel\cloudflared.exe"
        )) {
            if (-not (Test-Path -LiteralPath (Join-Path $payloadRoot $relative) -PathType Leaf)) {
                throw "Installer payload is missing $relative."
            }
        }
        if (Test-Path -LiteralPath $destination) {
            Remove-Item -LiteralPath $destination -Recurse -Force
        }
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        Copy-Item -Path (Join-Path $payloadRoot "*") -Destination $destination -Recurse -Force
        $manifestPath = Join-Path $destination "distribution.json"
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        $manifest.ui_sha256 = Get-Sha256 (Join-Path $destination "chatgpt-harness-ui.exe")
        $finalProvenance = Get-GitProvenance $repositoryRoot
        $manifest.build_sha = $finalProvenance.BuildSha
        $manifest.source_dirty = $finalProvenance.SourceDirty
        $manifest.source_state_sha256 = $finalProvenance.SourceStateSha
        $manifestJson = $manifest | ConvertTo-Json -Depth 8
        [IO.File]::WriteAllText($manifestPath, $manifestJson + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
        & (Join-Path $scriptDirectory "verify-portable.ps1") -PortableDirectory $destination
        if ($LASTEXITCODE -ne 0) { throw "Portable verification failed." }
        Write-Output "Built Portable distribution from verified NSIS payload: $destination"
        return
    } finally {
        $resolved = [IO.Path]::GetFullPath($payloadRoot)
        $tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if ($resolved.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -and
            [IO.Path]::GetFileName($resolved) -like "chatgpt-harness-installer-payload-*") {
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    }
}

& (Join-Path $scriptDirectory "stage-distribution.ps1")
if ($LASTEXITCODE -ne 0) { throw "Distribution staging failed." }

Push-Location $uiDirectory
try {
    # Release binaries must enable Tauri's custom protocol; without it the
    # embedded frontend falls back to devUrl (localhost:1420) and a portable
    # build incorrectly depends on a running Vite dev server.
    & npx tauri build --no-bundle --ci --features custom-protocol --config src-tauri/tauri.portable.conf.json
    if ($LASTEXITCODE -ne 0) { throw "Portable Tauri build failed." }
} finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
    throw "Portable UI release executable was not produced."
}
Copy-Item -LiteralPath $releaseExecutable -Destination (Join-Path $stagingDirectory "chatgpt-harness-ui.exe") -Force
$manifestPath = Join-Path $stagingDirectory "distribution.json"
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$manifest.ui_sha256 = Get-Sha256 (Join-Path $stagingDirectory "chatgpt-harness-ui.exe")
$finalProvenance = $null
for ($attempt = 0; $attempt -lt 60; $attempt++) {
    $finalProvenance = Get-GitProvenance $repositoryRoot
    if (-not $finalProvenance.SourceDirty) { break }
    Start-Sleep -Milliseconds 500
}
$manifest.build_sha = $finalProvenance.BuildSha
$manifest.source_dirty = $finalProvenance.SourceDirty
$manifest.source_state_sha256 = $finalProvenance.SourceStateSha
$manifestJson = $manifest | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText($manifestPath, $manifestJson + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

$temporary = Join-Path $portableRoot (".portable-staging-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null
try {
    Copy-Item -LiteralPath $stagingDirectory -Destination $temporary -Recurse
    if (Test-Path -LiteralPath $destination) {
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    Move-Item -LiteralPath $temporary -Destination $destination
    $temporary = $null
} finally {
    if ($null -ne $temporary -and (Test-Path -LiteralPath $temporary)) {
        Remove-Item -LiteralPath $temporary -Recurse -Force
    }
}

$finalManifestPath = Join-Path $destination "distribution.json"
$finalManifest = Get-Content -Raw -LiteralPath $finalManifestPath | ConvertFrom-Json
$finalProvenance = $null
for ($attempt = 0; $attempt -lt 60; $attempt++) {
    $finalProvenance = Get-GitProvenance $repositoryRoot
    if (-not $finalProvenance.SourceDirty) { break }
    Start-Sleep -Milliseconds 500
}
$finalManifest.build_sha = $finalProvenance.BuildSha
$finalManifest.source_dirty = $finalProvenance.SourceDirty
$finalManifest.source_state_sha256 = $finalProvenance.SourceStateSha
$finalManifestJson = $finalManifest | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText($finalManifestPath, $finalManifestJson + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

& (Join-Path $scriptDirectory "verify-portable.ps1") -PortableDirectory $destination
if ($LASTEXITCODE -ne 0) { throw "Portable verification failed." }
Write-Output "Built Portable distribution: $destination"
