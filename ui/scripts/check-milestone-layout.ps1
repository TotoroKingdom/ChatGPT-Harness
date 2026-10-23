[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "check-repository-layout.ps1")
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$milestonesRoot = Join-Path $repositoryRoot "docs-codex\v4\milestones"
$allowed = @("SPEC.md", "PLAN.md", "TASKS.md", "INTEGRATION_TEST_PLAN.md", "E2E_TEST_PLAN.md", "MANUAL_ACCEPTANCE_CHECKLIST.md")
foreach ($milestone in Get-ChildItem -LiteralPath $milestonesRoot -Directory) {
    $entries = @(Get-ChildItem -LiteralPath $milestone.FullName -Force)
    $unexpected = @($entries | Where-Object { $_.PSIsContainer -or $_.Name -cnotin $allowed })
    $missing = @($allowed | Where-Object { $_ -cnotin $entries.Name })
    if ($unexpected.Count -gt 0 -or $missing.Count -gt 0) {
        throw "Invalid milestone layout: $($milestone.Name); unexpected: $($unexpected.Name -join ', '); missing: $($missing -join ', ')"
    }
}
Write-Output "Milestone layout: PASS (six plan files per milestone)"
