# M0 Manual Acceptance Checklist — Write/Edit Capability Gate

> Status: DRAFT / READY FOR REVIEW  
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

- [ ] 记录验收 commit SHA。
- [ ] 确认验收代码来自计划中的 M0 实现分支/commit，而不是旧 binary。
- [ ] 确认工作区没有会影响验收的未说明源码修改。
- [ ] 记录 Runtime / desktop build 版本或可执行文件身份。
- [ ] 测试使用专用 Workspace，不包含重要用户数据。

Evidence:

```text
Commit:
Build:
Workspace:
Reviewer:
Date:
```

## 3. Development Completion Review

根据 `TASKS.md` 核对：

- [ ] T1 WorkspaceWrite Capability / Registry / Policy PASS。
- [ ] T2 WriteTarget Workspace Validation PASS。
- [ ] T3 Runtime `write` PASS。
- [ ] T4 Runtime `edit` PASS。
- [ ] T5 MCP Adapters / Errors / Evidence PASS。
- [ ] 每个 Task 有对应 Unit Test 或明确自动化验证。
- [ ] 未混入 Tool Toggle / Approval / Bash / i18n。
- [ ] 未把 M1 atomic hardening 冒充 M0 必需实现。

## 4. Repository Verification

- [ ] `cargo fmt --all -- --check` PASS。
- [ ] `cargo check --workspace` PASS。
- [ ] `cargo test --workspace` PASS。
- [ ] `git diff --check` PASS。
- [ ] Existing read/glob tests PASS。
- [ ] `WorkspaceGuard::approve()` 对 missing existing-target 仍 fail closed。
- [ ] 没有 MCP handler → filesystem 的 authorization bypass。
- [ ] `ExecutionPermit` 仍只能由 Pipeline 创建。

Evidence / command output reference:

```text

```

## 5. Integration Acceptance

根据 `INTEGRATION_TEST_PLAN.md` 核对：

- [ ] Integration result = PASS。
- [ ] Ready `tools/list` 包含 `read_file`, `glob`, `write`, `edit`。
- [ ] NotReady mutation 被拒绝。
- [ ] `write` create PASS。
- [ ] `write` full overwrite PASS。
- [ ] `edit` unique exact replacement PASS。
- [ ] edit zero-match 不修改文件。
- [ ] edit multi-match 不修改文件。
- [ ] relative outside write 被 Runtime 拒绝。
- [ ] absolute outside write 被 Runtime 拒绝。
- [ ] missing parent 不被自动创建。
- [ ] malformed arguments 不产生 mutation。
- [ ] unsupported edit content 不产生 mutation。
- [ ] read/glob regression PASS。
- [ ] Audit/Activity 不包含 raw mutation content。
- [ ] Boundary rejection 没有 ExecutionStarted / ExecutionCompleted success evidence。

若 Windows symlink/junction case 因环境无法创建 fixture：

- [ ] 已明确记录 `ENVIRONMENT_SKIP` 原因。
- [ ] 其它 Windows path-form boundary tests 仍 PASS。

Integration evidence reference:

```text

```

## 6. External ChatGPT E2E Acceptance

根据 `E2E_TEST_PLAN.md`，必须人工观察真实 External ChatGPT：

### Tool discovery

- [ ] External ChatGPT 能发现/调用 `write`。
- [ ] External ChatGPT 能发现/调用 `edit`。

### Golden write

- [ ] 请求 ChatGPT 创建 `v3-write-test.txt`。
- [ ] ChatGPT 实际发起 `write` tool call。
- [ ] 文件由 Harness 真实创建，不是人工预先创建。
- [ ] 文件内容完全为 `hello from ChatGPT`。
- [ ] Runtime Activity/Audit 有对应成功 evidence。

### Golden edit

- [ ] 请求把 `hello from ChatGPT` 精确替换成 `hello from V3`。
- [ ] ChatGPT 实际发起 `edit` tool call。
- [ ] 最终文件内容完全为 `hello from V3`。
- [ ] 只发生预期一次 exact replacement。
- [ ] Runtime Activity/Audit 有对应成功 evidence。

### Boundary rejection

- [ ] External ChatGPT 发起一个真实到达 Runtime 的 Workspace 外 mutation 请求。
- [ ] Runtime 拒绝该请求。
- [ ] Workspace 外目标没有创建/修改。
- [ ] Runtime evidence 为 Workspace validation failure / rejection。
- [ ] 没有成功 execution evidence。
- [ ] 返回给 ChatGPT 的错误不泄露不必要绝对路径。

### Read regression smoke

- [ ] External ChatGPT 能读取最终 `v3-write-test.txt`。
- [ ] 读取结果为 `hello from V3`。

E2E evidence reference:

```text

```

## 7. Security Boundary Review

人工确认实现没有通过“跑通功能”削弱 V2 边界：

- [ ] `write` missing-target support 使用独立 write-target validation。
- [ ] Existing-path validation 未被改成允许 missing path。
- [ ] Workspace 外 relative/absolute path 均 fail closed。
- [ ] Windows special/root-relative path semantics 未弱化。
- [ ] Tool Adapter 只负责 schema/deserialize/result mapping。
- [ ] Workspace authorization 仍由 Runtime enforcement 完成。
- [ ] Policy 仍在 ExecutionPipeline 中执行。
- [ ] `write` / `edit` 不通过 Tauri 代理普通 MCP tool call。
- [ ] raw `content` / `oldText` / `newText` 不进入 Audit。
- [ ] Secret 不进入 MCP result、Activity 或 Audit。

## 8. Scope Review

确认 M0 没有无意扩大范围：

- [ ] 未实现 Policy Editor。
- [ ] 未实现 Approval Center / real ASK resume。
- [ ] 未实现 Tool Toggle config。
- [ ] 未实现 Bash。
- [ ] 未实现 i18n。
- [ ] 未引入新的 protocol major。
- [ ] 未改变 Tauri / Tunnel ownership。
- [ ] 未引入新的 execution path。
- [ ] 未声称 M0 已具备 M1 的 production atomicity guarantee。

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
[ ] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — EXTERNAL E2E FAILURE
[ ] BLOCKED_BY_EXTERNAL_CAPABILITY
```

Reviewer conclusion:

```text

```

## 11. Freeze Condition

勾选 `ACCEPTED / FROZEN` 前，Reviewer 必须确认：

- [ ] Development / Unit / Integration / External E2E 全部满足真实覆盖范围。
- [ ] 没有使用 mock、MCP Inspector、loopback 或 local HTTP 替代 External ChatGPT E2E。
- [ ] 所有 M0 Exit Criteria 都有可追溯 evidence。
- [ ] 没有未解决的 M0 blocking defect。
- [ ] 没有未解决的 security boundary regression。

满足后：

```text
M0 — ACCEPTED / FROZEN
```

随后才允许进入 M1 — Write/Edit Production Hardening。
