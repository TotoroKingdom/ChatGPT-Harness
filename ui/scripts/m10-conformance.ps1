[CmdletBinding()]
param(
    [string]$RuntimeBinary = "",
    [string]$ResultDirectory = "",
    [string]$NodeExecutable = "",
    [string]$NpmExecutable = "",
    [switch]$KeepTemp,
    [switch]$SkipServerRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDirectory = (Resolve-Path (Join-Path $scriptDirectory "..")).Path
$repositoryRoot = (Resolve-Path (Join-Path $uiDirectory "..")).Path
$runtimeProject = Join-Path $repositoryRoot "runtime"
$runnerCommit = "c37eec888e1c6ff140af79987a40008548b7cc5f"
$runnerRepository = "https://github.com/modelcontextprotocol/conformance.git"
$runnerPackage = "0.2.0-alpha.12"
$runnerSdk = "1.29.0"
$mcpRevision = "2025-03-26"

function Resolve-FullPath([string]$Path) {
    return [IO.Path]::GetFullPath($Path)
}

if ([string]::IsNullOrWhiteSpace($NodeExecutable)) {
    $NodeExecutable = (Get-Command node.exe -ErrorAction Stop).Source
} else {
    $NodeExecutable = Resolve-FullPath $NodeExecutable
}
if ([string]::IsNullOrWhiteSpace($NpmExecutable)) {
    $NpmExecutable = (Get-Command npm.cmd -ErrorAction Stop).Source
} else {
    $NpmExecutable = Resolve-FullPath $NpmExecutable
}
$gitExecutable = (Get-Command git.exe -ErrorAction Stop).Source

function Write-Utf8Text([string]$Path, [string]$Text) {
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}

function Write-Utf8Json([string]$Path, [object]$Value) {
    Write-Utf8Text $Path (($Value | ConvertTo-Json -Depth 20) + [Environment]::NewLine)
}

function Set-ProcessArguments([Diagnostics.ProcessStartInfo]$StartInfo, [string[]]$Arguments) {
    # Windows PowerShell 5.1 does not expose ProcessStartInfo.ArgumentList.
    # These M10 commands receive paths/URLs without embedded quotes; quoting
    # every argument keeps spaces in temporary/user paths intact.
    $StartInfo.Arguments = (($Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' ')
}

function Invoke-Checked([string]$File, [string[]]$Arguments, [string]$WorkingDirectory, [string]$StdoutPath, [string]$StderrPath) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $File
    $start.WorkingDirectory = $WorkingDirectory
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    Set-ProcessArguments $start $Arguments
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "Could not start $File." }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $stdoutText = $stdout.Result
    $stderrText = $stderr.Result
    Write-Utf8Text $StdoutPath $stdoutText
    Write-Utf8Text $StderrPath $stderrText
    if ($process.ExitCode -ne 0) {
        throw "$File exited with $($process.ExitCode). See $StdoutPath and $StderrPath."
    }
    return [pscustomobject]@{ ExitCode = $process.ExitCode; Stdout = $stdoutText; Stderr = $stderrText }
}

function Invoke-CheckedAllowFailure([string]$File, [string[]]$Arguments, [string]$WorkingDirectory, [string]$StdoutPath, [string]$StderrPath) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $File
    $start.WorkingDirectory = $WorkingDirectory
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    Set-ProcessArguments $start $Arguments
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "Could not start $File." }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $stdoutText = $stdout.Result
    $stderrText = $stderr.Result
    Write-Utf8Text $StdoutPath $stdoutText
    Write-Utf8Text $StderrPath $stderrText
    return [pscustomobject]@{ ExitCode = $process.ExitCode; Stdout = $stdoutText; Stderr = $stderrText }
}

function Start-Runtime([string]$Binary, [string]$Workspace, [string]$EvidenceDatabase, [string]$LogDirectory) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Binary
    $start.WorkingDirectory = $repositoryRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "Could not start Runtime: $Binary" }
    $stderrTask = $process.StandardError.ReadToEndAsync()

    function Send-Control([object]$Message) {
        $json = $Message | ConvertTo-Json -Depth 20 -Compress
        $process.StandardInput.WriteLine($json)
        $process.StandardInput.Flush()
        while ($true) {
            $line = $process.StandardOutput.ReadLine()
            if ($null -eq $line) { throw "Runtime stdout closed while waiting for control response." }
            if ([string]::IsNullOrWhiteSpace($line)) { continue }
            $value = $line | ConvertFrom-Json
            if ($value.id -eq $Message.id) { return $value }
        }
    }

    $runtimeConfig = [ordered]@{
        configRevision = "1"
        workspace = [ordered]@{ rootPath = $Workspace }
        security = [ordered]@{ mode = "normal"; developerTestMode = $false }
        runtimeLimits = [ordered]@{ maxConcurrentRequests = 4; maxActivityEvents = 256; maxReadBytes = 1048576 }
        tools = [ordered]@{
            readFile = [ordered]@{ enabled = $true }
            glob = [ordered]@{ enabled = $true }
            grep = [ordered]@{ enabled = $true }
            write = [ordered]@{ enabled = $true }
            edit = [ordered]@{ enabled = $true }
            bash = [ordered]@{ enabled = $false }
        }
        bash = [ordered]@{ autoDetect = $true }
        policies = [ordered]@{
            toolDefaults = [ordered]@{ readFile = "allow"; glob = "allow"; grep = "allow"; write = "ask"; edit = "ask"; bash = "deny" }
            userRules = @()
        }
    }
    $initialize = Send-Control ([ordered]@{
        jsonrpc = "2.0"
        id = "m10-control-1"
        method = "initialize"
        params = [ordered]@{
            clientInfo = [ordered]@{ name = "m10-conformance-runner"; title = "M10 Development Conformance"; version = "1" }
            protocolVersion = 1
            capabilities = [ordered]@{ activityReplay = $true }
            runtimeConfig = $runtimeConfig
            hostContext = [ordered]@{
                hostSessionId = "m10-development"
                supervisorGenerationId = "m10-conformance"
                evidenceStore = [ordered]@{ enabled = $true; databasePath = $EvidenceDatabase; retentionDays = 30; maxStorageMiB = 32 }
            }
        }
    })
    if ($null -ne $initialize.PSObject.Properties["error"]) { throw "Runtime control initialize failed." }
    $process.StandardInput.WriteLine((ConvertTo-Json ([ordered]@{ jsonrpc = "2.0"; method = "initialized"; params = [ordered]@{} }) -Depth 10 -Compress))
    $process.StandardInput.Flush()
    $endpoint = Send-Control ([ordered]@{ jsonrpc = "2.0"; id = "m10-control-2"; method = "mcp.endpoint.get"; params = [ordered]@{} })
    $url = $endpoint.result.url
    if ([string]::IsNullOrWhiteSpace($url) -or $url -notmatch '^http://127\.0\.0\.1:\d+/mcp$') {
        throw "Runtime returned an invalid loopback MCP endpoint."
    }
    return [pscustomobject]@{ Process = $process; StderrTask = $stderrTask; Endpoint = $url }
}

function Stop-Runtime([object]$Runtime, [string]$LogDirectory) {
    if ($null -eq $Runtime) { return }
    try {
        if (-not $Runtime.Process.HasExited) {
            $Runtime.Process.StandardInput.WriteLine((ConvertTo-Json ([ordered]@{ jsonrpc = "2.0"; id = "m10-control-stop"; method = "runtime.stop"; params = [ordered]@{} }) -Depth 10 -Compress))
            $Runtime.Process.StandardInput.Flush()
            $Runtime.Process.WaitForExit(5000) | Out-Null
        }
    } catch { }
    if (-not $Runtime.Process.HasExited) { try { $Runtime.Process.Kill() } catch { } }
    try { Write-Utf8Text (Join-Path $LogDirectory "runtime-stderr.txt") $Runtime.StderrTask.Result } catch { }
    $Runtime.Process.Dispose()
}

function Get-CheckSummary([string]$OutputDirectory) {
    $files = @(Get-ChildItem -LiteralPath $OutputDirectory -Recurse -File -Filter "checks.json" -ErrorAction SilentlyContinue)
    if ($files.Count -eq 0) { throw "Official runner emitted no checks.json." }
    $checks = @()
    foreach ($file in $files) {
        $value = Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json
        if ($value -is [array]) { $checks += $value } elseif ($null -ne $value.checks) { $checks += @($value.checks) }
    }
    $failures = @($checks | Where-Object { $_.status -eq "FAILURE" })
    $warnings = @($checks | Where-Object { $_.status -eq "WARNING" })
    $successes = @($checks | Where-Object { $_.status -eq "SUCCESS" })
    $skips = @($checks | Where-Object { $_.status -in @("SKIP", "NOT_APPLICABLE") })
    return [ordered]@{
        checksFiles = @($files | ForEach-Object { $_.FullName })
        total = $checks.Count
        successes = $successes.Count
        failures = $failures.Count
        warnings = $warnings.Count
        skips = $skips.Count
        failureNames = @($failures | ForEach-Object {
            if ($_.name) { $_.name }
            elseif ($_.check) { $_.check }
            else { $_.id }
        })
        status = if ($failures.Count -eq 0) { "PASS" } else { "FAILURE" }
    }
}

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("m10-conformance-" + [guid]::NewGuid().ToString("N"))
$runnerRoot = Join-Path $tempRoot "runner"
$runtimeRoot = Join-Path $tempRoot "runtime"
$workspaceRoot = Join-Path $runtimeRoot "workspace"
$evidenceDatabase = Join-Path $runtimeRoot "evidence.db"
$runnerOutput = Join-Path $tempRoot "official-results"
if ([string]::IsNullOrWhiteSpace($ResultDirectory)) { $ResultDirectory = Join-Path $repositoryRoot ("target-exe\m10-conformance-" + (Get-Date -Format "yyyyMMdd-HHmmss")) }
$resultPath = Resolve-FullPath $ResultDirectory
$runtime = $null
$exitCode = 0

try {
    New-Item -ItemType Directory -Force -Path $runnerRoot, $workspaceRoot, $runnerOutput, $resultPath | Out-Null
    Set-Content -LiteralPath (Join-Path $workspaceRoot "m10-safe-read.txt") -Value "M10 official conformance fixture" -Encoding utf8
    Set-Content -LiteralPath (Join-Path $runtimeRoot "outside-m10-secret.txt") -Value "M10 outside sentinel" -Encoding utf8
    if ([string]::IsNullOrWhiteSpace($RuntimeBinary)) { $RuntimeBinary = Join-Path $repositoryRoot "target\debug\chatgpt-harness-runtime.exe" }
    $RuntimeBinary = Resolve-FullPath $RuntimeBinary
    if (-not (Test-Path -LiteralPath $RuntimeBinary -PathType Leaf)) { throw "Runtime binary is missing: $RuntimeBinary" }

    Invoke-Checked $gitExecutable @("clone", "--no-checkout", $runnerRepository, $runnerRoot) $repositoryRoot (Join-Path $resultPath "git-clone.stdout.txt") (Join-Path $resultPath "git-clone.stderr.txt") | Out-Null
    Invoke-Checked $gitExecutable @("checkout", "--detach", $runnerCommit) $runnerRoot (Join-Path $resultPath "git-checkout.stdout.txt") (Join-Path $resultPath "git-checkout.stderr.txt") | Out-Null
    $actualCommit = (& $gitExecutable -C $runnerRoot rev-parse HEAD).Trim()
    if ($actualCommit -ne $runnerCommit) { throw "Official runner commit mismatch: $actualCommit" }
    $package = Get-Content -Raw -LiteralPath (Join-Path $runnerRoot "package.json") | ConvertFrom-Json
    if ($package.version -ne $runnerPackage) { throw "Official runner package mismatch: $($package.version)" }
    Invoke-Checked $npmExecutable @("ci") $runnerRoot (Join-Path $resultPath "npm-ci.stdout.txt") (Join-Path $resultPath "npm-ci.stderr.txt") | Out-Null
    Invoke-Checked $npmExecutable @("run", "build") $runnerRoot (Join-Path $resultPath "npm-build.stdout.txt") (Join-Path $resultPath "npm-build.stderr.txt") | Out-Null
    $lockfilePath = Join-Path $runnerRoot "package-lock.json"
    $sdkVersion = (& $nodeExecutable -e "const p=require(process.argv[1]); process.stdout.write(p.packages['node_modules/@modelcontextprotocol/sdk'].version)" $lockfilePath).Trim()
    if ($sdkVersion -ne $runnerSdk) { throw "Official runner SDK mismatch: $sdkVersion" }

    $list = Invoke-Checked $nodeExecutable @("dist/index.js", "list", "--server", "--spec-version", $mcpRevision) $runnerRoot (Join-Path $resultPath "official-list.stdout.txt") (Join-Path $resultPath "official-list.stderr.txt")
    $listHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $resultPath "official-list.stdout.txt")).Hash
    $runtime = Start-Runtime $RuntimeBinary $workspaceRoot $evidenceDatabase $resultPath
    $serverResult = $null
    if (-not $SkipServerRun) {
        $serverResult = Invoke-CheckedAllowFailure $nodeExecutable @("dist/index.js", "server", "--url", $runtime.Endpoint, "--suite", "active", "--spec-version", $mcpRevision, "--output-dir", $runnerOutput, "--verbose") $runnerRoot (Join-Path $resultPath "official-server.stdout.txt") (Join-Path $resultPath "official-server.stderr.txt")
    }
    $summary = if ($SkipServerRun) { [ordered]@{ status = "NOT_VERIFIED"; reason = "server run skipped" } } else { Get-CheckSummary $runnerOutput }
    if ($null -ne $serverResult -and $serverResult.ExitCode -ne 0) {
        $summary.status = "NOT_VERIFIED"
        $summary.reason = "official runner exited with code $($serverResult.ExitCode)"
    }
    $preservedResults = Join-Path $resultPath "official-results"
    if (Test-Path -LiteralPath $preservedResults) {
        Remove-Item -LiteralPath $preservedResults -Recurse -Force
    }
    if (-not $SkipServerRun) {
        Copy-Item -LiteralPath $runnerOutput -Destination $preservedResults -Recurse -Force
        $summary.checksFiles = @(
            Get-ChildItem -LiteralPath $preservedResults -Recurse -File -Filter "checks.json" |
                ForEach-Object { $_.FullName }
        )
    }
    $manifest = [ordered]@{
        schema = "m10-development-conformance-v1"
        runnerRepository = "modelcontextprotocol/conformance"
        runnerCommit = $runnerCommit
        runnerPackage = $runnerPackage
        runnerSdk = $runnerSdk
        standaloneSdk = "1.29.0"
        mcpRevision = $mcpRevision
        runtimeEndpoint = $runtime.Endpoint
        runtimeBinary = $RuntimeBinary
        nodeVersion = (& $nodeExecutable --version).Trim()
        npmVersion = (& $npmExecutable --version).Trim()
        packageLockSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $runnerRoot "package-lock.json")).Hash
        officialListSha256 = $listHash
        officialServerCommand = "node dist/index.js server --url <runtime-loopback-mcp-url> --suite active --spec-version 2025-03-26 --output-dir <disposable-output-dir> --verbose"
        officialResultsDirectory = if ($SkipServerRun) { $null } else { $preservedResults }
        serverExitCode = if ($null -eq $serverResult) { $null } else { $serverResult.ExitCode }
        officialSummary = $summary
    }
    Write-Utf8Json (Join-Path $resultPath "m10-conformance-manifest.json") $manifest
    if ($summary.status -ne "PASS") { $exitCode = 1 }
} catch {
    $exitCode = 1
    $diagnostic = [ordered]@{
        schema = "m10-development-conformance-v1"
        status = "NOT_VERIFIED"
        error = $_.Exception.Message
        exceptionType = $_.Exception.GetType().FullName
        invocationInfo = if ($null -ne $_.InvocationInfo) { $_.InvocationInfo.PositionMessage } else { $null }
        scriptStackTrace = $_.ScriptStackTrace
    }
    Write-Utf8Json (Join-Path $resultPath "m10-conformance-manifest.json") $diagnostic
    Write-Error $_.Exception.Message
} finally {
    Stop-Runtime $runtime $resultPath
    if (-not $KeepTemp -and (Test-Path -LiteralPath $tempRoot)) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}

Write-Output "M10 conformance result: $resultPath"
exit $exitCode
