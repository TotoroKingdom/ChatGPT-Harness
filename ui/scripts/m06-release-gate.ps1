[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet(
        "collect-identity",
        "validate-identity",
        "eligibility",
        "validate-matrix",
        "equivalence-f05",
        "equivalence-f12",
        "audit-package",
        "release-notes",
        "gate",
        "self-test"
    )]
    [string]$Command,

    [string]$CandidatePath = "",
    [string]$MatrixPath = "",
    [string]$UpstreamPath = "",
    [string]$M02EvidencePath = "",
    [string]$M04BaselinePath = "",
    [string]$M05RegressionPath = "",
    [string]$ConfigProfilePath = "",
    [string]$DistributionDirectory = "",
    [string]$PortableDirectory = "",
    [string]$InstallerPath = "",
    [string]$M04EvalBinary = "",
    [string]$OutputPath = "",
    [string]$Case = "",
    [switch]$RequireInstaller,
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$fixtureRoot = Join-Path $repositoryRoot "fixtures\v4\m06"
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$EmDash = [string][char]0x2014

$CoreScopeOwners = [ordered]@{
    "SEC-AC01" = "M03"
    "SEC-AC02" = "M03"
    "SEC-AC03" = "M03"
    "F01-AC01" = "M01"
    "F01-AC02" = "M01"
    "F01-AC03" = "M01"
    "F03-AC01" = "M01"
    "F03-AC02" = "M01"
    "F03-AC03" = "M01"
    "F04-AC01" = "M01"
    "F04-AC02" = "M01"
    "F04-AC03" = "M01"
    "F05-AC01" = "M02"
    "F05-AC02" = "M02"
    "F05-AC03" = "M02"
    "F06-AC01" = "M02"
    "F06-AC02" = "M02"
    "F06-AC03" = "M02"
    "F07-AC01" = "M03"
    "F07-AC02" = "M03"
    "F07-AC03" = "M03"
    "F10-AC01" = "M05"
    "F10-AC02" = "M05"
    "F10-AC03" = "M05"
    "F12-AC01" = "M04"
    "F12-AC02" = "M04"
    "F12-AC03" = "M04"
    "F12-AC04" = "M05"
}

$RequiredReleaseRows = @(
    "V3-WORKSPACE-ESCAPE",
    "V3-TOOL-TOGGLE",
    "V3-POLICY",
    "V3-APPROVAL",
    "V3-SESSION-GRANT",
    "V3-WRITE-EDIT-ATOMICITY",
    "V3-BASH",
    "V3-SECRETSTORE",
    "V3-LIFECYCLE",
    "V3-DESIRED-EFFECTIVE",
    "V3-CONFIG-REVISION-RESTART-DEBT",
    "V3-I18N-APPEARANCE",
    "V3-SECRET-PATH-LEAKAGE",
    "REL-PORTABLE-PACKAGE",
    "REL-NSIS-INSTALLER",
    "REL-INSTALLED-PAYLOAD",
    "REL-SHIPPING-CONTENT",
    "REL-UPGRADE-V2",
    "REL-UPGRADE-V3",
    "REL-HISTORY-COMPAT",
    "REL-ROLLBACK",
    "PERF-F05-EVIDENCE",
    "EVAL-F12-EVIDENCE",
    "EXT-TUNNEL-E2E",
    "EXT-WINDOWS-TRAY-TOAST",
    "EXT-REAL-CHATGPT"
)

$AllowedFinalStatuses = @("PASS", "FAIL", "BLOCKED", "INCONCLUSIVE", "NOT_VERIFIED")
$AllowedValidationStatuses = @("PASS", "FAIL", "BLOCKED", "INCONCLUSIVE", "NOT_VERIFIED", "NOT_REQUIRED")
$AllowedClassifications = @("CORE_MANDATORY", "OUT_OF_CORE_P1", "DEFERRED_P2", "SEPARATELY_ACCEPTED_OPTIONAL")
$RequiredCandidateHashes = @(
    "portable_package_sha256",
    "portable_ui_sha256",
    "portable_runtime_sha256",
    "portable_tunnel_sha256",
    "installer_sha256",
    "installed_ui_sha256",
    "installed_runtime_sha256",
    "installed_tunnel_sha256",
    "generated_protocol_schema_hash",
    "config_profile_hash",
    "effective_tool_catalog_hash",
    "m04_dataset_hash",
    "m04_system_prompt_hash",
    "m04_scorer_hash",
    "m04_tool_schema_hash",
    "m05_candidate_profile_hash",
    "m05_tool_schema_hash"
)
$ForbiddenShippingNames = @(
    "config.json",
    "history.sqlite3",
    "history.sqlite",
    "history.db",
    ".env"
)
$ForbiddenShippingSuffixes = @(".db", ".sqlite", ".sqlite3", ".pem", ".key", ".p12", ".pfx")
$ForbiddenShippingDirectories = @("`$PLUGINSDIR", "m04-eval", "m05-eval", "workspaces", "stale-previous")
$ShippingSentinels = @(
    "M06_TEST_SECRET",
    "M06_SECRET_SENTINEL",
    "sk-test-",
    "BEGIN PRIVATE KEY",
    "M04_EVAL_API_KEY",
    "M05_EVAL_API_KEY"
)

function Get-Field {
    param([object]$Object, [string]$Name)
    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
}

function Read-JsonFile {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required JSON file is missing: $Path"
    }
    return (Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json)
}

function Copy-JsonObject {
    param([object]$Value)
    return ($Value | ConvertTo-Json -Depth 32 | ConvertFrom-Json)
}

function Get-Sha256Text {
    param([string]$Text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text))
    } finally {
        $sha.Dispose()
    }
    return ([BitConverter]::ToString($hash)).Replace("-", "")
}

function Get-Sha256File {
    param([string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try {
            $hash = $sha.ComputeHash($stream)
        } finally {
            $sha.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
    return ([BitConverter]::ToString($hash)).Replace("-", "")
}

function Get-OptionalSha256File {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $null
    }
    return (Get-Sha256File $Path)
}

function ConvertTo-CanonicalJson {
    param([object]$Value)
    if ($null -eq $Value) { return "null" }
    if ($Value -is [string]) { return (ConvertTo-Json -InputObject ([string]$Value) -Compress) }
    if ($Value -is [bool]) {
        if ($Value) { return "true" }
        return "false"
    }
    if ($Value -is [int] -or $Value -is [long] -or $Value -is [double] -or
        $Value -is [single] -or $Value -is [decimal]) {
        return ([Convert]::ToString($Value, [Globalization.CultureInfo]::InvariantCulture))
    }
    if ($Value -is [System.Collections.IDictionary]) {
        $names = @($Value.Keys | ForEach-Object { [string]$_ } | Sort-Object)
        $parts = @()
        foreach ($name in $names) {
            $parts += ((ConvertTo-Json -InputObject $name -Compress) + ":" + (ConvertTo-CanonicalJson $Value[$name]))
        }
        return "{" + ($parts -join ",") + "}"
    }
    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        $names = @($Value.PSObject.Properties | ForEach-Object { $_.Name } | Sort-Object)
        $parts = @()
        foreach ($name in $names) {
            $property = $Value.PSObject.Properties[$name]
            $parts += ((ConvertTo-Json -InputObject $name -Compress) + ":" + (ConvertTo-CanonicalJson $property.Value))
        }
        return "{" + ($parts -join ",") + "}"
    }
    if ($Value -is [System.Collections.IEnumerable]) {
        $parts = @()
        foreach ($item in $Value) {
            $parts += (ConvertTo-CanonicalJson $item)
        }
        return "[" + ($parts -join ",") + "]"
    }
    return (ConvertTo-Json -InputObject $Value -Compress -Depth 32)
}

function Get-TreeHash {
    param([string]$Root)
    $Root = [IO.Path]::GetFullPath($Root)
    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        throw "Tree root is missing: $Root"
    }
    $entries = @()
    foreach ($file in (Get-ChildItem -LiteralPath $Root -Recurse -File | Sort-Object FullName)) {
        $relative = $file.FullName.Substring($Root.Length + 1).Replace('\', '/')
        $entries += [pscustomobject]@{
            path = $relative
            sha256 = (Get-Sha256File $file.FullName)
        }
    }
    return (Get-Sha256Text (ConvertTo-CanonicalJson $entries))
}

function Get-RepoProvenance {
    $gitArgs = @('-C', $repositoryRoot, '-c', 'core.autocrlf=false', '-c', 'core.safecrlf=false', '-c', 'core.trustctime=false')
    $buildSha = (& git @gitArgs rev-parse HEAD 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($buildSha)) {
        throw "Could not determine the build SHA."
    }
    $buildSha = $buildSha.Trim()
    $branch = (& git @gitArgs rev-parse --abbrev-ref HEAD 2>$null)
    if ($LASTEXITCODE -ne 0) { $branch = "unknown" }
    $statusRaw = (& git @gitArgs status --porcelain 2>$null) -join "`n"
    $diff = (& git @gitArgs diff --binary HEAD 2>$null) -join "`n"
    $cachedDiff = (& git @gitArgs diff --cached --binary 2>$null) -join "`n"
    $worktreeDiff = (& git @gitArgs diff --quiet HEAD 2>$null)
    $hasWorktreeDiff = $LASTEXITCODE -ne 0
    $cachedDiffCheck = (& git @gitArgs diff --cached --quiet 2>$null)
    $hasCachedDiff = $LASTEXITCODE -ne 0
    $status = $statusRaw
    if (-not $hasWorktreeDiff -and -not $hasCachedDiff) {
        $diff = ""
        $cachedDiff = ""
        $status = (($statusRaw -split "`n" | Where-Object { $_.StartsWith("??") }) -join "`n")
    }
    return [pscustomobject]@{
        BuildSha = $buildSha
        Branch = ([string]$branch).Trim()
        SourceStatus = $status
        SourceDirty = -not [string]::IsNullOrWhiteSpace($status)
        SourceStateSha = (Get-Sha256Text ($buildSha + "`n" + $status + "`n" + $diff + "`n" + $cachedDiff))
    }
}

function Get-ProductVersion {
    $package = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "ui\package.json") | ConvertFrom-Json
    return [string]$package.version
}

function Get-ProtocolIdentity {
    $protocolLib = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "protocol\src\lib.rs")
    $protocolMatch = [regex]::Match($protocolLib, 'PROTOCOL_VERSION\s*:\s*u32\s*=\s*(\d+)')
    if (-not $protocolMatch.Success) { throw "Could not read PROTOCOL_VERSION." }
    $configSource = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "protocol\src\v1\config.rs")
    $configMatch = [regex]::Match($configSource, 'CURRENT_SCHEMA_VERSION\s*:\s*u32\s*=\s*(\d+)')
    if (-not $configMatch.Success) { throw "Could not read CURRENT_SCHEMA_VERSION." }
    $historySource = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "protocol\src\v1\history.rs")
    $evidenceMatch = [regex]::Match($historySource, 'EVIDENCE_SCHEMA_VERSION\s*:\s*u32\s*=\s*(\d+)')
    if (-not $evidenceMatch.Success) { throw "Could not read EVIDENCE_SCHEMA_VERSION." }
    $schemaSource = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot "runtime\evidence-store\src\schema.rs")
    $checksumMatch = [regex]::Match($schemaSource, 'SCHEMA_CHECKSUM\s*:\s*&str\s*=\s*"([A-Fa-f0-9]{64})"')
    if (-not $checksumMatch.Success) { throw "Could not read SCHEMA_CHECKSUM." }

    $schemaEntries = @()
    $generatedRoot = Join-Path $repositoryRoot "protocol\generated"
    foreach ($file in (Get-ChildItem -LiteralPath (Join-Path $generatedRoot "json-schema") -File | Sort-Object Name)) {
        $schemaEntries += [pscustomobject]@{
            path = "json-schema/" + $file.Name
            sha256 = (Get-Sha256File $file.FullName)
        }
    }
    $typescript = Join-Path $generatedRoot "typescript\v1.ts"
    if (Test-Path -LiteralPath $typescript -PathType Leaf) {
        $schemaEntries += [pscustomobject]@{
            path = "typescript/v1.ts"
            sha256 = (Get-Sha256File $typescript)
        }
    }
    return [pscustomobject]@{
        ProtocolRevision = [int]$protocolMatch.Groups[1].Value
        ProtocolSchemaHash = (Get-Sha256Text (ConvertTo-CanonicalJson $schemaEntries))
        ConfigSchemaVersion = [int]$configMatch.Groups[1].Value
        EvidenceSchemaVersion = [int]$evidenceMatch.Groups[1].Value
        EvidenceSchemaChecksum = $checksumMatch.Groups[1].Value.ToLowerInvariant()
    }
}

function Get-ConfigProfileIdentity {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) {
        $Path = Join-Path $fixtureRoot "config\profile.json"
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return [pscustomobject]@{ Path = $Path; Revision = $null; Hash = $null }
    }
    $profile = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
    return [pscustomobject]@{
        Path = $Path
        Revision = [string](Get-Field $profile "configRevision")
        Hash = (Get-Sha256Text (ConvertTo-CanonicalJson $profile))
    }
}

function Find-M04EvalBinary {
    param([string]$ExplicitPath)
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        if (Test-Path -LiteralPath $ExplicitPath -PathType Leaf) { return $ExplicitPath }
        throw "M04 eval binary is missing: $ExplicitPath"
    }
    foreach ($candidate in @(
        (Join-Path $repositoryRoot "target\release\m04_eval.exe"),
        (Join-Path $repositoryRoot "target\debug\m04_eval.exe")
    )) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
    }
    return $null
}

function Invoke-M04Preflight {
    param(
        [string]$EvalBinary,
        [string]$RuntimeBinary,
        [string]$Profile
    )
    if ([string]::IsNullOrWhiteSpace($EvalBinary) -or -not (Test-Path -LiteralPath $RuntimeBinary -PathType Leaf)) {
        return [pscustomobject]@{ Status = "UNAVAILABLE"; Reason = "M04 eval binary or Runtime binary is unavailable." }
    }
    $raw = & $EvalBinary preflight --json --runtime-bin $RuntimeBinary --profile $Profile 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace(($raw -join ""))) {
        return [pscustomobject]@{
            Status = "BLOCKED"
            Reason = "M04 preflight failed for profile $Profile."
        }
    }
    try {
        $parsed = (($raw -join "`n") | ConvertFrom-Json)
    } catch {
        return [pscustomobject]@{ Status = "BLOCKED"; Reason = "M04 preflight output was not valid JSON." }
    }
    return [pscustomobject]@{ Status = "PASS"; Value = $parsed }
}

function Get-WindowsEnvironmentClass {
    $version = [Environment]::OSVersion.Version
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ([string]::IsNullOrWhiteSpace($architecture)) { $architecture = "x64" }
    return ("windows-{0}.{1}.{2}-{3}" -f $version.Major, $version.Minor, $version.Build, $architecture.ToLowerInvariant())
}

function Get-InstallerPayloadHashes {
    param([string]$Installer)
    $sevenZip = "C:\Program Files\7-Zip\7z.exe"
    if (-not (Test-Path -LiteralPath $sevenZip -PathType Leaf)) {
        throw "7-Zip is required to inspect the NSIS payload: $sevenZip"
    }
    $extractRoot = Join-Path ([IO.Path]::GetTempPath()) ("m06-installer-payload-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $extractRoot | Out-Null
    try {
        & $sevenZip x $Installer ("-o" + $extractRoot) -y | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "7-Zip could not extract the NSIS installer." }
        return [pscustomobject]@{
            Ui = (Get-OptionalSha256File (Join-Path $extractRoot "chatgpt-harness-ui.exe"))
            Runtime = (Get-OptionalSha256File (Join-Path $extractRoot "runtime\chatgpt-harness-runtime.exe"))
            Tunnel = (Get-OptionalSha256File (Join-Path $extractRoot "tunnel\tunnel-client.exe"))
            Cloudflared = (Get-OptionalSha256File (Join-Path $extractRoot "tunnel\cloudflared.exe"))
        }
    } finally {
        $resolved = [IO.Path]::GetFullPath($extractRoot)
        if ($resolved.StartsWith($tempRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
            [IO.Path]::GetFileName($resolved) -like "m06-installer-payload-*") {
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    }
}

function Get-CandidateIdentity {
    param(
        [string]$Distribution,
        [string]$Portable,
        [string]$Installer,
        [string]$ProfilePath,
        [string]$M04Baseline,
        [string]$M05Regression,
        [string]$EvalBinary
    )
    $provenance = Get-RepoProvenance
    $version = Get-ProductVersion
    if ([string]::IsNullOrWhiteSpace($Distribution)) {
        $Distribution = Join-Path $repositoryRoot "target-exe\distribution"
    }
    if ([string]::IsNullOrWhiteSpace($Portable)) {
        $Portable = Join-Path $repositoryRoot ("target\release\bundle\portable\ChatGPT-Harness_{0}_x64-portable" -f $version)
    }
    if ([string]::IsNullOrWhiteSpace($Installer)) {
        $Installer = Join-Path $repositoryRoot ("target\release\bundle\nsis\ChatGPT-Harness_{0}_x64-setup.exe" -f $version)
    }

    $manifest = $null
    $manifestPath = Join-Path $Distribution "distribution.json"
    if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    }
    $portableUi = Get-OptionalSha256File (Join-Path $Portable "chatgpt-harness-ui.exe")
    $portableRuntime = Get-OptionalSha256File (Join-Path $Portable "runtime\chatgpt-harness-runtime.exe")
    $portableTunnel = Get-OptionalSha256File (Join-Path $Portable "tunnel\tunnel-client.exe")
    $portableCloudflared = Get-OptionalSha256File (Join-Path $Portable "tunnel\cloudflared.exe")
    $portablePackage = $null
    if (Test-Path -LiteralPath $Portable -PathType Container) {
        $portablePackage = Get-TreeHash $Portable
    }
    $installerHash = Get-OptionalSha256File $Installer
    $installed = [pscustomobject]@{ Ui = $null; Runtime = $null; Tunnel = $null; Cloudflared = $null }
    $installerIssue = $null
    if ($null -ne $installerHash) {
        try {
            $installed = Get-InstallerPayloadHashes $Installer
        } catch {
            $installerIssue = $_.Exception.Message
        }
    }

    $protocol = Get-ProtocolIdentity
    $profile = Get-ConfigProfileIdentity $ProfilePath
    $baseline = $null
    if (-not [string]::IsNullOrWhiteSpace($M04Baseline) -and (Test-Path -LiteralPath $M04Baseline -PathType Leaf)) {
        $baseline = Get-Content -Raw -LiteralPath $M04Baseline | ConvertFrom-Json
    }
    $regression = $null
    if (-not [string]::IsNullOrWhiteSpace($M05Regression) -and (Test-Path -LiteralPath $M05Regression -PathType Leaf)) {
        $regression = Get-Content -Raw -LiteralPath $M05Regression | ConvertFrom-Json
    }

    $evalBinaryPath = Find-M04EvalBinary $EvalBinary
    $m04Preflight = $null
    $m05Preflight = $null
    $evalReason = $null
    if ($null -ne $evalBinaryPath -and $null -ne $portableRuntime) {
        $m04Preflight = Invoke-M04Preflight -EvalBinary $evalBinaryPath -RuntimeBinary (Join-Path $Portable "runtime\chatgpt-harness-runtime.exe") -Profile "m04-baseline"
        $m05Preflight = Invoke-M04Preflight -EvalBinary $evalBinaryPath -RuntimeBinary (Join-Path $Portable "runtime\chatgpt-harness-runtime.exe") -Profile "m05-grep-candidate-v1"
        if ($m04Preflight.Status -ne "PASS") { $evalReason = $m04Preflight.Reason }
        elseif ($m05Preflight.Status -ne "PASS") { $evalReason = $m05Preflight.Reason }
    } else {
        $evalReason = "M04 eval binary or candidate Runtime is unavailable."
    }

    $m04Value = $null
    $m05Value = $null
    if ($null -ne $m04Preflight -and $m04Preflight.Status -eq "PASS") { $m04Value = $m04Preflight.Value }
    if ($null -ne $m05Preflight -and $m05Preflight.Status -eq "PASS") { $m05Value = $m05Preflight.Value }

    $candidateId = "M06-" + $provenance.BuildSha.Substring(0, [Math]::Min(7, $provenance.BuildSha.Length))
    if (-not [string]::IsNullOrWhiteSpace($portableUi)) {
        $candidateId += "-" + $portableUi.Substring(0, 8).ToLowerInvariant()
    }

    $identity = [ordered]@{
        schema = "m06-candidate-identity-v1"
        candidate_id = $candidateId
        git_commit_sha = $provenance.BuildSha
        branch = $provenance.Branch
        source_dirty = $provenance.SourceDirty
        source_state_sha256 = if ($null -ne $manifest) { [string](Get-Field $manifest "source_state_sha256") } else { $provenance.SourceStateSha }
        product_version = $version
        windows_environment_class = (Get-WindowsEnvironmentClass)
        startup_benchmark_harness_version = "m02-startup-benchmark-v1"
        portable_package_sha256 = $portablePackage
        portable_ui_sha256 = $portableUi
        portable_runtime_sha256 = $portableRuntime
        portable_tunnel_sha256 = $portableTunnel
        portable_cloudflared_sha256 = $portableCloudflared
        installer_sha256 = $installerHash
        installed_ui_sha256 = $installed.Ui
        installed_runtime_sha256 = $installed.Runtime
        installed_tunnel_sha256 = $installed.Tunnel
        installed_cloudflared_sha256 = $installed.Cloudflared
        control_protocol_revision = $protocol.ProtocolRevision
        generated_protocol_schema_hash = $protocol.ProtocolSchemaHash
        config_schema_version = $protocol.ConfigSchemaVersion
        effective_config_revision = $profile.Revision
        config_profile_hash = $profile.Hash
        evidence_schema_version = $protocol.EvidenceSchemaVersion
        evidence_schema_checksum = $protocol.EvidenceSchemaChecksum
        effective_tool_catalog_hash = if ($null -ne $m05Value) { [string](Get-Field $m05Value "toolSchemaHash") } else { $null }
        per_tool_schema_hashes = if ($null -ne $m05Value) { Get-Field $m05Value "toolSchemaHashes" } else { $null }
        m04_tool_schema_hash = if ($null -ne $m04Value) { [string](Get-Field $m04Value "toolSchemaHash") } else { $null }
        accepted_m04_baseline_identity = if ($null -ne $baseline) { [string](Get-Field $baseline "baselineId") } else { $null }
        m04_dataset_hash = if ($null -ne $m04Value) { [string](Get-Field $m04Value "datasetHash") } else { $null }
        m04_system_prompt_hash = if ($null -ne $m04Value) { [string](Get-Field $m04Value "systemPromptHash") } else { $null }
        m04_scorer_hash = if ($null -ne $m04Value) { [string](Get-Field $m04Value "scorerHash") } else { $null }
        m05_candidate_profile_hash = if ($null -ne $m05Value) { [string](Get-Field $m05Value "evalProfileHash") } else { $null }
        m05_tool_schema_hash = if ($null -ne $m05Value) { [string](Get-Field $m05Value "toolSchemaHash") } else { $null }
        adapter_id_version = if ($null -ne $m05Value) { ([string](Get-Field $m05Value "adapterId")) + "/" + ([string](Get-Field $m05Value "adapterVersion")) } else { $null }
        requested_model = if ($null -ne $m05Value) { [string](Get-Field $m05Value "requestedModel") } else { $null }
        resolved_model = if ($null -ne $baseline) { [string](Get-Field $baseline "resolvedModel") } else { $null }
        distribution_build_sha = if ($null -ne $manifest) { [string](Get-Field $manifest "build_sha") } else { $null }
        distribution_source_dirty = if ($null -ne $manifest) { Get-Field $manifest "source_dirty" } else { $null }
        installer_payload_status = if ($null -ne $installerIssue) { "EXTRACTION_FAILED" } elseif ($null -ne $installerHash) { "EXTRACTED" } else { "UNAVAILABLE" }
        eval_identity_status = if ($null -ne $evalReason) { "BLOCKED" } else { "PASS" }
        eval_identity_reason = $evalReason
    }
    return [pscustomobject]$identity
}

function Test-CandidateIdentity {
    param([object]$Candidate, [switch]$AllowIncompleteEval)
    $reasons = @()
    if ($null -eq $Candidate) { return [pscustomobject]@{ Pass = $false; Reasons = @("Candidate identity is missing.") } }
    if ((Get-Field $Candidate "schema") -ne "m06-candidate-identity-v1") {
        $reasons += "Candidate identity schema is not m06-candidate-identity-v1."
    }
    foreach ($field in @("candidate_id", "git_commit_sha", "branch", "product_version", "windows_environment_class")) {
        if ([string]::IsNullOrWhiteSpace([string](Get-Field $Candidate $field))) {
            $reasons += "Candidate identity is missing $field."
        }
    }
    if ((Get-Field $Candidate "source_dirty") -ne $false) {
        $reasons += "source_dirty must be false for a release candidate."
    }
    foreach ($field in $RequiredCandidateHashes) {
        $value = [string](Get-Field $Candidate $field)
        if ($value -notmatch '^[A-Fa-f0-9]{64}$') {
            $reasons += "Candidate identity has an invalid or missing SHA-256: $field."
        }
    }
    if ((Get-Field $Candidate "control_protocol_revision") -lt 1) {
        $reasons += "control_protocol_revision is invalid."
    }
    if ((Get-Field $Candidate "config_schema_version") -lt 1) {
        $reasons += "config_schema_version is invalid."
    }
    if ((Get-Field $Candidate "evidence_schema_version") -lt 1) {
        $reasons += "evidence_schema_version is invalid."
    }
    $perTool = Get-Field $Candidate "per_tool_schema_hashes"
    if ($null -eq $perTool -or @($perTool.PSObject.Properties).Count -lt 1) {
        $reasons += "per_tool_schema_hashes is empty."
    }
    if ((Get-Field $Candidate "portable_ui_sha256") -ne (Get-Field $Candidate "installed_ui_sha256")) {
        $reasons += "Portable and installer UI hashes differ."
    }
    if ((Get-Field $Candidate "portable_runtime_sha256") -ne (Get-Field $Candidate "installed_runtime_sha256")) {
        $reasons += "Portable and installer Runtime hashes differ."
    }
    if ((Get-Field $Candidate "portable_tunnel_sha256") -ne (Get-Field $Candidate "installed_tunnel_sha256")) {
        $reasons += "Portable and installer Tunnel hashes differ."
    }
    if (-not $AllowIncompleteEval) {
        foreach ($field in @("accepted_m04_baseline_identity", "adapter_id_version", "requested_model", "resolved_model")) {
            if ([string]::IsNullOrWhiteSpace([string](Get-Field $Candidate $field))) {
                $reasons += "Candidate identity is missing $field."
            }
        }
        if ((Get-Field $Candidate "eval_identity_status") -ne "PASS") {
            $reasons += "Candidate eval identity is not PASS."
        }
    }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons }
}

function Test-UpstreamEligibility {
    param([object]$Upstream)
    $reasons = @()
    $statuses = @{}
    if ($null -eq $Upstream -or (Get-Field $Upstream "schema") -ne "m06-upstream-state-v1") {
        return [pscustomobject]@{ Pass = $false; Reasons = @("Upstream state is missing or has an unknown schema."); Statuses = $statuses }
    }
    foreach ($item in @(Get-Field $Upstream "milestones")) {
        $id = [string](Get-Field $item "id")
        $status = [string](Get-Field $item "status")
        if ($statuses.ContainsKey($id)) { $reasons += "Upstream milestone $id is duplicated." }
        $statuses[$id] = $status
    }
    foreach ($id in @("M00", "M01", "M02", "M03", "M04", "M05")) {
        if (-not $statuses.ContainsKey($id)) {
            $reasons += "Upstream milestone $id is missing."
        } elseif ($statuses[$id] -ne "ACCEPTED_FROZEN") {
            $reasons += "Upstream milestone $id is $($statuses[$id]); ACCEPTED_FROZEN is required."
        }
    }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons; Statuses = $statuses }
}

function Test-CoreMatrix {
    param([object]$Matrix, [object]$Candidate)
    $reasons = @()
    $seen = @{}
    $rowsById = @{}
    if ($null -eq $Matrix -or (Get-Field $Matrix "schema") -ne "m06-core-matrix-v1") {
        return [pscustomobject]@{ Pass = $false; Reasons = @("Core matrix is missing or has an unknown schema."); RowsById = $rowsById }
    }
    if ($null -ne $Candidate -and [string](Get-Field $Matrix "candidate_id") -ne [string](Get-Field $Candidate "candidate_id")) {
        $reasons += "Core matrix candidate_id does not match the candidate."
    }
    foreach ($row in @(Get-Field $Matrix "rows")) {
        $scope = [string](Get-Field $row "scope_id")
        $classification = [string](Get-Field $row "classification")
        if ([string]::IsNullOrWhiteSpace($scope)) {
            $reasons += "A Core matrix row has no scope_id."
            continue
        }
        if ($seen.ContainsKey($scope)) {
            $reasons += "Core matrix row $scope is duplicated."
        }
        $seen[$scope] = $true
        $rowsById[$scope] = $row
        if ($AllowedClassifications -notcontains $classification) {
            $reasons += "Core matrix row $scope has an unknown classification."
        }
        foreach ($field in @("owner_milestone", "upstream_verdict", "m06_required_recheck", "candidate_equivalence_key", "evidence_reference")) {
            if ([string]::IsNullOrWhiteSpace([string](Get-Field $row $field))) {
                $reasons += "Core matrix row $scope is missing $field."
            }
        }
        $final = [string](Get-Field $row "final_status")
        if ($AllowedFinalStatuses -notcontains $final) {
            $reasons += "Core matrix row $scope has an unknown final_status: $final."
        }
        foreach ($field in @("integration_status", "e2e_status", "manual_status")) {
            $value = [string](Get-Field $row $field)
            if ($AllowedValidationStatuses -notcontains $value) {
                $reasons += "Core matrix row $scope has an unknown ${field}: $value."
            }
        }
        $isCore = $CoreScopeOwners.Contains($scope)
        $isRelease = $RequiredReleaseRows -contains $scope
        if (-not $isCore -and -not $isRelease) {
            if ($classification -eq "CORE_MANDATORY") {
                $reasons += "Unknown Core matrix row $scope is classified CORE_MANDATORY."
            }
            continue
        }
        if ($classification -ne "CORE_MANDATORY") {
            $reasons += "Mandatory row $scope must be CORE_MANDATORY."
        }
        if ($isCore) {
            if ([string](Get-Field $row "upstream_verdict") -ne "ACCEPTED_FROZEN") {
                $reasons += "Core row $scope does not cite ACCEPTED_FROZEN upstream evidence."
            }
            $expectedOwner = $CoreScopeOwners[$scope]
            if ([string](Get-Field $row "owner_milestone") -ne $expectedOwner) {
                $reasons += "Core row $scope has the wrong owner_milestone."
            }
        }
        if ($final -ne "PASS") {
            $reasons += "Mandatory row $scope is $final, not PASS."
        }
        $validationStatuses = @(
            [string](Get-Field $row "integration_status"),
            [string](Get-Field $row "e2e_status"),
            [string](Get-Field $row "manual_status")
        )
        if ($validationStatuses -notcontains "PASS") {
            $reasons += "Mandatory row $scope has no PASS validation result."
        }
        foreach ($value in $validationStatuses) {
            if ($value -in @("FAIL", "BLOCKED", "INCONCLUSIVE", "NOT_VERIFIED")) {
                $reasons += "Mandatory row $scope has a non-PASS validation result: $value."
            }
        }
        $equivalence = [string](Get-Field $row "candidate_equivalence_key")
        if ($null -ne $Candidate -and
            $equivalence -ne [string](Get-Field $Candidate "candidate_id") -and
            -not $equivalence.StartsWith("EQUIVALENT:", [StringComparison]::Ordinal)) {
            $reasons += "Mandatory row $scope is not bound to the candidate."
        }
        if ($scope -in @("EXT-REAL-CHATGPT", "EXT-WINDOWS-TRAY-TOAST")) {
            if ([string](Get-Field $row "manual_status") -ne "PASS") {
                $reasons += "Mandatory row $scope requires a human PASS result."
            }
        }
    }
    foreach ($scope in $CoreScopeOwners.Keys) {
        if (-not $seen.ContainsKey($scope)) {
            $reasons += "Canonical Core row $scope is missing."
        }
    }
    foreach ($scope in $RequiredReleaseRows) {
        if (-not $seen.ContainsKey($scope)) {
            $reasons += "Mandatory release row $scope is missing."
        }
    }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons; RowsById = $rowsById }
}

function Test-M02ContractValues {
    param([object]$Contract)
    $expected = [ordered]@{
        statistics = "nearest_rank_raw_sample"
        cold_counted_samples = 30
        warm_counted_samples = 30
        cold_p50_max_ratio = 0.9
        warm_p50_max_ratio = 0.85
        p95_max_regression_ratio = 1.05
        at_least_one_p95_max_ratio = 0.9
        min_ready_cold = 29
        min_ready_warm = 29
        max_failures_cold = 0
        max_failures_warm = 1
        material_bottleneck_phase_p50_improvement = 0.25
        material_bottleneck_total_p95_improvement = 0.1
    }
    $reasons = @()
    foreach ($name in $expected.Keys) {
        $actual = Get-Field $Contract $name
        if ($null -eq $actual) {
            $reasons += "M02 frozen contract is missing $name."
            continue
        }
        if ([string]$actual -ne [string]$expected[$name]) {
            $reasons += "M02 frozen contract changed $name."
        }
    }
    return $reasons
}

function Get-M02EquivalenceDecision {
    param([string]$EvidencePath, [object]$Candidate)
    if ([string]::IsNullOrWhiteSpace($EvidencePath) -or -not (Test-Path -LiteralPath $EvidencePath -PathType Leaf)) {
        return [pscustomobject]@{
            Status = "BLOCKED_MISSING_ACCEPTED_M02"
            Pass = $false
            Reasons = @("Accepted M02 performance evidence is unavailable.")
            ReuseAllowed = $false
        }
    }
    $evidence = Get-Content -Raw -LiteralPath $EvidencePath | ConvertFrom-Json
    $reasons = @()
    if ((Get-Field $evidence "schema") -ne "m02-accepted-performance-v1" -or
        (Get-Field $evidence "accepted") -ne $true) {
        return [pscustomobject]@{
            Status = "BLOCKED_MISSING_ACCEPTED_M02"
            Pass = $false
            Reasons = @("M02 evidence is not an accepted performance record.")
            ReuseAllowed = $false
        }
    }
    $reasons += Test-M02ContractValues (Get-Field $evidence "contract")
    if ($reasons.Count -gt 0) {
        return [pscustomobject]@{ Status = "FAIL_M02_CONTRACT_DRIFT"; Pass = $false; Reasons = $reasons; ReuseAllowed = $false }
    }
    $identity = Get-Field $evidence "identity"
    $result = Get-Field $evidence "result"
    $baseline = Get-Field $evidence "baseline"
    if ($null -eq $identity -or $null -eq $result -or $null -eq $baseline) {
        return [pscustomobject]@{ Status = "BLOCKED_MISSING_ACCEPTED_M02"; Pass = $false; Reasons = @("M02 evidence identity/result/baseline is incomplete."); ReuseAllowed = $false }
    }
    $resultVerdict = [string](Get-Field $result "verdict")
    if ($resultVerdict -ne "PASS") {
        $reasons += "Accepted M02 evidence records a non-PASS result: $resultVerdict."
    }
    $cold = Get-Field $result "cold"
    $warm = Get-Field $result "warm"
    $baseCold = Get-Field $baseline "cold"
    $baseWarm = Get-Field $baseline "warm"
    if ($null -eq $cold -or $null -eq $warm -or $null -eq $baseCold -or $null -eq $baseWarm) {
        $reasons += "M02 result or baseline cohort data is incomplete."
    } else {
        if ((Get-Field $cold "counted") -lt 30 -or (Get-Field $warm "counted") -lt 30) {
            $reasons += "M02 counted sample size is below 30."
        }
        if ((Get-Field $cold "ready") -lt 29 -or (Get-Field $warm "ready") -lt 29) {
            $reasons += "M02 Ready sample gate is below 29."
        }
        if ((Get-Field $cold "failures") -gt 0 -or (Get-Field $warm "failures") -gt 1) {
            $reasons += "M02 failure-rate rule is violated."
        }
        if ([double](Get-Field $cold "p50_ms") -gt ([double](Get-Field $baseCold "p50_ms") * 0.9)) {
            $reasons += "M02 cold P50 threshold is not met."
        }
        if ([double](Get-Field $warm "p50_ms") -gt ([double](Get-Field $baseWarm "p50_ms") * 0.85)) {
            $reasons += "M02 warm P50 threshold is not met."
        }
        if ([double](Get-Field $cold "p95_ms") -gt ([double](Get-Field $baseCold "p95_ms") * 1.05) -or
            [double](Get-Field $warm "p95_ms") -gt ([double](Get-Field $baseWarm "p95_ms") * 1.05)) {
            $reasons += "M02 P95 regression guard is not met."
        }
        if ([double](Get-Field $cold "p95_ms") -gt ([double](Get-Field $baseCold "p95_ms") * 0.9) -and
            [double](Get-Field $warm "p95_ms") -gt ([double](Get-Field $baseWarm "p95_ms") * 0.9)) {
            $reasons += "M02 requires at least one P95 to improve by 10%."
        }
        if ([string](Get-Field $result "material_bottleneck") -ne "PASS") {
            $reasons += "M02 material bottleneck clause is not PASS."
        }
    }
    $identityMatches = $true
    $identityReasons = @()
    $pairs = @(
        @("app_binary_sha256", "portable_ui_sha256"),
        @("runtime_sha256", "portable_runtime_sha256"),
        @("tunnel_sha256", "portable_tunnel_sha256"),
        @("config_profile_hash", "config_profile_hash"),
        @("benchmark_harness_version", "startup_benchmark_harness_version"),
        @("environment_class", "windows_environment_class")
    )
    foreach ($pair in $pairs) {
        if ([string](Get-Field $identity $pair[0]) -ne [string](Get-Field $Candidate $pair[1])) {
            $identityMatches = $false
            $identityReasons += "M02 equivalence key differs at $($pair[0])."
        }
    }
    if ($reasons.Count -gt 0) {
        return [pscustomobject]@{ Status = "FAIL"; Pass = $false; Reasons = $reasons; ReuseAllowed = $false }
    }
    if (-not $identityMatches) {
        return [pscustomobject]@{ Status = "FULL_M02_BENCHMARK_REQUIRED"; Pass = $false; Reasons = $identityReasons; ReuseAllowed = $false }
    }
    return [pscustomobject]@{ Status = "EQUIVALENT_REUSE_ALLOWED"; Pass = $true; Reasons = @(); ReuseAllowed = $true }
}

function Get-F12EquivalenceDecision {
    param([string]$BaselinePath, [string]$RegressionPath, [object]$Candidate)
    if ([string]::IsNullOrWhiteSpace($BaselinePath) -or -not (Test-Path -LiteralPath $BaselinePath -PathType Leaf) -or
        [string]::IsNullOrWhiteSpace($RegressionPath) -or -not (Test-Path -LiteralPath $RegressionPath -PathType Leaf)) {
        return [pscustomobject]@{
            Status = "BLOCKED_MISSING_ACCEPTED_EVIDENCE"
            Pass = $false
            Reasons = @("Accepted M04 baseline or M05 formal regression evidence is unavailable.")
            ReuseAllowed = $false
        }
    }
    $baseline = Get-Content -Raw -LiteralPath $BaselinePath | ConvertFrom-Json
    $regression = Get-Content -Raw -LiteralPath $RegressionPath | ConvertFrom-Json
    $reasons = @()
    if ((Get-Field $baseline "schemaVersion") -ne 1 -or
        [string](Get-Field $baseline "source") -ne "accepted-m04-formal-run" -or
        [string](Get-Field $baseline "validity") -ne "VALID") {
        $reasons += "M04 baseline is not a valid accepted formal run."
    }
    $metrics = Get-Field $baseline "metrics"
    if ($null -eq $metrics -or
        (Get-Field $metrics "totalRuns") -ne 72 -or
        (Get-Field $metrics "scoreableRuns") -lt 70 -or
        (Get-Field $metrics "securityViolationRuns") -ne 0 -or
        @(Get-Field $metrics "cases").Count -ne 24 -or
        @(Get-Field $metrics "categories").Count -ne 6) {
        $reasons += "M04 baseline aggregate shape is not the frozen 72-run result."
    }
    if ((Get-Field $regression "schema") -ne "m05-formal-regression-v1" -or
        (Get-Field $regression "formal") -ne $true -or
        [string](Get-Field $regression "validity") -ne "VALID") {
        $reasons += "M05 regression is not a valid formal run."
    }
    $comparison = Get-Field $regression "comparison"
    if ($null -eq $comparison -or (Get-Field $comparison "pass") -ne $true) {
        $reasons += "compare_m05 did not PASS."
    }
    if ((Get-Field $regression "securityViolationRuns") -ne 0) {
        $reasons += "M05 regression has security violations."
    }
    if ((Get-Field $regression "passiveTelemetryUsed") -ne $false) {
        $reasons += "M05 regression used passive telemetry."
    }
    $paired = Get-Field $regression "pairedControl"
    if ($null -ne $paired -and (Get-Field $paired "applicable") -eq $true -and (Get-Field $paired "withinGuard") -ne $true) {
        $reasons += "Paired-control provider drift exceeded its guard."
    }
    if ($reasons.Count -gt 0) {
        $status = if (($reasons -join " ") -match "provider drift") { "INCONCLUSIVE" } else { "FAIL" }
        return [pscustomobject]@{ Status = $status; Pass = $false; Reasons = $reasons; ReuseAllowed = $false }
    }

    $identity = Get-Field $regression "candidateIdentity"
    if ($null -eq $identity) {
        return [pscustomobject]@{ Status = "BLOCKED_MISSING_ACCEPTED_EVIDENCE"; Pass = $false; Reasons = @("M05 regression candidate identity is missing."); ReuseAllowed = $false }
    }
    $identityChecks = @(
        @("runtimeBinaryHash", "portable_runtime_sha256", "Runtime"),
        @("datasetHash", "m04_dataset_hash", "M04 dataset"),
        @("systemPromptHash", "m04_system_prompt_hash", "system prompt"),
        @("scorerHash", "m04_scorer_hash", "scorer"),
        @("toolSchemaHash", "m05_tool_schema_hash", "M05 Tool schema"),
        @("evalProfileHash", "m05_candidate_profile_hash", "M05 candidate profile"),
        @("adapterId", "adapter_id_version", "adapter"),
        @("requestedModel", "requested_model", "requested model"),
        @("resolvedModel", "resolved_model", "resolved model")
    )
    $identityMatches = $true
    foreach ($check in $identityChecks) {
        $actual = [string](Get-Field $identity $check[0])
        $expected = [string](Get-Field $Candidate $check[1])
        if ($check[0] -eq "adapterId") {
            $expected = ([string](Get-Field $Candidate "adapter_id_version")).Split("/")[0]
        }
        if ($actual -ne $expected) {
            $identityMatches = $false
            $reasons += "F12 equivalence key differs at $($check[2])."
        }
    }
    if ([string](Get-Field $baseline "datasetHash") -ne [string](Get-Field $Candidate "m04_dataset_hash") -or
        [string](Get-Field $baseline "systemPromptHash") -ne [string](Get-Field $Candidate "m04_system_prompt_hash") -or
        [string](Get-Field $baseline "scorerHash") -ne [string](Get-Field $Candidate "m04_scorer_hash") -or
        [string](Get-Field $baseline "toolSchemaHash") -ne [string](Get-Field $Candidate "m04_tool_schema_hash")) {
        $identityMatches = $false
        $reasons += "M04 baseline identity does not match the candidate."
    }
    if (-not $identityMatches) {
        return [pscustomobject]@{ Status = "REQUALIFICATION_REQUIRED"; Pass = $false; Reasons = $reasons; ReuseAllowed = $false }
    }
    return [pscustomobject]@{ Status = "EQUIVALENT_REUSE_ALLOWED"; Pass = $true; Reasons = @(); ReuseAllowed = $true }
}

function Test-PackageContract {
    param(
        [string]$Portable,
        [string]$Installer,
        [object]$Candidate,
        [switch]$RequireInstallerPayload
    )
    $reasons = @()
    if (-not [string]::IsNullOrWhiteSpace($Portable)) {
        $Portable = [IO.Path]::GetFullPath($Portable)
    }
    $details = [ordered]@{
        portable_status = "UNAVAILABLE"
        installer_status = "UNAVAILABLE"
        portable_tree_sha256 = $null
        installer_sha256 = $null
        extracted_ui_sha256 = $null
        extracted_runtime_sha256 = $null
        extracted_tunnel_sha256 = $null
    }
    if ([string]::IsNullOrWhiteSpace($Portable) -or -not (Test-Path -LiteralPath $Portable -PathType Container)) {
        $reasons += "Portable directory is unavailable."
    } else {
        $required = @(
            "chatgpt-harness-ui.exe",
            "distribution.json",
            "runtime\chatgpt-harness-runtime.exe",
            "tunnel\tunnel-client.exe",
            "tunnel\cloudflared.exe"
        )
        foreach ($relative in $required) {
            if (-not (Test-Path -LiteralPath (Join-Path $Portable $relative) -PathType Leaf)) {
                $reasons += "Portable payload is missing $relative."
            }
        }
        $manifest = $null
        if (Test-Path -LiteralPath (Join-Path $Portable "distribution.json") -PathType Leaf) {
            $manifest = Get-Content -Raw -LiteralPath (Join-Path $Portable "distribution.json") | ConvertFrom-Json
        }
        if ($null -eq $manifest) {
            $reasons += "Portable distribution.json is unavailable."
        } else {
            if ([string](Get-Field $manifest "build_sha") -ne [string](Get-Field $Candidate "git_commit_sha")) {
                $reasons += "Portable build_sha does not match the candidate."
            }
            if ((Get-Field $manifest "source_dirty") -ne $false) {
                $reasons += "Portable distribution was built from a dirty source tree."
            }
            foreach ($pair in @(
                @("ui_sha256", "portable_ui_sha256", "UI"),
                @("runtime_sha256", "portable_runtime_sha256", "Runtime"),
                @("tunnel_client_sha256", "portable_tunnel_sha256", "Tunnel")
            )) {
                if ([string](Get-Field $manifest $pair[0]) -ne [string](Get-Field $Candidate $pair[1])) {
                    $reasons += "Portable manifest $($pair[2]) hash does not match the candidate."
                }
            }
        }
        $treeHash = Get-TreeHash $Portable
        $details.portable_tree_sha256 = $treeHash
        if ([string](Get-Field $Candidate "portable_package_sha256") -ne $treeHash) {
            $reasons += "Portable package tree hash does not match the candidate."
        }
        $allowedExes = @(
            "chatgpt-harness-ui.exe",
            "runtime\chatgpt-harness-runtime.exe",
            "tunnel\tunnel-client.exe",
            "tunnel\cloudflared.exe"
        )
        foreach ($file in (Get-ChildItem -LiteralPath $Portable -Recurse -File)) {
            $relative = $file.FullName.Substring($Portable.Length + 1)
            if ($file.Extension -in @(".exe", ".dll") -and $allowedExes -notcontains $relative) {
                $reasons += "Portable contains an unexpected executable: $relative."
            }
            if ($ForbiddenShippingNames -contains $file.Name -or
                $ForbiddenShippingSuffixes -contains $file.Extension.ToLowerInvariant()) {
                $reasons += "Portable contains a forbidden runtime artifact: $relative."
            }
            $parentName = Split-Path -Leaf (Split-Path -Parent $file.FullName)
            if ($ForbiddenShippingDirectories -contains $parentName) {
                $reasons += "Portable contains a forbidden test directory: $relative."
            }
            if ($file.Length -le 2097152 -and $file.Extension -in @(".json", ".txt", ".md", ".yml", ".yaml", ".toml", ".ini", ".log", ".config", ".env")) {
                $content = Get-Content -Raw -LiteralPath $file.FullName -ErrorAction SilentlyContinue
                foreach ($sentinel in $ShippingSentinels) {
                    if ($content -match [regex]::Escape($sentinel)) {
                        $reasons += "Portable file $relative contains a forbidden sentinel."
                    }
                }
            }
        }
        if ($reasons.Count -eq 0) { $details.portable_status = "PASS" }
    }

    if ([string]::IsNullOrWhiteSpace($Installer) -or -not (Test-Path -LiteralPath $Installer -PathType Leaf)) {
        if ($RequireInstallerPayload) { $reasons += "NSIS installer is unavailable." }
    } else {
        $installerHash = Get-Sha256File $Installer
        $details.installer_sha256 = $installerHash
        if ($installerHash -ne [string](Get-Field $Candidate "installer_sha256")) {
            $reasons += "NSIS installer hash does not match the candidate."
        }
        try {
            $payload = Get-InstallerPayloadHashes $Installer
            $details.extracted_ui_sha256 = $payload.Ui
            $details.extracted_runtime_sha256 = $payload.Runtime
            $details.extracted_tunnel_sha256 = $payload.Tunnel
            if ($payload.Ui -ne [string](Get-Field $Candidate "installed_ui_sha256")) {
                $reasons += "NSIS extracted UI hash does not match the candidate."
            }
            if ($payload.Runtime -ne [string](Get-Field $Candidate "installed_runtime_sha256")) {
                $reasons += "NSIS extracted Runtime hash does not match the candidate."
            }
            if ($payload.Tunnel -ne [string](Get-Field $Candidate "installed_tunnel_sha256")) {
                $reasons += "NSIS extracted Tunnel hash does not match the candidate."
            }
            if ($details.installer_sha256 -eq [string](Get-Field $Candidate "installer_sha256") -and
                $payload.Ui -eq [string](Get-Field $Candidate "portable_ui_sha256") -and
                $payload.Runtime -eq [string](Get-Field $Candidate "portable_runtime_sha256") -and
                $payload.Tunnel -eq [string](Get-Field $Candidate "portable_tunnel_sha256")) {
                $details.installer_status = "PASS"
            }
        } catch {
            $reasons += $_.Exception.Message
            $details.installer_status = "EXTRACTION_FAILED"
        }
    }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons; Details = [pscustomobject]$details }
}

function New-ReleaseNotes {
    param([object]$Candidate, [object]$Matrix, [object]$GateResult)
    $rows = @{}
    foreach ($row in @(Get-Field $Matrix "rows")) {
        $rows[[string](Get-Field $row "scope_id")] = $row
    }
    $external = "NOT_VERIFIED"
    if ($rows.ContainsKey("EXT-REAL-CHATGPT")) {
        $external = [string](Get-Field $rows["EXT-REAL-CHATGPT"] "final_status")
    }
    $p1 = @($rows.Values | Where-Object { [string](Get-Field $_ "classification") -eq "OUT_OF_CORE_P1" })
    $p2 = @($rows.Values | Where-Object { [string](Get-Field $_ "classification") -eq "DEFERRED_P2" })
    $verdict = [string](Get-Field $GateResult "final_verdict")
    $productVerdict = if ($verdict -eq "PASS") { "V4 Core $EmDash ACCEPTED" } else { "V4 Core $EmDash $verdict" }
    $lines = @(
        "# V4 Core Release Notes (candidate)",
        "",
        "- Product scope: V4 Core",
        "- Product version: $([string](Get-Field $Candidate 'product_version'))",
        "- Candidate: $([string](Get-Field $Candidate 'candidate_id'))",
        "- Git commit: $([string](Get-Field $Candidate 'git_commit_sha'))",
        "- Source dirty: $([string](Get-Field $Candidate 'source_dirty'))",
        "- Portable SHA-256: $([string](Get-Field $Candidate 'portable_package_sha256'))",
        "- Installer SHA-256: $([string](Get-Field $Candidate 'installer_sha256'))",
        "- UI SHA-256: $([string](Get-Field $Candidate 'portable_ui_sha256'))",
        "- Runtime SHA-256: $([string](Get-Field $Candidate 'portable_runtime_sha256'))",
        "- Tunnel SHA-256: $([string](Get-Field $Candidate 'portable_tunnel_sha256'))",
        "- Protocol revision: $([string](Get-Field $Candidate 'control_protocol_revision'))",
        "- Config schema: $([string](Get-Field $Candidate 'config_schema_version'))",
        "- Evidence schema: $([string](Get-Field $Candidate 'evidence_schema_version')) / $([string](Get-Field $Candidate 'evidence_schema_checksum'))",
        "- Tool catalog hash: $([string](Get-Field $Candidate 'effective_tool_catalog_hash'))",
        "",
        "## Core status",
        "",
        "- F01 / F03 / F04: see the Core Product Acceptance Matrix.",
        "- F05 performance: $([string](Get-Field $GateResult 'f05_status'))",
        "- F06 Diagnostics: see the Core Product Acceptance Matrix.",
        "- F07 / Shared Evidence: see the Core Product Acceptance Matrix.",
        "- F10 grep: see the Core Product Acceptance Matrix.",
        "- F12 evaluation: $([string](Get-Field $GateResult 'f12_status'))",
        "- Real External ChatGPT: $external",
        "",
        "## Scope exclusions",
        "",
        "- P1 capabilities are OUT_OF_CORE / NOT_CORE_ACCEPTED. Count: $($p1.Count).",
        "- P2 capabilities are DEFERRED or separately accepted. Count: $($p2.Count).",
        "- This document does not claim official MCP conformance, a full Agent reasoning trace, or V4 Full.",
        "",
        "## Upgrade / rollback",
        "",
        "- Upgrade and rollback rows are recorded in the Core Product Acceptance Matrix.",
        "",
        "## Final verdict",
        "",
        "- $productVerdict"
    )
    return ($lines -join "`n") + "`n"
}

function Test-ReleaseNotesText {
    param([string]$Text)
    $reasons = @()
    if ($Text -notmatch "Product scope: V4 Core") { $reasons += "Release notes do not declare V4 Core scope." }
    foreach ($forbidden in @("V4 $EmDash ACCEPTED / FROZEN", "V4 Full $EmDash ACCEPTED", "V4 $EmDash ACCEPTED")) {
        if ($Text.Contains($forbidden)) { $reasons += "Release notes contain forbidden product wording: $forbidden." }
    }
    if ($Text -notmatch "OUT_OF_CORE / NOT_CORE_ACCEPTED") {
        $reasons += "Release notes do not classify P1 scope correctly."
    }
    return $reasons
}

function New-GateResult {
    param(
        [object]$Candidate,
        [object]$Upstream,
        [object]$Matrix,
        [object]$M02,
        [object]$F12,
        [object]$Package
    )
    $reasons = @()
    $upstreamEligibility = Test-UpstreamEligibility $Upstream
    $candidateValidation = Test-CandidateIdentity $Candidate
    $matrixValidation = Test-CoreMatrix $Matrix $Candidate
    $reasons += $upstreamEligibility.Reasons
    $reasons += $candidateValidation.Reasons
    $reasons += $matrixValidation.Reasons
    if ($null -ne $M02 -and $M02.Status -ne "EQUIVALENT_REUSE_ALLOWED") { $reasons += "F05: $($M02.Status)" }
    if ($null -ne $M02) { $reasons += $M02.Reasons }
    if ($null -ne $F12 -and $F12.Status -ne "EQUIVALENT_REUSE_ALLOWED") { $reasons += "F12: $($F12.Status)" }
    if ($null -ne $F12) { $reasons += $F12.Reasons }
    if ($null -ne $Package -and -not $Package.Pass) { $reasons += $Package.Reasons }
    $reasons = @($reasons | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })

    $status = "PASS"
    if ($reasons.Count -gt 0) {
        $status = "FAIL"
        if (-not $upstreamEligibility.Pass -or
            ($null -ne $M02 -and $M02.Status -like "BLOCKED*") -or
            ($null -ne $F12 -and $F12.Status -like "BLOCKED*") -or
            (-not $candidateValidation.Pass -and ($candidateValidation.Reasons -join " ") -match "missing|unavailable")) {
            $status = "BLOCKED"
        }
    }
    return [pscustomobject]@{
        schema = "m06-release-gate-verdict-v1"
        candidate_id = [string](Get-Field $Candidate "candidate_id")
        generated_at_utc = [DateTime]::UtcNow.ToString("O")
        final_verdict = $status
        product_verdict = if ($status -eq "PASS") { "V4 Core $EmDash ACCEPTED" } else { "V4 Core $EmDash NOT ACCEPTED" }
        upstream_status = if ($upstreamEligibility.Pass) { "PASS" } else { "BLOCKED" }
        candidate_status = if ($candidateValidation.Pass) { "PASS" } else { "FAIL" }
        matrix_status = if ($matrixValidation.Pass) { "PASS" } else { "FAIL" }
        f05_status = if ($null -ne $M02) { $M02.Status } else { "BLOCKED_MISSING_ACCEPTED_M02" }
        f12_status = if ($null -ne $F12) { $F12.Status } else { "BLOCKED_MISSING_ACCEPTED_EVIDENCE" }
        package_status = if ($null -ne $Package -and $Package.Pass) { "PASS" } else { "FAIL" }
        reasons = $reasons
        matrix_rows = $matrixValidation.RowsById.Count
        canonical_core_rows = $CoreScopeOwners.Count
    }
}

function Write-JsonResult {
    param([object]$Value, [string]$Path)
    $json = $Value | ConvertTo-Json -Depth 32
    if ([string]::IsNullOrWhiteSpace($Path)) {
        Write-Output $json
    } else {
        $directory = Split-Path -Parent $Path
        if (-not [string]::IsNullOrWhiteSpace($directory)) {
            New-Item -ItemType Directory -Force -Path $directory | Out-Null
        }
        [IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
        Write-Output "Wrote $Path"
    }
}

function Invoke-SelfTest {
    $script:SelfTestFailures = @()
    $script:SelfTestCount = 0
    function Assert-Case {
        param([string]$Name, [scriptblock]$Body)
        $script:SelfTestCount++
        try {
            $result = & $Body
            if ($result -ne $true) {
                $script:SelfTestFailures += "$Name failed"
                Write-Host "FAIL $Name"
            } else {
                Write-Host "PASS $Name"
            }
        } catch {
            $script:SelfTestFailures += "$Name error: $($_.Exception.Message)"
            Write-Host "FAIL $Name ($($_.Exception.Message))"
        }
    }

    $validCandidate = Read-JsonFile (Join-Path $fixtureRoot "candidate\valid-synthetic.json")
    $validUpstream = Read-JsonFile (Join-Path $fixtureRoot "upstream\accepted.json")
    $validMatrix = Read-JsonFile (Join-Path $fixtureRoot "matrix\valid-synthetic.json")
    $acceptedM02Path = Join-Path $fixtureRoot "m02\accepted-synthetic.json"
    $failingM02Path = Join-Path $fixtureRoot "m02\failing-synthetic.json"
    $m04Path = Join-Path $fixtureRoot "m04\accepted-baseline-synthetic.json"
    $m05Path = Join-Path $fixtureRoot "m05\formal-regression-synthetic.json"

    Assert-Case "upstream-all-accepted" {
        (Test-UpstreamEligibility $validUpstream).Pass -eq $true
    }
    Assert-Case "upstream-development-complete-is-blocked" {
        (Test-UpstreamEligibility (Read-JsonFile (Join-Path $fixtureRoot "upstream\development-complete.json"))).Pass -eq $false
    }
    Assert-Case "candidate-clean" {
        (Test-CandidateIdentity $validCandidate).Pass -eq $true
    }
    Assert-Case "candidate-dirty-is-rejected" {
        $candidate = Copy-JsonObject $validCandidate
        $candidate.source_dirty = $true
        (Test-CandidateIdentity $candidate).Pass -eq $false
    }
    Assert-Case "candidate-missing-installed-hash-is-rejected" {
        $candidate = Copy-JsonObject $validCandidate
        $candidate.installed_runtime_sha256 = $null
        (Test-CandidateIdentity $candidate).Pass -eq $false
    }
    Assert-Case "candidate-mixed-installed-hash-is-rejected" {
        $candidate = Copy-JsonObject $validCandidate
        $candidate.installed_ui_sha256 = ("0" * 64)
        (Test-CandidateIdentity $candidate).Pass -eq $false
    }
    Assert-Case "matrix-complete" {
        (Test-CoreMatrix $validMatrix $validCandidate).Pass -eq $true
    }
    Assert-Case "matrix-missing-core-is-rejected" {
        $matrix = Copy-JsonObject $validMatrix
        $matrix.rows = @($matrix.rows | Where-Object { $_.scope_id -ne "F10-AC02" })
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "matrix-duplicate-core-is-rejected" {
        $matrix = Copy-JsonObject $validMatrix
        $matrix.rows += (Copy-JsonObject ($matrix.rows | Where-Object { $_.scope_id -eq "SEC-AC01" })[0])
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "matrix-blocked-core-is-rejected" {
        $matrix = Copy-JsonObject $validMatrix
        ($matrix.rows | Where-Object { $_.scope_id -eq "F07-AC02" }).final_status = "BLOCKED"
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "matrix-p2-cannot-satisfy-core" {
        $matrix = Copy-JsonObject $validMatrix
        ($matrix.rows | Where-Object { $_.scope_id -eq "F12-AC01" }).classification = "DEFERRED_P2"
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "matrix-unknown-status-is-rejected" {
        $matrix = Copy-JsonObject $validMatrix
        ($matrix.rows | Where-Object { $_.scope_id -eq "F01-AC01" }).final_status = "UNKNOWN"
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "matrix-missing-real-chatgpt-is-rejected" {
        $matrix = Copy-JsonObject $validMatrix
        $matrix.rows = @($matrix.rows | Where-Object { $_.scope_id -ne "EXT-REAL-CHATGPT" })
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "matrix-real-chatgpt-not-manual-is-rejected" {
        $matrix = Copy-JsonObject $validMatrix
        ($matrix.rows | Where-Object { $_.scope_id -eq "EXT-REAL-CHATGPT" }).manual_status = "NOT_REQUIRED"
        (Test-CoreMatrix $matrix $validCandidate).Pass -eq $false
    }
    Assert-Case "f05-equivalent" {
        (Get-M02EquivalenceDecision $acceptedM02Path $validCandidate).Status -eq "EQUIVALENT_REUSE_ALLOWED"
    }
    Assert-Case "f05-mixed-runtime-requires-rerun" {
        $candidate = Copy-JsonObject $validCandidate
        $candidate.portable_runtime_sha256 = ("1" * 64)
        (Get-M02EquivalenceDecision $acceptedM02Path $candidate).Status -eq "FULL_M02_BENCHMARK_REQUIRED"
    }
    Assert-Case "f05-threshold-failure-cannot-become-pass" {
        (Get-M02EquivalenceDecision $failingM02Path $validCandidate).Status -eq "FAIL"
    }
    Assert-Case "f05-missing-evidence-is-blocked" {
        (Get-M02EquivalenceDecision (Join-Path $fixtureRoot "m02\missing.json") $validCandidate).Status -eq "BLOCKED_MISSING_ACCEPTED_M02"
    }
    Assert-Case "f12-equivalent" {
        (Get-F12EquivalenceDecision $m04Path $m05Path $validCandidate).Status -eq "EQUIVALENT_REUSE_ALLOWED"
    }
    Assert-Case "f12-mixed-tool-schema-requires-requalification" {
        $regression = Read-JsonFile $m05Path
        $regression.candidateIdentity.toolSchemaHash = ("2" * 64)
        $temp = Join-Path ([IO.Path]::GetTempPath()) ("m06-f12-" + [guid]::NewGuid().ToString("N") + ".json")
        $regression | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $temp -Encoding utf8
        try {
            (Get-F12EquivalenceDecision $m04Path $temp $validCandidate).Status -eq "REQUALIFICATION_REQUIRED"
        } finally {
            Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
        }
    }
    Assert-Case "f12-missing-evidence-is-blocked" {
        (Get-F12EquivalenceDecision (Join-Path $fixtureRoot "m04\missing.json") (Join-Path $fixtureRoot "m05\missing.json") $validCandidate).Status -eq "BLOCKED_MISSING_ACCEPTED_EVIDENCE"
    }
    Assert-Case "f12-inconclusive-drift-is-not-pass" {
        $regression = Read-JsonFile $m05Path
        $regression.pairedControl = [pscustomobject]@{ applicable = $true; withinGuard = $false }
        $temp = Join-Path ([IO.Path]::GetTempPath()) ("m06-f12-" + [guid]::NewGuid().ToString("N") + ".json")
        $regression | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $temp -Encoding utf8
        try {
            (Get-F12EquivalenceDecision $m04Path $temp $validCandidate).Status -eq "INCONCLUSIVE"
        } finally {
            Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
        }
    }
    Assert-Case "gate-synthetic-accepted" {
        $m02 = Get-M02EquivalenceDecision $acceptedM02Path $validCandidate
        $f12 = Get-F12EquivalenceDecision $m04Path $m05Path $validCandidate
        $gate = New-GateResult $validCandidate $validUpstream $validMatrix $m02 $f12 $null
        $gate.final_verdict -eq "PASS" -and $gate.product_verdict -eq "V4 Core $EmDash ACCEPTED"
    }
    Assert-Case "gate-current-dependencies-are-blocked" {
        $currentUpstream = [pscustomobject]@{
            schema = "m06-upstream-state-v1"
            milestones = @(
                [pscustomobject]@{ id = "M00"; status = "ACCEPTED_FROZEN" },
                [pscustomobject]@{ id = "M01"; status = "NOT_ACCEPTED" },
                [pscustomobject]@{ id = "M02"; status = "FAIL" },
                [pscustomobject]@{ id = "M03"; status = "DEVELOPMENT_COMPLETE" },
                [pscustomobject]@{ id = "M04"; status = "DEVELOPMENT_COMPLETE" },
                [pscustomobject]@{ id = "M05"; status = "DEVELOPMENT_COMPLETE" }
            )
        }
        $m02 = Get-M02EquivalenceDecision (Join-Path $fixtureRoot "m02\missing.json") $validCandidate
        $f12 = Get-F12EquivalenceDecision (Join-Path $fixtureRoot "m04\missing.json") (Join-Path $fixtureRoot "m05\missing.json") $validCandidate
        $gate = New-GateResult $validCandidate $currentUpstream $validMatrix $m02 $f12 $null
        $gate.final_verdict -eq "BLOCKED"
    }
    Assert-Case "package-tamper-is-rejected" {
        $temp = Join-Path ([IO.Path]::GetTempPath()) ("m06-package-" + [guid]::NewGuid().ToString("N"))
        try {
            New-Item -ItemType Directory -Force -Path (Join-Path $temp "runtime"), (Join-Path $temp "tunnel") | Out-Null
            foreach ($relative in @("chatgpt-harness-ui.exe", "runtime\chatgpt-harness-runtime.exe", "tunnel\tunnel-client.exe", "tunnel\cloudflared.exe")) {
                [IO.File]::WriteAllText((Join-Path $temp $relative), "synthetic payload")
            }
            $manifest = [ordered]@{
                build_sha = $validCandidate.git_commit_sha
                source_dirty = $false
                ui_sha256 = (Get-Sha256File (Join-Path $temp "chatgpt-harness-ui.exe"))
                runtime_sha256 = (Get-Sha256File (Join-Path $temp "runtime\chatgpt-harness-runtime.exe"))
                tunnel_client_sha256 = (Get-Sha256File (Join-Path $temp "tunnel\tunnel-client.exe"))
            }
            $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $temp "distribution.json") -Encoding utf8
            $candidate = Copy-JsonObject $validCandidate
            $candidate.portable_ui_sha256 = $manifest.ui_sha256
            $candidate.portable_runtime_sha256 = $manifest.runtime_sha256
            $candidate.portable_tunnel_sha256 = $manifest.tunnel_client_sha256
            $candidate.installed_ui_sha256 = $manifest.ui_sha256
            $candidate.installed_runtime_sha256 = $manifest.runtime_sha256
            $candidate.installed_tunnel_sha256 = $manifest.tunnel_client_sha256
            $candidate.portable_package_sha256 = Get-TreeHash $temp
            $before = Test-PackageContract $temp "" $candidate
            [IO.File]::AppendAllText((Join-Path $temp "chatgpt-harness-ui.exe"), "tamper")
            $after = Test-PackageContract $temp "" $candidate
            $before.Pass -eq $true -and $after.Pass -eq $false
        } finally {
            if (Test-Path -LiteralPath $temp) {
                $resolved = [IO.Path]::GetFullPath($temp)
                if ($resolved.StartsWith($tempRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
                    [IO.Path]::GetFileName($resolved) -like "m06-package-*") {
                    Remove-Item -LiteralPath $resolved -Recurse -Force
                }
            }
        }
    }
    Assert-Case "package-secret-content-is-rejected" {
        $temp = Join-Path ([IO.Path]::GetTempPath()) ("m06-package-" + [guid]::NewGuid().ToString("N"))
        try {
            New-Item -ItemType Directory -Force -Path (Join-Path $temp "runtime"), (Join-Path $temp "tunnel") | Out-Null
            foreach ($relative in @("chatgpt-harness-ui.exe", "runtime\chatgpt-harness-runtime.exe", "tunnel\tunnel-client.exe", "tunnel\cloudflared.exe")) {
                [IO.File]::WriteAllText((Join-Path $temp $relative), "synthetic payload")
            }
            [IO.File]::WriteAllText((Join-Path $temp "config.json"), '{"apiKey":"M06_SECRET_SENTINEL"}')
            $candidate = Copy-JsonObject $validCandidate
            $result = Test-PackageContract $temp "" $candidate
            $result.Pass -eq $false
        } finally {
            if (Test-Path -LiteralPath $temp) {
                $resolved = [IO.Path]::GetFullPath($temp)
                if ($resolved.StartsWith($tempRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
                    [IO.Path]::GetFileName($resolved) -like "m06-package-*") {
                    Remove-Item -LiteralPath $resolved -Recurse -Force
                }
            }
        }
    }
    Assert-Case "release-notes-reject-v4-full" {
        @(Test-ReleaseNotesText "# V4 Full $EmDash ACCEPTED`nProduct scope: V4 Core`nOUT_OF_CORE / NOT_CORE_ACCEPTED").Count -gt 0
    }

    if ($script:SelfTestFailures.Count -gt 0) {
        Write-Output ("M06 gate self-test: FAIL ({0} cases, {1} failures)" -f $script:SelfTestCount, $script:SelfTestFailures.Count)
        foreach ($failure in $script:SelfTestFailures) { Write-Output " - $failure" }
        exit 1
    }
    Write-Output ("M06 gate self-test: PASS ({0} cases)" -f $script:SelfTestCount)
}

switch ($Command) {
    "collect-identity" {
        $identity = Get-CandidateIdentity `
            -Distribution $DistributionDirectory `
            -Portable $PortableDirectory `
            -Installer $InstallerPath `
            -ProfilePath $ConfigProfilePath `
            -M04Baseline $M04BaselinePath `
            -M05Regression $M05RegressionPath `
            -EvalBinary $M04EvalBinary
        $validation = Test-CandidateIdentity $identity -AllowIncompleteEval
        Write-JsonResult ([pscustomobject]@{ identity = $identity; validation = $validation }) $OutputPath
    }
    "validate-identity" {
        $candidate = Read-JsonFile $CandidatePath
        $validation = Test-CandidateIdentity $candidate
        $result = [pscustomobject]@{ pass = $validation.Pass; reasons = $validation.Reasons }
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("Candidate identity: " + $(if ($validation.Pass) { "PASS" } else { "FAIL" })); $validation.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $validation.Pass) { exit 1 }
    }
    "eligibility" {
        $upstream = Read-JsonFile $UpstreamPath
        $result = Test-UpstreamEligibility $upstream
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("Upstream eligibility: " + $(if ($result.Pass) { "PASS" } else { "BLOCKED" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "validate-matrix" {
        $candidate = Read-JsonFile $CandidatePath
        $matrix = Read-JsonFile $MatrixPath
        $result = Test-CoreMatrix $matrix $candidate
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("Core matrix: " + $(if ($result.Pass) { "PASS" } else { "FAIL" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "equivalence-f05" {
        $candidate = Read-JsonFile $CandidatePath
        $result = Get-M02EquivalenceDecision $M02EvidencePath $candidate
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("F05 equivalence: " + $result.Status); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "equivalence-f12" {
        $candidate = Read-JsonFile $CandidatePath
        $result = Get-F12EquivalenceDecision $M04BaselinePath $M05RegressionPath $candidate
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("F12 equivalence: " + $result.Status); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "audit-package" {
        $candidate = Read-JsonFile $CandidatePath
        $result = Test-PackageContract $PortableDirectory $InstallerPath $candidate -RequireInstallerPayload:$RequireInstaller
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("Package audit: " + $(if ($result.Pass) { "PASS" } else { "FAIL" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "release-notes" {
        $candidate = Read-JsonFile $CandidatePath
        $matrix = Read-JsonFile $MatrixPath
        $gate = [pscustomobject]@{ final_verdict = "BLOCKED"; f05_status = "NOT_EXECUTED"; f12_status = "NOT_EXECUTED" }
        $notes = New-ReleaseNotes $candidate $matrix $gate
        $issues = Test-ReleaseNotesText $notes
        if ([string]::IsNullOrWhiteSpace($OutputPath)) { Write-Output $notes } else {
            [IO.File]::WriteAllText($OutputPath, $notes, [Text.UTF8Encoding]::new($false))
            Write-Output "Wrote $OutputPath"
        }
        if ($issues.Count -gt 0) { $issues | ForEach-Object { Write-Output " - $_" }; exit 1 }
    }
    "gate" {
        $candidate = if (-not [string]::IsNullOrWhiteSpace($CandidatePath) -and (Test-Path -LiteralPath $CandidatePath -PathType Leaf)) { Read-JsonFile $CandidatePath } else { $null }
        $upstream = if (-not [string]::IsNullOrWhiteSpace($UpstreamPath) -and (Test-Path -LiteralPath $UpstreamPath -PathType Leaf)) { Read-JsonFile $UpstreamPath } else { $null }
        $matrix = if (-not [string]::IsNullOrWhiteSpace($MatrixPath) -and (Test-Path -LiteralPath $MatrixPath -PathType Leaf)) { Read-JsonFile $MatrixPath } else { $null }
        $m02 = Get-M02EquivalenceDecision $M02EvidencePath $candidate
        $f12 = Get-F12EquivalenceDecision $M04BaselinePath $M05RegressionPath $candidate
        $package = $null
        if (-not [string]::IsNullOrWhiteSpace($PortableDirectory)) {
            $package = Test-PackageContract $PortableDirectory $InstallerPath $candidate -RequireInstallerPayload:$RequireInstaller
        }
        $gate = New-GateResult $candidate $upstream $matrix $m02 $f12 $package
        if ($Json -or -not [string]::IsNullOrWhiteSpace($OutputPath)) { Write-JsonResult $gate $OutputPath }
        else {
            Write-Output ("M06 Core gate: " + $gate.final_verdict)
            Write-Output ("Product verdict: " + $gate.product_verdict)
            Write-Output ("F05: " + $gate.f05_status)
            Write-Output ("F12: " + $gate.f12_status)
            $gate.reasons | ForEach-Object { Write-Output " - $_" }
        }
        if ($gate.final_verdict -ne "PASS") { exit 2 }
    }
    "self-test" {
        Invoke-SelfTest
    }
}
