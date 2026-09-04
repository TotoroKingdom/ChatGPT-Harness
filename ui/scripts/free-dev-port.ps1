[CmdletBinding()]
param(
    [int]$Port = 1420,
    [int]$MaxWaitSeconds = 5
)

$ErrorActionPreference = 'Stop'

$scriptRoot = (Resolve-Path $PSScriptRoot).Path
$repoRoot = (Resolve-Path (Join-Path $scriptRoot '..\..')).Path.TrimEnd('\', '/')
$repoRootNormalized = $repoRoot.Replace('/', '\').ToLowerInvariant()

function Get-PortListeners {
    @(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique)
}

function Get-ProcessInfo([int]$ProcessId) {
    Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId" -ErrorAction SilentlyContinue
}

function Test-ProjectViteProcess($ProcessInfo) {
    if ($null -eq $ProcessInfo -or [string]::IsNullOrWhiteSpace($ProcessInfo.CommandLine)) {
        return $false
    }

    $commandLine = $ProcessInfo.CommandLine.Replace('/', '\').ToLowerInvariant()
    $hasRepositoryPath = $commandLine.Contains($repoRootNormalized)
    $hasViteMarker =
        $commandLine -match '(?i)(^|[\s"/\\])vite([.\-\s/\\]|$)' -or
        $commandLine -match '(?i)node_modules[\\/]vite[\\/]' -or
        $commandLine -match '(?i)npm(?:\.cmd)?[\s"/\\]+run[\s"/\\]+dev'

    return $hasRepositoryPath -and $hasViteMarker
}

$listenerPids = Get-PortListeners
if ($listenerPids.Count -eq 0) {
    Write-Host "Port $Port is free."
    exit 0
}

$projectPids = @()
$otherOccupants = @()

foreach ($listenerPid in $listenerPids) {
    $processInfo = Get-ProcessInfo $listenerPid
    $name = if ($null -ne $processInfo -and $processInfo.Name) { $processInfo.Name } else { 'unknown' }

    if (Test-ProjectViteProcess $processInfo) {
        $projectPids += [int]$listenerPid
        Write-Host "Found leftover ChatGPT-Harness Vite listener: PID $listenerPid ($name)."
    }
    else {
        $otherOccupants += [pscustomobject]@{
            ProcessId = [int]$listenerPid
            Name = $name
        }
    }
}

if ($otherOccupants.Count -gt 0) {
    foreach ($occupant in $otherOccupants) {
        [Console]::Error.WriteLine("Port $Port is occupied by PID $($occupant.ProcessId) ($($occupant.Name)). It does not appear to belong to ChatGPT-Harness, so it was not terminated.")
    }
    exit 1
}

foreach ($projectPid in ($projectPids | Sort-Object -Unique)) {
    try {
        Stop-Process -Id $projectPid -Force -ErrorAction Stop
        Write-Host "Terminated leftover ChatGPT-Harness Vite process PID $projectPid."
    }
    catch {
        [Console]::Error.WriteLine("Failed to terminate leftover ChatGPT-Harness Vite process PID ${projectPid}: $($_.Exception.Message)")
        exit 1
    }
}

$deadline = (Get-Date).AddSeconds($MaxWaitSeconds)
do {
    Start-Sleep -Milliseconds 200
    $remainingPids = Get-PortListeners
} while ($remainingPids.Count -gt 0 -and (Get-Date) -lt $deadline)

if ($remainingPids.Count -gt 0) {
    foreach ($remainingPid in $remainingPids) {
        $processInfo = Get-ProcessInfo $remainingPid
        $name = if ($null -ne $processInfo -and $processInfo.Name) { $processInfo.Name } else { 'unknown' }
        [Console]::Error.WriteLine("Port $Port is still occupied by PID $remainingPid ($name) after cleanup.")
    }
    exit 1
}

Write-Host "Port $Port was released."
exit 0
