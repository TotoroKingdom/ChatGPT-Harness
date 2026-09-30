# M2 Manual Acceptance Checklist — Tool Enable / Disable

> Status: DRAFT / READY FOR FINAL HUMAN REVIEW  
> Milestone: M2 — Tool Enable / Disable  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Purpose: final human gate. Unit / Integration / Local Black-box E2E are Coding Agent responsibilities; Manual Acceptance is the only human-intervention layer.

## 1. Acceptance Responsibility

M2 verification ownership：

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

Manual Acceptance 不重复人工执行 Unit / Integration / automated E2E。

人工层只负责：

1. 审阅自动化证据与 build/commit identity。
2. 检查 M2 scope / security / lifecycle claim 是否准确。
3. 验证真实桌面 UI 中 desired / effective / restart-required UX。
4. 必要时确认真实 ChatGPT/Tunnel 用户链路没有明显 Tool Toggle 回归，但不把 External ChatGPT缓存刷新行为当成 Runtime correctness gate。
5. 做最终 `ACCEPTED / FROZEN` 决策。

## 2. Entry Criteria

Manual Acceptance 开始前 Coding Agent 必须提供：

```text
Development = PASS
Unit / Regression = PASS
Integration = PASS
Local Black-box E2E = PASS
```

并完成：

- [ ] T1–T5 development complete。
- [ ] `cargo fmt --all -- --check` PASS。
- [ ] `cargo check --workspace` PASS。
- [ ] `cargo test --workspace` PASS。
- [ ] frontend tests/typecheck/build PASS（按仓库命令）。
- [ ] `git diff --check` PASS。
- [ ] `INTEGRATION_TEST_PLAN.md` PASS。
- [ ] `E2E_TEST_PLAN.md` PASS。

人工 Reviewer 不得通过手工操作替代缺失的自动化验证层。

## 3. Build / Source Identity

- [ ] 记录 M2 production implementation commit SHA。
- [ ] 记录后续 test-only / docs-only commit（如有）。
- [ ] 确认验收 build 包含 T1–T5 实现。
- [ ] 确认没有未说明的 production source change发生在自动化验证之后。
- [ ] 记录 Portable / executable / Runtime identity。

Evidence：

```text
Production implementation commit:
Automated test/evidence commit:
Final acceptance build:
Reviewer:
Date:
```

## 4. Automated Evidence Review

### 4.1 Development / Unit

- [ ] T1 Persisted Tool Config / Protocol PASS。
- [ ] T2 Runtime Snapshot / Registry / Capability Enforcement PASS。
- [ ] T3 MCP Exposure / Stale-call Rejection PASS。
- [ ] T4 Tools UI Desired / Effective / Restart Required PASS。
- [ ] T5 Regression / Completion PASS。
- [ ] 没有实现 M3 User Policy。
- [ ] 没有实现 M4 Approval。
- [ ] 没有实现 M5 Bash executor。
- [ ] 没有实现 M7 schemaVersion v1→v2 migration pipeline。

### 4.2 Integration Evidence

根据 `INTEGRATION_TEST_PLAN.md`：

- [ ] INT-01 ～ INT-18 全部 PASS。
- [ ] legacy schema v1 missing tools compatibility PASS。
- [ ] tool semantic change正确更新 configRevision。
- [ ] same semantic save不增加 revision。
- [ ] RuntimeConfigSnapshot携带 tools。
- [ ] generation-specific Registry生效。
- [ ] current generation immutable。
- [ ] restart/new generation convergence PASS。
- [ ] disabled absent from MCP tools/list。
- [ ] stale disabled calls被 Runtime拒绝且无 side effect。
- [ ] disabled / policy denied / not-ready / unknown状态可区分。
- [ ] UI desired/effective/restart组合测试 PASS。
- [ ] M1 regression PASS。

### 4.3 Local Black-box E2E Evidence

根据 `E2E_TEST_PLAN.md`：

- [ ] E2E-01 ～ E2E-12 全部 PASS。
- [ ] 使用真实 compiled Runtime executable。
- [ ] 使用真实 stdin/stdout Control RPC。
- [ ] 使用真实 loopback MCP HTTP endpoint。
- [ ] 至少启动多个独立 Runtime generations。
- [ ] active generation不被 later desired save live mutate。
- [ ] restart/new generation应用 saved state。
- [ ] OFF tool不在真实 tools/list。
- [ ] stale cached tools/call被真实 Runtime拒绝。
- [ ] disabled mutation无 filesystem side effect。
- [ ] re-enable在下一 generation恢复。
- [ ] old schema v1 without tools可走 production config path启动。
- [ ] Runtime shutdown/cleanup无 orphan process。

Automated evidence reference：

```text
Integration command/result:
E2E command/result:
Runtime/build identity:
```

## 5. Config / Architecture Review

人工确认：

- [ ] `schemaVersion` 在 M2 仍为 1。
- [ ] old v1 config缺 `tools` 通过 backward-compatible defaults加载。
- [ ] M2 没有偷偷实现半套 v1→v2 migration。
- [ ] ConfigStore仍是 desired state owner。
- [ ] Runtime snapshot仍是 immutable-at-startup。
- [ ] Runtime没有监听 config.json 做 live mutation。
- [ ] ToolRegistry仍是 built-in tool definition / capability single source of truth。
- [ ] UI没有自己复制 authorization logic。
- [ ] active revision / latest revision仍是 Restart Required事实源。

## 6. Tool State Semantics Review

- [ ] default `read_file/glob/write/edit` = ON。
- [ ] persisted `bash` default = OFF，但 M2没有 bash executor/exposure。
- [ ] disabled known tool仍保留 built-in identity/capability metadata。
- [ ] disabled executable capability不可用。
- [ ] disabled effective state = Disabled/UnavailableDisabled 等明确状态。
- [ ] disabled不被错误展示/记录成 Policy DENY。
- [ ] unknown tool与disabled known tool可区分。
- [ ] Runtime NotReady与disabled可区分。

## 7. Manual UI Acceptance

这一节是 M2 最主要的人工产品验收。

### MANUAL-UI-01 — Stopped Harness toggle persistence

前置：Harness stopped。

操作：

1. 打开 Tools 页面。
2. 将 `write` 从 ON切为 OFF。
3. 保存/等待配置保存完成。
4. 离开页面再返回或重启桌面 app（不启动 Harness）。

确认：

- [ ] Desired `write`仍为 OFF。
- [ ] UI没有虚构 active Runtime effective state。
- [ ] 不显示“当前 Runtime 已 disabled”的错误暗示。

### MANUAL-UI-02 — Start applies desired state

从 MANUAL-UI-01 的 `write OFF` 配置启动 Harness。

确认：

- [ ] Desired write = OFF。
- [ ] Effective write = Disabled/UnavailableDisabled。
- [ ] Restart Required = false。
- [ ] 其它 ON工具状态正常。

### MANUAL-UI-03 — Running toggle creates pending restart state

Harness运行时把 `write` 从 OFF切回 ON。

确认：

```text
Desired: ON
Effective: OFF / Disabled
Restart Required: visible
```

- [ ] UI没有立即把 Effective改成 ON。
- [ ] UI没有声称 Runtime live reload成功。
- [ ] 用户能够理解“保存成功，但需重启 Harness生效”。

### MANUAL-UI-04 — Restart converges state

执行正常 Harness restart。

确认：

```text
Desired: ON
Effective: ON / Available (subject to normal policy/readiness)
Restart Required: false
```

### MANUAL-UI-05 — Independent toggles

至少分别改变：

- `read_file`
- `write`

确认：

- [ ] 每个 tool独立保存。
- [ ] 关闭一个工具不会把其它工具错误显示为 OFF。
- [ ] effective list/reason逐工具正确。

### MANUAL-UI-06 — Disabled vs denied/not-running presentation

如果现有测试/开发模式能够展示对应状态，人工确认视觉/文案上：

- [ ] Disabled。
- [ ] Policy denied（现有只读状态即可，不要求 M3 editor）。
- [ ] Runtime not running/not ready。

不会被显示为同一个模糊状态。

### MANUAL-UI-07 — Save failure UX

如可安全构造 config save failure（例如测试环境只读目录/专用 fault setup），确认：

- [ ] UI明确显示 save failure。
- [ ] 不永久保留假的 desired saved state。
- [ ] 不错误显示 Restart Required成功产生。

若正式验收环境不适合人工制造 I/O failure，可依赖 automated evidence并标：

```text
MANUAL_NOT_REPEATED — covered by automated integration/UI tests
```

## 8. Optional External ChatGPT Regression Check

M2 不要求 External ChatGPT作为自动化 E2E 层，也不要求当前会话在 toggle 后瞬时刷新客户端 tool cache。

如果 Reviewer希望做真实用户链路 smoke，可以：

1. 将 `write` OFF并 restart Harness。
2. 新建/刷新 External ChatGPT 会话或工具连接。
3. 观察新的 Runtime generation不再暴露 write。

允许出现：

```text
old ChatGPT session still visually remembers cached tool
```

这本身不构成 M2 failure；真正的安全 gate 是 Runtime stale-call enforcement，已由 automated E2E验证。

若 External ChatGPT真的发送 cached write call，Runtime必须拒绝；可以把该观察作为额外 evidence，但不是必须人工重复的 blocking case。

## 9. Security Boundary Review

- [ ] tool hiding不是唯一 enforcement。
- [ ] stale disabled call在 executor前被拒绝。
- [ ] disabled write/edit没有 filesystem mutation。
- [ ] disabled read/glob没有成功 read/enumeration结果。
- [ ] Tool Toggle不绕过 WorkspaceGuard。
- [ ] Tool Toggle不绕过 PolicyEngine。
- [ ] Tool Toggle不修改 M1 atomic write/edit internals。
- [ ] Tauri/UI没有成为 MCP tool-call proxy。
- [ ] Tunnel职责未改变。
- [ ] Config中没有 raw secret新增泄露。

## 10. Scope Review

确认 M2 没有扩大成后续 milestone：

- [ ] 无 User Policy CRUD/editor。
- [ ] 无真实 ASK/Approval resume。
- [ ] 无 bash executor。
- [ ] 无 same-generation persistent hot reload。
- [ ] 无强制 `tools/list_changed`。
- [ ] 无 schemaVersion v2 migration pipeline。
- [ ] 无 i18n framework。
- [ ] 无 protocol major bump。

## 11. Failure Classification

Manual Acceptance失败时选择真实原因：

```text
REJECTED — IMPLEMENTATION FAILURE
REJECTED — CONFIG / REVISION FAILURE
REJECTED — RUNTIME SNAPSHOT FAILURE
REJECTED — STALE-CALL ENFORCEMENT FAILURE
REJECTED — UI DESIRED/EFFECTIVE FAILURE
REJECTED — INTEGRATION FAILURE
REJECTED — AUTOMATED E2E FAILURE
REJECTED — SECURITY BOUNDARY FAILURE
REJECTED — M1 REGRESSION
BLOCKED — LOCAL TEST ENVIRONMENT
BLOCKED — MANUAL UI ENVIRONMENT
```

## 12. Final Decision

只允许 Human Reviewer选择最终状态：

```text
[ ] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — CONFIG / REVISION FAILURE
[ ] REJECTED — RUNTIME SNAPSHOT FAILURE
[ ] REJECTED — STALE-CALL ENFORCEMENT FAILURE
[ ] REJECTED — UI DESIRED/EFFECTIVE FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — AUTOMATED E2E FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — M1 REGRESSION
[ ] BLOCKED — LOCAL TEST ENVIRONMENT
[ ] BLOCKED — MANUAL UI ENVIRONMENT
```

Reviewer conclusion：

```text

```

## 13. Freeze Condition

M2 只有在 Human Reviewer确认以下全部满足后才能标记：

```text
M2 — ACCEPTED / FROZEN
```

- [ ] Unit/Regression evidence PASS。
- [ ] Integration evidence PASS。
- [ ] Local Black-box E2E evidence PASS。
- [ ] desired/effective/restart-required manual UI UX PASS。
- [ ] stale disabled Runtime enforcement已自动化证明。
- [ ] legacy schema v1 compatibility PASS。
- [ ] M1 frozen behavior无回归。
- [ ] 没有未解决的 M2 blocking defect。
- [ ] 没有未解决的 security boundary regression。

随后才进入：

```text
M3 — Configurable Policy
```
