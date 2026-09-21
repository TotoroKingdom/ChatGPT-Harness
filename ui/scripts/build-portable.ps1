[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDirectory = (Resolve-Path (Join-Path $scriptDirectory "..")).Path
$repositoryRoot = (Resolve-Path (Join-Path $uiDirectory "..")).Path
$stagingDirectory = Join-Path $repositoryRoot "target-exe\distribution"
$releaseExecutable = Join-Path $repositoryRoot "target\release\chatgpt-harness-ui.exe"
$portableRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\release\bundle\portable"))

function Get-CargoPackageVersion([string]$ManifestPath) {
    $content = Get-Content -Raw -LiteralPath $ManifestPath
    $match = [regex]::Match($content, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) { throw "Could not read package version from $ManifestPath." }
    return $match.Groups[1].Value
}

$appVersion = Get-CargoPackageVersion (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")
$portableName = "ChatGPT-Harness_${appVersion}_x64-portable"
$destination = [IO.Path]::GetFullPath((Join-Path $portableRoot $portableName))
if (-not $destination.StartsWith($portableRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Portable destination must remain under target/release/bundle/portable."
}

& (Join-Path $scriptDirectory "stage-distribution.ps1")
if ($LASTEXITCODE -ne 0) { throw "Distribution staging failed." }

Push-Location $uiDirectory
try {
    & npx tauri build --no-bundle --ci --config src-tauri/tauri.portable.conf.json
    if ($LASTEXITCODE -ne 0) { throw "Portable Tauri build failed." }
} finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
    throw "Portable UI release executable was not produced."
}
Copy-Item -LiteralPath $releaseExecutable -Destination (Join-Path $stagingDirectory "chatgpt-harness-ui.exe") -Force

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

& (Join-Path $scriptDirectory "verify-portable.ps1") -PortableDirectory $destination
if ($LASTEXITCODE -ne 0) { throw "Portable verification failed." }
Write-Output "Built Portable distribution: $destination"
