# M2 Manual Acceptance Checklist — Tool Enable / Disable

> Status: ACCEPTED / FROZEN
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

- [x] T1–T5 development complete。
- [x] `cargo fmt --all -- --check` PASS。
- [x] `cargo check --workspace` PASS。
- [x] `cargo test --workspace` PASS。
- [x] frontend tests/typecheck/build PASS（按仓库命令）。
- [x] `git diff --check` PASS。
- [x] `INTEGRATION_TEST_PLAN.md` PASS。
- [x] `E2E_TEST_PLAN.md` PASS。

人工 Reviewer 不得通过手工操作替代缺失的自动化验证层。

## 3. Build / Source Identity

- [x] 记录 M2 production implementation commit SHA。
- [x] 记录后续 test-only / docs-only commit（如有）。
- [x] 确认验收 build 包含 T1–T5 实现。
- [x] 确认没有未说明的 production source change发生在自动化验证之后。
- [x] 记录 Portable / executable / Runtime identity。

Evidence：

```text
Production implementation commit: ceb91cc36ef71e38a489357580af3cd8bcd9a495
Automated test/evidence commit: aafc63fe86d857245b483e4b1613daa8d92edd13
Final acceptance build: `target-exe/distribution` built from `aafc63fe86d857245b483e4b1613daa8d92edd13`; UI SHA-256 `79702B7634FBDA8DAE9C48C8D7B2994E8DCFF9F6AA1DC46AECAD5BAC836FCCCB`; Runtime SHA-256 `E75EFCBD0D8D6086CC534CE7C425A3234260064D478DAFF41226C7B9816EE8E5`
Reviewer: Human Reviewer (user-confirmed)
Date: 2026-10-02
```

## 4. Automated Evidence Review

### 4.1 Development / Unit

- [x] T1 Persisted Tool Config / Protocol PASS。
- [x] T2 Runtime Snapshot / Registry / Capability Enforcement PASS。
- [x] T3 MCP Exposure / Stale-call Rejection PASS。
- [x] T4 Tools UI Desired / Effective / Restart Required PASS。
- [x] T5 Regression / Completion PASS。
- [x] 没有实现 M3 User Policy。
- [x] 没有实现 M4 Approval。
- [x] 没有实现 M5 Bash executor。
- [x] 没有实现 M7 schemaVersion v1→v2 migration pipeline。

### 4.2 Integration Evidence

根据 `INTEGRATION_TEST_PLAN.md`：

- [x] INT-01 ～ INT-18 全部 PASS。
- [x] legacy schema v1 missing tools compatibility PASS。
- [x] tool semantic change正确更新 configRevision。
- [x] same semantic save不增加 revision。
- [x] RuntimeConfigSnapshot携带 tools。
- [x] generation-specific Registry生效。
- [x] current generation immutable。
- [x] restart/new generation convergence PASS。
- [x] disabled absent from MCP tools/list。
- [x] stale disabled calls被 Runtime拒绝且无 side effect。
- [x] disabled / policy denied / not-ready / unknown状态可区分。
- [x] UI desired/effective/restart组合测试 PASS。
- [x] M1 regression PASS。

### 4.3 Local Black-box E2E Evidence

根据 `E2E_TEST_PLAN.md`：

- [x] E2E-01 ～ E2E-12 全部 PASS。
- [x] 使用真实 compiled Runtime executable。
- [x] 使用真实 stdin/stdout Control RPC。
- [x] 使用真实 loopback MCP HTTP endpoint。
- [x] 至少启动多个独立 Runtime generations。
- [x] active generation不被 later desired save live mutate。
- [x] restart/new generation应用 saved state。
- [x] OFF tool不在真实 tools/list。
- [x] stale cached tools/call被真实 Runtime拒绝。
- [x] disabled mutation无 filesystem side effect。
- [x] re-enable在下一 generation恢复。
- [x] old schema v1 without tools可走 production config path启动。
- [x] Runtime shutdown/cleanup无 orphan process。

Automated evidence reference：

```text
Integration command/result: Existing M2 Integration evidence — PASS (INT-01–INT-18); retained from the prior run and not re-executed during closeout.
E2E command/result: Existing M2 Local Black-box E2E evidence — PASS (E2E-01–E2E-12); retained from the prior run and not re-executed during closeout.
Runtime/build identity: See Section 3; evidence/test implementation is anchored at `aafc63fe86d857245b483e4b1613daa8d92edd13`.
```

## 5. Config / Architecture Review

人工确认：

- [x] `schemaVersion` 在 M2 仍为 1。
- [x] old v1 config缺 `tools` 通过 backward-compatible defaults加载。
- [x] M2 没有偷偷实现半套 v1→v2 migration。
- [x] ConfigStore仍是 desired state owner。
- [x] Runtime snapshot仍是 immutable-at-startup。
- [x] Runtime没有监听 config.json 做 live mutation。
- [x] ToolRegistry仍是 built-in tool definition / capability single source of truth。
- [x] UI没有自己复制 authorization logic。
- [x] active revision / latest revision仍是 Restart Required事实源。

## 6. Tool State Semantics Review

- [x] default `read_file/glob/write/edit` = ON。
- [x] persisted `bash` default = OFF，但 M2没有 bash executor/exposure。
- [x] disabled known tool仍保留 built-in identity/capability metadata。
- [x] disabled executable capability不可用。
- [x] disabled effective state = Disabled/UnavailableDisabled 等明确状态。
- [x] disabled不被错误展示/记录成 Policy DENY。
- [x] unknown tool与disabled known tool可区分。
- [x] Runtime NotReady与disabled可区分。

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

- [x] Desired `write`仍为 OFF。
- [x] UI没有虚构 active Runtime effective state。
- [x] 不显示“当前 Runtime 已 disabled”的错误暗示。

### MANUAL-UI-02 — Start applies desired state

从 MANUAL-UI-01 的 `write OFF` 配置启动 Harness。

确认：

- [x] Desired write = OFF。
- [x] Effective write = Disabled/UnavailableDisabled。
- [x] Restart Required = false。
- [x] 其它 ON工具状态正常。

### MANUAL-UI-03 — Running toggle creates restart-required state

Harness运行时把 `write` 从 OFF切回 ON。

确认：

```text
Desired: ON
Effective: OFF / Disabled
Restart Required: visible
```

- [x] UI没有立即把 Effective改成 ON。
- [x] UI没有声称 Runtime live reload成功。
- [x] 用户能够理解“保存成功，但需重启 Harness生效”。

### MANUAL-UI-04 — Restart converges state

执行正常 Harness restart。

确认：

```text
Desired: ON
Effective: ON / Available (subject to normal policy/readiness)
Restart Required: false
```

- [x] Desired write = ON。
- [x] Effective write = ON / Available（subject to normal policy/readiness）。
- [x] Restart Required = false。

### MANUAL-UI-05 — Independent toggles

至少分别改变：

- `read_file`
- `write`

确认：

- [x] 每个 tool独立保存。
- [x] 关闭一个工具不会把其它工具错误显示为 OFF。
- [x] effective list/reason逐工具正确。

### MANUAL-UI-06 — Disabled vs denied/not-running presentation

如果现有测试/开发模式能够展示对应状态，人工确认视觉/文案上：

- [x] Disabled。
- [x] Policy denied（现有只读状态即可，不要求 M3 editor）。
- [x] Runtime not running/not ready。

不会被显示为同一个模糊状态。

### MANUAL-UI-07 — Save failure UX

如可安全构造 config save failure（例如测试环境只读目录/专用 fault setup），确认：

- [x] UI明确显示 save failure。
- [x] 不永久保留假的 desired saved state。
- [x] 不错误显示 Restart Required成功产生。

正式验收记录：Save failure UX 已由人工验收确认，且 automated evidence 已保留。

```text
MANUAL-UI-07 = PASS — human confirmation plus retained automated reconciliation evidence.
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

若 External ChatGPT真的发送 cached write call，Runtime必须拒绝；可以把该观察作为额外 evidence，但不是必须人工重复的 gating case。

## 9. Security Boundary Review

- [x] tool hiding不是唯一 enforcement。
- [x] stale disabled call在 executor前被拒绝。
- [x] disabled write/edit没有 filesystem mutation。
- [x] disabled read/glob没有成功 read/enumeration结果。
- [x] Tool Toggle不绕过 WorkspaceGuard。
- [x] Tool Toggle不绕过 PolicyEngine。
- [x] Tool Toggle不修改 M1 atomic write/edit internals。
- [x] Tauri/UI没有成为 MCP tool-call proxy。
- [x] Tunnel职责未改变。
- [x] Config中没有 raw secret新增泄露。

## 10. Scope Review

确认 M2 没有扩大成后续 milestone：

- [x] 无 User Policy CRUD/editor。
- [x] 无真实 ASK/Approval resume。
- [x] 无 bash executor。
- [x] 无 same-generation persistent hot reload。
- [x] 无强制 `tools/list_changed`。
- [x] 无 schemaVersion v2 migration pipeline。
- [x] 无 i18n framework。
- [x] 无 protocol major bump。

## 11. Failure Classification

本次 Manual Acceptance 结果为 PASS，以下拒绝分类均未选择：

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
```

## 12. Final Decision

只允许 Human Reviewer选择最终状态：

```text
[x] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — CONFIG / REVISION FAILURE
[ ] REJECTED — RUNTIME SNAPSHOT FAILURE
[ ] REJECTED — STALE-CALL ENFORCEMENT FAILURE
[ ] REJECTED — UI DESIRED/EFFECTIVE FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — AUTOMATED E2E FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — M1 REGRESSION
```

Reviewer conclusion：

```text
Human Reviewer confirmed that all required M2 Manual Acceptance cases passed on 2026-10-02. Desired/effective/restart-required UI behavior, independent toggles, disabled-vs-denied/not-running presentation, and save-failure UX were accepted. Existing Integration and Local Black-box E2E PASS evidence was retained without re-execution. No unresolved M2 defect or security regression remains.
```

## 13. Freeze Condition

M2 只有在 Human Reviewer确认以下全部满足后才能标记：

```text
M2 — ACCEPTED / FROZEN
```

- [x] Unit/Regression evidence PASS。
- [x] Integration evidence PASS。
- [x] Local Black-box E2E evidence PASS。
- [x] desired/effective/restart-required manual UI UX PASS。
- [x] stale disabled Runtime enforcement已自动化证明。
- [x] legacy schema v1 compatibility PASS。
- [x] M1 frozen behavior无回归。
- [x] 没有未解决的 M2 defect。
- [x] 没有未解决的 security boundary regression。

下一 milestone 仍为（仅规划，不在本次提交中实现）：

```text
M3 — Configurable Policy (PLANNED / NOT IMPLEMENTED)
```

## 14. Final Acceptance Evidence

2026-10-02，Human Reviewer 已人工完成并确认 M2 Manual Acceptance 全部通过。MANUAL-UI-01～MANUAL-UI-07 均为 PASS；External ChatGPT / Tunnel smoke 仅作为用户链路补充观察，不改变 Runtime stale-call enforcement 的自动化事实源。M2 最终状态为：

```text
M2 — ACCEPTED / FROZEN
```
