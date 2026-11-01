[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AppDataRoot,

    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,

    [switch]$KeepGeneratedConfig
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDirectory = (Resolve-Path (Join-Path $scriptDirectory "..")).Path
$repositoryRoot = (Resolve-Path (Join-Path $uiDirectory "..")).Path
$targetExeRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target-exe"))
$stagingDirectory = [IO.Path]::GetFullPath((Join-Path $targetExeRoot "distribution"))
$releaseExecutable = Join-Path $repositoryRoot "target\release\chatgpt-harness-ui.exe"

function Resolve-FullPath([string]$Path) {
    return [IO.Path]::GetFullPath($Path)
}

function Assert-DirectoryUnder([string]$Path, [string]$Parent, [string]$Description) {
    $resolvedPath = Resolve-FullPath $Path
    $resolvedParent = (Resolve-FullPath $Parent).TrimEnd('\') + '\'
    if (-not $resolvedPath.StartsWith($resolvedParent, [StringComparison]::OrdinalIgnoreCase)) {
        throw "$Description must remain under $Parent."
    }
    return $resolvedPath
}

function Write-Utf8Json([string]$Path, [object]$Value) {
    $json = $Value | ConvertTo-Json -Depth 10
    [IO.File]::WriteAllText(
        $Path,
        $json + [Environment]::NewLine,
        [Text.UTF8Encoding]::new($false)
    )
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
}

$appDataPath = Resolve-FullPath $AppDataRoot
$candidatePath = Assert-DirectoryUnder $CandidateDirectory $targetExeRoot "Candidate directory"

if (-not (Test-Path -LiteralPath $appDataPath -PathType Container)) {
    New-Item -ItemType Directory -Force -Path $appDataPath | Out-Null
}
if (Test-Path -LiteralPath $candidatePath) {
    throw "Candidate directory already exists; use a fresh directory: $candidatePath"
}
New-Item -ItemType Directory -Force -Path $candidatePath | Out-Null

$generatedConfig = Join-Path ([IO.Path]::GetTempPath()) ("m03-tauri-override-" + [guid]::NewGuid().ToString("N") + ".json")
$generatedConfigValue = [ordered]@{
    app = [ordered]@{
        # This is intentionally absolute. The runner creates a new disposable
        # root for each invocation and verifies config_metadata at runtime.
        appDirectoriesOverride = $appDataPath
        # The product imports @tauri-apps/api/core at runtime. Expose the real
        # Tauri bridge in this QA-only candidate so the CDP driver can exercise
        # the same invoke path as the React application without injecting a
        # mock IPC implementation.
        withGlobalTauri = $true
    }
}

try {
    Write-Utf8Json $generatedConfig $generatedConfigValue

    # stage-distribution.ps1 owns the repository's canonical Runtime/Tunnel/UI
    # resource layout. It is intentionally invoked before Tauri so the custom
    # protocol build can resolve the staged Runtime resources.
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $scriptDirectory "stage-distribution.ps1")
    if ($LASTEXITCODE -ne 0) {
        throw "stage-distribution.ps1 failed with exit code $LASTEXITCODE."
    }

    Push-Location $uiDirectory
    try {
        # The generated config is the final merge layer. It adds only the
        # disposable app-directory override and QA bridge exposure; the
        # portable config keeps the normal custom-protocol/no-installer shape.
        & npx tauri build `
            --no-bundle `
            --ci `
            --features custom-protocol `
            --config src-tauri/tauri.portable.conf.json `
            --config $generatedConfig
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri no-bundle build failed with exit code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }

    if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
        throw "Tauri did not produce the release executable: $releaseExecutable"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $stagingDirectory "runtime\chatgpt-harness-runtime.exe") -PathType Leaf)) {
        throw "Staged Runtime is missing from $stagingDirectory."
    }
    if (-not (Test-Path -LiteralPath (Join-Path $stagingDirectory "tunnel\tunnel-client.exe") -PathType Leaf)) {
        throw "Staged Tunnel client is missing from $stagingDirectory."
    }

    # Keep the candidate self-contained. Tauri's resource mapping is resolved
    # into the packaged executable, while the adjacent files are retained for
    # provenance, process identity checks, and the Runtime discovery fallback.
    Copy-Item -Path (Join-Path $stagingDirectory "*") -Destination $candidatePath -Recurse -Force

    $manifestPath = Join-Path $candidatePath "distribution.json"
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $manifest.ui_sha256 = Get-Sha256 $releaseExecutable
    $manifestJson = $manifest | ConvertTo-Json -Depth 10
    [IO.File]::WriteAllText(
        $manifestPath,
        $manifestJson + [Environment]::NewLine,
        [Text.UTF8Encoding]::new($false)
    )
    Copy-Item -LiteralPath $releaseExecutable -Destination (Join-Path $candidatePath "chatgpt-harness-ui.exe") -Force

    $metadata = [ordered]@{
        schema = "m03-e2e-build-v1"
        candidateDirectory = $candidatePath
        appDataRoot = $appDataPath
        executable = (Join-Path $candidatePath "chatgpt-harness-ui.exe")
        executableSha256 = Get-Sha256 (Join-Path $candidatePath "chatgpt-harness-ui.exe")
        generatedConfig = $generatedConfig
        generatedConfigValue = $generatedConfigValue
        manifest = $manifest
        builtAtUtc = [DateTime]::UtcNow.ToString("O")
    }
    Write-Utf8Json (Join-Path $candidatePath "m03-build-metadata.json") $metadata

    [ordered]@{
        ok = $true
        candidateDirectory = $candidatePath
        executable = (Join-Path $candidatePath "chatgpt-harness-ui.exe")
        executableSha256 = $metadata.executableSha256
        appDataRoot = $appDataPath
        generatedConfig = $generatedConfig
    } | ConvertTo-Json -Depth 8 -Compress | Write-Output
} finally {
    if (-not $KeepGeneratedConfig -and (Test-Path -LiteralPath $generatedConfig -PathType Leaf)) {
        Remove-Item -LiteralPath $generatedConfig -Force
    }
}
