[CmdletBinding()]
param(
    [string]$OutputDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$targetDirectory = Join-Path $repositoryRoot "target\release"
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

Push-Location $repositoryRoot
try {
    & cargo build --workspace --release --manifest-path (Join-Path $repositoryRoot "Cargo.toml")
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }
} finally {
    Pop-Location
}

$uiExecutable = Join-Path $targetDirectory "chatgpt-harness-ui.exe"
$runtimeExecutable = Join-Path $targetDirectory "chatgpt-harness-runtime.exe"
foreach ($path in @($uiExecutable, $runtimeExecutable)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required release executable was not produced."
    }
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

$stagingPath = Join-Path (Split-Path -Parent $outputPath) (".distribution-staging-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path (Join-Path $stagingPath "runtime") | Out-Null
try {
    Copy-Item -LiteralPath $uiExecutable -Destination (Join-Path $stagingPath "chatgpt-harness-ui.exe")
    Copy-Item -LiteralPath $runtimeExecutable -Destination (Join-Path $stagingPath "runtime\chatgpt-harness-runtime.exe")
    $manifest = [ordered]@{
        distribution_schema_version = 1
        distribution_id = "chatgpt-harness"
        app_version = $appVersion
        runtime_version = $runtimeVersion
        protocol_version = $protocolVersion
        runtime = [ordered]@{ relative_path = "runtime/chatgpt-harness-runtime.exe" }
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

$manifestPath = Join-Path $outputPath "distribution.json"
$runtimePath = Join-Path $outputPath "runtime\chatgpt-harness-runtime.exe"
if (-not (Test-Path -LiteralPath (Join-Path $outputPath "chatgpt-harness-ui.exe") -PathType Leaf) -or
    -not (Test-Path -LiteralPath $runtimePath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Distribution staging validation failed."
}
Write-Output "Staged distribution: $outputPath"
