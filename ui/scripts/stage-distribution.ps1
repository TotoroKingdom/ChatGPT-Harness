[CmdletBinding()]
param(
    [string]$OutputDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$targetDirectory = Join-Path $repositoryRoot "target\release"
$tunnelPackageDirectory = Join-Path $repositoryRoot "tunnel-client-v0.0.13-windows-amd64"
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repositoryRoot "target-exe\distribution"
}
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
$targetExeRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target-exe"))
if (-not $outputPath.StartsWith($targetExeRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Output directory must remain under target-exe."
}

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

Push-Location $repositoryRoot
try {
    # Build Runtime first. The Tauri UI build consumes the staged Runtime and
    # distribution manifest as bundle resources, so building the whole
    # workspace here would create a first-build circular dependency.
    & cargo build --release --manifest-path (Join-Path $repositoryRoot "Cargo.toml") -p chatgpt-harness-runtime
    if ($LASTEXITCODE -ne 0) { throw "Runtime release build failed." }
} finally {
    Pop-Location
}

$tunnelClientSource = Join-Path $tunnelPackageDirectory "tunnel-client.exe"
$cloudflaredSource = Join-Path $tunnelPackageDirectory "cloudflared.exe"
if (-not (Test-Path -LiteralPath $tunnelClientSource -PathType Leaf) -or
    -not (Test-Path -LiteralPath $cloudflaredSource -PathType Leaf)) {
    throw "The bundled Tunnel package is incomplete."
}
$tunnelVersion = "0.0.13"

$runtimeExecutable = Join-Path $targetDirectory "chatgpt-harness-runtime.exe"
if (-not (Test-Path -LiteralPath $runtimeExecutable -PathType Leaf)) {
    throw "Required Runtime release executable was not produced."
}

$appVersion = Get-CargoPackageVersion (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")
$runtimeVersion = Get-CargoPackageVersion (Join-Path $repositoryRoot "runtime\Cargo.toml")
$uiPackage = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\package.json") | ConvertFrom-Json
if ($uiPackage.version -ne $appVersion -or $runtimeVersion -ne $appVersion) {
    throw "UI, Runtime, and package versions must match before distribution staging."
}
$protocolContent = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "protocol\src\lib.rs")
$protocolMatch = [regex]::Match($protocolContent, 'PROTOCOL_VERSION\s*:\s*u32\s*=\s*(\d+)')
if (-not $protocolMatch.Success) { throw "Could not read protocol version." }
$protocolVersion = [int]$protocolMatch.Groups[1].Value
$gitProvenanceArgs = @('-C', $repositoryRoot, '-c', 'core.autocrlf=false', '-c', 'core.safecrlf=false', '-c', 'core.trustctime=false')
$buildSha = (& git @gitProvenanceArgs rev-parse HEAD 2>$null)
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($buildSha)) {
    throw "Could not determine the build SHA."
}
$buildSha = $buildSha.Trim()
$sourceStatus = (& git @gitProvenanceArgs status --porcelain 2>$null) -join "`n"
$sourceDiff = (& git @gitProvenanceArgs diff --binary HEAD 2>$null) -join "`n"
$sourceCachedDiff = (& git @gitProvenanceArgs diff --cached --binary 2>$null) -join "`n"
$sourceDirty = -not [string]::IsNullOrWhiteSpace($sourceStatus)
$sourceStateSha = Get-Sha256Text ($buildSha + "`n" + $sourceStatus + "`n" + $sourceDiff + "`n" + $sourceCachedDiff)

$stagingPath = Join-Path (Split-Path -Parent $outputPath) (".distribution-staging-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path (Join-Path $stagingPath "runtime") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stagingPath "tunnel") | Out-Null
try {
    Copy-Item -LiteralPath $runtimeExecutable -Destination (Join-Path $stagingPath "runtime\chatgpt-harness-runtime.exe")
    Copy-Item -Path (Join-Path $tunnelPackageDirectory "*") -Destination (Join-Path $stagingPath "tunnel") -Recurse -Force
    $manifest = [ordered]@{
        distribution_schema_version = 2
        distribution_id = "chatgpt-harness"
        app_version = $appVersion
        runtime_version = $runtimeVersion
        build_sha = $buildSha
        source_dirty = $sourceDirty
        source_state_sha256 = $sourceStateSha
        ui_sha256 = ""
        runtime_sha256 = (Get-Sha256 $runtimeExecutable)
        tunnel_client_sha256 = (Get-Sha256 $tunnelClientSource)
        cloudflared_sha256 = (Get-Sha256 $cloudflaredSource)
        protocol_version = $protocolVersion
        runtime = [ordered]@{ relative_path = "runtime/chatgpt-harness-runtime.exe" }
        tunnel = [ordered]@{
            version = $tunnelVersion
            relative_path = "tunnel/tunnel-client.exe"
            cloudflared_relative_path = "tunnel/cloudflared.exe"
        }
    }
    $json = $manifest | ConvertTo-Json -Depth 4
    [IO.File]::WriteAllText((Join-Path $stagingPath "distribution.json"), $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

    if (Test-Path -LiteralPath $outputPath) {
        Remove-Item -LiteralPath $outputPath -Recurse -Force
    }
    Move-Item -LiteralPath $stagingPath -Destination $outputPath
    $stagingPath = $null
} finally {
    if ($null -ne $stagingPath -and (Test-Path -LiteralPath $stagingPath)) {
        Remove-Item -LiteralPath $stagingPath -Recurse -Force
    }
}

$uiExecutable = Join-Path $targetDirectory "chatgpt-harness-ui.exe"
Push-Location $repositoryRoot
try {
    # The staging directory now exists, allowing the Tauri build script to
    # resolve its configured Runtime and distribution resources.
    & cargo build --release --manifest-path (Join-Path $repositoryRoot "Cargo.toml") -p chatgpt-harness-ui
    if ($LASTEXITCODE -ne 0) { throw "UI release build failed." }
} finally {
    Pop-Location
}
if (-not (Test-Path -LiteralPath $uiExecutable -PathType Leaf)) {
    throw "Required UI release executable was not produced."
}
Copy-Item -LiteralPath $uiExecutable -Destination (Join-Path $outputPath "chatgpt-harness-ui.exe") -Force

$manifestPath = Join-Path $outputPath "distribution.json"
$runtimePath = Join-Path $outputPath "runtime\chatgpt-harness-runtime.exe"
$tunnelClientPath = Join-Path $outputPath "tunnel\tunnel-client.exe"
$cloudflaredPath = Join-Path $outputPath "tunnel\cloudflared.exe"
if (-not (Test-Path -LiteralPath (Join-Path $outputPath "chatgpt-harness-ui.exe") -PathType Leaf) -or
    -not (Test-Path -LiteralPath $runtimePath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $tunnelClientPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $cloudflaredPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Distribution staging validation failed."
}
Write-Output "Staged distribution: $outputPath"
