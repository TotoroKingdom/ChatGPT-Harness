# M7 TASKS — Migration, Evidence & UX Hardening

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Baseline: M6 ACCEPTED / FROZEN  
> 每个 Task 必须包含实现 + Unit Test / code-level automated verification。  
> Integration / E2E / Manual Acceptance 不属于本文件。

## Dependency

```text
T1 AppConfig v2 / Migration Pipeline
        ↓
T2 Hardened Evidence Contracts / Sanitization
        ↓
T3 Runtime Instrumentation / Counters / Typed Reasons
        ↓
T4 Activity / Diagnostics / Recovery / Remediation UI
        ↓
T5 Historical Migration + Security Regression Matrix
        ↓
T6 Cross-cutting Regression / Development Completion
```

---

## T1 — AppConfig v2 / Migration Pipeline

**Primary files:** `protocol/src/v1/config.rs`, generated TS/schema/fixtures, `ui/src-tauri/src/config/*`, config commands/tests.

### Implement

#### AppConfig v2

设置：

```text
CURRENT_SCHEMA_VERSION = 2
```

新创建/保存的 config必须：

```text
schemaVersion = 2
```

保持当前 M6 AppConfig字段与 ownership：

- workspace。
- tunnel。
- security。
- ui。
- tools。
- policies。
- bash。

不得把 M5 Bash hard limits变成 user config。

#### Explicit migration dispatcher

ConfigStore load：

```text
read raw JSON
→ inspect schemaVersion
→ v1 migrate
→ v2 normal load
→ other unsupported
```

新增 dedicated legacy model / helper：

```text
LegacyAppConfigV1
migrate_v1_to_v2
```

不要用当前 AppConfig default-deserialization掩盖 migration source差异。

#### Pure V2 v1 migration

保留：

```text
configRevision
workspace
tunnelId
proxy
tunnel client config
autoDetect
apiKeyRef
security.mode
ui.theme
```

补 defaults：

```text
tools = M2 defaults
policies = M3 defaults
bash = M5 default config
ui.language = None
```

#### Transitional M2–M6 v1 migration

如果 source已有：

```text
tools
policies
bash
ui.language
```

完整保留其合法值，不套 defaults覆盖。

#### Security normalization

```text
security.mode = authoritative
security.developerTestMode = (mode == developer_test)
```

#### Revision

migration保持原 `configRevision`。

migration persist不能走普通 save的 revision increment path。

#### Validation

migrated config写盘前执行：

- proxy normalization/validation。
- tunnel client path normalization。
- policy validation。
- language enum validation。
- tools/bash typed validation。
- v2 schema validation assumptions。

malformed transitional field必须 fail，而不是回默认。

#### Errors

增加 machine codes：

```text
CONFIG_SCHEMA_UNSUPPORTED
CONFIG_MIGRATION_FAILED
```

保留 CONFIG_INVALID / CONFIG_WRITE_FAILED。

#### Atomicity

migration只在 full validation后 atomic replace。

failure：

- original exact bytes unchanged。
- no partial v2。
- temp cleanup。

#### Idempotency

v2再次 load：

- no migration。
- no rewrite。
- no revision change。

### Required Unit Tests

至少覆盖：

- new defaults schemaVersion 2。
- v2 load/save round trip。
- pure V2 v1 migration。
- transitional v1 migration preserves Tool OFF。
- preserves User Rules。
- preserves Bash override/autoDetect。
- preserves ui.language。
- preserves workspace/tunnel/proxy/client/apiKeyRef/security。
- security compatibility bool normalized from mode。
- migration preserves configRevision。
- missing V3 fields get correct defaults。
- malformed present V3 field fails instead of defaulting。
- invalid Policy fails migration。
- unsupported schema fails typed。
- missing/invalid schema fails typed。
- migration write failure leaves original bytes unchanged。
- successful migration second load no rewrite。
- API Key raw secret never materialized。
- UI-only language save after migration still preserves revision。

### Done

- current production config format is v2。
- both real v1 history classes supported。
- migration does not silently lose user config。

---

## T2 — Hardened Evidence Contracts / Sanitization

**Primary files:** `protocol/src/v1/activity.rs`, `control_read.rs`, runtime audit/activity/request/tool result models, generated TS/schema, tests.

### Implement

#### Shared evidence DTOs

Additive typed contracts for：

```text
SanitizedArgumentsEvidence
ExecutionResultEvidence
```

Harden ActivityEntry with：

```text
toolEnabled
capability?
sanitizedArguments?
approvalId?
approvalDecision?
durationMs?
resultEvidence?
```

Keep：

```text
requestId
toolName
policyDecision
matchedRuleIds
policyReason
outcome
failureCategory
bash
```

#### Event enum alignment

Shared protocol/control-read/Runtime/TS event variants must align，including：

- ApprovalResolved。
- ApprovalExpired。
- ApprovalCancelled。
- SessionGrantMatched。

Remove stale duplicated hand-maintained UI type definitions where generated/shared type can be authoritative。

#### Sanitized arguments producers

Typed safe facts：

```text
read_file → logical path
glob      → bounded pattern
write     → logical path
edit      → logical path
bash      → sanitized command preview
```

Forbidden：

- write content。
- edit old/new text。
- raw file contents。
- secret-bearing env。
- unbounded output。

#### Result evidence

Success scalar projection：

```text
read_file → bytesRead
glob      → matchCount
write     → bytesWritten + created/overwritten
edit      → replacement count + bytesWritten
bash      → exitCode
```

No result payload bodies。

#### Generic Approval correlation

Move approval ID out of Bash-only evidence into shared request evidence while preserving Bash-specific display facts。

ASK lifecycle events share：

```text
requestId
approvalId
```

#### Duration model

RequestContext records monotonic start instant。

Terminal evidence gets：

```text
durationMs
```

Bash process-specific duration may remain separately available。

### Required Tests

至少覆盖：

- read path evidence。
- glob pattern evidence bound。
- write path evidence, content sentinel absent。
- edit path evidence, old/new sentinel absent。
- Bash preview remains bounded/sanitized。
- result scalar per built-in tool。
- approvalId on file ASK。
- approvalId on Bash ASK。
- approval resolution/cancel/expire correlation。
- session grant event correlation。
- terminal duration exists and nonnegative。
- nonterminal events do not invent terminal duration/result。
- malformed/unsafe evidence rejected or redacted。
- generated TS matches Rust enum variants。

### Done

- Activity/Audit has enough safe typed facts to answer the V3 observability questions。
- raw payload logging is not introduced。

---

## T3 — Runtime Instrumentation / Counters / Typed Reasons

**Primary files:** `runtime/src/execution/*`, `state.rs`, `diagnostics.rs`, `tools/effective.rs`, `approval.rs`, protocol runtime/control-read DTOs, tests.

### Implement

#### Real counters

Add Runtime-owned counters：

```text
activeRequests
runningExecutions
```

Use ApprovalManager for：

```text
pendingApprovals
```

Semantics：

- activeRequests includes validation/policy/approval wait/execution until terminal。
- runningExecutions starts only when executor begins。
- pendingApprovals only current Pending ApprovalManager entries。

Use RAII/atomic guards so every exit path decrements exactly once。

#### Counter wiring

Update：

- RuntimeState.status()。
- control diagnostics。
- shutdown/rollback tests。

Remove placeholder constant zero / NotInstrumented where current architecture now has real data。

#### Typed ToolStatusReason

Add shared enum such as：

```text
available
approval_required
tool_disabled
developer_mode_required
runtime_not_ready
capability_unavailable
bash_executable_unavailable
policy_denied
invalid_policy_snapshot
invalid_policy_context
unsupported_capability
metadata_invalid
```

EffectiveTool / ToolDiagnostic must expose machine reason。

Optional freeform technical detail may remain bounded, but UI logic must not parse it。

#### Policy reason mapping

Ensure invalid policy snapshot/context distinguishes ordinary explicit DENY。

#### Runtime last error / diagnostics

Continue safe machine code only；do not expose arbitrary stack/message as authoritative state。

### Required Tests

至少覆盖：

- active counter during validation/policy。
- active counter during approval wait。
- running counter only during executor。
- pending approval count real。
- concurrent requests counts。
- success cleanup to zero。
- Policy DENY cleanup。
- validation failure cleanup。
- Approval Deny/Expire/Cancel cleanup。
- Bash timeout/cancel cleanup。
- Runtime stop cleanup。
- no underflow/double decrement。
- each EffectiveTool availability has deterministic ToolStatusReason。
- invalid policy gets invalid-policy reason, not generic allowed/denied prose。
- Bash unavailable gets typed reason plus safe detail。

### Done

- Runtime status/diagnostics no longer expose skeleton request counts。
- Tool blocked states have typed machine reasons suitable for UX remediation。

---

## T4 — Activity / Diagnostics / Recovery / Remediation UI

**Primary files:** UI services/adapters/providers, Activity/Overview/Tools/Policies/Settings pages, status components, i18n resources, Tauri config recovery commands as needed, tests.

### Implement

#### Activity detail

Upgrade Activity page from summary-only table to summary + diagnostic detail/expand view。

Display Runtime-provided：

- request ID。
- tool。
- sanitized args。
- tool enabled/capability。
- policy decision/reason/matched IDs。
- approval ID/decision。
- outcome/failure。
- result evidence。
- duration。
- Bash details。

Do not infer missing values。

#### State distinction

Tools / Policies use explicit categories：

```text
Available
Ask / Approval required
Disabled
Denied
Unavailable
```

Execution failure remains Activity outcome，不混入 Tool availability。

#### Remediation

Map typed ToolStatusReason / config error code to localized next action。

Minimum：

- enable + restart。
- switch Developer/Test + restart。
- configure Git Bash + restart。
- edit matching Policy + restart。
- repair invalid policy。
- use Approval Center for ASK。
- start/restart Runtime。
- inspect Activity failure。

#### Migration recovery

Config migration failure page/banner：

- localized typed error。
- original config unchanged explanation。
- config file path。
- Retry。
- copy/inspect path action。
- Reset only explicit confirmed destructive action。

Do not auto-reset。

Ensure config metadata can load independently from config content failure。

#### Shared revision UX

Create shared revision/restart presentation used by relevant pages。

Show or make diagnosable：

```text
saved revision
active revision
Restart Required
```

Do not create mismatch while Runtime stopped。

#### Policies invalid snapshot UX

Show fail-closed status and remediation；do not offer “ignore invalid rule”。

#### Diagnostics

Overview/Diagnostics show real：

- activeRequests。
- pendingApprovals。
- runningExecutions。
- runtime readiness。
- active/saved revision。
- last safe code。

Tunnel remains Tauri-owned observation。

#### i18n

Add en/zh-CN keys for all M7 status/recovery/evidence/remediation copy。

### Required UI Tests

至少覆盖：

- Activity detail happy path。
- missing evidence shows not recorded/not applicable。
- raw IDs remain unchanged。
- Disabled/Denied/Ask/Unavailable distinct labels/badges。
- ASK remediation points to approval, not config error。
- disabled remediation。
- developer mode remediation。
- Bash executable remediation。
- invalid Policy remediation。
- migration failed recovery actions。
- unsupported schema UX。
- metadata path still visible when config load fails。
- reset requires confirmation。
- saved/active revision mismatch。
- stopped Runtime revision presentation。
- language-only save does not introduce restart warning。
- en/zh-CN parity for new keys。

### Done

- user can understand blocked/failing state without reading raw logs。
- UI remains an observer/controller, not authorization authority。

---

## T5 — Historical Migration + Security Regression Matrix

**Primary files:** source-controlled fixtures, ConfigStore migration tests, Runtime evidence security tests, frontend contract tests.

### Implement

Add immutable historical fixtures：

```text
v1-v2-era.json
v1-transitional-m6.json
v1-invalid-policy.json
v1-invalid-proxy.json
v1-unsupported-field.json
v2-current.json
```

Fixtures use fake IDs/paths/references only。

### Migration regression

For every fixture assert：

- source bytes before。
- expected migration/error。
- output schema。
- preserved fields。
- revision。
- second load behavior。

### Secret/content sentinel tests

Inject sentinel values into：

- API secret test double / unsupported raw field。
- write content。
- edit old/new text。
- Bash environment/output。

Assert sentinel absent from：

- migrated config except allowed opaque reference identifier。
- Audit serialized evidence。
- Activity snapshot。
- diagnostics。
- UI rendered evidence fixtures where applicable。

### Evidence lifecycle matrix

At least：

```text
ALLOW success
DENY
ASK → AllowOnce
ASK → Deny
ASK → Expire/Cancel
SessionGrant match
Tool disabled
Workspace violation
Execution failure
Bash timeout/output/cancel
```

Each row asserts correlation fields and final counters。

### Done

- upgrade path is protected by realistic historical fixtures。
- evidence sanitization has adversarial regression coverage。

---

## T6 — Cross-cutting Regression / Development Completion

**Primary scope:** full repository automated regression；no new product feature。

### Config regression

- schema2 new install。
- v1 migration。
- ConfigStore atomic save。
- UI language semantics。
- API Key reference/SecretStore boundary。
- restart revision semantics。

### M6 regression

- bilingual UI/resources。
- instant language switch。
- no Runtime restart for UI-only preference。

### M5 regression

- Bash mode/policy/approval/limits/process-tree behavior unchanged。

### M4 regression

- same-request Approval。
- session grants。
- expiry/cancel。

### M3 regression

- Policy precedence/fallback/fail closed。

### M2 regression

- Tool disabled/stale calls。

### M1/M0 regression

- atomic write/edit。
- read/glob。
- Workspace boundaries。

### Evidence regression

Confirm hardening does not affect authorization result or executor side effects。

### Development Commands

Run：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm test
npm run typecheck
npm run build
git diff --check
```

All PASS。

### Done

T1–T6 completion only means：

```text
M7 Development Complete
```

Then separately generate/execute：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Do not mark M7 or V3 ACCEPTED/FROZEN from TASK completion alone。

---

## Development Completion Checklist

- [ ] T1 AppConfig v2 / Migration Pipeline PASS
- [ ] T2 Hardened Evidence Contracts / Sanitization PASS
- [ ] T3 Runtime Instrumentation / Counters / Typed Reasons PASS
- [ ] T4 Activity / Diagnostics / Recovery / Remediation UI PASS
- [ ] T5 Historical Migration + Security Regression Matrix PASS
- [ ] T6 Cross-cutting Regression PASS
- [ ] CURRENT_SCHEMA_VERSION = 2
- [ ] pure V2 v1 migration preserves core config
- [ ] transitional v1 migration preserves M2–M6 values
- [ ] migration preserves configRevision
- [ ] migration failure leaves original bytes unchanged
- [ ] unsupported schema fails closed
- [ ] SecretStore raw secret never materialized
- [ ] hardened sanitized arguments evidence
- [ ] generic approval ID correlation
- [ ] request duration/result evidence
- [ ] no write/edit content leak
- [ ] activeRequests real
- [ ] pendingApprovals real
- [ ] runningExecutions real
- [ ] typed ToolStatusReason
- [ ] Disabled / Denied / Ask / Unavailable distinct UX
- [ ] actionable remediation
- [ ] migration recovery UX
- [ ] desired/effective/revision truthfulness
- [ ] en/zh-CN M7 resource parity
- [ ] M0–M6 regression PASS
- [ ] cargo fmt/check/test PASS
- [ ] frontend test/typecheck/build PASS
- [ ] git diff --check PASS
- [ ] TASKS未声称 Integration/E2E/Manual Acceptance/V3 final PASS
