# M0 Manual Acceptance Checklist — Write/Edit Capability Gate

> Status: ACCEPTED / FROZEN
> Milestone: M0 — Write/Edit Capability Acceptance Gate  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Purpose: final human gate. This checklist does not replace tests; it verifies their evidence and determines Milestone status.

## 1. Acceptance Rule

M0 是 V3 的 blocking gate。

只有在以下四类证据全部满足后，才允许标记：

```text
M0 — ACCEPTED / FROZEN
```

四类证据：

1. Development Tasks complete。
2. Unit / workspace regression checks complete。
3. Integration Test Plan PASS。
4. External ChatGPT E2E PASS。

任何 local-only evidence 均不能替代第 4 项。

## 2. Build / Source Identity

- [x] 记录验收 commit SHA。
- [x] 确认验收代码来自计划中的 M0 实现分支/commit，而不是旧 binary。
- [x] 确认工作区没有会影响验收的未说明源码修改。
- [x] 记录 Runtime / desktop build 版本或可执行文件身份。
- [x] 测试使用专用 Workspace，不包含重要用户数据。

Evidence:

```text
Commit: d750e5cf3e92e3bfbf7996094f5fa79064266a35 (working tree contains documented test/evidence changes)
Build: target/release/bundle/portable/ChatGPT-Harness_M0-write-edit-pass_DEV_2026-09-29
Workspace: C:\AI\M0-E2E-Workspace-20260929
Reviewer: External ChatGPT operator / Codex filesystem and Activity/Audit evidence review
Date: 2026-09-29
```

## 3. Development Completion Review

根据 `TASKS.md` 核对：

- [x] T1 WorkspaceWrite Capability / Registry / Policy PASS。
- [x] T2 WriteTarget Workspace Validation PASS。
- [x] T3 Runtime `write` PASS。
- [x] T4 Runtime `edit` PASS。
- [x] T5 MCP Adapters / Errors / Evidence PASS。
- [x] 每个 Task 有对应 Unit Test 或明确自动化验证。
- [x] 未混入 Tool Toggle / Approval / Bash / i18n。
- [x] 未把 M1 atomic hardening 冒充 M0 必需实现。

## 4. Repository Verification

- [x] `cargo fmt --all -- --check` PASS。
- [x] `cargo check --workspace` PASS。
- [x] `cargo test --workspace` PASS。
- [x] `git diff --check` PASS。
- [x] Existing read/glob tests PASS。
- [x] `WorkspaceGuard::approve()` 对 missing existing-target 仍 fail closed。
- [x] 没有 MCP handler → filesystem 的 authorization bypass。
- [x] `ExecutionPermit` 仍只能由 Pipeline 创建。

Evidence / command output reference:

```text
Integration evidence: docs-codex/feat1-M0-Write-Edit-Acceptance-Gate/INTEGRATION_EVIDENCE.md
Workspace test summary: Runtime 183, UI 57, Protocol 9 passed.
```

## 5. Integration Acceptance

根据 `INTEGRATION_TEST_PLAN.md` 核对：

- [x] Integration result = PASS。
- [x] Ready `tools/list` 包含 `read_file`, `glob`, `write`, `edit`。
- [x] NotReady mutation 被拒绝。
- [x] `write` create PASS。
- [x] `write` full overwrite PASS。
- [x] `edit` unique exact replacement PASS。
- [x] edit zero-match 不修改文件。
- [x] edit multi-match 不修改文件。
- [x] relative outside write 被 Runtime 拒绝。
- [x] absolute outside write 被 Runtime 拒绝。
- [x] missing parent 不被自动创建。
- [x] malformed arguments 不产生 mutation。
- [x] unsupported edit content 不产生 mutation。
- [x] read/glob regression PASS。
- [x] Audit/Activity 不包含 raw mutation content。
- [x] Boundary rejection 没有 ExecutionStarted / ExecutionCompleted success evidence。

若 Windows symlink/junction case 因环境无法创建 fixture：

- [x] 已明确记录 `ENVIRONMENT_SKIP` 原因。
- [x] 其它 Windows path-form boundary tests 仍 PASS。

Integration evidence reference:

```text
Integration evidence: docs-codex/feat1-M0-Write-Edit-Acceptance-Gate/INTEGRATION_EVIDENCE.md
```

## 6. External ChatGPT E2E Acceptance

根据 `E2E_TEST_PLAN.md`，必须人工观察真实 External ChatGPT：

### Tool discovery

- [x] External ChatGPT 能发现/调用 `write`。
- [x] External ChatGPT 能发现/调用 `edit`。

### Golden write

- [x] 请求 ChatGPT 创建 `v3-write-test.txt`。
- [x] ChatGPT 实际发起 `write` tool call（用户报告）。
- [x] 文件由 Harness 真实创建，不是人工预先创建。
- [x] 文件内容完全为 `hello from ChatGPT`。
- [x] Runtime Activity/Audit 有对应成功 evidence。

### Golden edit

- [x] 请求把 `hello from ChatGPT` 精确替换成 `hello from V3`。
- [x] ChatGPT 实际发起 `edit` tool call（用户报告）。
- [x] 最终文件内容完全为 `hello from V3`。
- [x] 只发生预期一次 exact replacement。
- [x] Runtime Activity/Audit 有对应成功 evidence。

### Boundary rejection

- [x] External ChatGPT 发起一个真实到达 Runtime 的 Workspace 外 mutation 请求（用户报告）。
- [x] Runtime 返回 `WORKSPACE_VIOLATION`。
- [x] Workspace 外目标没有创建/修改。
- [x] Runtime evidence 为 Workspace validation failure / rejection。
- [x] 没有成功 execution evidence。
- [x] 返回给 ChatGPT 的错误不泄露不必要绝对路径。

### Read regression smoke

- [x] External ChatGPT 能读取最终 `v3-write-test.txt`（用户报告）。
- [x] 读取结果为 `hello from V3`。

E2E evidence reference:

```text
E2E evidence: docs-codex/feat1-M0-Write-Edit-Acceptance-Gate/E2E_EVIDENCE.md
E2E-01/02/03/04/05 External ChatGPT session, filesystem, and Runtime Activity/Audit evidence confirmed.
```

## 7. Security Boundary Review

人工确认实现没有通过“跑通功能”削弱 V2 边界：

- [x] `write` missing-target support 使用独立 write-target validation。
- [x] Existing-path validation 未被改成允许 missing path。
- [x] Workspace 外 relative/absolute path 均 fail closed。
- [x] Windows special/root-relative path semantics 未弱化。
- [x] Tool Adapter 只负责 schema/deserialize/result mapping。
- [x] Workspace authorization 仍由 Runtime enforcement 完成。
- [x] Policy 仍在 ExecutionPipeline 中执行。
- [x] `write` / `edit` 不通过 Tauri 代理普通 MCP tool call。
- [x] raw `content` / `oldText` / `newText` 不进入 Audit。
- [x] Secret 不进入 MCP result、Activity 或 Audit。

## 8. Scope Review

确认 M0 没有无意扩大范围：

- [x] 未实现 Policy Editor。
- [x] 未实现 Approval Center / real ASK resume。
- [x] 未实现 Tool Toggle config。
- [x] 未实现 Bash。
- [x] 未实现 i18n。
- [x] 未引入新的 protocol major。
- [x] 未改变 Tauri / Tunnel ownership。
- [x] 未引入新的 execution path。
- [x] 未声称 M0 已具备 M1 的 production atomicity guarantee。

## 9. External Capability Block Procedure

如果 External ChatGPT 产品当前不能调用 write/modify MCP action：

- [ ] 已确认 local Integration PASS，排除明显 Harness implementation failure。
- [ ] 已确认 Tunnel connection / credentials 正常。
- [ ] 已记录 ChatGPT product-side capability limitation evidence。
- [ ] Milestone 状态设置为：

```text
BLOCKED_BY_EXTERNAL_CAPABILITY
```

- [ ] 未标记 `PASS`。
- [ ] 未标记 `ACCEPTED / FROZEN`。
- [ ] 未进入依赖 M0 PASS 的 M1+ 开发。

## 10. Final Decision

只允许选择一个状态：

```text
[x] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — EXTERNAL E2E FAILURE
[ ] BLOCKED_BY_EXTERNAL_CAPABILITY
```

Reviewer conclusion:

```text
Integration 与 External ChatGPT E2E-01～E2E-05 全部完成；真实 filesystem、External ChatGPT session
以及 Runtime Activity/Audit evidence 已由 reviewer 确认。M0 满足冻结条件。
```

## 11. Freeze Condition

勾选 `ACCEPTED / FROZEN` 前，Reviewer 必须确认：

- [x] Development / Unit / Integration / External E2E 全部满足真实覆盖范围。
- [x] 没有使用 mock、MCP Inspector、loopback 或 local HTTP 替代 External ChatGPT E2E。
- [x] 所有 M0 Exit Criteria 都有可追溯 evidence。
- [x] 没有未解决的 M0 blocking defect。
- [x] 没有未解决的 security boundary regression。

满足后：

```text
M0 — ACCEPTED / FROZEN
```

随后才允许进入 M1 — Write/Edit Production Hardening。
