[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet(
        "collect-identity",
        "validate-identity",
        "eligibility",
        "generate-matrix",
        "validate-matrix",
        "equivalence",
        "validate-m09-decision",
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
    [string]$M09DecisionPath = "",
    [string]$M10ConformancePath = "",
    [string]$M08MetricsPath = "",
    [string]$M08TracePath = "",
    [string]$F05EvidencePath = "",
    [string]$F12EvidencePath = "",
    [string]$F13EvidencePath = "",
    [string]$ConfigProfilePath = "",
    [string]$DistributionDirectory = "",
    [string]$PortableDirectory = "",
    [string]$InstallerPath = "",
    [string]$M04EvalBinary = "",
    [string]$EvidenceClass = "",
    [string]$EvidencePath = "",
    [string]$OutputPath = "",
    [switch]$RequireInstaller,
    [switch]$AllowIncompleteEvidence,
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "..\..")).Path
$m06GateScript = Join-Path $scriptDirectory "m06-release-gate.ps1"
$fixtureRoot = Join-Path $repositoryRoot "fixtures\v4\m11-release"
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$EmDash = [string][char]0x2014

$FeatureDefinitions = [ordered]@{
    F01 = [pscustomobject]@{ Priority = "P0"; Owner = "M01/M06"; Disposition = "PASS"; Ac = @("F01-AC01", "F01-AC02", "F01-AC03") }
    F02 = [pscustomobject]@{ Priority = "P1"; Owner = "M07"; Disposition = "PASS"; Ac = @("F02-AC01", "F02-AC02", "F02-AC03") }
    F03 = [pscustomobject]@{ Priority = "P0"; Owner = "M01/M06"; Disposition = "PASS"; Ac = @("F03-AC01", "F03-AC02", "F03-AC03") }
    F04 = [pscustomobject]@{ Priority = "P0"; Owner = "M01/M06"; Disposition = "PASS"; Ac = @("F04-AC01", "F04-AC02", "F04-AC03") }
    F05 = [pscustomobject]@{ Priority = "P0"; Owner = "M02/M06"; Disposition = "PASS"; Ac = @("F05-AC01", "F05-AC02", "F05-AC03") }
    F06 = [pscustomobject]@{ Priority = "P0"; Owner = "M02/M06"; Disposition = "PASS"; Ac = @("F06-AC01", "F06-AC02", "F06-AC03") }
    F07 = [pscustomobject]@{ Priority = "P0"; Owner = "M03/M06"; Disposition = "PASS"; Ac = @("F07-AC01", "F07-AC02", "F07-AC03") }
    F08 = [pscustomobject]@{ Priority = "P1"; Owner = "M08"; Disposition = "PASS"; Ac = @("F08-AC01", "F08-AC02", "F08-AC03") }
    F09 = [pscustomobject]@{ Priority = "P2"; Owner = "O01"; Disposition = "DEFERRED"; Ac = @("F09-AC01", "F09-AC02", "F09-AC03") }
    F10 = [pscustomobject]@{ Priority = "P0"; Owner = "M05/M06"; Disposition = "PASS"; Ac = @("F10-AC01", "F10-AC02", "F10-AC03") }
    F11 = [pscustomobject]@{ Priority = "P1"; Owner = "M09"; Disposition = "PASS"; Ac = @("F11-AC01", "F11-AC02", "F11-AC03") }
    F12 = [pscustomobject]@{ Priority = "P0"; Owner = "M04/M05/M06/M09"; Disposition = "PASS"; Ac = @("F12-AC01", "F12-AC02", "F12-AC03", "F12-AC04") }
    F13 = [pscustomobject]@{ Priority = "P1"; Owner = "M10"; Disposition = "PASS"; Ac = @("F13-AC01", "F13-AC02", "F13-AC03") }
    F14 = [pscustomobject]@{ Priority = "P1"; Owner = "M08"; Disposition = "PASS"; Ac = @("F14-AC01", "F14-AC02", "F14-AC03") }
    F15 = [pscustomobject]@{ Priority = "P2"; Owner = "O02"; Disposition = "DEFERRED"; Ac = @("F15-AC01", "F15-AC02", "F15-AC03") }
}
$SharedEvidenceAc = @("SEC-AC01", "SEC-AC02", "SEC-AC03")
$MandatoryUpstream = @("M06", "M07", "M08", "M09", "M10")
$M09Candidates = @("mkdir", "list_directory", "copy", "move", "rename", "delete")
$M09Verdicts = @("ADMIT", "REJECT", "DEFER")
$M11HashFields = @(
    "source_state_sha256",
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
    "desktop_preferences_hash",
    "evidence_schema_checksum",
    "evidence_migration_identity",
    "effective_tool_catalog_hash",
    "m09_decision_set_hash",
    "m02_performance_contract_identity",
    "m04_dataset_hash",
    "m04_system_prompt_hash",
    "m04_scorer_hash",
    "m05_regression_profile_hash",
    "m09_eval_profile_or_decision_hash",
    "m08_metrics_contract_identity",
    "m08_trace_contract_identity",
    "m10_conformance_result_identity"
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
    return ($Value | ConvertTo-Json -Depth 64 | ConvertFrom-Json)
}

function Get-Sha256Text {
    param([string]$Text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text))
    } finally {
        $sha.Dispose()
    }
    return ([BitConverter]::ToString($hash)).Replace("-", "").ToLowerInvariant()
}

function Get-Sha256File {
    param([string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $hash = $sha.ComputeHash($stream) } finally { $sha.Dispose() }
    } finally { $stream.Dispose() }
    return ([BitConverter]::ToString($hash)).Replace("-", "").ToLowerInvariant()
}

function Get-OptionalSha256File {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    return Get-Sha256File $Path
}

function ConvertTo-CanonicalJson {
    param([object]$Value)
    if ($null -eq $Value) { return "null" }
    if ($Value -is [string]) { return (ConvertTo-Json -InputObject ([string]$Value) -Compress) }
    if ($Value -is [bool]) { return $(if ($Value) { "true" } else { "false" }) }
    if ($Value -is [int] -or $Value -is [long] -or $Value -is [double] -or $Value -is [single] -or $Value -is [decimal]) {
        return ([Convert]::ToString($Value, [Globalization.CultureInfo]::InvariantCulture))
    }
    if ($Value -is [System.Collections.IDictionary]) {
        $parts = @()
        foreach ($name in @($Value.Keys | ForEach-Object { [string]$_ } | Sort-Object)) {
            $parts += ((ConvertTo-Json -InputObject $name -Compress) + ":" + (ConvertTo-CanonicalJson $Value[$name]))
        }
        return "{" + ($parts -join ",") + "}"
    }
    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        $parts = @()
        foreach ($name in @($Value.PSObject.Properties | ForEach-Object { $_.Name } | Sort-Object)) {
            $parts += ((ConvertTo-Json -InputObject $name -Compress) + ":" + (ConvertTo-CanonicalJson $Value.PSObject.Properties[$name].Value))
        }
        return "{" + ($parts -join ",") + "}"
    }
    if ($Value -is [System.Collections.IEnumerable]) {
        $parts = @()
        foreach ($item in $Value) { $parts += (ConvertTo-CanonicalJson $item) }
        return "[" + ($parts -join ",") + "]"
    }
    return (ConvertTo-Json -InputObject $Value -Compress -Depth 64)
}

function Get-TreeHash {
    param([string]$Root)
    $Root = [IO.Path]::GetFullPath($Root)
    if (-not (Test-Path -LiteralPath $Root -PathType Container)) { throw "Tree root is missing: $Root" }
    $entries = @()
    foreach ($file in (Get-ChildItem -LiteralPath $Root -Recurse -File | Sort-Object FullName)) {
        $relative = $file.FullName.Substring($Root.Length + 1).Replace('\', '/')
        $entries += [ordered]@{ path = $relative; sha256 = Get-Sha256File $file.FullName }
    }
    return Get-Sha256Text (ConvertTo-CanonicalJson $entries)
}

function Get-CodeIdentityHash {
    param([string[]]$RelativePaths)
    $entries = @()
    foreach ($relative in $RelativePaths) {
        $path = Join-Path $repositoryRoot ($relative.Replace('/', '\'))
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
        $entries += [ordered]@{ path = $relative.Replace('\', '/'); sha256 = Get-Sha256File $path }
    }
    return Get-Sha256Text (ConvertTo-CanonicalJson $entries)
}

function Invoke-M06IdentityCollection {
    param(
        [string]$Distribution,
        [string]$Portable,
        [string]$Installer,
        [string]$Profile,
        [string]$M04Baseline,
        [string]$M05Regression,
        [string]$EvalBinary
    )
    $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $m06GateScript, "collect-identity", "-Json")
    if (-not [string]::IsNullOrWhiteSpace($Distribution)) { $arguments += @("-DistributionDirectory", $Distribution) }
    if (-not [string]::IsNullOrWhiteSpace($Portable)) { $arguments += @("-PortableDirectory", $Portable) }
    if (-not [string]::IsNullOrWhiteSpace($Installer)) { $arguments += @("-InstallerPath", $Installer) }
    if (-not [string]::IsNullOrWhiteSpace($Profile)) { $arguments += @("-ConfigProfilePath", $Profile) }
    if (-not [string]::IsNullOrWhiteSpace($M04Baseline)) { $arguments += @("-M04BaselinePath", $M04Baseline) }
    if (-not [string]::IsNullOrWhiteSpace($M05Regression)) { $arguments += @("-M05RegressionPath", $M05Regression) }
    if (-not [string]::IsNullOrWhiteSpace($EvalBinary)) { $arguments += @("-M04EvalBinary", $EvalBinary) }
    $raw = & powershell.exe @arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "M06 identity collection failed: $($raw -join "`n")" }
    return (($raw -join "`n") | ConvertFrom-Json)
}

function Get-ArtifactIdentity {
    param([string]$Path, [string]$FallbackHash = $null)
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    $artifact = Read-JsonFile $Path
    foreach ($name in @("identity_hash", "contract_identity", "decision_set_hash", "result_identity")) {
        $value = [string](Get-Field $artifact $name)
        if (-not [string]::IsNullOrWhiteSpace($value)) { return $value }
    }
    $identity = Get-Field $artifact "identity"
    if ($null -ne $identity) {
        foreach ($name in @("identity_hash", "contract_identity", "result_identity")) {
            $value = [string](Get-Field $identity $name)
            if (-not [string]::IsNullOrWhiteSpace($value)) { return $value }
        }
        return Get-Sha256Text (ConvertTo-CanonicalJson $identity)
    }
    if ($null -ne $FallbackHash) { return $FallbackHash }
    return Get-Sha256File $Path
}

function Get-DesktopPreferencesHash {
    param([string]$ProfilePath)
    if ([string]::IsNullOrWhiteSpace($ProfilePath) -or -not (Test-Path -LiteralPath $ProfilePath -PathType Leaf)) { return $null }
    $profile = Read-JsonFile $ProfilePath
    $desktop = Get-Field $profile "desktop"
    if ($null -eq $desktop) { return $null }
    return Get-Sha256Text (ConvertTo-CanonicalJson $desktop)
}

function Get-M11CandidateIdentity {
    param(
        [string]$Distribution,
        [string]$Portable,
        [string]$Installer,
        [string]$Profile,
        [string]$M02Evidence,
        [string]$M04Baseline,
        [string]$M05Regression,
        [string]$M09Decision,
        [string]$M10Conformance,
        [string]$M08Metrics,
        [string]$M08Trace,
        [string]$EvalBinary
    )
    $baseResult = Invoke-M06IdentityCollection $Distribution $Portable $Installer $Profile $M04Baseline $M05Regression $EvalBinary
    $base = $baseResult.identity
    $identity = [ordered]@{}
    foreach ($property in $base.PSObject.Properties) { $identity[$property.Name] = $property.Value }

    $identity.schema = "m11-full-candidate-identity-v1"
    $identity.candidate_id = "M11-" + ([string]$base.git_commit_sha).Substring(0, [Math]::Min(7, ([string]$base.git_commit_sha).Length))
    if (-not [string]::IsNullOrWhiteSpace([string]$base.portable_ui_sha256)) {
        $identity.candidate_id += "-" + ([string]$base.portable_ui_sha256).Substring(0, 8).ToLowerInvariant()
    }
    $identity.desktop_preferences_hash = Get-DesktopPreferencesHash $Profile
    $identity.evidence_migration_identity = Get-CodeIdentityHash @(
        "runtime/evidence-store/src/schema.rs",
        "runtime/evidence-store/src/migration.rs"
    )
    $identity.m02_performance_contract_identity = Get-ArtifactIdentity $M02Evidence
    $identity.m05_regression_profile_hash = Get-ArtifactIdentity $M05Regression
    $identity.m09_decision_set_hash = Get-ArtifactIdentity $M09Decision
    $identity.m09_eval_profile_or_decision_hash = if ($null -ne $identity.m09_decision_set_hash) { $identity.m09_decision_set_hash } else { $null }
    $identity.m08_metrics_contract_identity = if (-not [string]::IsNullOrWhiteSpace($M08Metrics)) {
        Get-ArtifactIdentity $M08Metrics
    } else {
        Get-CodeIdentityHash @("protocol/generated/json-schema/metrics-query-params.schema.json", "protocol/generated/json-schema/metrics-query-result.schema.json", "runtime/evidence-store/src/metrics.rs")
    }
    $identity.m08_trace_contract_identity = if (-not [string]::IsNullOrWhiteSpace($M08Trace)) {
        Get-ArtifactIdentity $M08Trace
    } else {
        Get-CodeIdentityHash @("protocol/generated/json-schema/trace-get-params.schema.json", "protocol/generated/json-schema/trace-get-result.schema.json", "runtime/evidence-store/src/trace.rs")
    }
    $identity.m10_mcp_revision = "2025-03-26"
    $identity.m10_rmcp_version = "3.2.0"
    $identity.m10_official_runner_commit = "c37eec888e1c6ff140af79987a40008548b7cc5f"
    $identity.m10_official_runner_package = "0.2.0-alpha.12"
    $identity.m10_runner_sdk_version = "1.29.0"
    $identity.m10_transport_identity = Get-CodeIdentityHash @("runtime/src/mcp/http.rs", "runtime/src/mcp/service.rs", "runtime/Cargo.toml", "Cargo.lock")
    $identity.m10_conformance_result_identity = Get-ArtifactIdentity $M10Conformance
    $identity.optional_f09_disposition = "DEFERRED"
    $identity.optional_f15_disposition = "DEFERRED"
    return [pscustomobject]$identity
}

function Test-M11CandidateIdentity {
    param([object]$Candidate, [switch]$AllowIncompleteEvidence)
    $reasons = @()
    if ($null -eq $Candidate) { return [pscustomobject]@{ Pass = $false; Reasons = @("M11 candidate identity is missing.") } }
    if ((Get-Field $Candidate "schema") -ne "m11-full-candidate-identity-v1") { $reasons += "Candidate identity schema is not m11-full-candidate-identity-v1." }
    foreach ($field in @("candidate_id", "git_commit_sha", "branch", "product_version", "windows_environment_class")) {
        if ([string]::IsNullOrWhiteSpace([string](Get-Field $Candidate $field))) { $reasons += "Candidate identity is missing $field." }
    }
    if ((Get-Field $Candidate "source_dirty") -ne $false) { $reasons += "source_dirty must be false for a Full RC." }
    $incompleteEvidenceFields = @(
        "m02_performance_contract_identity",
        "m04_dataset_hash",
        "m04_system_prompt_hash",
        "m04_scorer_hash",
        "m05_regression_profile_hash",
        "m09_eval_profile_or_decision_hash",
        "m09_decision_set_hash",
        "m08_metrics_contract_identity",
        "m08_trace_contract_identity",
        "m10_conformance_result_identity"
    )
    foreach ($field in $M11HashFields) {
        $value = [string](Get-Field $Candidate $field)
        if (-not $AllowIncompleteEvidence -or $field -notin $incompleteEvidenceFields) {
            if ($value -notmatch '^[A-Fa-f0-9]{64}$') { $reasons += "Candidate identity has an invalid or missing SHA-256: $field." }
        }
    }
    foreach ($field in @("control_protocol_revision", "config_schema_version", "evidence_schema_version")) {
        if ([int](Get-Field $Candidate $field) -lt 1) { $reasons += "$field is invalid." }
    }
    if ([int](Get-Field $Candidate "config_schema_version") -ne 3) { $reasons += "AppConfig schema must remain 3." }
    if ([int](Get-Field $Candidate "control_protocol_revision") -ne 1) { $reasons += "Control Protocol must remain v1." }
    foreach ($field in @("effective_config_revision", "desktop_preferences_hash", "evidence_migration_identity", "m10_transport_identity", "m10_official_runner_commit", "m10_official_runner_package", "m10_runner_sdk_version")) {
        if ([string]::IsNullOrWhiteSpace([string](Get-Field $Candidate $field))) { $reasons += "Candidate identity is missing $field." }
    }
    foreach ($field in @("adapter_id_version", "requested_model", "resolved_model")) {
        if ([string]::IsNullOrWhiteSpace([string](Get-Field $Candidate $field))) { $reasons += "Candidate identity is missing $field." }
    }
    if ([string](Get-Field $Candidate "m10_mcp_revision") -ne "2025-03-26") { $reasons += "M10 MCP revision drifted from 2025-03-26." }
    if ([string](Get-Field $Candidate "m10_rmcp_version") -ne "3.2.0") { $reasons += "M10 rmcp version drifted from 3.2.0." }
    if ([string](Get-Field $Candidate "optional_f09_disposition") -ne "DEFERRED") { $reasons += "F09 must remain DEFERRED." }
    if ([string](Get-Field $Candidate "optional_f15_disposition") -ne "DEFERRED") { $reasons += "F15 must remain DEFERRED." }
    if ([string](Get-Field $Candidate "portable_ui_sha256") -ne [string](Get-Field $Candidate "installed_ui_sha256")) { $reasons += "Portable and installed UI hashes differ." }
    if ([string](Get-Field $Candidate "portable_runtime_sha256") -ne [string](Get-Field $Candidate "installed_runtime_sha256")) { $reasons += "Portable and installed Runtime hashes differ." }
    if ([string](Get-Field $Candidate "portable_tunnel_sha256") -ne [string](Get-Field $Candidate "installed_tunnel_sha256")) { $reasons += "Portable and installed Tunnel hashes differ." }
    $perTool = Get-Field $Candidate "per_tool_schema_hashes"
    if ($null -eq $perTool -or @($perTool.PSObject.Properties).Count -lt 1) { $reasons += "per_tool_schema_hashes is empty." }
    if (-not $AllowIncompleteEvidence -and [string](Get-Field $Candidate "m10_conformance_result_identity") -notmatch '^[A-Fa-f0-9]{64}$') { $reasons += "M10 conformance result identity is unavailable." }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons }
}

function Test-M11UpstreamEligibility {
    param([object]$Upstream)
    $reasons = @(); $statuses = @{}; $identities = @{}
    if ($null -eq $Upstream -or (Get-Field $Upstream "schema") -ne "m11-upstream-state-v1") {
        return [pscustomobject]@{ Pass = $false; Reasons = @("M11 upstream state is missing or has an unknown schema."); Statuses = $statuses; Identities = $identities }
    }
    foreach ($item in @(Get-Field $Upstream "milestones")) {
        $id = [string](Get-Field $item "id")
        if ($statuses.ContainsKey($id)) { $reasons += "Upstream milestone $id is duplicated." }
        $statuses[$id] = [string](Get-Field $item "status")
        $identities[$id] = [string](Get-Field $item "identity")
        if ([string]::IsNullOrWhiteSpace([string](Get-Field $item "evidence_reference"))) { $reasons += "Upstream milestone $id has no evidence_reference." }
    }
    foreach ($id in $MandatoryUpstream) {
        if (-not $statuses.ContainsKey($id)) { $reasons += "Upstream milestone $id is missing." }
        elseif ($statuses[$id] -ne "ACCEPTED_FROZEN") { $reasons += "Upstream milestone $id is $($statuses[$id]); ACCEPTED_FROZEN is required." }
        if ([string]::IsNullOrWhiteSpace([string]$identities[$id])) { $reasons += "Upstream milestone $id has no accepted identity." }
    }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons; Statuses = $statuses; Identities = $identities }
}

function Test-M11Matrix {
    param([object]$Matrix, [object]$Candidate)
    $reasons = @(); $rowsByFeature = @{}; $seenAc = @{}; $shared = Get-Field $Matrix "shared_evidence"
    if ($null -eq $Matrix -or (Get-Field $Matrix "schema") -ne "m11-full-matrix-v1") { return [pscustomobject]@{ Pass = $false; Reasons = @("M11 matrix is missing or has an unknown schema."); RowsByFeature = $rowsByFeature } }
    if ($null -ne $Candidate -and [string](Get-Field $Matrix "candidate_id") -ne [string](Get-Field $Candidate "candidate_id")) { $reasons += "M11 matrix candidate_id does not match the candidate." }
    foreach ($row in @(Get-Field $Matrix "features")) {
        $feature = [string](Get-Field $row "feature")
        if (-not $FeatureDefinitions.Contains($feature)) { $reasons += "Unknown M11 feature row: $feature."; continue }
        if ($rowsByFeature.ContainsKey($feature)) { $reasons += "M11 feature row $feature is duplicated." }
        $rowsByFeature[$feature] = $row
        $definition = $FeatureDefinitions[$feature]
        foreach ($field in @("priority", "upstream_owner", "required_disposition", "canonical_ac_ids", "upstream_identity", "final_candidate_recheck", "candidate_equivalence_key", "integration_status", "e2e_status", "manual_status", "final_status", "evidence_reference", "blocker")) {
            if ($null -eq (Get-Field $row $field)) { $reasons += "M11 feature $feature is missing $field." }
        }
        if ([string](Get-Field $row "priority") -ne $definition.Priority) { $reasons += "M11 feature $feature has the wrong priority." }
        if ([string](Get-Field $row "upstream_owner") -ne $definition.Owner) { $reasons += "M11 feature $feature has the wrong owner." }
        if ([string](Get-Field $row "required_disposition") -ne $definition.Disposition) { $reasons += "M11 feature $feature has the wrong required disposition." }
        $actualAc = @((Get-Field $row "canonical_ac_ids") | ForEach-Object { [string]$_ })
        if ($actualAc.Count -ne $definition.Ac.Count -or (($actualAc | Sort-Object) -join "|") -cne (($definition.Ac | Sort-Object) -join "|")) { $reasons += "M11 feature $feature canonical AC coverage is incorrect." }
        foreach ($ac in $actualAc) {
            if ($seenAc.ContainsKey($ac)) { $reasons += "Canonical AC $ac is duplicated." }
            $seenAc[$ac] = $feature
        }
        $allowedStatus = @("PASS", "FAIL", "BLOCKED", "INCONCLUSIVE", "NOT_VERIFIED", "NOT_EXECUTED", "DEFERRED", "NOT_REQUIRED")
        foreach ($field in @("integration_status", "e2e_status", "manual_status", "final_status")) {
            if ($allowedStatus -notcontains [string](Get-Field $row $field)) { $reasons += "M11 feature $feature has an invalid $field." }
        }
        if ($definition.Disposition -eq "DEFERRED") {
            if ([string](Get-Field $row "final_status") -ne "DEFERRED") { $reasons += "Optional feature $feature must be DEFERRED." }
        } elseif ([string](Get-Field $row "final_status") -eq "DEFERRED") {
            $reasons += "Mandatory feature $feature cannot be DEFERRED."
        }
        if ($null -ne $Candidate -and [string](Get-Field $row "candidate_equivalence_key") -ne [string](Get-Field $Candidate "candidate_id")) { $reasons += "M11 feature $feature is not bound to the candidate." }
    }
    foreach ($feature in $FeatureDefinitions.Keys) { if (-not $rowsByFeature.ContainsKey($feature)) { $reasons += "M11 feature $feature is missing." } }
    foreach ($ac in ($FeatureDefinitions.Values | ForEach-Object { $_.Ac })) { if (-not $seenAc.ContainsKey($ac)) { $reasons += "Canonical AC $ac is missing." } }
    if ($null -eq $shared -or [string](Get-Field $shared "section") -ne "Shared Evidence") { $reasons += "Separate Shared Evidence section is missing." }
    else {
        $sharedAc = @((Get-Field $shared "canonical_ac_ids") | ForEach-Object { [string]$_ })
        if ($sharedAc.Count -ne $SharedEvidenceAc.Count -or (($sharedAc | Sort-Object) -join "|") -cne (($SharedEvidenceAc | Sort-Object) -join "|")) { $reasons += "Shared Evidence canonical AC coverage is incorrect." }
        foreach ($ac in $sharedAc) { if ($seenAc.ContainsKey($ac)) { $reasons += "Canonical AC $ac is duplicated." }; $seenAc[$ac] = "Shared Evidence" }
    }
    $mandatoryCount = @($seenAc.Keys | Where-Object { $_ -match '^(SEC|F(0[1-8]|1[0-4]))-AC' }).Count
    if ($mandatoryCount -ne 43) { $reasons += "M11 mandatory canonical AC count is $mandatoryCount, expected 43." }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons; RowsByFeature = $rowsByFeature; CanonicalAcCount = $seenAc.Count }
}

function New-M11Matrix {
    param([object]$Candidate)
    if ($null -eq $Candidate -or [string]::IsNullOrWhiteSpace([string](Get-Field $Candidate "candidate_id"))) {
        throw "A candidate identity is required to generate the M11 matrix."
    }
    $features = [System.Collections.Generic.List[object]]::new()
    foreach ($feature in $FeatureDefinitions.Keys) {
        $definition = $FeatureDefinitions[$feature]
        $deferred = $definition.Disposition -eq "DEFERRED"
        $features.Add([pscustomobject][ordered]@{
            feature = $feature
            priority = $definition.Priority
            upstream_owner = $definition.Owner
            required_disposition = $definition.Disposition
            canonical_ac_ids = @($definition.Ac)
            upstream_identity = if ($deferred) { "DEFERRED" } else { "NOT_VERIFIED" }
            final_candidate_recheck = if ($deferred) { "OUT_OF_SCOPE" } else { "REQUIRES_EXECUTION" }
            candidate_equivalence_key = [string](Get-Field $Candidate "candidate_id")
            integration_status = if ($deferred) { "DEFERRED" } else { "NOT_EXECUTED" }
            e2e_status = if ($deferred) { "DEFERRED" } else { "NOT_EXECUTED" }
            manual_status = if ($deferred) { "DEFERRED" } else { "NOT_EXECUTED" }
            final_status = if ($deferred) { "DEFERRED" } else { "NOT_EXECUTED" }
            evidence_reference = if ($deferred) { "M11-SCOPE" } else { "NOT_EXECUTED" }
            blocker = if ($deferred) { "Optional P2 feature is not shipped." } else { "Formal M11 validation has not executed." }
        })
    }
    return [pscustomobject][ordered]@{
        schema = "m11-full-matrix-v1"
        candidate_id = [string](Get-Field $Candidate "candidate_id")
        generated_at_utc = [DateTime]::UtcNow.ToString("O")
        features = @($features)
        shared_evidence = [pscustomobject][ordered]@{
            section = "Shared Evidence"
            canonical_ac_ids = @($SharedEvidenceAc)
            candidate_equivalence_key = [string](Get-Field $Candidate "candidate_id")
            integration_status = "NOT_EXECUTED"
            e2e_status = "NOT_EXECUTED"
            manual_status = "NOT_EXECUTED"
            final_status = "NOT_EXECUTED"
            evidence_reference = "NOT_EXECUTED"
            blocker = "Formal M11 validation has not executed."
        }
    }
}

function Get-M11EquivalenceKeys {
    param([string]$Class)
    switch ($Class.ToUpperInvariant()) {
        "F05" { return @("portable_ui_sha256", "portable_runtime_sha256", "portable_tunnel_sha256", "config_profile_hash", "startup_benchmark_harness_version", "windows_environment_class", "m02_performance_contract_identity") }
        "F12" { return @("portable_runtime_sha256", "effective_tool_catalog_hash", "m04_dataset_hash", "m04_system_prompt_hash", "m04_scorer_hash", "m05_regression_profile_hash", "m09_eval_profile_or_decision_hash", "m09_decision_set_hash", "adapter_id_version", "requested_model", "resolved_model") }
        "F13" { return @("portable_runtime_sha256", "m10_rmcp_version", "m10_mcp_revision", "m10_transport_identity", "effective_tool_catalog_hash", "m10_official_runner_commit", "m10_official_runner_package", "m10_runner_sdk_version") }
        "F08" { return @("portable_runtime_sha256", "evidence_schema_version", "evidence_schema_checksum", "evidence_migration_identity", "m08_metrics_contract_identity") }
        "F14" { return @("portable_runtime_sha256", "evidence_schema_version", "evidence_schema_checksum", "evidence_migration_identity", "m08_trace_contract_identity") }
        "CORE" { return @("portable_ui_sha256", "portable_runtime_sha256", "portable_tunnel_sha256", "config_profile_hash", "effective_tool_catalog_hash") }
        default { throw "Unknown M11 evidence class: $Class" }
    }
}

function Get-M11EquivalenceDecision {
    param([string]$Class, [string]$Evidence, [object]$Candidate)
    if ([string]::IsNullOrWhiteSpace($Evidence) -or -not (Test-Path -LiteralPath $Evidence -PathType Leaf)) {
        return [pscustomobject]@{ classification = "INVALID_BLOCKER"; pass = $false; reasons = @("Accepted $Class evidence identity is unavailable.") }
    }
    $artifact = Read-JsonFile $Evidence
    if ((Get-Field $artifact "schema") -ne "m11-evidence-identity-v1" -or (Get-Field $artifact "accepted") -ne $true -or [string](Get-Field $artifact "status") -ne "PASS") {
        return [pscustomobject]@{ classification = "INVALID_BLOCKER"; pass = $false; reasons = @("$Class evidence is not an accepted PASS identity record.") }
    }
    $identity = Get-Field $artifact "identity"
    if ($null -eq $identity) { return [pscustomobject]@{ classification = "INVALID_BLOCKER"; pass = $false; reasons = @("$Class evidence identity object is missing.") } }
    $reasons = @(); $mismatch = $false
    foreach ($key in (Get-M11EquivalenceKeys $Class)) {
        $expected = [string](Get-Field $Candidate $key); $actual = [string](Get-Field $identity $key)
        if ([string]::IsNullOrWhiteSpace($expected) -or [string]::IsNullOrWhiteSpace($actual)) { $reasons += "Equivalence key $key is missing."; continue }
        if ($expected -cne $actual) { $mismatch = $true; $reasons += "Equivalence key differs at $key." }
    }
    foreach ($key in @("optional_f09_disposition", "optional_f15_disposition")) {
        if ([string](Get-Field $identity $key) -cne [string](Get-Field $Candidate $key)) { $reasons += "Disposition drift at $key."; $mismatch = $true }
    }
    if ($reasons | Where-Object { $_ -match "missing" }) { return [pscustomobject]@{ classification = "INVALID_BLOCKER"; pass = $false; reasons = $reasons } }
    if (-not $mismatch) { return [pscustomobject]@{ classification = "EXACTLY_EQUIVALENT"; pass = $true; reasons = @() } }
    if ($Class.ToUpperInvariant() -eq "CORE" -and @($reasons | Where-Object { $_ -match "portable_ui_sha256" }).Count -gt 0 -and $reasons.Count -eq 1) {
        return [pscustomobject]@{ classification = "REQUIRES_TARGETED_RERUN"; pass = $false; reasons = $reasons }
    }
    return [pscustomobject]@{ classification = "REQUIRES_FULL_RERUN"; pass = $false; reasons = $reasons }
}

function Test-M09DecisionSet {
    param([object]$DecisionSet, [object]$Candidate = $null)
    $reasons = @(); $seen = @{}
    if ($null -eq $DecisionSet -or [string](Get-Field $DecisionSet "schema") -ne "m09-decision-set-v1") { return [pscustomobject]@{ Pass = $false; Reasons = @("M09 decision set is missing or has an unknown schema.") } }
    if ([string](Get-Field $DecisionSet "decision_set_id") -ne "m09-file-tool-admission-v1") { $reasons += "M09 decision_set_id is not the accepted v1 identity." }
    if ([string](Get-Field $DecisionSet "quality_status") -ne "PASS") { $reasons += "M09 decision set quality_status is not PASS." }
    $hash = [string](Get-Field $DecisionSet "decision_set_hash")
    if ($hash -notmatch '^[A-Fa-f0-9]{64}$') { $reasons += "M09 decision_set_hash is missing or invalid." }
    foreach ($decision in @(Get-Field $DecisionSet "decisions")) {
        $name = [string](Get-Field $decision "candidate")
        if ($M09Candidates -notcontains $name) { $reasons += "Unknown M09 candidate $name." }
        if ($seen.ContainsKey($name)) { $reasons += "M09 candidate $name is duplicated." }
        $seen[$name] = $true
        if ($M09Verdicts -notcontains [string](Get-Field $decision "verdict")) { $reasons += "M09 candidate $name has an invalid verdict." }
        if ([string]::IsNullOrWhiteSpace([string](Get-Field $decision "reason_code"))) { $reasons += "M09 candidate $name has no reason_code." }
        if ([string](Get-Field $decision "schema_hash") -notmatch '^[A-Fa-f0-9]{64}$') { $reasons += "M09 candidate $name has no schema_hash." }
        if ([string](Get-Field $decision "verdict") -eq "ADMIT" -and [string](Get-Field $decision "tool_catalog_schema_hash") -notmatch '^[A-Fa-f0-9]{64}$') { $reasons += "ADMIT candidate $name has no tool_catalog_schema_hash." }
    }
    foreach ($name in $M09Candidates) { if (-not $seen.ContainsKey($name)) { $reasons += "M09 candidate $name is missing." } }
    if ($null -ne $Candidate -and $hash -ne [string](Get-Field $Candidate "m09_decision_set_hash")) { $reasons += "M09 decision set hash does not match the candidate." }
    return [pscustomobject]@{ Pass = ($reasons.Count -eq 0); Reasons = $reasons; DecisionCount = $seen.Count }
}

function Invoke-M06PackageAudit {
    param([string]$Candidate, [string]$Portable, [string]$Installer, [switch]$RequireInstallerPayload)
    $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $m06GateScript, "audit-package", "-CandidatePath", $Candidate, "-PortableDirectory", $Portable, "-Json")
    if (-not [string]::IsNullOrWhiteSpace($Installer)) { $arguments += @("-InstallerPath", $Installer) }
    if ($RequireInstallerPayload) { $arguments += "-RequireInstaller" }
    $raw = & powershell.exe @arguments 2>&1
    $text = $raw -join "`n"
    try { $parsed = $text | ConvertFrom-Json } catch { return [pscustomobject]@{ Pass = $false; Reasons = @("M06 package audit did not return JSON: $text") } }
    return $parsed
}

function Test-M11PackageContamination {
    param([string]$Portable)
    $reasons = @()
    if ([string]::IsNullOrWhiteSpace($Portable) -or -not (Test-Path -LiteralPath $Portable -PathType Container)) { return @("Portable directory is unavailable.") }
    $forbiddenNames = @("package-lock.json", "npm-shrinkwrap.json", "conformance", "credentials.json", "eval-provider.json", "m09-debug-tool.exe")
    foreach ($file in Get-ChildItem -LiteralPath $Portable -Recurse -File) {
        $relative = $file.FullName.Substring($Portable.Length + 1).Replace('\', '/')
        if ($forbiddenNames -contains $file.Name -or $relative -match '(?i)(conformance|m09-debug|eval-provider|provider-credential)') { $reasons += "Portable contains forbidden release artifact: $relative." }
        if ($file.Length -le 2097152 -and $file.Extension -in @(".json", ".txt", ".md", ".log", ".config", ".env")) {
            $content = Get-Content -Raw -LiteralPath $file.FullName -ErrorAction SilentlyContinue
            if ($content -match '(?i)(OPENAI_API_KEY|M04_EVAL_API_KEY|M05_EVAL_API_KEY|M09_TEST_SECRET|BEGIN PRIVATE KEY|sk-[A-Za-z0-9])') { $reasons += "Portable contains a secret or provider credential sentinel: $relative." }
        }
    }
    return $reasons
}

function New-M11ReleaseNotes {
    param([object]$Candidate, [object]$Matrix, [object]$Gate)
    $lines = @(
        "# V4 Full Release Candidate Notes",
        "",
        "- Candidate: $([string](Get-Field $Candidate 'candidate_id'))",
        "- Git commit: $([string](Get-Field $Candidate 'git_commit_sha'))",
        "- Product version: $([string](Get-Field $Candidate 'product_version'))",
        "- Control Protocol: $([string](Get-Field $Candidate 'control_protocol_revision'))",
        "- AppConfig schema: $([string](Get-Field $Candidate 'config_schema_version'))",
        "- MCP revision: $([string](Get-Field $Candidate 'm10_mcp_revision'))",
        "",
        "## Scope",
        "",
        "- Mandatory V4 Full scope is candidate-bound and remains subject to independent Integration, E2E and Manual validation.",
        "- F09 Estimated Tokens: DEFERRED.",
        "- F15 Policy Simulator: DEFERRED.",
        "",
        "## Truth boundary",
        "",
        "- F05 final-candidate result: $([string](Get-Field $Gate 'f05_status')).",
        "- F12 final-candidate result: $([string](Get-Field $Gate 'f12_status')).",
        "- F13 official conformance result: $([string](Get-Field $Gate 'f13_status')).",
        "- Real External ChatGPT: NOT_EXECUTED.",
        "- Formal M11 Integration: NOT_EXECUTED.",
        "- Formal M11 E2E: NOT_EXECUTED.",
        "- Formal M11 Manual: NOT_EXECUTED.",
        "",
        "## Verdict",
        "",
        "- V4 Full Gate: $([string](Get-Field $Gate 'final_verdict'))."
    )
    return ($lines -join "`n") + "`n"
}

function Test-M11ReleaseNotesText {
    param([string]$Text)
    $issues = @()
    if ($Text -notmatch "F09 Estimated Tokens: DEFERRED") { $issues += "Release notes do not mark F09 DEFERRED." }
    if ($Text -notmatch "F15 Policy Simulator: DEFERRED") { $issues += "Release notes do not mark F15 DEFERRED." }
    if ($Text -match "V4 Full $EmDash ACCEPTED|V4 $EmDash ACCEPTED / FROZEN") { $issues += "Release notes claim final acceptance prematurely." }
    return $issues
}

function New-M11GateResult {
    param([object]$Candidate, [object]$Upstream, [object]$Matrix, [object]$M09, [object]$F05, [object]$F12, [object]$F13, [object]$Package)
    $reasons = @()
    $candidateResult = Test-M11CandidateIdentity $Candidate
    $upstreamResult = Test-M11UpstreamEligibility $Upstream
    $matrixResult = Test-M11Matrix $Matrix $Candidate
    $reasons += $candidateResult.Reasons; $reasons += $upstreamResult.Reasons; $reasons += $matrixResult.Reasons
    if ($null -eq $M09 -or -not $M09.Pass) { $reasons += if ($null -eq $M09) { "M09 decision set validation is unavailable." } else { $M09.Reasons } }
    foreach ($gate in @($F05, $F12, $F13)) { if ($null -eq $gate -or -not $gate.pass) { $reasons += if ($null -eq $gate) { "Required equivalence result is unavailable." } else { $gate.reasons } } }
    if ($null -ne $Package -and -not $Package.Pass) { $reasons += $Package.Reasons }
    $reasons = @($reasons | ForEach-Object { [string]$_ } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $status = if ($reasons.Count -eq 0) { "PASS" } else { "BLOCKED" }
    return [pscustomobject]@{
        schema = "m11-release-gate-verdict-v1"
        candidate_id = [string](Get-Field $Candidate "candidate_id")
        generated_at_utc = [DateTime]::UtcNow.ToString("O")
        final_verdict = $status
        product_verdict = if ($status -eq "PASS") { "V4 $EmDash ACCEPTED / FROZEN" } else { "V4 $EmDash NOT ACCEPTED" }
        upstream_status = if ($upstreamResult.Pass) { "PASS" } else { "BLOCKED" }
        candidate_status = if ($candidateResult.Pass) { "PASS" } else { "FAIL" }
        matrix_status = if ($matrixResult.Pass) { "PASS" } else { "FAIL" }
        m09_status = if ($null -ne $M09 -and $M09.Pass) { "PASS" } else { "BLOCKED" }
        f05_status = if ($null -ne $F05) { [string]$F05.classification } else { "INVALID_BLOCKER" }
        f12_status = if ($null -ne $F12) { [string]$F12.classification } else { "INVALID_BLOCKER" }
        f13_status = if ($null -ne $F13) { [string]$F13.classification } else { "INVALID_BLOCKER" }
        package_status = if ($null -ne $Package -and $Package.Pass) { "PASS" } else { "NOT_EXECUTED" }
        reasons = $reasons
        feature_rows = $matrixResult.RowsByFeature.Count
        mandatory_canonical_ac_count = $matrixResult.CanonicalAcCount
    }
}

function Write-JsonResult {
    param([object]$Value, [string]$Path)
    $json = $Value | ConvertTo-Json -Depth 64
    if ([string]::IsNullOrWhiteSpace($Path)) { Write-Output $json; return }
    $directory = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($directory)) { New-Item -ItemType Directory -Force -Path $directory | Out-Null }
    [IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Write-Output "Wrote $Path"
}

function Invoke-SelfTest {
    $script:SelfTestFailures = @(); $script:SelfTestCount = 0
    function Assert-Case { param([string]$Name, [scriptblock]$Body); $script:SelfTestCount++; try { if ((& $Body) -ne $true) { $script:SelfTestFailures += "$Name failed"; Write-Host "FAIL $Name" } else { Write-Host "PASS $Name" } } catch { $script:SelfTestFailures += "$Name error: $($_.Exception.Message)"; Write-Host "FAIL $Name ($($_.Exception.Message))" } }
    $candidate = Read-JsonFile (Join-Path $fixtureRoot "candidate\valid-synthetic.json")
    $upstream = Read-JsonFile (Join-Path $fixtureRoot "upstream\accepted-synthetic.json")
    $matrix = Read-JsonFile (Join-Path $fixtureRoot "matrix\valid-synthetic.json")
    $decision = Read-JsonFile (Join-Path $fixtureRoot "m09\decision-set-v1-synthetic.json")
    $f05 = Join-Path $fixtureRoot "evidence\f05-equivalent.json"
    $f12 = Join-Path $fixtureRoot "evidence\f12-equivalent.json"
    $f13 = Join-Path $fixtureRoot "evidence\f13-equivalent.json"
    Assert-Case "candidate-valid" { (Test-M11CandidateIdentity $candidate).Pass -eq $true }
    Assert-Case "upstream-all-accepted" { (Test-M11UpstreamEligibility $upstream).Pass -eq $true }
    Assert-Case "development-complete-is-blocked" { $u = Copy-JsonObject $upstream; $u.milestones[2].status = "DEVELOPMENT_COMPLETE"; (Test-M11UpstreamEligibility $u).Pass -eq $false }
    Assert-Case "matrix-15-features-and-43-mandatory-ac" { $r = Test-M11Matrix $matrix $candidate; $r.Pass -eq $true -and $r.RowsByFeature.Count -eq 15 -and $r.CanonicalAcCount -eq 49 }
    Assert-Case "matrix-missing-feature-is-blocked" { $m = Copy-JsonObject $matrix; $m.features = @($m.features | Where-Object { $_.feature -ne "F14" }); (Test-M11Matrix $m $candidate).Pass -eq $false }
    Assert-Case "matrix-duplicate-ac-is-blocked" { $m = Copy-JsonObject $matrix; $m.features[0].canonical_ac_ids += "F02-AC01"; (Test-M11Matrix $m $candidate).Pass -eq $false }
    Assert-Case "generated-dry-run-matrix-is-not-acceptance" { $m = New-M11Matrix $candidate; $r = Test-M11Matrix $m $candidate; $r.Pass -eq $true -and [string](Get-Field $m.features[0] "final_status") -eq "NOT_EXECUTED" -and [string](Get-Field $m.features[8] "final_status") -eq "DEFERRED" }
    Assert-Case "decision-set-valid" { (Test-M09DecisionSet $decision $candidate).Pass -eq $true }
    Assert-Case "decision-set-missing-candidate-is-blocked" { $d = Copy-JsonObject $decision; $d.decisions = @($d.decisions | Where-Object { $_.candidate -ne "delete" }); (Test-M09DecisionSet $d $candidate).Pass -eq $false }
    Assert-Case "f05-equivalent" { (Get-M11EquivalenceDecision "F05" $f05 $candidate).classification -eq "EXACTLY_EQUIVALENT" }
    Assert-Case "runtime-change-requires-full-f05-rerun" { $c = Copy-JsonObject $candidate; $c.portable_runtime_sha256 = "1" * 64; (Get-M11EquivalenceDecision "F05" $f05 $c).classification -eq "REQUIRES_FULL_RERUN" }
    Assert-Case "ui-change-requires-full-f05-rerun" { $c = Copy-JsonObject $candidate; $c.portable_ui_sha256 = "2" * 64; (Get-M11EquivalenceDecision "F05" $f05 $c).classification -eq "REQUIRES_FULL_RERUN" }
    Assert-Case "tool-change-requires-full-f12-rerun" { $c = Copy-JsonObject $candidate; $c.effective_tool_catalog_hash = "3" * 64; (Get-M11EquivalenceDecision "F12" $f12 $c).classification -eq "REQUIRES_FULL_RERUN" }
    Assert-Case "mcp-change-requires-full-f13-rerun" { $c = Copy-JsonObject $candidate; $c.m10_mcp_revision = "2025-06-18"; (Get-M11EquivalenceDecision "F13" $f13 $c).classification -eq "REQUIRES_FULL_RERUN" }
    Assert-Case "evidence-schema-change-requires-full-f14-rerun" { $c = Copy-JsonObject $candidate; $c.evidence_schema_checksum = "4" * 64; (Get-M11EquivalenceDecision "F14" (Join-Path $fixtureRoot "evidence\f14-equivalent.json") $c).classification -eq "REQUIRES_FULL_RERUN" }
    Assert-Case "optional-disposition-drift-is-blocker" { $c = Copy-JsonObject $candidate; $c.optional_f09_disposition = "PASS"; (Test-M11CandidateIdentity $c).Pass -eq $false }
    Assert-Case "canonical-json-is-order-independent" { $a = [pscustomobject][ordered]@{ z = 1; a = [pscustomobject]@{ y = $true; x = "v" } }; $b = [pscustomobject][ordered]@{ a = [pscustomobject]@{ x = "v"; y = $true }; z = 1 }; (Get-Sha256Text (ConvertTo-CanonicalJson $a)) -eq (Get-Sha256Text (ConvertTo-CanonicalJson $b)) }
    if ($script:SelfTestFailures.Count -gt 0) { Write-Output ("M11 gate self-test: FAIL ({0} cases, {1} failures)" -f $script:SelfTestCount, $script:SelfTestFailures.Count); $script:SelfTestFailures | ForEach-Object { Write-Output " - $_" }; exit 1 }
    Write-Output ("M11 gate self-test: PASS ({0} cases)" -f $script:SelfTestCount)
}

switch ($Command) {
    "collect-identity" {
        $identity = Get-M11CandidateIdentity $DistributionDirectory $PortableDirectory $InstallerPath $ConfigProfilePath $M02EvidencePath $M04BaselinePath $M05RegressionPath $M09DecisionPath $M10ConformancePath $M08MetricsPath $M08TracePath $M04EvalBinary
        $validation = Test-M11CandidateIdentity $identity -AllowIncompleteEvidence:$AllowIncompleteEvidence
        Write-JsonResult ([pscustomobject]@{ identity = $identity; validation = $validation }) $OutputPath
    }
    "validate-identity" {
        $candidate = Read-JsonFile $CandidatePath; $validation = Test-M11CandidateIdentity $candidate
        $result = [pscustomobject]@{ pass = $validation.Pass; reasons = $validation.Reasons }
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("M11 candidate identity: " + $(if ($validation.Pass) { "PASS" } else { "FAIL" })); $validation.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $validation.Pass) { exit 1 }
    }
    "eligibility" {
        $result = Test-M11UpstreamEligibility (Read-JsonFile $UpstreamPath)
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("M11 upstream eligibility: " + $(if ($result.Pass) { "PASS" } else { "BLOCKED" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "generate-matrix" {
        $candidate = Read-JsonFile $CandidatePath
        $matrix = New-M11Matrix $candidate
        $validation = Test-M11Matrix $matrix $candidate
        if (-not $validation.Pass) { throw ("Generated M11 matrix is invalid: " + ($validation.Reasons -join "; ")) }
        Write-JsonResult $matrix $OutputPath
    }
    "validate-matrix" {
        $candidate = if (-not [string]::IsNullOrWhiteSpace($CandidatePath)) { Read-JsonFile $CandidatePath } else { $null }
        $result = Test-M11Matrix (Read-JsonFile $MatrixPath) $candidate
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("M11 matrix: " + $(if ($result.Pass) { "PASS" } else { "FAIL" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "equivalence" {
        $result = Get-M11EquivalenceDecision $EvidenceClass $EvidencePath (Read-JsonFile $CandidatePath)
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("M11 $EvidenceClass equivalence: " + $result.classification); $result.reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.pass) { exit 1 }
    }
    "validate-m09-decision" {
        $candidate = if (-not [string]::IsNullOrWhiteSpace($CandidatePath)) { Read-JsonFile $CandidatePath } else { $null }
        $result = Test-M09DecisionSet (Read-JsonFile $M09DecisionPath) $candidate
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("M09 decision set: " + $(if ($result.Pass) { "PASS" } else { "BLOCKED" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "audit-package" {
        $candidate = Read-JsonFile $CandidatePath
        $base = Invoke-M06PackageAudit $CandidatePath $PortableDirectory $InstallerPath -RequireInstallerPayload:$RequireInstaller
        $contamination = Test-M11PackageContamination $PortableDirectory
        $reasons = @($base.Reasons) + @($contamination)
        $result = [pscustomobject]@{ Pass = ($base.Pass -and $contamination.Count -eq 0); Reasons = $reasons; BaseAudit = $base }
        if ($Json) { Write-JsonResult $result $OutputPath } else { Write-Output ("M11 package audit: " + $(if ($result.Pass) { "PASS" } else { "FAIL" })); $result.Reasons | ForEach-Object { Write-Output " - $_" } }
        if (-not $result.Pass) { exit 1 }
    }
    "release-notes" {
        $candidate = Read-JsonFile $CandidatePath; $matrix = Read-JsonFile $MatrixPath
        $gate = [pscustomobject]@{ final_verdict = "BLOCKED"; f05_status = "NOT_EXECUTED"; f12_status = "NOT_EXECUTED"; f13_status = "NOT_EXECUTED" }
        $notes = New-M11ReleaseNotes $candidate $matrix $gate; $issues = Test-M11ReleaseNotesText $notes
        if ([string]::IsNullOrWhiteSpace($OutputPath)) { Write-Output $notes } else { [IO.File]::WriteAllText($OutputPath, $notes, [Text.UTF8Encoding]::new($false)); Write-Output "Wrote $OutputPath" }
        if ($issues.Count -gt 0) { $issues | ForEach-Object { Write-Output " - $_" }; exit 1 }
    }
    "gate" {
        $candidate = if (Test-Path -LiteralPath $CandidatePath -PathType Leaf) { Read-JsonFile $CandidatePath } else { $null }
        $upstream = if (Test-Path -LiteralPath $UpstreamPath -PathType Leaf) { Read-JsonFile $UpstreamPath } else { $null }
        $matrix = if (Test-Path -LiteralPath $MatrixPath -PathType Leaf) { Read-JsonFile $MatrixPath } else { $null }
        $m09 = if (Test-Path -LiteralPath $M09DecisionPath -PathType Leaf) { Test-M09DecisionSet (Read-JsonFile $M09DecisionPath) $candidate } else { $null }
        $f05Path = if (-not [string]::IsNullOrWhiteSpace($F05EvidencePath)) { $F05EvidencePath } else { $EvidencePath }
        $f12Path = if (-not [string]::IsNullOrWhiteSpace($F12EvidencePath)) { $F12EvidencePath } else { $EvidencePath }
        $f13Path = if (-not [string]::IsNullOrWhiteSpace($F13EvidencePath)) { $F13EvidencePath } else { $EvidencePath }
        $f05 = if ($null -ne $candidate) { Get-M11EquivalenceDecision "F05" $f05Path $candidate } else { $null }
        $f12 = if ($null -ne $candidate) { Get-M11EquivalenceDecision "F12" $f12Path $candidate } else { $null }
        $f13 = if ($null -ne $candidate) { Get-M11EquivalenceDecision "F13" $f13Path $candidate } else { $null }
        $package = $null
        if (-not [string]::IsNullOrWhiteSpace($PortableDirectory)) { $package = Invoke-M06PackageAudit $CandidatePath $PortableDirectory $InstallerPath -RequireInstallerPayload:$RequireInstaller }
        $gate = New-M11GateResult $candidate $upstream $matrix $m09 $f05 $f12 $f13 $package
        if ($Json -or -not [string]::IsNullOrWhiteSpace($OutputPath)) { Write-JsonResult $gate $OutputPath } else { Write-Output ("M11 Full gate: " + $gate.final_verdict); Write-Output ("Product verdict: " + $gate.product_verdict); $gate.reasons | ForEach-Object { Write-Output " - $_" } }
        if ($gate.final_verdict -ne "PASS") { exit 2 }
    }
    "self-test" { Invoke-SelfTest }
}
