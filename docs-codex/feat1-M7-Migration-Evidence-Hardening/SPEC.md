# M7 SPEC — Migration, Evidence & UX Hardening

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M7 — Migration, Evidence & UX Hardening  
> Baseline: M6 — Chinese / English UI, ACCEPTED / FROZEN  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, M0–M6 milestone docs

## 1. Objective

M7 是 M8 Full External Acceptance 前最后一个实现里程碑。

目标分为三条互相关联的收口链：

```text
A. Config Migration
schemaVersion v1
→ explicit v1 → v2 migration
→ validated AppConfig v2
→ atomic persist

B. Evidence Hardening
Runtime request
→ capability/workspace/policy/approval/execution
→ typed + sanitized evidence
→ Activity / Audit / diagnostics

C. UX Hardening
Runtime machine state / reason
→ typed presentation reason
→ clear state distinction
→ actionable remediation
→ desired/effective truthfulness
```

M7 不新增新的工具能力；它让 M0–M6 已实现的 V3 能力能够安全升级、可靠诊断、长期使用，并为 M8 最终 External ChatGPT 验收提供可信 evidence。

## 2. Baseline Facts

当前代码已经具备：

- M6 `ACCEPTED / FROZEN`。
- 产品版本已具备完整 M0–M6 V3 功能。
- `AppConfig.schemaVersion` 当前仍为 `1`。
- M2–M6 为避免提前执行 M7 migration，已经把 `tools / policies / bash / ui.language` 以 backward-compatible additive 字段加入 schema v1。
- 因此现实中存在两类合法 v1 source：
  1. V2-era v1：只有 Workspace/Tunnel/Security/UI 等核心字段。
  2. Transitional V3 v1：已经保存 Tool/Policy/Bash/Language 等用户配置。
- ConfigStore 当前对非当前 schema fail closed，但没有 migration pipeline。
- ConfigStore 已有 atomic replace primitive。
- SecretStore secret只通过 opaque `apiKeyRef`出现在 config。
- `RuntimeConfigSnapshot` 已经不包含 Tunnel secret/UI language。
- Runtime已有 structured AuditEvent / Activity projection。
- 当前 evidence 已包含 request/tool/policy/matched rules/outcome/failure，Bash evidence额外包含 command/approval/duration。
- file-tool Activity目前没有统一的 sanitized arguments、approval ID、general request duration和 tool-specific result evidence。
- RuntimeState `pendingApprovals`已有真实来源，但 `activeRequests` / `runningExecutions`仍为 skeleton `0`。
- EffectiveTool目前有明确 availability/decision，但 reason/readiness reason仍部分是自由英文字符串。
- M6 已提供 machine-code → localized presentation基础。
- restartRequired当前由 active configRevision vs saved configRevision决定。

M7应收口这些 gaps，而不是推翻当前 Runtime/Tauri/MCP架构。

## 3. Scope

M7 实现：

### Config Migration

1. AppConfig正式升级到 `schemaVersion = 2`。
2. explicit raw-JSON migration pipeline。
3. V2-era v1 → v2。
4. Transitional V3 v1 → v2。
5. migration atomicity / idempotency。
6. preserve existing settings / API Key reference。
7. typed migration failure / unsupported-schema errors。
8. migration recovery UX。

### Evidence Hardening

9. canonical typed Runtime execution evidence。
10. sanitized arguments evidence for all built-in tools。
11. tool enabled/capability evidence。
12. policy decision / matched IDs / reason。
13. approval ID / decision。
14. terminal execution outcome / failure category。
15. general request duration。
16. safe tool-specific scalar result evidence。
17. real active/running/pending counters。
18. Activity detail UX。
19. evidence sanitization/size bounds。

### UX Hardening

20. typed effective tool reason codes。
21. Disabled / Denied / Ask / Unavailable明确区分。
22. actionable remediation mapping。
23. invalid Policy fail-closed UX。
24. config migration error recovery UX。
25. shared desired/effective/restart presentation。
26. diagnostics/runtime counters truthfulness。
27. English / zh-CN hardening strings。
28. M0–M6 regression。

M7 不实现：

- M8 External ChatGPT final acceptance。
- new tool。
- PowerShell/CMD。
- OS sandbox。
- durable audit database。
- cross-restart Activity persistence。
- remote audit backend。
- new protocol major version。
- policy simulation engine。
- multi-workspace。
- automatic config rollback to older schema。
- schema v2 → v1 downgrade。
- arbitrary translation of Runtime data。

## 4. Product Version vs Config Schema Version

必须明确区分：

```text
Product upgrade:
ChatGPT-Harness V2 → V3

Persistent config upgrade:
AppConfig schemaVersion 1 → 2

Wire protocol:
protocol/src/v1 remains v1
```

M7升级 AppConfig schema，不创建 `protocol/v2`。

`PROTOCOL_VERSION` 只有发生真正 wire-incompatible change时才允许调整；本 M7按 additive evolution设计，不要求 protocol major bump。

## 5. AppConfig v2 Contract

M7正式设置：

```text
CURRENT_SCHEMA_VERSION = 2
```

AppConfig v2的 serialized ownership保持 M6最终模型：

```text
AppConfig v2
├ schemaVersion = 2
├ configRevision
├ workspace
├ tunnel
│  ├ tunnelId
│  ├ controlPlaneHttpProxy?
│  ├ tunnelClientPath?
│  ├ autoDetect
│  └ apiKeyRef?
├ security
│  ├ mode
│  └ developerTestMode   # compatibility mirror
├ ui
│  ├ theme?
│  └ language?
├ tools
├ policies
└ bash
```

### 5.1 M5 Bash limits remain Runtime-owned

M5 已冻结 Bash hard limits为 Runtime constants。

因此 M7 **不得**因为早期 Architecture概念图曾出现 `bash.limits` 而把用户可放宽的 Bash limits重新加入 AppConfig v2。

这是 M5安全边界的延续。

### 5.2 Runtime snapshot remains host-data-free

`RuntimeConfigSnapshot`继续仅携带 Runtime需要的 non-secret fields：

```text
configRevision
workspace
security
tools
policies
bash
runtimeLimits
```

不得携带：

```text
Tunnel ID
apiKeyRef / secret
UI language/theme
```

## 6. Supported Migration Sources

M7的 v1 migrator必须同时支持两类真实历史来源。

### 6.1 V2-era v1

历史结构至少包括：

```text
schemaVersion
configRevision
workspace
tunnel
security
ui(theme?)
```

缺少 V3新增字段。

### 6.2 Transitional V3 v1

M2–M6保存的 v1可能已经包含：

```text
tools
policies
bash
ui.language
```

**如果这些字段已经存在且合法，migration必须原样保留其用户语义。**

不得把例如：

```text
write.enabled = false
user policy DENY
bash manual path
language = zh-CN
```

重置成 M7 defaults。

### 6.3 Unsupported source

```text
schemaVersion > 2
schemaVersion = 0 / invalid
missing schemaVersion
invalid type
```

不得猜测 migration。

返回 typed config error并保持原文件 untouched。

## 7. Migration Pipeline

ConfigStore load正式变为：

```text
read config.json bytes
      ↓
parse raw JSON Value
      ↓
inspect schemaVersion
      ├ v2 → deserialize AppConfig v2
      ├ v1 → parse LegacyV1Config
      │       ↓
      │     migrate_v1_to_v2 in memory
      │       ↓
      │     deserialize/construct AppConfig v2
      └ other → unsupported schema
      ↓
normalize
      ↓
validate full v2 config
      ↓
if migrated:
  serialize
  atomic persist
      ↓
return typed AppConfig v2
```

Migration不得通过：

```text
load old typed AppConfig
→ mutate file field-by-field in place
```

实现应有独立、可测试的：

```text
LegacyAppConfigV1
migrate_v1_to_v2(...)
validate_v2(...)
```

## 8. Migration Defaults

仅当对应 v1字段**不存在**时填充 V3 defaults。

### Tools

```text
read_file = ON
glob      = ON
write     = ON
edit      = ON
bash      = OFF
```

### Tool Defaults

```text
read_file = ALLOW
glob      = ALLOW
write     = ALLOW
edit      = ALLOW
bash      = ASK
```

### User Rules

```text
[]
```

### Bash

```text
autoDetect = true
executablePath = null
```

### Language

```text
UiConfig.language = null
→ Follow system
```

已有合法 transitional field时不套用 default。

## 9. Preservation Contract

v1 → v2必须保留：

```text
workspace.rootPath
tunnel.tunnelId
tunnel.controlPlaneHttpProxy
tunnel.tunnelClientPath
tunnel.autoDetect
tunnel.apiKeyRef
security.mode
ui.theme
configRevision
```

Transitional v1还必须保留：

```text
ui.language
tools
policies
bash
```

### 9.1 Security compatibility normalization

`security.mode`继续是 authoritative source。

迁移后：

```text
developerTestMode = (mode == developer_test)
```

如果旧 compatibility boolean与 `mode`不一致：

- preserve authoritative `mode`。
- normalize boolean mirror。
- 不允许 boolean反向改变 SecurityMode。

### 9.2 configRevision

Schema migration本身不是用户改变 desired Runtime semantics。

因此：

> **v1 → v2 migration preserves the existing configRevision.**

不要仅因为 `schemaVersion 1 → 2` 自动 `revision + 1`。

对于旧 V2 config，首次启动 V3 Runtime直接使用迁移后的同一 revision snapshot。

后续真正 Runtime-affecting config mutation继续按现有 revision semantics增长。

UI-only language/theme仍不推进 Runtime configRevision。

## 10. Migration Strictness / No Silent Data Loss

Migration不能静默丢弃 migration-owned JSON数据。

### Required behavior

- malformed known field → fail migration。
- partially malformed `tools/policies/bash` → fail migration，不退回 defaults。
- invalid Policy → fail migration，不跳过坏 rule。
- unsupported language value → fail migration，不猜测。
- invalid proxy → fail migration。
- unknown future schemaVersion → fail unsupported。

对于 v1 migration-owned objects中的未知字段，优先 fail with diagnostic而不是 silent drop。

原文件必须保持不变。

## 11. Secret Safety

Migration只复制 opaque：

```text
apiKeyRef
```

不得：

- 读取 SecretStore raw API key再写回 config。
- 将 API key放进 migration error/evidence。
- 自动把任何 legacy plaintext-looking field解释成可迁移 secret。

如果 source config出现不受支持的 raw-secret field：

```text
migration fail closed
→ original file unchanged
```

用户无需重新输入已有有效 SecretStore credential。

## 12. Atomicity / Idempotency

### Migration success

```text
old v1 file remains authoritative
until
v2 fully constructed + validated + serialized
then
atomic replace
```

### Migration failure

```text
no replacement
no partial v2
no temp-file residue after cleanup
```

### Second launch

迁移后的 v2：

```text
load directly
→ no second migration
→ no revision change
→ no unnecessary rewrite
```

Migration必须 idempotent at application level。

## 13. Migration Error Contract

扩展 Config/Tauri machine error分类，至少能够区分：

```text
CONFIG_INVALID
CONFIG_WRITE_FAILED
CONFIG_SCHEMA_UNSUPPORTED
CONFIG_MIGRATION_FAILED
```

建议内部 migration diagnostics包含安全字段：

```text
sourceSchemaVersion?
targetSchemaVersion = 2
stage
reasonCode
```

不得把完整 config JSON、secret reference内容或任意 raw value拼进 user-facing error。

M6 presentation layer为新 codes提供 en / zh-CN localized explanation。

## 14. Migration Recovery UX

如果 migration失败：

- 原 config仍在原位置。
- App可以进入 recovery UI，而不是 crash/blank。
- 明确说明“原配置未被覆盖”。
- 显示 config file path（这是本地用户自己的路径）。
- 提供可执行 remediation：
  - Retry reload/migration。
  - Inspect/repair config（至少可复制路径；如已有安全 open-folder能力可复用）。
  - Reset local config作为显式 destructive last resort。
- Reset继续要求确认。

不得自动 reset。

`configMetadata`获取不应因为 `configGet()` migration失败而一起丢失；recovery UI仍应能够显示配置位置。

## 15. Evidence Goal

每个有意义的 tool request最终应能够回答：

```text
What was requested?
Was the tool eligible/enabled?
How did workspace validation resolve?
Why was it ALLOW / ASK / DENY?
Which rules matched?
Was approval involved?
Was execution started?
What was the result?
Why did it fail if it failed?
How long did the request take?
```

UI不得重新推断这些事实。

## 16. Canonical Runtime Evidence

Runtime仍是 evidence owner。

M7 hardened request evidence至少包含：

```text
requestId
eventId
timestamp
tool
toolEnabled
capability/workspace result
sanitizedArguments
policyDecision?
matchedRuleIds[]
policyReason?
approvalId?
approvalDecision?
outcome
failureCategory?
durationMs?
resultEvidence?
```

Bash保留现有：

```text
classification
riskFlags
workingDirectory
commandPreview
sessionGrantHit
exitCode
limit/cancel evidence
```

## 17. Sanitized Arguments Contract

不得把原始 Tool request直接复制进 Activity/Audit。

新增 typed/bounded sanitized projection。

推荐：

```text
SanitizedArgumentsEvidence
├ read_file { logicalPath }
├ glob      { pattern }
├ write     { logicalPath }
├ edit      { logicalPath }
└ bash      { commandPreview }
```

### File tools

允许记录：

- normalized workspace-logical relative path。

禁止记录：

- raw file content。
- `write.content`。
- `edit.oldText/newText`。
- physical canonical Workspace root unless needed for explicit diagnostics。

### glob

允许记录 bounded validated pattern。

### Bash

复用 analyzer-sanitized bounded command preview。

禁止记录：

- full environment。
- private normalized grant matcher if it contains hidden details。
- unbounded stdout/stderr。

## 18. Tool Enabled / Capability Evidence

PRD要求 evidence能说明 tool enabled state。

每个 request在 authorization时记录 Runtime generation观察到的：

```text
toolEnabled
capability
mode eligibility where relevant
```

这必须来自 current Runtime ToolRegistry/CapabilityGate，不从 UI desired config推断。

Disabled stale call应明确产生：

```text
toolEnabled = false
failureCategory = tool_disabled
no execution
```

## 19. Policy / Approval Evidence

Policy evidence继续由 PolicyEngine提供：

```text
policyDecision
matchedRuleIds
policyReason
```

Approval evidence必须适用于所有 ASK tools，而不是只在 Bash-specific evidence里存在：

```text
approvalId
approvalDecision?
```

生命周期：

```text
ASK
→ approvalId present
→ pending/resolved/denied/expired/cancelled events share requestId + approvalId
```

SessionGrant hit也必须明确 evidence，不伪造成 ordinary Policy ALLOW。

## 20. Result / Duration Evidence

### 20.1 General duration

新增 request-level monotonic duration：

```text
request accepted
→ terminal state
→ durationMs
```

terminal events至少包含 duration。

使用 monotonic clock计算，不用 wall-clock timestamp相减。

Bash process duration可继续作为 Bash-specific scalar，但不得替代 general request duration。

### 20.2 Safe result evidence

M7增加 bounded scalar result evidence，建议：

```text
ExecutionResultEvidence
├ read_file { bytesRead }
├ glob      { matchCount }
├ write     { bytesWritten, disposition: created|overwritten }
├ edit      { replacements, bytesWritten }
└ bash      { exitCode }
```

失败路径使用：

```text
outcome + failureCategory
```

不得把 content/output本体放入 result evidence。

## 21. Activity / Audit Storage Boundary

保持 V3 non-goal：

- 不引入数据库。
- 不承诺 durable cross-restart audit。

当前 Runtime/session bounded Activity model继续使用。

M7只 harden：

- schema/content。
- correlation。
- sanitization。
- diagnostics。
- UI presentation。

不把 M7变成日志平台项目。

## 22. Activity Protocol / Projection

优先 additive扩展现有：

```text
activity.*
control.activity.snapshot
```

不创建第二套同义 evidence API。

必须收敛当前 Runtime与control-read projection的 enum/field drift，例如 ApprovalResolved / Expired / Cancelled / SessionGrantMatched 应能通过 shared typed contract完整表示。

Generated TypeScript应成为 UI typed source；避免 UI长期维护与 Rust不一致的手写 enum副本。

## 23. Runtime Counters

Architecture要求 RuntimeState真实连接：

```text
activeRequests
pendingApprovals
runningExecutions
```

M7冻结语义：

### activeRequests

已经进入 Runtime ExecutionPipeline、尚未 terminal 的 request数，包括：

- validation/policy。
- awaiting approval。
- executing。

### pendingApprovals

ApprovalManager当前 Pending count。

### runningExecutions

已经进入 executor/process execution且尚未结束的 request数。

不包含 awaiting approval。

### Counter safety

- all success/failure/cancel paths必须 decrement。
- use RAII/guard or equivalent exception-safe pattern。
- Runtime stop最终收敛为 0。
- no negative/underflow。

Control diagnostics应显示这些真实 counters，而不是 `NotInstrumented` / constant zero。

## 24. Effective Tool Reason Contract

M7将 EffectiveTool中的用户决策原因从自由 prose收敛到 typed reason code。

建议 additive：

```text
ToolStatusReason
├ available
├ approval_required
├ tool_disabled
├ developer_mode_required
├ runtime_not_ready
├ capability_unavailable
├ bash_executable_unavailable
├ policy_denied
├ invalid_policy_snapshot
├ invalid_policy_context
├ unsupported_capability
└ metadata_invalid
```

可保留 bounded fallback detail用于 diagnostics，但 UI的主状态/remediation必须使用 machine reason，而不是匹配英文句子。

## 25. UX State Distinction

UI必须明确区分：

### Disabled

```text
availability = disabled
```

含义：用户desired/effective tool开关导致不可执行。

### Denied

```text
availability = unavailable_by_policy
policyDecision = DENY
```

含义：Policy阻止执行。

### Ask

```text
availability = available_with_approval
policyDecision = ASK
```

含义：工具可调用，但需要人类审批；不是“不可用”。

### Unavailable

例如：

```text
unavailable_by_mode
unavailable_by_runtime
```

含义：capability/readiness问题，不等同 Policy DENY。

### Execution failure

Activity显示 request曾被允许/批准并开始执行，但 executor失败/timeout/limit。

不得把这五类状态使用同一个 generic error badge。

## 26. Remediation Contract

UI根据 typed reason提供明确下一步。

最低 mapping：

| State/reason | Remediation |
| --- | --- |
| tool disabled | Enable tool; restart Harness if current generation differs |
| Developer mode required | Switch to Developer/Test Mode and restart |
| Bash executable unavailable | Configure/auto-detect valid Git Bash and restart |
| Policy DENY by user rule | Review/edit matching User Policy, save, restart |
| invalid policy snapshot | Fix invalid Policy config; no execution until valid |
| ASK | Invoke tool and resolve pending Approval; no config fix required |
| Runtime not ready | Start/restart Harness; inspect Diagnostics if repeated |
| config migration failed | Repair original config or explicitly reset; original remains intact |
| execution failure | Inspect Activity failure category/evidence; retry only after cause is fixed |

Remediation不等于authorization；UI建议不能改变 Runtime decision。

## 27. Desired vs Effective Hardening

继续以：

```text
saved AppConfig.configRevision
vs
current RuntimeState.configRevision
```

作为唯一 Restart Required identity。

M7要求 shared UI presentation在所有相关页面保持一致：

- Tools。
- Policies。
- Settings/Security/Bash configuration。
- Overview/Diagnostics。

运行中保存 Runtime-affecting配置：

```text
Desired updated
Effective unchanged
Restart Required visible
```

语言等 UI-only config不制造 restart debt。

UI不得从 desired config推断 effective状态。

## 28. Invalid Policy UX

Policy compile/validation失败继续 fail closed。

UI必须：

- 显示 Policy invalid / DENY状态。
- 不把其它 valid ALLOW rule展示为当前可执行依据。
- 显示可定位的 safe rule ID / validation category。
- 给出“修复并重启”remediation。
- 不提供“ignore invalid rule and run”按钮。

## 29. Activity UX Hardening

Activity页面从简单表格升级为可诊断 surface。

列表仍可简洁显示：

```text
time
tool
event/outcome
policy
```

每条 request/event应可展开或查看 detail：

- request ID。
- sanitized arguments。
- enabled/capability evidence。
- Policy decision / matched IDs / reason。
- Approval ID/decision。
- result evidence。
- failure category。
- duration。
- Bash evidence when applicable。

Raw machine identifiers保持可复制。

Activity UI不得自行补造缺失 evidence。

## 30. Diagnostics UX Hardening

Overview/Diagnostics至少能够显示真实：

```text
Runtime lifecycle/readiness
active config revision
saved config revision
Restart Required
activeRequests
pendingApprovals
runningExecutions
last safe machine error
```

Tunnel状态继续来自 Tauri/Harness ownership，不要求 Runtime伪造它无法观察的 Tunnel facts。

## 31. i18n Boundary

M7新增的：

- migration errors。
- tool reason labels。
- remediation。
- Activity detail labels。
- counter/diagnostic labels。

必须加入：

```text
en
zh-CN
```

resources，并保持 M6 key/placeholder parity tests。

machine codes、tool names、rule IDs、request IDs、paths/commands保持 raw。

## 32. Protocol Evolution

保持：

```text
protocol/src/v1
```

M7优先 additive：

- new config schema value。
- new typed evidence DTO fields。
- new reason enums。
- new result evidence DTO。
- diagnostics fields。

不得因为 AppConfig schema升到 2就创建 wire protocol v2。

已有客户端/Runtime同一 release一起升级；如果实现发现必须做真正 wire-incompatible change，应停止并修改 Architecture，而不是 silent break。

## 33. Expected Code Touchpoints

```text
protocol/src/v1/config.rs
protocol/src/v1/activity.rs
protocol/src/v1/control_read.rs
protocol/src/v1/runtime.rs
protocol generated TS / JSON schema / fixtures

ui/src-tauri/src/config/mod.rs
ui/src-tauri/src/commands/*
ui/src-tauri/src/harness/* or runtime snapshot construction as needed

runtime/src/execution/context.rs
runtime/src/execution/pipeline.rs
runtime/src/audit/*
runtime/src/activity/*
runtime/src/state.rs
runtime/src/diagnostics.rs
runtime/src/tools/effective.rs
runtime/src/tools/* result/evidence helpers
runtime/src/approval.rs as needed for shared approval evidence

ui/src/app/providers/*
ui/src/services/harness/control-read.ts
ui/src/i18n/*
ui/src/components/status/*
ui/src/pages/overview/*
ui/src/pages/tools/*
ui/src/pages/policies/*
ui/src/pages/activity/*
ui/src/pages/settings/*
related tests
```

MCP tool execution semantics themselves不应因 evidence/UX hardening改变。

## 34. Development Verification Requirements

每个 Task必须包含 unit/code-level tests。

开发完成至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm test
npm run typecheck
npm run build
git diff --check
```

必须自动化覆盖：

```text
pure V2 schema v1 → v2
transitional M6 v1 → v2 preserving V3 fields
migration preserves configRevision
migration preserves apiKeyRef without SecretStore raw secret
migration failure leaves original bytes unchanged
unsupported schema leaves file unchanged
successful migration atomic + idempotent
v2 normal load
v2 save semantics
UI-only config still no restart revision

file-tool sanitized arguments evidence
Bash sanitized evidence
no write/edit content leak
policy/matched IDs/reason evidence
approval ID for file + Bash ASK
session grant evidence
terminal duration
safe scalar result evidence
failure category
real active/running/pending counters
counter cleanup on success/failure/approval/cancel/shutdown

Disabled vs Denied vs Ask vs Unavailable UI
reason-code remediation
invalid Policy fail-closed UX
migration recovery UX
Restart Required desired/effective truthfulness
Activity detail
English/Chinese resources
M0–M6 regression
```

## 35. Post-TASK Verification Documents

SPEC / PLAN / TASKS冻结后独立生成：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；Manual Acceptance才需要人工介入。

M7 Local Black-box E2E必须至少验证：

```text
real v1 config file
→ start current app/config path
→ automatic v2 migration
→ real Runtime starts with preserved settings

real MCP requests
→ hardened Activity evidence
→ desired/effective/revision UX data
```

不得用只调用 migration helper的 unit test冒充完整 migration E2E。

## 36. Final Acceptance Criteria

M7可 `ACCEPTED / FROZEN` 前必须证明：

- `CURRENT_SCHEMA_VERSION = 2`。
- V2-era v1 config自动迁移到 v2。
- M2–M6 transitional v1 config迁移且不丢 Tool/Policy/Bash/Language设置。
- Workspace/Tunnel/API Key reference/Proxy/Tunnel client/Security Mode全部保留。
- migration只在全部验证通过后 atomic persist。
- migration失败/unsupported schema不覆盖原文件。
- migration不明文化任何 SecretStore secret。
- migration保持 configRevision，且迁移后重复启动不再 rewrite。
- Activity/Audit能关联 request、tool、sanitized arguments、tool enabled/capability、Policy、matched IDs、Approval、result/failure、duration。
- file write/edit content与secret不进入 evidence。
- Runtime activeRequests/pendingApprovals/runningExecutions是真实计数。
- Disabled / Denied / Ask / Unavailable / execution failure在 UI中可区分。
- 关键 blocked/error状态有 actionable remediation。
- invalid Policy始终 fail closed且 UI不误导为可执行。
- desired/effective/configRevision mismatch明确显示 Restart Required。
- M6 i18n对新 M7 UX保持 en/zh-CN完整。
- M0–M6 regression PASS。
- Integration / Local Black-box E2E / Manual Acceptance完成。

## 37. Open Questions

```text
0
```

局部 DTO/module/component命名可在实现中调整，但 schema v1→v2 dual-source migration、preserve transitional values、secret safety、typed sanitized evidence、real Runtime counters、typed reason/remediation和 desired/effective truthfulness 是冻结边界。