[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Cold", "Warm")]
    [string]$Mode,
    [ValidateRange(1, 300)]
    [int]$Count = 30,
    [ValidateRange(0, 10)]
    [int]$PrimerCount = 2,
    [string]$PortableDirectory = "",
    [switch]$KeepRaw
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$portableRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\release\bundle\portable"))
$artifactsRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "artifacts"))
$cargoManifest = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\src-tauri\Cargo.toml")
$versionMatch = [regex]::Match($cargoManifest, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw "Could not read the UI package version."
}
$portableName = "ChatGPT-Harness_{0}_x64-portable" -f $versionMatch.Groups[1].Value
if ([string]::IsNullOrWhiteSpace($PortableDirectory)) {
    $PortableDirectory = Join-Path $portableRoot $portableName
}
$portablePath = [IO.Path]::GetFullPath($PortableDirectory)
if (-not $portablePath.StartsWith($portableRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -and
    -not $portablePath.StartsWith($artifactsRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Benchmark target must remain under target/release/bundle/portable or artifacts."
}

$uiExecutable = Join-Path $portablePath "chatgpt-harness-ui.exe"
if (-not (Test-Path -LiteralPath $uiExecutable -PathType Leaf)) {
    throw "Portable UI executable is missing: $uiExecutable"
}
if (-not (Test-Path -LiteralPath (Join-Path $portablePath "distribution.json") -PathType Leaf)) {
    throw "Portable distribution manifest is missing: $portablePath"
}

$existingProcesses = @(Get-CimInstance Win32_Process -Filter "Name='chatgpt-harness-ui.exe'" |
    Where-Object { $_.ExecutablePath -eq $uiExecutable })
if ($existingProcesses.Count -gt 0) {
    $pids = @($existingProcesses | ForEach-Object { $_.ProcessId }) -join ", "
    throw "The benchmark target is already running (PID $pids). Stop that test-owned instance before measuring."
}

$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
$benchmarkDirectory = Join-Path ([IO.Path]::GetTempPath()) ("harness-m02-benchmark-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $benchmarkDirectory | Out-Null

function Invoke-BenchmarkProcess([string[]]$Arguments, [string]$OutputPath) {
    $process = Start-Process -FilePath $uiExecutable -ArgumentList $Arguments -PassThru
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        throw "Benchmark process exited with code $($process.ExitCode): $OutputPath"
    }
    if (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
        throw "Benchmark process did not write its StartupAttempt output: $OutputPath"
    }
}

try {
    $rawPaths = @()
    if ($Mode -eq "Cold") {
        $totalRuns = $PrimerCount + $Count
        for ($index = 0; $index -lt $totalRuns; $index++) {
            $outputPath = Join-Path $benchmarkDirectory ("cold-{0:D2}.ndjson" -f ($index + 1))
            Invoke-BenchmarkProcess @(
                "--startup-benchmark-once",
                "--startup-benchmark-output",
                ('"{0}"' -f $outputPath)
            ) $outputPath
            $rawPaths += $outputPath
            Write-Output ("Completed cold attempt {0}/{1}" -f ($index + 1), $totalRuns)
        }
    } else {
        $outputPath = Join-Path $benchmarkDirectory "warm.ndjson"
        $warmAttempts = $PrimerCount + $Count
        Invoke-BenchmarkProcess @(
            "--startup-benchmark-warm",
            "--startup-benchmark-warm-attempts",
            $warmAttempts.ToString(),
            "--startup-benchmark-output",
            ('"{0}"' -f $outputPath)
        ) $outputPath
        $rawPaths += $outputPath
        Write-Output ("Completed warm process with {0} warm attempts plus one bootstrap attempt" -f $warmAttempts)
    }

    $analyzer = Join-Path $scriptDirectory "analyze-startup-benchmark.ps1"
    $aggregatePath = Join-Path $benchmarkDirectory ("{0}-aggregate.ndjson" -f $Mode.ToLowerInvariant())
    foreach ($rawPath in $rawPaths) {
        Get-Content -LiteralPath $rawPath | Add-Content -LiteralPath $aggregatePath -Encoding utf8
    }
    $analysis = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $analyzer `
        -InputPath $aggregatePath -Cohort ($Mode.ToLowerInvariant()) -SkipPrimer $PrimerCount
    if ($LASTEXITCODE -ne 0) {
        throw "Startup benchmark analysis failed."
    }
    $analysis
    if ($KeepRaw) {
        Write-Output "Raw benchmark data retained at: $benchmarkDirectory"
    }
} finally {
    if (-not $KeepRaw -and (Test-Path -LiteralPath $benchmarkDirectory)) {
        $resolved = [IO.Path]::GetFullPath($benchmarkDirectory)
        if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolved) -notlike "harness-m02-benchmark-*") {
            throw "Refusing to clean an unexpected benchmark directory: $resolved"
        }
        Remove-Item -LiteralPath $benchmarkDirectory -Recurse -Force
    }
}
