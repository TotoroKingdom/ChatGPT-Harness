[CmdletBinding()]
param(
    [string]$Case = "",
    [switch]$Preflight,
    [switch]$SkipPackage,
    [string]$ResultDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$gateScript = Join-Path $scriptDirectory "m06-release-gate.ps1"
$stamp = [DateTime]::UtcNow.ToString("yyyyMMdd-HHmmss")
$runId = [guid]::NewGuid().ToString("N").Substring(0, 8)
if ([string]::IsNullOrWhiteSpace($ResultDirectory)) {
    $ResultDirectory = Join-Path $repositoryRoot ("target-exe\m06-integration-results-{0}-{1}" -f $stamp, $runId)
}
$evidenceRoot = [IO.Path]::GetFullPath($ResultDirectory)
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null

$requested = $Case.Trim().ToUpperInvariant()
if ($requested -and $requested -notmatch '^M06-INT-\d{2}$') {
    throw "Case must be empty or M06-INT-NN."
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
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $FilePath @Arguments 2>&1 | Tee-Object -FilePath $logPath | Out-String
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previous
        Pop-Location
    }
    if ($exitCode -ne 0) {
        Write-Output $output
        throw "$Id command failed with exit code $exitCode. Evidence: $logPath"
    }
    return [pscustomobject]@{ Output = $output; Log = $logPath }
}

function Invoke-TestCase {
    param(
        [string]$Id,
        [string]$Package,
        [string]$Filter,
        [switch]$Ignored
    )
    $arguments = @("test", "-p", $Package, $Filter)
    if ($Ignored) {
        $arguments += @("--", "--ignored", "--nocapture")
    } else {
        $arguments += @("--", "--nocapture")
    }
    $result = Invoke-LoggedCommand -Id $Id -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments $arguments
    if ($result.Output -notmatch 'test result: ok\. (\d+) passed; 0 failed;') {
        throw "$Id did not report a passing test result."
    }
    if ([int]$Matches[1] -lt 1) {
        throw "$Id matched zero tests."
    }
    Write-Host "$Id PASS"
    return $result.Log
}

function Invoke-GateSelfTest {
    $result = Invoke-LoggedCommand -Id "M06-GATE-SELFTEST" -WorkingDirectory $repositoryRoot -FilePath "powershell.exe" `
        -Arguments @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $gateScript, "self-test")
    if ($result.Output -notmatch "M06 gate self-test: PASS") {
        throw "M06 gate self-test did not PASS."
    }
    Write-Host "M06-GATE-SELFTEST PASS"
    return $result.Log
}

function Invoke-PackageAudit {
    $package = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\package.json") | ConvertFrom-Json
    $portable = Join-Path $repositoryRoot ("target\release\bundle\portable\ChatGPT-Harness_{0}_x64-portable" -f $package.version)
    $installer = Join-Path $repositoryRoot ("target\release\bundle\nsis\ChatGPT-Harness_{0}_x64-setup.exe" -f $package.version)
    if (-not (Test-Path -LiteralPath $portable -PathType Container) -or
        -not (Test-Path -LiteralPath $installer -PathType Leaf)) {
        return [pscustomobject]@{
            Status = "NOT_EXECUTED"
            Log = $null
            Reason = "Portable or NSIS installer has not been built for this worktree."
        }
    }
    $identityPath = Join-Path $evidenceRoot "candidate-identity.json"
    $collectLog = Join-Path $evidenceRoot "M06-IDENTITY.log"
    $collect = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $gateScript "collect-identity" `
        -PortableDirectory $portable -InstallerPath $installer -Json 2>&1 | Tee-Object -FilePath $collectLog | Out-String
    if ($LASTEXITCODE -ne 0) {
        return [pscustomobject]@{ Status = "BLOCKED"; Log = $collectLog; Reason = "Candidate identity collection failed." }
    }
    $parsed = $collect | ConvertFrom-Json
    $parsed.identity | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $identityPath -Encoding utf8
    $auditLog = Join-Path $evidenceRoot "M06-PACKAGE-AUDIT.log"
    $audit = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $gateScript "audit-package" `
        -CandidatePath $identityPath -PortableDirectory $portable -InstallerPath $installer -RequireInstaller -Json 2>&1 |
        Tee-Object -FilePath $auditLog | Out-String
    if ($LASTEXITCODE -ne 0) {
        return [pscustomobject]@{ Status = "FAIL"; Log = $auditLog; Reason = "Package audit rejected the candidate." }
    }
    return [pscustomobject]@{ Status = "PASS"; Log = $auditLog; Reason = "" }
}

$cases = @(
    @{ Id = "M06-INT-01"; Kind = "gate"; Description = "Dependency gate and candidate identity" },
    @{ Id = "M06-INT-02"; Kind = "gate"; Description = "28-row Core matrix fail-closed logic" },
    @{ Id = "M06-INT-03"; Kind = "package"; Description = "Portable/NSIS identity and shipping-content integrity" },
    @{ Id = "M06-INT-04"; Kind = "upgrade"; Description = "v2/v3 upgrade, History compatibility and rollback prep" },
    @{ Id = "M06-INT-05"; Kind = "m01"; Description = "F01 lifecycle and Tray authority" },
    @{ Id = "M06-INT-06"; Kind = "m01"; Description = "F03/F04 notification state machines" },
    @{ Id = "M06-INT-07"; Kind = "gate"; Description = "Frozen M02 contract import and F05 equivalence" },
    @{ Id = "M06-INT-08"; Kind = "diagnostics"; Description = "F06 Diagnostics truthfulness and privacy" },
    @{ Id = "M06-INT-09"; Kind = "evidence"; Description = "Shared evidence exact identity and authorization isolation" },
    @{ Id = "M06-INT-10"; Kind = "history"; Description = "F07 History ownership, persistence and degradation" },
    @{ Id = "M06-INT-11"; Kind = "grep"; Description = "Same-candidate read/grep/ASK/DENY authority chain" },
    @{ Id = "M06-INT-12"; Kind = "v3"; Description = "V3 security/lifecycle regression matrix" },
    @{ Id = "M06-INT-13"; Kind = "blackbox"; Description = "Runtime/Tunnel crash, restart and orphan cleanup" },
    @{ Id = "M06-INT-14"; Kind = "gate"; Description = "M04/M05 F12 equivalence and requalification gates" },
    @{ Id = "M06-INT-15"; Kind = "external"; Description = "Real Tunnel external-client probe" },
    @{ Id = "M06-INT-16"; Kind = "gate"; Description = "Final Core gate output and fail-closed verdict" }
)

$results = @()
$gateLog = $null
$packageResult = $null

Push-Location $repositoryRoot
try {
    Invoke-LoggedCommand -Id "M06-BUILD-RUNTIME" -WorkingDirectory $repositoryRoot -FilePath "cargo" `
        -Arguments @("build", "-p", "chatgpt-harness-runtime", "--bins") | Out-Null
    $debugRuntime = Join-Path $repositoryRoot "target\debug\chatgpt-harness-runtime.exe"
    if (-not (Test-Path -LiteralPath $debugRuntime -PathType Leaf)) {
        throw "Debug Runtime executable was not produced."
    }
    $env:HARNESS_E2E_RUNTIME_BIN = $debugRuntime

    if ($Preflight) {
        Invoke-LoggedCommand -Id "PREFLIGHT-FMT" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("fmt", "--all", "--", "--check") | Out-Null
        Invoke-LoggedCommand -Id "PREFLIGHT-CHECK" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("check", "--workspace") | Out-Null
        Invoke-LoggedCommand -Id "PREFLIGHT-CARGO-TEST" -WorkingDirectory $repositoryRoot -FilePath "cargo" -Arguments @("test", "--workspace") | Out-Null
        Invoke-LoggedCommand -Id "PREFLIGHT-UI-TYPECHECK" -WorkingDirectory (Join-Path $repositoryRoot "ui") -FilePath "npm" -Arguments @("run", "typecheck") | Out-Null
        Invoke-LoggedCommand -Id "PREFLIGHT-UI-TEST" -WorkingDirectory (Join-Path $repositoryRoot "ui") -FilePath "npm" -Arguments @("test") | Out-Null
        Invoke-LoggedCommand -Id "PREFLIGHT-UI-BUILD" -WorkingDirectory (Join-Path $repositoryRoot "ui") -FilePath "npm" -Arguments @("run", "build") | Out-Null
        Write-Host "PREFLIGHT PASS"
    }

    foreach ($entry in $cases) {
        if ($requested -and $entry.Id -ne $requested) { continue }
        $status = "PASS"
        $log = $null
        $reason = ""
        try {
            switch ($entry.Kind) {
                "gate" {
                    if ($null -eq $gateLog) { $gateLog = Invoke-GateSelfTest }
                    $log = $gateLog
                }
                "package" {
                    if ($SkipPackage) {
                        $status = "NOT_EXECUTED"
                        $reason = "Package audit explicitly skipped."
                    } else {
                        if ($null -eq $packageResult) { $packageResult = Invoke-PackageAudit }
                        $status = $packageResult.Status
                        $log = $packageResult.Log
                        $reason = $packageResult.Reason
                    }
                }
                "upgrade" {
                    $log = Invoke-TestCase -Id $entry.Id -Package "chatgpt-harness-ui" -Filter "m06_upgrade_and_rollback_contract_cases"
                }
                "m01" {
                    $log = Invoke-TestCase -Id $entry.Id -Package "chatgpt-harness-ui" -Filter "m01_real_runtime_approval_integration_cases" -Ignored
                }
                "diagnostics" {
                    $log = Invoke-TestCase -Id $entry.Id -Package "chatgpt-harness-ui" -Filter "diagnostics::tests"
                }
                "evidence" {
                    $log = Invoke-TestCase -Id ($entry.Id + "-A") -Package "chatgpt-harness-ui" -Filter "m03_int_01_canonical_identity_reaches_activity_and_history" -Ignored
                    $null = Invoke-TestCase -Id ($entry.Id + "-B") -Package "chatgpt-harness-ui" -Filter "m03_int_14_approval_terminal_matrix_is_observational_after_restart" -Ignored
                    $null = Invoke-TestCase -Id ($entry.Id + "-C") -Package "chatgpt-harness-runtime" -Filter "grep_activity_and_m03_share_exact_identity_and_durable_scalars_only"
                }
                "history" {
                    $log = Invoke-TestCase -Id ($entry.Id + "-A") -Package "chatgpt-harness-ui" -Filter "m03_int_11_history_filter_matrix_and_keyset_pagination" -Ignored
                    $null = Invoke-TestCase -Id ($entry.Id + "-B") -Package "chatgpt-harness-ui" -Filter "m03_int_13_history_facade_orders_detail_and_preserves_read_only_semantics" -Ignored
                    $null = Invoke-TestCase -Id ($entry.Id + "-C") -Package "chatgpt-harness-ui" -Filter "m03_int_15_history_clear_barrier_retains_open_requests_and_releases_offline" -Ignored
                    foreach ($filter in @("int_04", "int_06", "int_07", "int_08", "int_09", "int_10", "int_16", "int_17")) {
                        $null = Invoke-TestCase -Id ($entry.Id + "-R-" + $filter) -Package "chatgpt-harness-runtime" -Filter $filter -Ignored
                    }
                }
                "grep" {
                    $log = Invoke-TestCase -Id ($entry.Id + "-A") -Package "chatgpt-harness-runtime" -Filter "grep_exposes_exact_schema_and_routes_literal_regex_filters"
                    $null = Invoke-TestCase -Id ($entry.Id + "-B") -Package "chatgpt-harness-runtime" -Filter "grep_argument_scope_filter_and_disabled_errors_are_typed_and_sanitized"
                    $null = Invoke-TestCase -Id ($entry.Id + "-C") -Package "chatgpt-harness-runtime" -Filter "grep_policy_deny_and_ask_deny_do_not_scan_and_allow_once_resumes_exact_request"
                    $null = Invoke-TestCase -Id ($entry.Id + "-D") -Package "chatgpt-harness-runtime" -Filter "grep_history_covers_success_truncation_deny_and_timeout_without_raw_text"
                }
                "v3" {
                    $log = Invoke-TestCase -Id ($entry.Id + "-A") -Package "chatgpt-harness-runtime" -Filter "m5_analyzer_security"
                    $null = Invoke-TestCase -Id ($entry.Id + "-B") -Package "chatgpt-harness-runtime" -Filter "m5_bash_pipeline_tests"
                    $null = Invoke-TestCase -Id ($entry.Id + "-C") -Package "chatgpt-harness-ui" -Filter "m03_int_18_m01_m02_v3_regression_and_repository_boundary" -Ignored
                }
                "blackbox" {
                    $log = Invoke-TestCase -Id ($entry.Id + "-A") -Package "chatgpt-harness-ui" -Filter "m2_local_black_box_e2e" -Ignored
                    $null = Invoke-TestCase -Id ($entry.Id + "-B") -Package "chatgpt-harness-ui" -Filter "m3_local_black_box_e2e" -Ignored
                }
                "external" {
                    $status = "NOT_EXECUTED"
                    $reason = "Requires the packaged candidate, a QA-controlled external MCP client and Tunnel/control-plane authorization."
                }
                default { throw "Unknown M06 integration kind $($entry.Kind)" }
            }
        } catch {
            $status = "FAIL"
            $reason = $_.Exception.Message
        }
        $results += [ordered]@{
            id = $entry.Id
            description = $entry.Description
            status = $status
            evidence = $log
            reason = $reason
        }
        Write-Host ("{0} {1}" -f $entry.Id, $status)
    }

    $formalBlocked = @($results | Where-Object { $_.status -in @("FAIL", "BLOCKED") }).Count -gt 0
    $summary = [ordered]@{
        schema = "m06-development-integration-result-v1"
        scope = "M06 development regression; formal M06 Integration remains NOT_EXECUTED"
        candidate = (& git -C $repositoryRoot rev-parse HEAD).Trim()
        worktreeClean = [string]::IsNullOrWhiteSpace((& git -C $repositoryRoot status --porcelain) -join "`n")
        generatedAtUtc = [DateTime]::UtcNow.ToString("O")
        cases = $results
        formalIntegration = "NOT_EXECUTED"
        formalE2e = "NOT_EXECUTED"
        manualAcceptance = "NOT_EXECUTED"
        externalChatGpt = "NOT_EXECUTED"
        verdict = if ($formalBlocked) { "DEVELOPMENT_REGRESSION_FAIL" } else { "DEVELOPMENT_REGRESSION_PASS" }
    }
    $summary | ConvertTo-Json -Depth 16 |
        Set-Content -LiteralPath (Join-Path $evidenceRoot "m06-integration-result.json") -Encoding utf8
    Write-Output "M06 development integration evidence: $evidenceRoot"
    Write-Output ("M06 development integration verdict: " + $summary.verdict)
} finally {
    Pop-Location
}

if ($formalBlocked) { exit 1 }
