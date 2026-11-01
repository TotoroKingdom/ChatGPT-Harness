[CmdletBinding()]
param(
    [string]$Case = "",
    [switch]$Preflight
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$runId = "{0}-{1}" -f (Get-Date -Format "yyyyMMdd-HHmmss"), ([guid]::NewGuid().ToString("N").Substring(0, 8))
$evidenceRoot = Join-Path ([IO.Path]::GetTempPath()) "M03-INT-$runId"
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null

$requested = $Case.Trim().ToUpperInvariant()
if ($requested -and $requested -notmatch '^M03-INT-\d{2}$') {
    throw "Case must be empty or M03-INT-NN."
}

function Invoke-LoggedCommand {
    param(
        [string]$Id,
        [string]$WorkingDirectory,
        [string]$FilePath,
        [string[]]$Arguments
    )

    $logPath = Join-Path $evidenceRoot "$Id.log"
    Push-Location $WorkingDirectory
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $FilePath @Arguments 2>&1 | Tee-Object -FilePath $logPath | Out-String
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }

    if ($exitCode -ne 0) {
        Write-Output $output
        throw "$Id command failed with exit code $exitCode. Evidence: $logPath"
    }
    if ($output -notmatch 'test result: ok\. 1 passed; 0 failed;') {
        Write-Output $output
        throw "$Id did not execute exactly one passing formal QA test. Evidence: $logPath"
    }
    Write-Host "$Id PASS"
    return (Join-Path $evidenceRoot "$Id.log")
}

function Invoke-LoggedExitCommand {
    param(
        [string]$Id,
        [string]$WorkingDirectory,
        [string]$FilePath,
        [string[]]$Arguments
    )

    $logPath = Join-Path $evidenceRoot "$Id.log"
    Push-Location $WorkingDirectory
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $FilePath @Arguments 2>&1 | Tee-Object -FilePath $logPath | Out-Null
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }
    if ($exitCode -ne 0) {
        throw "$Id command failed with exit code $exitCode. Evidence: $logPath"
    }
    Write-Host "$Id PASS"
}

$cases = @(
    @{ Id = "M03-INT-01"; Package = "ui"; Filter = "m03_qa_integration::m03_int_01_canonical_identity_reaches_activity_and_history" },
    @{ Id = "M03-INT-02"; Package = "ui"; Filter = "m03_qa_integration::m03_int_02_runtime_identity_host_mapping_and_retry_boundaries" },
    @{ Id = "M03-INT-03"; Package = "ui"; Filter = "m03_qa_integration::m03_int_03_history_config_generation_semantics" },
    @{ Id = "M03-INT-04"; Package = "runtime"; Filter = "m03_qa_integration::int_04" },
    @{ Id = "M03-INT-05"; Package = "ui"; Filter = "m03_qa_integration::m03_int_05_evidence_store_public_boundary" },
    @{ Id = "M03-INT-06"; Package = "runtime"; Filter = "m03_qa_integration::int_06" },
    @{ Id = "M03-INT-07"; Package = "runtime"; Filter = "m03_qa_integration::int_07" },
    @{ Id = "M03-INT-08"; Package = "runtime"; Filter = "m03_qa_integration::int_08" },
    @{ Id = "M03-INT-09"; Package = "runtime"; Filter = "m03_qa_integration::int_09" },
    @{ Id = "M03-INT-10"; Package = "runtime"; Filter = "m03_qa_integration::int_10" },
    @{ Id = "M03-INT-11"; Package = "ui"; Filter = "m03_qa_integration::m03_int_11_history_filter_matrix_and_keyset_pagination" },
    @{ Id = "M03-INT-12"; Package = "performance"; Filter = "tests::performance_50k_requests_250k_events_p95" },
    @{ Id = "M03-INT-13"; Package = "ui"; Filter = "m03_qa_integration::m03_int_13_history_facade_orders_detail_and_preserves_read_only_semantics" },
    @{ Id = "M03-INT-14"; Package = "ui"; Filter = "m03_qa_integration::m03_int_14_approval_terminal_matrix_is_observational_after_restart" },
    @{ Id = "M03-INT-15"; Package = "ui"; Filter = "m03_qa_integration::m03_int_15_history_clear_barrier_retains_open_requests_and_releases_offline" },
    @{ Id = "M03-INT-16"; Package = "runtime"; Filter = "m03_qa_integration::int_16" },
    @{ Id = "M03-INT-17"; Package = "runtime"; Filter = "m03_qa_integration::int_17" },
    @{ Id = "M03-INT-18"; Package = "ui"; Filter = "m03_qa_integration::m03_int_18_m01_m02_v3_regression_and_repository_boundary" }
)

Push-Location $repositoryRoot
try {
    # UI QA cases launch the real Runtime subprocess.
    Invoke-LoggedExitCommand -Id "RUNTIME-BUILD" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("build", "-p", "chatgpt-harness-runtime")

    if ($Preflight) {
        Invoke-LoggedExitCommand -Id "PREFLIGHT-FMT" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("fmt", "--all", "--", "--check")
        Invoke-LoggedExitCommand -Id "PREFLIGHT-CHECK" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("check", "--workspace")
        Invoke-LoggedExitCommand -Id "PREFLIGHT-CARGO-TEST" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("test", "--workspace")
        Invoke-LoggedExitCommand -Id "PREFLIGHT-UI-TYPECHECK" -WorkingDirectory (Join-Path $repositoryRoot "ui") -FilePath "npm" -Arguments @("run", "typecheck")
        Invoke-LoggedExitCommand -Id "PREFLIGHT-UI-TEST" -WorkingDirectory (Join-Path $repositoryRoot "ui") -FilePath "npm" -Arguments @("test")
        Invoke-LoggedExitCommand -Id "PREFLIGHT-UI-BUILD" -WorkingDirectory (Join-Path $repositoryRoot "ui") -FilePath "npm" -Arguments @("run", "build")
        Write-Output "PREFLIGHT PASS"
    }

    $results = @()
    foreach ($entry in $cases) {
        if ($requested -and $entry.Id -ne $requested) {
            continue
        }
        switch ($entry.Package) {
            "runtime" {
                $log = Invoke-LoggedCommand -Id $entry.Id -WorkingDirectory $repositoryRoot -FilePath "cargo" `
                    -Arguments @("test", "-p", "chatgpt-harness-runtime", $entry.Filter, "--", "--ignored", "--exact", "--nocapture")
            }
            "ui" {
                $log = Invoke-LoggedCommand -Id $entry.Id -WorkingDirectory $repositoryRoot -FilePath "cargo" `
                    -Arguments @("test", "-p", "chatgpt-harness-ui", "--lib", $entry.Filter, "--", "--ignored", "--exact", "--nocapture")
            }
            "performance" {
                $log = Invoke-LoggedCommand -Id $entry.Id -WorkingDirectory $repositoryRoot -FilePath "cargo" `
                    -Arguments @("test", "-p", "chatgpt-harness-evidence-store", $entry.Filter, "--", "--ignored", "--exact", "--nocapture")
            }
            default {
                throw "Unknown package kind $($entry.Package)"
            }
        }
        $results += [ordered]@{ id = $entry.Id; verdict = "PASS"; log = $log }
    }

    if (-not $requested) {
        $summary = [ordered]@{
            schema = "m03-integration-result-v1"
            candidate = (& git -C $repositoryRoot rev-parse HEAD).Trim()
            worktreeClean = [string]::IsNullOrWhiteSpace((& git -C $repositoryRoot status --porcelain) -join "`n")
            evidenceRoot = $evidenceRoot
            cases = $results
            verdict = "PASS"
        }
        $summary | ConvertTo-Json -Depth 8 |
            Set-Content -LiteralPath (Join-Path $evidenceRoot "integration-result.json") -Encoding utf8
    }

    Write-Output "M03 Integration evidence: $evidenceRoot"
} finally {
    Pop-Location
}
