# M7 Integration Test Plan — Migration, Evidence & UX Hardening

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M7 — Migration, Evidence & UX Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: Config migration / evidence model / Runtime instrumentation / typed reason + remediation / UI integration.  
> This is not Local Black-box E2E and does not perform M8 External ChatGPT acceptance.

## 1. Objective

验证 M7 三条收口链在 production components 组合后保持一致：

```text
A. Config Migration
v1 raw JSON
→ migrate in memory
→ validate AppConfig v2
→ atomic persist

B. Evidence Hardening
Runtime request
→ capability/workspace/policy/approval/execution
→ typed sanitized evidence
→ Activity / Diagnostics

C. UX Hardening
machine state/reason
→ typed reason
→ localized presentation
→ actionable remediation
```

Integration 的核心不是只证明单个 migration/helper 能运行，而是证明：

- 两类历史 v1 source 都能正确进入 v2。
- migration failure 永远不覆盖原 config。
- schema migration 不制造 configRevision/restart debt。
- Runtime evidence 能完整回答请求、Policy、Approval、执行和耗时。
- Activity 不泄露 file content / secret / full environment / unbounded output。
- `activeRequests / pendingApprovals / runningExecutions` 来自真实 lifecycle instrumentation。
- Disabled / Denied / Ask / Unavailable / Failed 是不同 machine state，不靠 UI 猜测。
- remediation 与当前 Runtime/config truth 一致。

## 2. Entry Criteria

执行前必须满足：

- M7 T1–T6 development complete。
- Unit/code-level tests PASS。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- frontend tests/typecheck/build PASS。
- `git diff --check` PASS。

未完成则：

```text
Integration = BLOCKED_BY_IMPLEMENTATION
```

## 3. Integration Boundary

Integration 可以：

- 使用真实 `ConfigStore` 读写 temporary config directory。
- 使用 source-controlled historical v1 fixtures。
- 使用真实 atomic replace implementation。
- 实例化真实 RuntimeCore / ExecutionPipeline / ApprovalManager / BashRuntime。
- 使用 temporary Workspace/filesystem。
- 使用 real Git Bash for selected process/evidence integration cases。
- 调用真实 Control dispatch / Activity snapshot / diagnostics projection。
- 使用 React provider/component integration tests。

Integration 不应：

- 只测 raw JSON transform就声称 migration integration完成。
- 用 mock evidence object代替 Runtime实际 emit。
- 用 UI字符串反推 Runtime policy/availability。
- 把 M8 External ChatGPT 手工链路算进 M7。

## 4. Required Integration Cases

### INT-01 — CURRENT_SCHEMA_VERSION is 2

确认 production ConfigStore：

```text
CURRENT_SCHEMA_VERSION = 2
```

新建默认 config：

- `schemaVersion = 2`。
- V3 defaults完整。
- wire protocol仍保持 `protocol/src/v1`。

### INT-02 — Pure V2-era v1 fixture migrates to v2

使用真实历史 shape：

```text
schemaVersion
configRevision
workspace
tunnel
security
ui.theme?
```

缺少 tools/policies/bash/language。

通过 production ConfigStore load。

期望：

- 返回 typed AppConfig v2。
- disk最终原子替换为 v2。
- Workspace/Tunnel/Security/UI theme保留。
- V3 missing fields按 frozen defaults补齐。

### INT-03 — Transitional M2–M6 v1 fixture preserves V3 settings

fixture包含非默认：

- `write.enabled=false`。
- custom Tool Defaults。
- User Policy DENY/ASK rule。
- Bash manual executable path / autoDetect value。
- `ui.language=zh-CN`。

迁移后全部语义保持，不得套 defaults覆盖。

### INT-04 — Migration preserves configRevision

例如 source：

```text
schemaVersion=1
configRevision=37
```

migration后：

```text
schemaVersion=2
configRevision=37
```

不得自动 +1。

### INT-05 — Security mode is authoritative

source：

```text
mode=normal
developerTestMode=true
```

迁移后：

```text
mode=normal
developerTestMode=false
```

反向 case DeveloperTest + false同理 normalize为 true。

### INT-06 — Migration preserves Tunnel and API key reference

逐字段验证：

- tunnelId。
- proxy。
- tunnel client path。
- autoDetect。
- apiKeyRef provider/key opaque reference。

不得触碰 SecretStore raw secret。

### INT-07 — Missing V3 fields receive frozen defaults only

逐项断言：

```text
read_file ON
glob ON
write ON
edit ON
bash OFF

read/glob/write/edit default ALLOW
bash default ASK
userRules=[]

bash.autoDetect=true
bash.executablePath=null
ui.language=null
```

### INT-08 — Malformed known transitional field fails migration

例如：

- `tools.write.enabled="yes"`。
- malformed Policy matcher。
- invalid Bash config type。
- unsupported language value。

期望：

- `CONFIG_MIGRATION_FAILED`/typed migration error。
- 不回退 defaults。
- 原 file bytes unchanged。

### INT-09 — Invalid Policy fails migration closed

source有 enabled invalid rule。

期望：

- migration整体失败。
- 不跳过 bad rule。
- 不产生部分 v2。

### INT-10 — Unsupported/future schema fails without mutation

覆盖：

- schemaVersion=0。
- schemaVersion=3+。
- missing schemaVersion。
- schemaVersion wrong type。

期望明确 `CONFIG_SCHEMA_UNSUPPORTED` 或 strict typed invalid error，原文件不变。

### INT-11 — Unknown migration-owned fields do not silently disappear

在 v1 known objects中加入未支持 field。

期望 fail diagnostic，而不是 deserialize/drop后静默迁移。

### INT-12 — Raw plaintext-secret-like field fails closed

source出现未支持 raw API key/plaintext secret字段 fixture。

期望：

- migration失败。
- secret不进入 v2。
- error/evidence不回显 secret value。
- original bytes unchanged。

### INT-13 — Atomic persistence success

migration完成后验证：

- v2文件为完整 valid JSON。
- no `.tmp` residue。
- file可重新 load。
- preserved fields一致。

### INT-14 — Atomic persistence failure keeps v1 source

使用 deterministic replace/write failure seam。

期望：

- original v1 remains exact bytes。
- no partial target。
- temp cleanup符合 ConfigStore contract。
- typed write/migration error。

### INT-15 — Migration is application-idempotent

第一次 load迁移 v1→v2。

第二次 load：

- 直接读 v2。
- no second migration。
- configRevision unchanged。
- file bytes不被无意义重写（允许格式冻结后的稳定序列化；测试应按实现契约断言）。

### INT-16 — v2 normal save revision semantics remain correct

v2上：

- identical runtime semantics → revision unchanged。
- Tool/Policy/Security/Bash/Workspace runtime-affecting change → +1。
- language/theme UI-only change → unchanged。

### INT-17 — Migration recovery metadata remains available

config migration失败时：

- `configGet`失败。
- `configMetadata`仍能返回 config directory/file path。
- recovery UI可定位文件。

### INT-18 — Reset is explicit destructive recovery only

migration failure不得自动 reset。

只有用户显式 confirm + reset action才删除/replace config。

### INT-19 — File-tool sanitized arguments evidence

分别真实执行：

- read_file。
- glob。
- write。
- edit。

Activity hardened evidence只包含允许字段：

```text
read_file → logicalPath
glob → bounded pattern
write → logicalPath
edit → logicalPath
```

不得出现：

- read content。
- write content。
- oldText/newText。
- physical Workspace root。

### INT-20 — Tool enabled/capability evidence

对 available/disabled request路径确认 evidence能表达：

- tool identity。
- enabled/eligibility state。
- declared capability。

Disabled stale call必须留下可诊断 evidence且无 executor side effect。

### INT-21 — Policy evidence is canonical

覆盖 ALLOW / ASK / DENY：

- policyDecision。
- matchedRuleIds。
- policyReason。

这些值必须来自 PolicyEngine，不由 Activity/UI重算。

### INT-22 — Approval ID + decision correlation

ASK request：

- Activity Pending evidence包含 approval ID。
- AllowOnce / AllowForSession / Deny terminal evidence关联同 request + approval。
- decision正确。

### INT-23 — Request duration on terminal evidence

成功、DENY、approval denied、execution failed至少各一例。

期望 terminal evidence存在 bounded `durationMs`/equivalent，非负且 request-local。

### INT-24 — Safe file result scalar evidence

成功执行：

- read_file → bytesRead。
- glob → matchCount。
- write → bytesWritten + created/overwritten。
- edit → replacement count/bytesWritten equivalent。

不得复制 tool raw result/content。

### INT-25 — Bash hardened evidence remains bounded

执行 classified Bash和limit/error fixture。

保留：

- commandPreview bounded/sanitized。
- classification/risk flags。
- cwd logical identity。
- approval/session grant info。
- exitCode/duration/limit outcome。

不新增 argv/raw env/full output leakage。

### INT-26 — Evidence size bounds

对超长 path/pattern/command/error fixture验证：

- sanitized projection bounded。
- Activity record bounded。
- no panic/oversized unbounded metadata。

### INT-27 — Unknown/untrusted identifiers are sanitized

对 malformed rule ID / arbitrary unexpected metadata：

- allowlisted/validated identifiers保留。
- unsafe arbitrary text redacted/dropped。

### INT-28 — Active request counter lifecycle

创建一个 request并让其停留在不同阶段。

验证：

```text
before request = 0
request active = 1
after terminal = 0
```

包括 validation failure / DENY / ASK wait / execution error都必须归零。

### INT-29 — pendingApprovals counter is real

ASK进入 Pending：

```text
pendingApprovals = 1
```

Allow/Deny/Expire/Cancel：

```text
pendingApprovals = 0
```

多 request并发时计数正确。

### INT-30 — runningExecutions counter is real

executor真正开始前=0。

长运行 Bash或 deterministic executor fixture执行中：

```text
runningExecutions = 1
```

结束/timeout/cancel/shutdown后=0。

Approval wait不应误计为 running execution。

### INT-31 — Counter race cleanup

覆盖：

- approval resolve vs stop。
- Bash timeout。
- request cancellation。
- executor failure。

所有 counters最终归零，不 underflow/double-decrement。

### INT-32 — Typed ToolStatusReason for all availability classes

验证至少覆盖：

- Available。
- ApprovalRequired/Ask。
- Disabled。
- DeniedByPolicy。
- UnavailableByMode。
- UnavailableByRuntime/NotReady。
- BashExecutableUnavailable。
- InvalidPolicySnapshot。

UI无需解析自由英文 reason判断状态。

### INT-33 — Remediation mapping is deterministic

机器 reason → remediation action/key，例如：

```text
Disabled → enable tool + restart
Mode → switch Developer/Test + restart
Bash unavailable → configure Bash + restart
Policy denied → review Policies + restart after save
ASK → Approval Center
Runtime unavailable → start/restart/diagnostics
Migration failed → repair/retry/reset-last-resort
```

无适用 action时必须明确 no-action，而不是虚构按钮。

### INT-34 — Invalid Policy UX remains fail closed

invalid snapshot：

- Runtime decision DENY/unavailable。
- UI明确 configuration/policy error。
- remediation指向 Policy repair。
- 不显示“Allowed”或默认 fallback。

### INT-35 — Desired/effective/restart truthfulness

运行中保存 Runtime-affecting v2 config：

- desired立即更新。
- effective保持 current Runtime snapshot。
- active revision != saved revision。
- Restart Required明确。

restart后收敛。

### INT-36 — Migration itself does not create Restart Required

已有 active revision N语义 fixture对应 v1→v2 migration后仍 revision N。

UI不得仅因 schemaVersion变化显示 Runtime restart debt。

### INT-37 — Activity detail UI presents hardened evidence

Activity detail/row expansion应能显示：

- tool。
- sanitized args。
- decision/matched rules。
- approval when present。
- outcome/failure。
- duration/result scalar。

不重新计算 machine state。

### INT-38 — English / zh-CN M7 presentation coverage

新增 migration/reason/remediation/evidence labels：

- en/zh-CN key parity。
- placeholder parity。
- no major hardcoded user-visible English outside allowed technical values。

### INT-39 — Diagnostics counters/reasons agree with Runtime state

Control diagnostics / runtime status / effective tools对同一时刻：

- counters一致。
- availability/reason一致。
- UI不显示互相矛盾状态。

### INT-40 — M0–M6 regression

至少验证：

- M1 atomic write/edit。
- M2 toggle/restart。
- M3 Policy fail closed。
- M4 same-request approval/grants。
- M5 Bash limits/process cleanup。
- M6 language instant switch + no runtime revision debt。

## 5. Historical Fixture Requirements

M7必须把历史配置作为 source-controlled fixtures，而不是在测试代码里临时拼出“想象中的旧格式”。

至少包含：

```text
v1-v2-era-minimal.json
v1-transitional-m2-tools.json
v1-transitional-m3-policy.json
v1-transitional-m5-bash.json
v1-transitional-m6-i18n.json
v1-invalid-policy.json
v1-invalid-secret-like-field.json
v1-future-or-unsupported-schema.json
```

fixture内容必须来自真实历史 schema/commit shape或与其严格等价。

## 6. Evidence Format

每个 case记录：

```text
Case ID
PASS / FAIL / BLOCKED
Test name / command
Source fixture (migration case)
Pre/post schemaVersion
Pre/post configRevision
Original-file integrity assertion
Runtime request ID / approval ID where applicable
Counters before/during/after
Evidence fields asserted
Sanitization assertion
UI reason/remediation assertion
```

不得在报告中复制 secret/raw payload。

## 7. Exit Criteria

INT-01 ～ INT-40全部 PASS 才能声明 M7 Integration PASS。

特别要求：

- pure V2 v1 migration PASS。
- transitional M2–M6 v1 migration PASS。
- migration failure preserves original bytes PASS。
- secret safety PASS。
- configRevision preservation PASS。
- hardened evidence completeness + sanitization PASS。
- active/pending/running counters PASS。
- typed status reason/remediation PASS。
- desired/effective/restart truthfulness PASS。
- M0–M6 regression PASS。

随后进入 Local Black-box E2E。

## 8. Non-Claims

Integration PASS不代表：

```text
real packaged upgrade path black-box PASS
External ChatGPT final V3 acceptance PASS
Activity is durable across Runtime restart
M8 complete
V3 ACCEPTED / FROZEN
```
