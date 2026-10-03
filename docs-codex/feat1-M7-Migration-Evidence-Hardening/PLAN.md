# M7 PLAN — Migration, Evidence & UX Hardening

> Input: `SPEC.md`  
> Baseline: M6 ACCEPTED / FROZEN  
> Goal: 正式完成 AppConfig v1→v2 migration，并收口 Runtime evidence、diagnostics 和关键 UX state。  
> Rule: PLAN 只定义实现顺序；Integration/E2E/Manual Acceptance另行生成。

## 1. Strategy

M7分三条工作流，但按依赖顺序实施：

```text
Config Schema / Migration
        ↓
Runtime Evidence Model
        ↓
Runtime Counters / Typed Reasons
        ↓
Control / UI Projection
        ↓
Recovery / Remediation UX
        ↓
Cross-version Regression
```

原则：

- migration先冻结，避免后续测试继续写 schema v1 fixtures。
- Runtime先产生 authoritative evidence，再改 UI。
- UI只展示 typed state，不从 prose或desired config推断 effective事实。
- 所有变化优先 additive，不创建 protocol v2。
- 不改变 M0–M6 tool authorization/execution semantics。

## 2. Phase A — Freeze AppConfig v2

修改 `protocol/src/v1/config.rs`：

```text
AppConfig schemaVersion target = 2
```

保持当前 serialized fields和 defaults。

同步：

- Rust DTO。
- JSON Schema。
- TS generated types。
- config fixtures。
- test helpers。

明确：

- `RuntimeConfigSnapshot`不增加 UI/Tunnel host-only fields。
- M5 Bash hard limits不进入 AppConfig。
- wire protocol major不变。

Exit：新建配置直接写 schema v2；production code不再创建 schema v1。

## 3. Phase B — Raw JSON Migration Pipeline

重构 Tauri `ConfigStore::load()`：

```text
read bytes
→ raw serde_json::Value
→ schema version dispatch
```

新增概念模块：

```text
config/migration.rs
├ LegacyAppConfigV1
├ migrate_v1_to_v2
├ validate_migrated_v2
└ MigrationError
```

### v2

- typed deserialize。
- normalize/validate。
- no migration rewrite。

### v1

- parse dedicated legacy DTO/raw fields。
- construct v2 in memory。
- validate。
- atomic persist。

### other

- typed unsupported schema error。
- no file write。

Exit：ConfigStore不再直接把 schema mismatch等同 generic invalid；v1能进入 explicit migration。

## 4. Phase C — Dual-source v1 Preservation

为真实历史建立 fixture matrix：

### Pure V2 fixture

至少包含：

```text
workspace
tunnelId
proxy
tunnel client override/autoDetect
apiKeyRef
security.mode + compatibility bool
ui.theme
configRevision
```

### Transitional V3 v1 fixture

额外包含：

```text
ui.language
tools
policies
bash
```

Migration helper按“present wins, missing defaults”处理。

不得先反序列化成当前 `AppConfig` 再猜哪些字段历史上缺失。

Exit：两类 source都可无损迁移。

## 5. Phase D — Migration Normalization / Validation

迁移后的 v2执行与普通 ConfigStore save一致的安全 validation：

- proxy normalize/validate。
- tunnel path normalize。
- Policy validate_for_save。
- UiLanguage strict enum。
- BashConfig typed parse。
- ToolConfig complete shape。
- SecurityMode authoritative mirror normalization。

### Strictness

对 known field malformed：fail。

对 v1 migration-owned object unknown fields：fail diagnostic，避免 silent loss。

### configRevision

migration preserve原 revision。

不要调用普通 `save()`的 semantic increment logic完成迁移；使用专门的 validated atomic persist path。

Exit：migration不会因 schema conversion自己制造 revision变化。

## 6. Phase E — Migration Atomicity / Errors / Recovery Data

扩展 `ConfigError`：

```text
Invalid
Io
UnsupportedSchema
Migration
```

machine codes：

```text
CONFIG_INVALID
CONFIG_WRITE_FAILED
CONFIG_SCHEMA_UNSUPPORTED
CONFIG_MIGRATION_FAILED
```

Migration persist：

```text
serialize temp
→ sync
→ atomic replace
```

failure：

- original bytes unchanged。
- temp cleanup。

调整 config metadata/recovery plumbing：

- `configGet()`失败时仍能获取 config path metadata。
- recovery UI可 Retry / inspect path / Reset。

Exit：migration failure可诊断且不 destructive。

## 7. Phase F — Define Hardened Evidence DTO

在 shared protocol/control-read DTO中 additive定义：

```text
SanitizedArgumentsEvidence
ExecutionResultEvidence
ToolStatusReason
```

以及 ActivityEntry新增：

```text
toolEnabled
capability?
sanitizedArguments?
approvalId?
approvalDecision?
durationMs?
resultEvidence?
```

保持已有：

```text
requestId
eventId
toolName
policyDecision
matchedRuleIds
policyReason
outcome
failureCategory
bash
```

对 Activity event type enums做 Rust/Runtime/control-read/TS一致化。

Exit：UI有单一 typed evidence contract，不靠手写 enum猜 Runtime shape。

## 8. Phase G — Tool-specific Sanitization Producers

不要在 Activity projection层从 raw JSON泛化脱敏。

由 typed request/executor在 Runtime内部生产 safe evidence。

### read_file

```text
logicalPath
```

### glob

```text
bounded pattern
```

### write

```text
logicalPath
```

禁止 content。

### edit

```text
logicalPath
```

禁止 old/new text。

### bash

复用 analyzer sanitized command preview / classification。

所有 evidence设置 size/control-char bounds。

Exit：AuditEvent创建时只接触 safe typed projection，而不是 raw payload。

## 9. Phase H — Result Evidence / Duration

### Request duration

在 `RequestContext` 增加 monotonic start instant。

terminal event时：

```text
durationMs = elapsed
```

不要用 RFC3339 timestamp差计算。

### Result evidence

executor success后提供 bounded scalar evidence：

- read bytesRead。
- glob matchCount。
- write bytesWritten + created/overwritten。
- edit replacements + bytesWritten。
- bash exitCode。

失败继续使用 outcome/failure category。

Exit：Activity能够回答“执行结果是什么”，但不泄露内容。

## 10. Phase I — Approval Correlation for All Tools

当前 Bash evidence已经能携带 approval request ID。

M7提升为 request-level generic field：

```text
approvalId
approvalDecision
```

在：

- ApprovalRequired。
- ApprovalResolved。
- Denied/Expired/Cancelled。
- resumed execution。

均保持 same request/approval identity。

SessionGrantMatched继续显式事件/flag，不伪装普通 ALLOW。

Exit：file tools与Bash使用一致的 approval correlation model。

## 11. Phase J — Real Runtime Counters

新增 Runtime-owned lightweight counters：

```text
active_requests
running_executions
```

pending approvals继续由 ApprovalManager计算。

### Instrumentation

Pipeline entry：active +1。

terminal guard drop：active -1。

executor start：running +1。

executor terminal/cancel：running -1。

用 RAII guard / atomic owner避免漏 decrement。

Runtime stop最终 counters=0。

更新：

- RuntimeState。
- diagnostics snapshot。
- tests。

Exit：不再返回 constant zero / NotInstrumented。

## 12. Phase K — Typed Effective Tool Reasons

替换“UI主逻辑依赖 English `reason` string”的状态。

`EffectiveTool` / `ToolDiagnostic`增加：

```text
statusReason: ToolStatusReason
```

freeform detail仅作为 optional technical detail。

Runtime mapping覆盖：

- ready available。
- approval required。
- disabled。
- mode restriction。
- runtime not ready。
- Bash executable unavailable。
- policy deny。
- invalid policy snapshot/context。
- unsupported capability。
- metadata invalid。

Exit：UI可以纯 machine enum选择 badge/remediation。

## 13. Phase L — Activity Projection / UI

更新 `control.activity.snapshot` producer/parser。

Activity list保留简洁视图，增加 row detail/expand surface：

```text
Request
Arguments
Tool state/capability
Policy
Matched rules
Approval
Result/failure
Duration
Bash facts
```

要求：

- raw IDs可复制。
- null/missing evidence明确显示 Not recorded/Not applicable。
- 不推断缺失值。
- no secret/content。

Exit：用户无需读 Runtime log即可理解核心 request lifecycle。

## 14. Phase M — Tool / Policy Remediation UX

建立 shared presentation mapping：

```text
ToolStatusReason
→ localized label
→ localized remediation
```

Tools page明确五类：

```text
Disabled
Denied
Ask
Unavailable
Available
```

Policies invalid snapshot：

- warning/error banner。
- safe rule/validation reason。
- fix & restart remediation。

ASK显示“可用但需要审批”，不能显示成 unavailable error。

Exit：用户可从状态直接知道下一步操作。

## 15. Phase N — Migration Recovery UX

Settings/FirstRun/Config error surface增加：

### Unsupported schema

- explain app cannot safely load newer/unknown config。
- do not reset automatically。

### Migration failed

- original unchanged说明。
- config path。
- retry。
- inspect/copy path。
- reset with confirmation。

Config metadata load与 config content load解耦，保证失败时仍有 path。

Exit：用户遇到 migration problem时不是只有 generic CONFIG_INVALID。

## 16. Phase O — Shared Restart / Revision UX

建立 shared component/model，例如：

```text
ConfigRevisionStatus
RestartRequiredNotice
```

展示：

```text
saved revision
active revision
restart required
```

在 Tools/Policies/Settings/Overview复用。

约束：

- Runtime stopped时不要制造 active revision。
- language-only save不改变 revision。
- existing restart debt不能被 language change清除。

Exit：各页面不再各自解释 desired/effective mismatch。

## 17. Phase P — Diagnostics Hardening

Overview/Diagnostics使用真实 Runtime/Tauri sources：

- lifecycle/readiness。
- runtime ID。
- active/saved revision。
- active/pending/running counters。
- safe last error code。
- Tool effective reason。

Tunnel依旧用 Harness/Tauri source，不从 Runtime伪造。

修正旧 `RequestObservation::NotInstrumented` 等已经过时的 placeholder contract。

Exit：Diagnostics成为 M8验收问题定位入口。

## 18. Phase Q — i18n / Presentation Resource Update

为 M7新增：

- migration errors。
- migration recovery。
- status reason labels。
- remediation。
- evidence labels。
- result evidence。
- counters/revisions。

更新：

```text
en.json
zh-CN.json
presentation.ts
resource parity tests
```

技术 ID/raw data保持不翻译。

Exit：M7新 UX符合 M6双语边界。

## 19. Phase R — Cross-version / Security Regression

重点 regression：

### Config

- new install v2。
- V2 legacy v1 migrate。
- M6 transitional v1 migrate。
- v2 normal saves。
- ui language save。
- malformed/unknown schema。

### Authorization

- Tool OFF。
- Policy DENY/ASK/ALLOW。
- Approval。
- Bash。

### Evidence

- denied before execution。
- approval wait/resume。
- execution success/failure。
- timeout/cancel。

### Secret safety

scan serialized migrated config / Activity payload for sentinel secret/content values。

Exit：M7 hardening没有改变 M0–M6 execution behavior。

## 20. Migration Fixture Policy

将真实历史 shape固化为 source-controlled fixtures，而不是测试中只手写当前 DTO：

```text
fixtures/config/v1-v2-era.json
fixtures/config/v1-transitional-m6.json
fixtures/config/v1-invalid-*.json
fixtures/config/v2-current.json
```

Fixture必须使用 placeholder secret references，不含真实 credential。

这确保未来代码无法无意破坏升级路径。

## 21. Dependency Discipline

M7预期不需要大型新依赖。

允许使用现有：

- serde_json raw Value。
- atomic file primitive。
- std atomics/RAII。
- existing React/i18n stack。

不要为 migration引入通用 migration framework/database。

## 22. Change Discipline

M7不得：

- 改 M5 Bash security limits。
- 改 Policy precedence。
- 改 Approval scope。
- 改 Tool schemas。
- 改 Workspace boundary。
- 引入 DB。
- 引入 protocol/v2。
- 提前执行 M8 External ChatGPT acceptance并宣称 V3 frozen。
- 把 migration failure自动 reset。
- 把 evidence hardening变成 raw payload logging。

## 23. Development Completion Check

执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm test
npm run typecheck
npm run build
git diff --check
```

核心开发验证：

```text
schema2 + migration
preservation + atomicity
secret safety
revision invariants
evidence schema/sanitization
approval correlation
result/duration
runtime counters
typed reasons/remediation
migration recovery UI
activity detail
i18n
M0–M6 regression
```

完成后独立生成并执行 M7 Integration / Local Black-box E2E / Manual Acceptance。
