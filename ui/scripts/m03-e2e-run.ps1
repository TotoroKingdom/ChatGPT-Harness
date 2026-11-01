[CmdletBinding()]
param(
    [ValidateSet("all", "E2E-01", "E2E-02", "E2E-03", "E2E-04")]
    [string]$Case = "all",

    [ValidateRange(30, 1800)]
    [int]$TimeoutSeconds = 240,

    [string]$ResultDirectory = "",

    [switch]$KeepTemp
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDirectory = (Resolve-Path (Join-Path $scriptDirectory "..")).Path
$repositoryRoot = (Resolve-Path (Join-Path $uiDirectory "..")).Path
$targetExeRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target-exe"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$runId = [guid]::NewGuid().ToString("N")
$runRoot = Join-Path ([IO.Path]::GetTempPath()) ("chatgpt-harness-m03-e2e-" + $runId)
$appDataRoot = Join-Path $runRoot "appdata"
$workspaceRoot = Join-Path $runRoot "workspace"
$candidateDirectory = Join-Path $targetExeRoot ("m03-e2e-candidate-" + $runId)
$driverPath = Join-Path $scriptDirectory "m03-e2e-driver.mjs"
$buildPath = Join-Path $scriptDirectory "m03-e2e-build.ps1"

if ([string]::IsNullOrWhiteSpace($ResultDirectory)) {
    $stamp = [DateTime]::UtcNow.ToString("yyyyMMdd-HHmmss")
    $ResultDirectory = Join-Path $targetExeRoot ("m03-e2e-results-" + $stamp + "-" + $runId)
}
$resultDirectoryPath = [IO.Path]::GetFullPath($ResultDirectory)

function Write-Utf8Json([string]$Path, [object]$Value) {
    $json = $Value | ConvertTo-Json -Depth 12
    [IO.File]::WriteAllText(
        $Path,
        $json + [Environment]::NewLine,
        [Text.UTF8Encoding]::new($false)
    )
}

function Assert-TempRunRoot([string]$Path) {
    $resolved = [IO.Path]::GetFullPath($Path)
    $prefix = $tempRoot + '\'
    if (-not $resolved.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($resolved) -notlike "chatgpt-harness-m03-e2e-*") {
        throw "Refusing to clean an unexpected M03 E2E temp root: $resolved"
    }
}

function Stop-CandidateProcesses {
    $candidatePaths = @(
        [IO.Path]::GetFullPath((Join-Path $candidateDirectory "chatgpt-harness-ui.exe")),
        [IO.Path]::GetFullPath((Join-Path $candidateDirectory "runtime\chatgpt-harness-runtime.exe")),
        [IO.Path]::GetFullPath((Join-Path $candidateDirectory "tunnel\tunnel-client.exe")),
        [IO.Path]::GetFullPath((Join-Path $candidateDirectory "tunnel\cloudflared.exe"))
    )
    $processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ExecutablePath -and ($candidatePaths -contains ([IO.Path]::GetFullPath($_.ExecutablePath)))
    })
    foreach ($process in $processes) {
        try {
            Stop-Process -Id ([int]$process.ProcessId) -Force -ErrorAction SilentlyContinue
        } catch {
            # The driver already attempted bounded graceful shutdown.
        }
    }
}

function Write-FallbackResult([string]$Message, [string]$Phase) {
    New-Item -ItemType Directory -Force -Path $resultDirectoryPath | Out-Null
    $result = [ordered]@{
        schema = "m03-packaged-local-e2e-result-v1"
        scope = "M03 revised local packaged E2E boundary"
        verdict = "BLOCKED"
        acceptanceClaim = "NOT_CLAIMED"
        externalChatGPT = "MANUAL_ONLY_NOT_EXECUTED"
        tunnel = "NOT_STARTED_BY_LOCAL_SUITE"
        isolation = [ordered]@{
            appDirectoriesOverride = "absolute disposable path supplied at build time"
            realUserAppDataTouched = $false
            realUserHistoryTouched = $false
            temporaryRootCleaned = (-not $KeepTemp)
        }
        phase = $Phase
        error = $Message
        generatedAtUtc = [DateTime]::UtcNow.ToString("O")
    }
    Write-Utf8Json (Join-Path $resultDirectoryPath "m03-e2e-result.json") $result
    $markdown = @(
        "# M03 packaged local E2E result",
        "",
        "- Local runner verdict: **BLOCKED**",
        "- External ChatGPT: **MANUAL_ONLY_NOT_EXECUTED**",
        "- Tunnel: **NOT_STARTED_BY_LOCAL_SUITE**",
        "- Acceptance claim: **NOT_CLAIMED**",
        "- Phase: $Phase",
        "- Error: $Message",
        "",
        "Machine-readable detail is in `m03-e2e-result.json`.",
        ""
    ) -join "`n"
    [IO.File]::WriteAllText((Join-Path $resultDirectoryPath "m03-e2e-result.md"), $markdown + "`n", [Text.UTF8Encoding]::new($false))
}

$exitCode = 0
$phase = "initialization"
$failureMessage = $null

try {
    if (-not (Test-Path -LiteralPath $driverPath -PathType Leaf)) {
        throw "Missing QA driver: $driverPath"
    }
    if (-not (Test-Path -LiteralPath $buildPath -PathType Leaf)) {
        throw "Missing QA build script: $buildPath"
    }
    New-Item -ItemType Directory -Force -Path $runRoot, $appDataRoot, $workspaceRoot | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $runRoot "outside") | Out-Null

    # These fixtures are created only below the unique disposable run root.
    [IO.File]::WriteAllText(
        (Join-Path $workspaceRoot "safe-read.txt"),
        "M03_SAFE_CONTENT`n",
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllText(
        (Join-Path $workspaceRoot "mutable.txt"),
        "ORIGINAL_CONTENT`n",
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllText(
        (Join-Path $workspaceRoot "privacy-sentinel.txt"),
        "M03_SECRET_SENTINEL_NOT_DURABLE`n",
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllText(
        (Join-Path $runRoot "outside\outside.txt"),
        "M03_OUTSIDE_WORKSPACE_SENTINEL`n",
        [Text.UTF8Encoding]::new($false)
    )

    $phase = "build"
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $buildPath `
        -AppDataRoot $appDataRoot `
        -CandidateDirectory $candidateDirectory
    if ($LASTEXITCODE -ne 0) {
        throw "m03-e2e-build.ps1 failed with exit code $LASTEXITCODE."
    }

    $phase = "driver"
    $driverArguments = @(
        $driverPath,
        "--candidate", $candidateDirectory,
        "--appdata", $appDataRoot,
        "--workspace", $workspaceRoot,
        "--run-root", $runRoot,
        "--result-dir", $resultDirectoryPath,
        "--case", $Case.ToUpperInvariant(),
        "--timeout-seconds", [string]$TimeoutSeconds
    )
    & node @driverArguments
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        throw "m03-e2e-driver.mjs returned exit code $exitCode."
    }
} catch {
    $exitCode = 1
    $failureMessage = $_.Exception.Message
    if (-not (Test-Path -LiteralPath (Join-Path $resultDirectoryPath "m03-e2e-result.json") -PathType Leaf)) {
        Write-FallbackResult $failureMessage $phase
    }
} finally {
    Stop-CandidateProcesses
    if (-not $KeepTemp) {
        Assert-TempRunRoot $runRoot
        if (Test-Path -LiteralPath $runRoot) {
            Remove-Item -LiteralPath $runRoot -Recurse -Force
        }
    }
}

Write-Output "M03 E2E result directory: $resultDirectoryPath"
if ($failureMessage) {
    Write-Error $failureMessage
}
exit $exitCode
