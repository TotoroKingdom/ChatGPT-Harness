[CmdletBinding()]
param(
    [string]$Milestone = "",
    [switch]$AllowM09DevelopmentIncomplete
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "check-repository-layout.ps1")
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$milestonesRoot = Join-Path $repositoryRoot "docs-codex\v4\milestones"
$allowed = @("SPEC.md", "PLAN.md", "TASKS.md", "INTEGRATION_TEST_PLAN.md", "E2E_TEST_PLAN.md", "MANUAL_ACCEPTANCE_CHECKLIST.md")
$validationDocs = @("INTEGRATION_TEST_PLAN.md", "E2E_TEST_PLAN.md", "MANUAL_ACCEPTANCE_CHECKLIST.md")

if ($AllowM09DevelopmentIncomplete -and $Milestone -ne "M09-file-tool-admission-qualified-expansion") {
    throw "-AllowM09DevelopmentIncomplete is valid only for M09-file-tool-admission-qualified-expansion."
}

if ($AllowM09DevelopmentIncomplete) {
    $milestonePaths = @([IO.Path]::GetFullPath((Join-Path $milestonesRoot $Milestone)))
    Write-Output "Development-only scope: M09; global milestone layout is NOT validated by this invocation."
} elseif ([string]::IsNullOrWhiteSpace($Milestone)) {
    $milestonePaths = @(Get-ChildItem -LiteralPath $milestonesRoot -Directory | ForEach-Object { $_.FullName })
} else {
    $requestedPath = Join-Path $milestonesRoot $Milestone
    if (-not (Test-Path -LiteralPath $requestedPath -PathType Container)) {
        throw "Milestone plans do not exist: $Milestone"
    }
    $milestonePaths = @([IO.Path]::GetFullPath($requestedPath))
}

foreach ($milestonePath in $milestonePaths) {
    $milestoneName = Split-Path -Leaf $milestonePath
    $entries = @(Get-ChildItem -LiteralPath $milestonePath -Force)
    $entryNames = @($entries | ForEach-Object { $_.Name })
    $unexpected = @($entries | Where-Object { $_.PSIsContainer -or $_.Name -cnotin $allowed })
    $missing = @($allowed | Where-Object { $_ -cnotin $entryNames })
    if ($AllowM09DevelopmentIncomplete -and $milestoneName -eq "M09-file-tool-admission-qualified-expansion") {
        $missingValidationDocs = @($validationDocs | Where-Object { $_ -cnotin $entryNames })
        $unexpectedMissing = @($missing | Where-Object { $_ -notin $validationDocs })
        if ($unexpected.Count -gt 0 -or $unexpectedMissing.Count -gt 0) {
            $unexpectedNames = @($unexpected | ForEach-Object { $_.Name })
            throw "Invalid M09 development layout; unexpected: $($unexpectedNames -join ', '); missing non-validation files: $($unexpectedMissing -join ', ')"
        }
        Write-Output "M09 development layout: allowed missing validation plans: $($missingValidationDocs -join ', '); formal validation remains unexecuted"
        continue
    }
    if ($unexpected.Count -gt 0 -or $missing.Count -gt 0) {
        $unexpectedNames = @($unexpected | ForEach-Object { $_.Name })
        throw "Invalid milestone layout: $milestoneName; unexpected: $($unexpectedNames -join ', '); missing: $($missing -join ', ')"
    }
}

if ([string]::IsNullOrWhiteSpace($Milestone)) {
    Write-Output "Milestone layout: PASS (six plan files per milestone)"
} else {
    Write-Output "Milestone layout: PASS ($Milestone has six plan files)"
}
