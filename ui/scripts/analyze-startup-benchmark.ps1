[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string[]]$InputPath,
    [Parameter(Mandatory = $true)]
    [ValidateSet("cold", "warm")]
    [string]$Cohort,
    [ValidateRange(0, 300)]
    [int]$SkipPrimer = 0
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-NearestRank([object[]]$Values, [double]$Percentile) {
    if ($Values.Count -eq 0) {
        return $null
    }
    $sorted = @($Values | Sort-Object { [double]$_ })
    $rank = [Math]::Max(1, [int][Math]::Ceiling($Percentile * $sorted.Count))
    return [double]$sorted[$rank - 1]
}

function Get-PhaseStats([object[]]$Values) {
    [ordered]@{
        p50 = Get-NearestRank $Values 0.5
        p95 = Get-NearestRank $Values 0.95
    }
}

function Get-StartupStats([object[]]$Attempts, [string]$ExpectedCohort, [int]$PrimerCount) {
    $cohortAttempts = @($Attempts | Where-Object { $_.cohort -eq $ExpectedCohort })
    if ($cohortAttempts.Count -le $PrimerCount) {
        throw "Not enough $ExpectedCohort attempts: found $($cohortAttempts.Count), need more than $PrimerCount primer samples."
    }
    $counted = @($cohortAttempts | Select-Object -Skip $PrimerCount)
    $ready = @($counted | Where-Object { $_.outcome -eq "ready" -and $null -ne $_.totalDurationMs })
    $failures = $counted.Count - $ready.Count

    $totalValues = @($ready | ForEach-Object { [double]$_.totalDurationMs })
    $phaseNames = @($ready |
        ForEach-Object { @($_.phases) } |
        Where-Object { $_.status -eq "completed" } |
        Select-Object -ExpandProperty phase -Unique)
    $phaseStats = [ordered]@{}
    foreach ($phase in $phaseNames) {
        $phaseValues = @($ready |
            ForEach-Object { @($_.phases) } |
            Where-Object { $_.phase -eq $phase -and $_.status -eq "completed" } |
            ForEach-Object { [double]$_.durationMs })
        $phaseStats[$phase] = Get-PhaseStats $phaseValues
    }

    $hostPhases = @(
        "eligibility_prepare",
        "runtime_spawn",
        "runtime_initialize",
        "mcp_endpoint_ready",
        "tunnel_resolve",
        "tunnel_launch",
        "harness_ready"
    )
    $slowestPhase = $null
    $slowestValue = $null
    foreach ($phase in $hostPhases) {
        if ($phaseStats.Contains($phase) -and $null -ne $phaseStats[$phase].p50) {
            $value = [double]$phaseStats[$phase].p50
            if ($null -eq $slowestValue -or $value -gt $slowestValue) {
                $slowestPhase = $phase
                $slowestValue = $value
            }
        }
    }

    [ordered]@{
        cohort = $ExpectedCohort
        acceptedStarts = $counted.Count
        readySamples = $ready.Count
        failures = $failures
        failureRate = if ($counted.Count -eq 0) { 0 } else { $failures / $counted.Count }
        total = Get-PhaseStats $totalValues
        phases = $phaseStats
        slowestPhase = $slowestPhase
    }
}

$attempts = @()
foreach ($path in $InputPath) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Benchmark input is missing: $path"
    }
    foreach ($line in Get-Content -LiteralPath $path) {
        if (-not [string]::IsNullOrWhiteSpace($line)) {
            $attempts += ($line | ConvertFrom-Json)
        }
    }
}

$result = Get-StartupStats $attempts $Cohort $SkipPrimer
$result | ConvertTo-Json -Depth 12
