[CmdletBinding()]
param(
    [switch]$Preflight,
    [switch]$RunNative,
    [switch]$BuildPortable,
    [switch]$SkipFullCargoTest,
    [switch]$SkipUiTest,
    [string]$ResultDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$gateScript = Join-Path $scriptDirectory "m11-release-gate.ps1"
$layoutScript = Join-Path $scriptDirectory "check-milestone-layout.ps1"
$repoLayoutScript = Join-Path $scriptDirectory "check-repository-layout.ps1"
$milestone = "M11-v4-full-release-integrated-acceptance"
$stamp = [DateTime]::UtcNow.ToString("yyyyMMdd-HHmmss")
$runId = [guid]::NewGuid().ToString("N").Substring(0, 8)
if ([string]::IsNullOrWhiteSpace($ResultDirectory)) {
    $ResultDirectory = Join-Path $repositoryRoot ("target-exe\m11-development-results-{0}-{1}" -f $stamp, $runId)
}
$evidenceRoot = [IO.Path]::GetFullPath($ResultDirectory)
$targetRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target-exe"))
if (-not $evidenceRoot.StartsWith($targetRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "M11 development result directory must remain under target-exe."
}
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null

$results = [System.Collections.Generic.List[object]]::new()

function Invoke-LoggedStep {
    param(
        [string]$Id,
        [string]$WorkingDirectory,
        [string]$FilePath,
        [string[]]$Arguments,
        [switch]$ContinueOnFailure,
        [switch]$Advisory
    )
    $logPath = Join-Path $evidenceRoot "$Id.log"
    $output = ""
    $exitCode = 1
    Push-Location $WorkingDirectory
    try {
        $previous = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $output = (& $FilePath @Arguments 2>&1 | Tee-Object -FilePath $logPath | Out-String)
            $exitCode = $LASTEXITCODE
        } finally {
            $ErrorActionPreference = $previous
        }
    } finally {
        Pop-Location
    }
    $status = if ($exitCode -eq 0) { "DEVELOPMENT_PASS" } elseif ($Advisory) { "ADVISORY_BLOCKED" } else { "DEVELOPMENT_FAIL" }
    $results.Add([pscustomobject]@{
        id = $Id
        status = $status
        exit_code = $exitCode
        log = $logPath
        command = (($FilePath + " " + ($Arguments -join " ")).Trim())
        reason = if ($exitCode -eq 0) { "" } else { "Command failed; see log." }
    })
    Write-Host "$Id $status"
    if ($exitCode -ne 0 -and -not $ContinueOnFailure) { throw "$Id failed. Evidence: $logPath" }
    return $exitCode
}

function Invoke-PowerShellStep {
    param([string]$Id, [string]$ScriptPath, [string[]]$Arguments)
    return Invoke-LoggedStep -Id $Id -WorkingDirectory $repositoryRoot -FilePath "powershell.exe" `
        -Arguments (@("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $ScriptPath) + $Arguments) -ContinueOnFailure
}

function Invoke-OptionalStep {
    param([string]$Id, [string]$WorkingDirectory, [string]$FilePath, [string[]]$Arguments)
    $null = Invoke-LoggedStep -Id $Id -WorkingDirectory $WorkingDirectory -FilePath $FilePath -Arguments $Arguments -ContinueOnFailure
}

function Invoke-AdvisoryStep {
    param([string]$Id, [string]$WorkingDirectory, [string]$FilePath, [string[]]$Arguments)
    $null = Invoke-LoggedStep -Id $Id -WorkingDirectory $WorkingDirectory -FilePath $FilePath -Arguments $Arguments -ContinueOnFailure -Advisory
}

function Invoke-M11LayoutStep {
    $logPath = Join-Path $evidenceRoot "T1-M11-MILESTONE-LAYOUT.log"
    try {
        $output = (Test-M11MilestoneLayout 2>&1 | Tee-Object -FilePath $logPath | Out-String)
        $results.Add([pscustomobject]@{
            id = "T1-M11-MILESTONE-LAYOUT"
            status = "DEVELOPMENT_PASS"
            exit_code = 0
            log = $logPath
            command = "internal Test-M11MilestoneLayout"
            reason = ""
        })
        Write-Host "T1-M11-MILESTONE-LAYOUT DEVELOPMENT_PASS"
    } catch {
        ($_ | Out-String) | Set-Content -LiteralPath $logPath -Encoding utf8
        $results.Add([pscustomobject]@{
            id = "T1-M11-MILESTONE-LAYOUT"
            status = "DEVELOPMENT_FAIL"
            exit_code = 1
            log = $logPath
            command = "internal Test-M11MilestoneLayout"
            reason = $_.Exception.Message
        })
        Write-Host "T1-M11-MILESTONE-LAYOUT DEVELOPMENT_FAIL"
    }
}

function Test-M11MilestoneLayout {
    $milestonePath = Join-Path $repositoryRoot "docs-codex\v4\milestones\$milestone"
    $allowed = @("SPEC.md", "PLAN.md", "TASKS.md", "INTEGRATION_TEST_PLAN.md", "E2E_TEST_PLAN.md", "MANUAL_ACCEPTANCE_CHECKLIST.md")
    if (-not (Test-Path -LiteralPath $milestonePath -PathType Container)) { throw "M11 milestone directory is missing." }
    $entries = @(Get-ChildItem -LiteralPath $milestonePath -Force)
    $unexpected = @($entries | Where-Object { $_.PSIsContainer -or $_.Name -cnotin $allowed })
    $names = @($entries | ForEach-Object { $_.Name })
    $missing = @($allowed | Where-Object { $_ -notin $names })
    if ($unexpected.Count -gt 0 -or $missing.Count -gt 0) {
        $unexpectedNames = @($unexpected | ForEach-Object { $_.Name })
        throw "M11 layout invalid; unexpected: $($unexpectedNames -join ', '); missing: $($missing -join ', ')"
    }
    Write-Output "M11 milestone layout: PASS (six plan files)"
}

function Add-NotExecuted {
    param([string]$Id, [string]$Reason)
    $results.Add([pscustomobject]@{ id = $Id; status = "NOT_EXECUTED"; exit_code = $null; log = $null; command = $null; reason = $Reason })
    Write-Host "$Id NOT_EXECUTED"
}

Push-Location $repositoryRoot
try {
    Invoke-OptionalStep "T1-GATE-SELFTEST" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $gateScript, "self-test")
    Invoke-OptionalStep "T1-REPOSITORY-LAYOUT" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $repoLayoutScript)
    Invoke-AdvisoryStep "T1-GLOBAL-MILESTONE-LAYOUT" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $layoutScript)
    Invoke-M11LayoutStep

    $fixtureCandidate = Join-Path $repositoryRoot "fixtures\v4\m11-release\candidate\valid-synthetic.json"
    $fixtureMatrix = Join-Path $repositoryRoot "fixtures\v4\m11-release\matrix\valid-synthetic.json"
    $fixtureUpstream = Join-Path $repositoryRoot "fixtures\v4\m11-release\upstream\accepted-synthetic.json"
    $fixtureDecision = Join-Path $repositoryRoot "fixtures\v4\m11-release\m09\decision-set-v1-synthetic.json"
    Invoke-OptionalStep "T1-VALIDATE-SYNTHETIC-IDENTITY" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $gateScript, "validate-identity", "-CandidatePath", $fixtureCandidate)
    Invoke-OptionalStep "T1-VALIDATE-SYNTHETIC-MATRIX" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $gateScript, "validate-matrix", "-CandidatePath", $fixtureCandidate, "-MatrixPath", $fixtureMatrix)
    Invoke-OptionalStep "T1-VALIDATE-SYNTHETIC-UPSTREAM" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $gateScript, "eligibility", "-UpstreamPath", $fixtureUpstream)
    Invoke-OptionalStep "T1-VALIDATE-SYNTHETIC-M09" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $gateScript, "validate-m09-decision", "-CandidatePath", $fixtureCandidate, "-M09DecisionPath", $fixtureDecision)

    Invoke-OptionalStep "T2-STAGE-DISTRIBUTION" $repositoryRoot "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", (Join-Path $scriptDirectory "stage-distribution.ps1"))
    Invoke-OptionalStep "T2-CONFIG-UPGRADE-ROLLBACK" $repositoryRoot "cargo" @("test", "-p", "chatgpt-harness-ui", "--lib", "m06_upgrade_and_rollback_contract_cases", "--", "--nocapture")
    Invoke-OptionalStep "T2-EVIDENCE-STORE" $repositoryRoot "cargo" @("test", "-p", "chatgpt-harness-evidence-store", "--", "--nocapture")

    Invoke-OptionalStep "T3-RUNTIME-MCP-HTTP" $repositoryRoot "cargo" @("test", "-p", "chatgpt-harness-runtime", "--bin", "chatgpt-harness-runtime", "mcp::http::tests", "--", "--nocapture")
    Invoke-OptionalStep "T3-RUNTIME-MCP-SERVICE" $repositoryRoot "cargo" @("test", "-p", "chatgpt-harness-runtime", "--bin", "chatgpt-harness-runtime", "mcp::service::tests", "--", "--nocapture")
    Invoke-OptionalStep "T3-M08-OBSERVABILITY" $repositoryRoot "cargo" @("test", "-p", "chatgpt-harness-runtime", "--bin", "chatgpt-harness-runtime", "m08_qa_integration", "--", "--nocapture")
    Invoke-OptionalStep "T3-RUNTIME-SECURITY" $repositoryRoot "cargo" @("test", "-p", "chatgpt-harness-runtime", "--bin", "chatgpt-harness-runtime", "m5_analyzer_security", "--", "--nocapture")

    if ($RunNative) {
        Invoke-PowerShellStep "T3-NATIVE-DESKTOP" (Join-Path $scriptDirectory "m07-development-native.ps1") @()
    } else {
        Add-NotExecuted "T3-NATIVE-DESKTOP" "Native Tray/Auto Start test was not requested by this development run."
    }

    if (-not $SkipFullCargoTest) {
        Invoke-OptionalStep "T6-CARGO-FMT" $repositoryRoot "cargo" @("fmt", "--all", "--", "--check")
        Invoke-OptionalStep "T6-CARGO-CHECK" $repositoryRoot "cargo" @("check", "--workspace")
        Invoke-OptionalStep "T6-CARGO-TEST" $repositoryRoot "cargo" @("test", "--workspace", "--", "--test-threads=1")
    } else {
        Add-NotExecuted "T6-CARGO-FULL" "Full Cargo checks were explicitly skipped."
    }

    if (-not $SkipUiTest) {
        Invoke-OptionalStep "T6-UI-TYPECHECK" (Join-Path $repositoryRoot "ui") "npm" @("run", "typecheck")
        Invoke-OptionalStep "T6-UI-TEST" (Join-Path $repositoryRoot "ui") "npm" @("test")
        Invoke-OptionalStep "T6-UI-BUILD" (Join-Path $repositoryRoot "ui") "npm" @("run", "build")
    } else {
        Add-NotExecuted "T6-UI-FULL" "Frontend checks were explicitly skipped."
    }

    if ($Preflight) {
        Add-NotExecuted "T6-PREflight-M06" "Legacy M06 wrapper is not used as the M11 gate; its extra ignored M03 QA cases and parallel workspace run are recorded separately. M11 direct serial Rust/UI checks above are authoritative for this development run."
    } else {
        Add-NotExecuted "T6-PREflight-M06" "Legacy M06 wrapper was not requested; M11 direct checks remain the development gate."
    }

    Add-NotExecuted "T4-F05-F12-F13-FINAL" "Final-candidate quality evidence requires accepted upstream identities and, where needed, explicit execution-time authorization."
    Add-NotExecuted "T5-PACKAGED-E2E" "Formal packaged M11 E2E remains independent validation."
    Add-NotExecuted "T5-EXTERNAL-CHATGPT" "Real External ChatGPT requires explicit execution-time authorization and Manual acceptance."
    Add-NotExecuted "T5-MANUAL" "Windows Manual acceptance was not executed by the development driver."

    if ($BuildPortable) {
        Invoke-PowerShellStep "T6-PORTABLE-BUILD" (Join-Path $scriptDirectory "deliver-milestone.ps1") @("-Milestone", $milestone)
    } else {
        Add-NotExecuted "T6-PORTABLE-BUILD" "Portable delivery was not requested by this development run."
    }
} finally {
    Pop-Location
}

$failed = @($results | Where-Object { $_.status -eq "DEVELOPMENT_FAIL" })
$summary = [ordered]@{
    schema = "m11-development-result-v1"
    milestone = $milestone
    scope = "M11 development verification; formal Integration/E2E/Manual remain NOT_EXECUTED"
    candidate = (& git -C $repositoryRoot rev-parse HEAD).Trim()
    source_dirty = -not [string]::IsNullOrWhiteSpace(((& git -C $repositoryRoot status --porcelain) -join "`n"))
    generated_at_utc = [DateTime]::UtcNow.ToString("O")
    cases = $results
    formal_integration = "NOT_EXECUTED"
    formal_e2e = "NOT_EXECUTED"
    real_external_chatgpt = "NOT_EXECUTED"
    formal_manual = "NOT_EXECUTED"
    verdict = if ($failed.Count -eq 0) { "DEVELOPMENT_REGRESSION_PASS" } else { "DEVELOPMENT_REGRESSION_FAIL" }
    failed_case_count = $failed.Count
}
$resultPath = Join-Path $evidenceRoot "m11-development-result.json"
$summary | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $resultPath -Encoding utf8
Write-Output "M11 development evidence: $evidenceRoot"
Write-Output "M11 development verdict: $($summary.verdict)"
if ($failed.Count -gt 0) { exit 1 }
