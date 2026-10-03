# M7 Manual Acceptance Checklist — Migration, Evidence & UX Hardening

> Status: DRAFT / READY FOR FINAL HUMAN REVIEW  
> Milestone: M7 — Migration, Evidence & UX Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Owner: Human Reviewer  
> Purpose: M7唯一需要人工介入的最终验收层。

## 1. Acceptance Responsibility

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

人工验收重点：

- migration/recovery UX是否清楚、安全、不误导。
- Disabled / Denied / Ask / Unavailable / Failed视觉语义是否明显区分。
- remediation是否可执行且与真实状态一致。
- Activity evidence是否足够回答“请求了什么、为何允许/拒绝、是否执行、结果如何”。
- desired/effective/restart presentation是否真实。
- English / 简体中文下新增 M7 文案是否完整且安全语义一致。

人工不负责：

- 手工制造 atomic replace failure。
- 手工检查 concurrency race。
- 手工验证 secret sentinel scan。
- 手工重跑全部 M0–M6矩阵。
- M8 External ChatGPT最终 golden paths。

这些必须先由自动化 PASS。

## 2. Entry Criteria

只有以下全部 PASS才进入人工验收：

```text
M7 Development
Unit
Integration
Local Black-box E2E
```

必须提供：

```text
Production commit
Automated evidence commit
Acceptance build / Portable identity
Runtime binary SHA-256
Integration summary
LOCAL_M7_BLACK_BOX_E2E=PASS
```

若 v1→v2 migration或真实 evidence E2E仍缺失，人工 UI smoke不能替代。

## 3. Build / Source Identity

- [ ] 记录 M7 production implementation commit。
- [ ] 记录 QA/evidence commit。
- [ ] 记录人工验收 build / Portable目录。
- [ ] 记录 Runtime binary SHA-256。
- [ ] 确认 acceptance build之后没有未经重新验证的 migration/evidence production change。

记录：

```text
Production commit:
Evidence commit:
Acceptance build:
Runtime SHA-256:
Reviewer:
Date:
```

## 4. Automated Evidence Review

### 4.1 Migration

- [ ] pure V2-era v1 → v2 PASS。
- [ ] transitional M2–M6 v1 → v2 PASS。
- [ ] existing Tool/Policy/Bash/Language values preserved。
- [ ] configRevision preserved during migration。
- [ ] invalid migration leaves original bytes unchanged。
- [ ] future schema leaves original bytes unchanged。
- [ ] SecretStore/raw-secret safety PASS。
- [ ] atomicity/idempotency PASS。

### 4.2 Evidence / Counters

- [ ] file-tool sanitized arguments PASS。
- [ ] Policy matched-rule evidence PASS。
- [ ] Approval ID/decision correlation PASS。
- [ ] duration/result scalar evidence PASS。
- [ ] Bash evidence remains bounded PASS。
- [ ] `activeRequests`真实 instrumentation PASS。
- [ ] `pendingApprovals`真实 instrumentation PASS。
- [ ] `runningExecutions`真实 instrumentation PASS。
- [ ] failure/cancel/shutdown counters归零 PASS。

### 4.3 UX / Regression

- [ ] typed ToolStatusReason PASS。
- [ ] remediation mapping PASS。
- [ ] desired/effective/restart truthfulness PASS。
- [ ] en/zh-CN M7 resource coverage PASS。
- [ ] M0–M6 regression PASS。
- [ ] `LOCAL_M7_BLACK_BOX_E2E=PASS`。

## 5. Migration Success UX

人工不要直接拿唯一生产配置冒险测试。

优先使用：

- disposable profile/config directory。
- Portable acceptance fixture。
- 已备份的测试 config。

### MA-01 — Existing v1 config upgrades without setup reset

用准备好的合法历史 v1测试配置启动 acceptance build。

确认：

- [ ] 应用能正常进入主 UI。
- [ ] 没有重新要求填写已存在的 Workspace/Tunnel信息。
- [ ] config已升级为 schemaVersion 2。
- [ ] 原 Workspace保持。
- [ ] Tunnel ID保持。
- [ ] proxy/Tunnel client settings保持。
- [ ] API key reference仍有效/仍显示已配置状态。
- [ ] Security Mode保持。

### MA-02 — Transitional V3 settings survive migration

使用含非默认设置的 transitional v1 fixture。

确认：

- [ ] Tool OFF状态保留。
- [ ] User Policies保留。
- [ ] Bash配置保留。
- [ ] language偏好保留。
- [ ] 不被默认值重置。

### MA-03 — Migration does not create fake Restart Required

迁移完成后：

- [ ] 不仅因为 schemaVersion 1→2 就出现 Restart Required。
- [ ] configRevision与 migration前一致。

若随后人工修改真正 Runtime-affecting配置，Restart Required应正常出现。

## 6. Migration Failure / Recovery UX

使用 disposable invalid fixture，不破坏真实用户配置。

### MA-04 — Failure is recoverable, not blank/crash

启动含非法 v1 config的测试 profile。

确认：

- [ ] 应用显示明确 configuration/migration error。
- [ ] UI仍可操作 recovery surface。
- [ ] 不出现空白页/无限 loading。
- [ ] 明确说明原配置未被覆盖。

### MA-05 — Config location remains visible

migration失败时：

- [ ] 能看到 config file/path位置。
- [ ] 用户知道应该检查哪个文件。
- [ ] 不要求猜测 AppData目录。

### MA-06 — Recovery actions are explicit

至少检查：

- [ ] Retry/reload action可理解。
- [ ] Inspect/repair guidance可执行。
- [ ] Reset明确为 destructive last resort。
- [ ] Reset需要明确确认。
- [ ] 不会自动 reset。

### MA-07 — Error does not expose secrets

观察错误页、detail、Diagnostics：

- [ ] 不显示 raw API key。
- [ ] 不显示完整 config JSON。
- [ ] 不显示不必要的 secret-like raw value。

## 7. Tools Status Hardening

### MA-08 — Disabled is visually distinct

选择一个 Tool关闭并应用新 generation。

确认 Tools UI：

- [ ] 状态明确是 Disabled。
- [ ] 不显示成 Denied。
- [ ] 不显示成 Runtime unavailable。
- [ ] remediation指向 Enable + Restart流程。

### MA-09 — Policy DENY is distinct

对一个 enabled Tool设置 DENY并重启。

确认：

- [ ] 状态明确是 Denied by Policy/equivalent。
- [ ] Tool仍可与 Disabled概念区分。
- [ ] remediation指向 Policies而非 Tool toggle。

### MA-10 — ASK is distinct

对 enabled Tool设置 ASK。

确认：

- [ ] 状态明确说明 Requires Approval/Ask。
- [ ] 不显示成 unavailable/error。
- [ ] remediation指向 Approval Center或解释运行时会请求审批。

### MA-11 — Runtime/Bash Unavailable is distinct

制造安全的 Bash invalid executable或 Runtime unavailable状态。

确认：

- [ ] Unavailable与 Disabled/Denied/Ask视觉语义不同。
- [ ] reason清楚。
- [ ] remediation指向 Bash配置/Restart/Diagnostics等正确位置。

## 8. Desired vs Effective Truthfulness

### MA-12 — Runtime-affecting save shows mismatch

Harness运行中修改例如 Tool enabled或Policy。

确认：

- [ ] Desired State立即显示新值。
- [ ] Effective State仍显示当前 Runtime旧值。
- [ ] Restart Required出现。
- [ ] UI没有把 desired冒充为 effective。

### MA-13 — Restart convergence

执行 Restart Harness。

确认：

- [ ] Restart Required消失。
- [ ] Effective与 Desired收敛。
- [ ] 状态/remediation同步更新。

### MA-14 — Language-only change remains immediate

切换 English / 简体中文：

- [ ] 即时切换。
- [ ] 不产生新的 Restart Required。
- [ ] Runtime无需重启。

## 9. Activity Evidence UX

使用安全 Workspace fixture产生几类请求。

### MA-15 — Successful file request is explainable

执行一个安全 read/write。

Activity中确认能回答：

- [ ] 哪个 Tool。
- [ ] 哪个 sanitized target/path。
- [ ] Policy decision。
- [ ] matched rule（适用时）。
- [ ] 是否执行成功。
- [ ] duration。
- [ ] safe result scalar。

### MA-16 — Content is not leaked

使用明显 sentinel content进行 write/edit测试。

Activity UI：

- [ ] 不显示 write content。
- [ ] 不显示 edit oldText/newText。
- [ ] 不显示完整 file content。
- [ ] 仍有足够 path/result evidence理解发生了什么。

### MA-17 — DENY is explainable

产生一个 Policy DENY。

确认 Activity：

- [ ] request/tool可识别。
- [ ] decision=DENY。
- [ ] matched rule IDs/readable policy evidence可用。
- [ ] no execution success。
- [ ] terminal outcome明确。

### MA-18 — Approval chain is explainable

产生 ASK并 Allow once或Deny。

确认 Activity能串联：

```text
Request
→ Policy ASK
→ Approval ID
→ Approval decision
→ Execution/result or no-execution
```

同 request identity可理解。

### MA-19 — Failure is explainable

选择 harmless failure，例如 invalid args / expected execution failure。

确认：

- [ ] failure category明确。
- [ ] 不只是 generic Error。
- [ ] duration可见/可诊断。
- [ ] remediation如果适用则正确。

## 10. Runtime Counters / Diagnostics UX

### MA-20 — Pending approval counter

保持一个 ASK request Pending。

确认 Diagnostics/Overview（实际产品暴露位置）：

- [ ] pending approvals反映 1/equivalent。
- [ ] resolve后回到 0。

### MA-21 — Active/running counters do not lie

review自动化 E2E evidence为主；若 UI可自然观察长运行 Bash：

- [ ] active request在运行时非 0。
- [ ] running execution在 executor阶段非 0。
- [ ] 完成后归零。

若人工 UI窗口太短，不要求为了观察而制造危险命令；自动化 evidence可作为 acceptance依据。

## 11. Remediation Quality

逐项抽查至少 5 种状态。

### MA-22 — Actions point to the correct surface

例如：

```text
Disabled        → Tools
Policy Denied   → Policies
ASK             → Approval Center
Bash Unavailable→ Bash config / Tools
Runtime Error   → Diagnostics / Restart
Migration Error → Config recovery
```

确认：

- [ ] 不出现无关 action。
- [ ] 不建议用 Approval绕过 DENY。
- [ ] 不建议用 Tool enable解决 Policy DENY。
- [ ] 不建议重启解决必须先修配置的问题而不说明修复步骤。

### MA-23 — No false authority claims

检查状态/帮助文案：

- [ ] UI不声称自己是 Policy authority。
- [ ] Runtime effective state明确来自 Runtime。
- [ ] Bash仍不声称 OS sandbox。
- [ ] Activity被描述为 bounded/runtime evidence，不冒充 durable compliance audit database。

## 12. English / 简体中文 Hardening

### MA-24 — Migration UX bilingual

English和简体中文分别检查：

- migration success/recovery说明。
- Retry/Reset/remediation。
- unsupported schema/error explanation。

要求：

- [ ] 语义一致。
- [ ] 中文不弱化“原文件未覆盖”“Reset destructive”等安全含义。

### MA-25 — Tool status/remediation bilingual

两种语言检查 Disabled / Denied / Ask / Unavailable / Restart Required。

- [ ] 不混淆状态含义。
- [ ] action labels完整。
- [ ] technical identifiers保持原文。

### MA-26 — Activity evidence bilingual presentation

- [ ] labels/解释可翻译。
- [ ] tool name / rule ID / request ID / machine code等保持原值。
- [ ] user-authored Policy name/reason不被自动翻译。

## 13. Regression Smoke

人工只执行高价值 smoke，不重复自动化全矩阵：

- [ ] read_file正常。
- [ ] write/edit正常。
- [ ] Tool OFF状态正确。
- [ ] Policy DENY状态正确。
- [ ] ASK Approval仍 same-request resume。
- [ ] Bash页面仍明确 Developer/Test + no-sandbox boundary。
- [ ] language切换仍正常。

## 14. M8 Boundary Review

M7 Manual Acceptance **不等于 V3最终验收**。

确认：

- [ ] 未将 M7 标记为 `V3 ACCEPTED / FROZEN`。
- [ ] 未用本地 MCP/Runtime测试代替 External ChatGPT。
- [ ] M8 Full External Acceptance仍待执行。

M7完成后只允许：

```text
M7 — ACCEPTED / FROZEN
```

## 15. Rejection Conditions

出现任一项即不能 ACCEPT M7：

- migration重置合法用户 Tool/Policy/Bash/Language配置。
- migration自行增加 configRevision并制造假 Restart Required。
- migration failure覆盖/破坏原 config。
- future schema被静默降级。
- raw secret进入普通 config/evidence/error。
- invalid Policy迁移时被静默跳过。
- Activity显示 write/edit raw content。
- Approval evidence无法关联同 request。
- `activeRequests`/`runningExecutions`仍是固定 skeleton 0。
- Disabled / Denied / Ask / Unavailable无法区分。
- remediation与真实状态矛盾。
- desired state被冒充为 current effective state。
- M7新增文案仅英文或中文安全语义错误。
- M0–M6 regression未通过。

## 16. Final Decision

全部满足后：

- [ ] Development PASS
- [ ] Unit PASS
- [ ] Integration PASS
- [ ] Local Black-box E2E PASS
- [ ] Manual Acceptance PASS
- [ ] M7 — ACCEPTED / FROZEN

最终记录：

```text
M7 Decision:
Reviewer:
Date:
Production commit:
Evidence commit:
Acceptance build:
Runtime SHA-256:
Notes:
```

M7冻结后，下一步是 **M8 Full V3 External Acceptance**。M8才执行完整 External ChatGPT golden paths，并决定是否可以最终标记：

```text
V3 — ACCEPTED / FROZEN
```
