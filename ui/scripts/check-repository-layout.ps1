[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# Existing layout approved by the user. Adding an entry requires explicit user authorization.
$allowedDirectories = @(
    ".git", ".idea", "artifacts", "assets", "docs", "docs-codex", "fixtures",
    "protocol", "runtime", "target", "target-exe", "tunnel-client-v0.0.13-windows-amd64",
    "ui", "wiki"
)
$unexpected = @(Get-ChildItem -LiteralPath $repositoryRoot -Directory -Force |
    Where-Object { $_.Name -cnotin $allowedDirectories })
if ($unexpected.Count -gt 0) {
    throw "Unapproved top-level directories: $($unexpected.Name -join ', '). Reuse existing module directories; do not extend the allowlist without explicit user authorization."
}
Write-Output "Repository top-level layout: PASS"
