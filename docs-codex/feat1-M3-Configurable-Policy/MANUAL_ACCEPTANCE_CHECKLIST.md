# M3 Manual Acceptance Checklist — Configurable Policy

> Status: ACCEPTED / FROZEN
> Milestone: M3 — Configurable Policy  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Owner: Human Reviewer  
> Purpose: the only human-intervention layer for M3.

## 1. Acceptance Responsibility

M3 verification ownership：

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

Manual Acceptance负责：

1. review自动化 evidence；
2. 验证 Policies UI CRUD / desired/effective/restart UX；
3. 检查 System Guardrails不可编辑/覆盖；
4. 检查 ASK在 M3 的产品边界没有被误宣传为“已支持审批”；
5. 做最终 `ACCEPTED / FROZEN` 决策。

M3不要求人工重复 Coding Agent 已完成的低层 policy engine/fault cases。

## 2. Entry Criteria

只有以下全部完成才进入人工验收：

```text
Development = PASS
Unit Tests = PASS
Integration Tests = PASS
Local Black-box E2E = PASS
```

并提供：

- implementation commit/build identity。
- Integration evidence。
- E2E runner command/result。
- final Runtime binary identity。

人工不得用手工点击代替缺失的 Integration/E2E。

## 3. Build / Source Identity

- [x] 记录 M3 production implementation commit。
- [x] 记录最终 acceptance build/commit。
- [x] 确认 M3验证对象包含 T1–T5实现。
- [x] 确认之后若只有测试/文档改动，不改变 production policy semantics。
- [x] 若自动化完成后又改了 policy execution/config/runtime production code，重新评估受影响测试。

Evidence：

```text
Production implementation commit: `b49c7a8031edcfe6fdf39acd8b7fd43be34bb08b`
Automated test/evidence commit: `67b2fb81d5d1f3351d713512358493b0b2f62a85`
Final acceptance build/commit: Existing M3 acceptance build/evidence anchored to the commits above; this closeout changes documentation only.
Reviewer: Human Reviewer (user-confirmed)
Date: 2026-10-02
```

## 4. Automated Evidence Review

### 4.1 Development / Unit

- [x] T1 Policy Config / Protocol / Persistence PASS。
- [x] T2 Immutable Policy Snapshot / Engine PASS。
- [x] T3 Runtime Evidence / Control / MCP PASS。
- [x] T4 Policies UI CRUD / Desired / Effective / Restart PASS。
- [x] T5 Regression PASS。
- [x] no M4 Approval lifecycle introduced。
- [x] no M5 Bash execution introduced。
- [x] no M7 formal schema migration introduced。

### 4.2 Integration

- [x] INT-01 ～ INT-25 PASS。
- [x] old schema v1 missing policies defaults PASS。
- [x] CRUD persistence/revision PASS。
- [x] Tool Default fallback PASS。
- [x] `DENY > ASK > ALLOW` PASS。
- [x] System Guardrail cannot be overridden PASS。
- [x] invalid enabled rule invalidates whole snapshot PASS。
- [x] deterministic matched rule IDs/reason PASS。
- [x] ASK no-execution transition PASS。
- [x] desired/effective/restart UI integration PASS。
- [x] M0–M2 regression PASS。

### 4.3 Local Black-box E2E

- [x] E2E-01 ～ E2E-14 PASS。
- [x] actual compiled Runtime subprocess used。
- [x] real stdin/stdout Control RPC used。
- [x] real loopback MCP HTTP used。
- [x] multiple immutable policy generations tested。
- [x] Tool Default DENY through real MCP PASS。
- [x] explicit User ALLOW override of default fallback PASS。
- [x] DENY/ASK/ALLOW real execution outcomes PASS。
- [x] invalid snapshot fail closed PASS。
- [x] System Guardrail override attempt rejected。
- [x] M2 disabled capability remains authoritative。
- [x] Activity confidentiality PASS。
- [x] clean shutdown PASS。

Automated evidence references：

```text
Integration command/result: Existing `m3_config_integration_cases` + `m3_runtime_integration_cases` evidence — PASS (INT-01–INT-25); retained and not re-executed during final closeout.
E2E command/result: Existing `m3_local_black_box_e2e` evidence — PASS (E2E-01–E2E-14; `LOCAL_M3_BLACK_BOX_E2E=PASS`); retained and not re-executed during final closeout.
Build identity: Implementation `b49c7a8031edcfe6fdf39acd8b7fd43be34bb08b`; test/evidence `67b2fb81d5d1f3351d713512358493b0b2f62a85`.
```

## 5. Policy Model Review

人工从最终实现/自动 evidence确认：

- [x] System Guardrails与User Policy分层。
- [x] System Rules不来自可编辑 AppConfig userRules。
- [x] System Rules不能被编辑/禁用/删除。
- [x] User Rules使用 typed matcher，不继续把自由字符串 scope当authoritative user matcher。
- [x] Tool matcher identity由 Runtime tool registry验证。
- [x] Tool Default只在没有 explicit matched User Rule时fallback。
- [x] Tool Default不参与 explicit User Rule severity competition。
- [x] matched User Rules按 `DENY > ASK > ALLOW`。
- [x] duplicate enabled rule ID fail closed。
- [x] enabled invalid matcher fail closed。
- [x] User ALLOW不能绕过 WorkspaceGuard / disabled capability / unsupported capability。

## 6. Policies UI Manual Acceptance

### MANUAL-UI-01 — Tool Defaults are editable desired config

在 Harness stopped状态打开 Policies。

验证：

- [x] `read_file/glob/write/edit` Tool Default可选择 ALLOW/ASK/DENY。
- [x] bash default可按设计展示 ASK，但页面不会声称 bash已可执行。
- [x] 保存后重新进入页面，desired值持久化。
- [x] UI没有把 Tool Default展示成 System Rule。

### MANUAL-UI-02 — Create User Rule

创建例如：

```text
Name: Deny write
Matcher: tool = write
Decision: DENY
Reason: manual acceptance
Enabled: true
```

验证：

- [x] rule成功保存。
- [x] rule显示 source=user / editable。
- [x] reload后仍存在。
- [x] configRevision发生 semantic increment（可通过 UI/诊断/evidence确认）。

### MANUAL-UI-03 — Edit / Enable / Disable / Delete User Rule

对同一 user rule依次：

- edit name/reason/decision。
- disable。
- enable。
- delete。

验证每一步保存和reload行为符合预期，无 phantom state。

- [x] 每一步保存和 reload 行为符合预期，无 phantom state。

### MANUAL-UI-04 — System Rules are visibly read-only

验证 System Rules：

- [x] 无 Edit。
- [x] 无 Delete。
- [x] 无 Enable/Disable toggle。
- [x] source/read-only状态清晰。

不得仅靠“按钮点了没反应”实现安全；UI与Runtime ownership都应是只读。

### MANUAL-UI-05 — Running desired/effective mismatch

准备当前 Runtime generation：

```text
write effective = ALLOW
revision = N
```

运行中将 desired write policy改为 DENY并保存。

验证 UI：

```text
Desired = DENY
Effective = ALLOW
Restart Required = visible
```

- [x] current effective没有被假装立即改变。
- [x] refresh effective observation后 desired仍保持 DENY。

### MANUAL-UI-06 — Restart convergence

Restart Harness。

验证：

```text
Desired = DENY
Effective = DENY
Restart Required = false
```

并确认新的 Runtime generation/revision已生效。

- [x] 新的 Runtime generation/revision已生效。

### MANUAL-UI-07 — ASK is not presented as completed Approval feature

将某个 current file tool policy设为 ASK并restart。

验证 UI/文案：

- [x] effective decision明确为 ASK。
- [x] 不显示“approval workflow is ready/approved”等误导信息。
- [x] 如调用该工具，M3行为是需要 approval / 不执行。
- [x] Approval Center完整 pending/resume能力仍明确属于 M4。

### MANUAL-UI-08 — Invalid policy error is understandable

使用测试/受控方式制造 Runtime可识别的 invalid policy snapshot（不要破坏正式用户数据）。

验证：

- [x] UI/diagnostics能显示 policy invalid/fail-closed状态。
- [x] 不静默显示 ALLOW。
- [x] remediation指向修正 policy config，而不是建议绕过安全检查。

## 7. Optional External ChatGPT Regression Smoke

M3的正式自动化E2E不依赖 External ChatGPT。

如果本次 production改动触及 MCP exposure/policy behavior较多，可人工做一个最小真实用户链 regression smoke：

```text
External ChatGPT
→ Tunnel
→ Runtime MCP
→ policy-controlled tool
```

建议只验证：

### MANUAL-EXT-01 — DENY

- 配置 `write = DENY` 并restart。
- External ChatGPT尝试 write。
- 验证文件不变 + Activity有 DENY evidence。

### MANUAL-EXT-02 — ALLOW

- 配置 write ALLOW并restart。
- External ChatGPT执行简单 Workspace write/read-back。
- 验证成功。

这两个 case是 regression smoke，不替代 M8 Full V3 External Acceptance。

若项目当前验收流程不需要 M3 External smoke，可标：

```text
NOT_REQUIRED_FOR_M3
```

并以 Local Black-box E2E + UI Manual Acceptance作为 M3 gate。

本次 M3 记录：`NOT_REQUIRED_FOR_M3`。本次冻结以既有 Local Black-box E2E evidence + UI Manual Acceptance 为 gate，不新增 External ChatGPT acceptance claim。

## 8. Evidence / Confidentiality Review

- [x] effective decision可观察。
- [x] matched rule IDs可观察。
- [x] decision reason可观察。
- [x] Activity能关联 request/tool/policy result。
- [x] DENY/ASK不存在 ExecutionCompleted success。
- [x] raw mutation content不进入Activity/Audit。
- [x] secrets不进入policy config/evidence。
- [x] 不泄露无必要绝对路径。

## 9. Compatibility / Scope Review

- [x] M2 Tool Toggle仍正常。
- [x] disabled tool不能被 User ALLOW重新启用。
- [x] M1 atomic write/edit正常。
- [x] WorkspaceGuard semantics不变。
- [x] ExecutionPermit仍只能由Pipeline创建。
- [x] no protocol major bump。
- [x] schemaVersion仍按M3 transitional contract处理；未偷跑M7 migration。
- [x] no pending approvals/session grants。
- [x] no bash executor/command policy。
- [x] no policy hot reload。

## 10. Failure Classification

如验收失败，选择真实分类：

```text
REJECTED — POLICY CONFIG FAILURE
REJECTED — POLICY PRECEDENCE FAILURE
REJECTED — SYSTEM GUARDRAIL FAILURE
REJECTED — FAIL-CLOSED FAILURE
REJECTED — RUNTIME EVIDENCE FAILURE
REJECTED — UI DESIRED/EFFECTIVE FAILURE
REJECTED — AUTOMATED E2E FAILURE
REJECTED — M0/M1/M2 REGRESSION
BLOCKED — LOCAL TEST ENVIRONMENT
BLOCKED — MANUAL ACCEPTANCE ENVIRONMENT
```

## 11. Final Decision

只有 Human Reviewer可选择：

```text
[x] ACCEPTED / FROZEN
[ ] REJECTED — POLICY CONFIG FAILURE
[ ] REJECTED — POLICY PRECEDENCE FAILURE
[ ] REJECTED — SYSTEM GUARDRAIL FAILURE
[ ] REJECTED — FAIL-CLOSED FAILURE
[ ] REJECTED — RUNTIME EVIDENCE FAILURE
[ ] REJECTED — UI DESIRED/EFFECTIVE FAILURE
[ ] REJECTED — AUTOMATED E2E FAILURE
[ ] REJECTED — M0/M1/M2 REGRESSION
[ ] BLOCKED — LOCAL TEST ENVIRONMENT
[ ] BLOCKED — MANUAL ACCEPTANCE ENVIRONMENT
```

Reviewer conclusion：

```text
Manual Acceptance = PASS。Human Reviewer 于 2026-10-02 确认 M3 所有必需人工验收项通过；既有 Integration / Local Black-box E2E PASS evidence 已保留，未在本次收口中重新执行。未发现阻塞 M3 冻结的缺陷或安全回归。
```

## 12. Freeze Condition

M3可标记：

```text
M3 — ACCEPTED / FROZEN
```

仅当 Human Reviewer确认：

- [x] Unit PASS。
- [x] Integration PASS。
- [x] Local Black-box E2E PASS。
- [x] Tool Default fallback语义正确。
- [x] User Rule severity正确。
- [x] System Guardrails不可覆盖。
- [x] invalid enabled policy fail closed。
- [x] effective decision / matched IDs / reason evidence完整。
- [x] Policies UI CRUD + desired/effective/restart UX通过。
- [x] ASK边界未越权实现/宣传M4能力。
- [x] M0–M2无阻塞回归。
- [x] 无未解决的 M3 security blocker。

然后才能进入：

```text
M4 — Human Approval Workflow
```

## 13. Final Acceptance Evidence

```text
Manual Acceptance: PASS
Final status: M3 — ACCEPTED / FROZEN
Date: 2026-10-02
Production implementation commit: b49c7a8031edcfe6fdf39acd8b7fd43be34bb08b
Test/evidence commit: 67b2fb81d5d1f3351d713512358493b0b2f62a85
Integration/E2E evidence: retained; not re-executed during closeout
```

Non-blocking observation: 本次人工验收观察到 `read_file=ASK` 行为差异。该 observation 不影响 M3 Manual Acceptance PASS 或冻结结论；本次不修改 M3 已验收业务实现，也不在此继续修复。
