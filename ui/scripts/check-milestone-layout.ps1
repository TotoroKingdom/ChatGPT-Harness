[CmdletBinding()]
param(
    [string]$Milestone = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "check-repository-layout.ps1")
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$milestonesRoot = Join-Path $repositoryRoot "docs-codex\v4\milestones"
$allowed = @("SPEC.md", "PLAN.md", "TASKS.md", "INTEGRATION_TEST_PLAN.md", "E2E_TEST_PLAN.md", "MANUAL_ACCEPTANCE_CHECKLIST.md")
$milestoneDirectories = if ([string]::IsNullOrWhiteSpace($Milestone)) {
    @(Get-ChildItem -LiteralPath $milestonesRoot -Directory | ForEach-Object { $_.FullName })
} else {
    $requestedPath = Join-Path $milestonesRoot $Milestone
    if (-not (Test-Path -LiteralPath $requestedPath -PathType Container)) {
        throw "Milestone plans do not exist: $Milestone"
    }
    @([IO.Path]::GetFullPath($requestedPath))
}
foreach ($milestonePath in $milestoneDirectories) {
    $milestoneName = Split-Path -Leaf $milestonePath
    $entries = @(Get-ChildItem -LiteralPath $milestonePath -Force)
    $entryNames = @($entries | ForEach-Object { $_.Name })
    $unexpected = @($entries | Where-Object { $_.PSIsContainer -or $_.Name -cnotin $allowed })
    $missing = @($allowed | Where-Object { $_ -cnotin $entryNames })
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
