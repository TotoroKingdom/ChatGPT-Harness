# M1 Manual Acceptance Checklist — Write/Edit Production Hardening

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M1 — Write/Edit Production Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Purpose: final human gate for M1.

## 1. Acceptance Rule

M1 只有在以下证据全部满足后，才允许标记：

```text
M1 — ACCEPTED / FROZEN
```

必须确认：

1. Development Tasks complete。
2. Unit / regression / fault-path checks complete。
3. Runtime Integration PASS。
4. External ChatGPT regression smoke PASS。
5. Security / compatibility boundary review PASS。

M1 不重新证明“ChatGPT 是否具备 write/edit capability”；该事实已经由 M0 冻结。

## 2. Build / Source Identity

- [ ] 记录验收 commit SHA。
- [ ] 确认 build 包含本次 M1 T1–T4 实现。
- [ ] 确认不是 M0 旧 binary。
- [ ] 确认工作区没有未说明的源码变更影响验收。
- [ ] 记录 Runtime / desktop build identity。
- [ ] 使用专用、可丢弃 Workspace。

Evidence:

```text
Commit:
Build:
Workspace:
Reviewer:
Date:
```

## 3. Development Completion Review

根据 `TASKS.md`：

- [ ] T1 Shared Atomic File Mutation Primitive PASS。
- [ ] T2 Production Harden `write` PASS。
- [ ] T3 Production Harden `edit` PASS。
- [ ] T4 Errors / Evidence / M0 Regression PASS。
- [ ] Windows existing-target atomic replace 有真实 automated coverage。
- [ ] atomic fault-injection tests PASS。
- [ ] concurrent edit target-change test PASS。
- [ ] 没有把 M2 Tool Toggle 混入 M1。
- [ ] 没有把 M3 configurable Policy 混入 M1。
- [ ] 没有把 M4 Approval 混入 M1。

## 4. Repository Verification

- [ ] `cargo fmt --all -- --check` PASS。
- [ ] `cargo check --workspace` PASS。
- [ ] `cargo test --workspace` PASS。
- [ ] `git diff --check` PASS。
- [ ] read_file / glob regressions PASS。
- [ ] M0 write/edit contract regressions PASS。
- [ ] `WorkspaceGuard::approve()` existing-only semantics unchanged。
- [ ] `ExecutionPermit` ownership unchanged。
- [ ] MCP adapter 仍无 filesystem bypass。

Evidence / command reference:

```text

```

## 5. Atomic Primitive Acceptance

人工核对实现和测试证据：

- [ ] temp file 与 target 在同一 approved directory。
- [ ] temp 使用 exclusive create，不覆盖已有 temp。
- [ ] temp 写入完整 bytes 后才 commit。
- [ ] commit 前有 target/parent revalidation。
- [ ] existing-target replace 在 Windows 使用真实 atomic replace semantics。
- [ ] 实现不是 `delete target → rename temp`。
- [ ] TempWrite failure 保持 existing target unchanged。
- [ ] TempFlush failure 保持 existing target unchanged。
- [ ] Commit failure 保持 existing target unchanged。
- [ ] failed create 不留下正式 target。
- [ ] success 后无 temp residue。
- [ ] failure 后 temp best-effort cleanup verified。
- [ ] cleanup failure 不覆盖 primary failure。

## 6. `write` Production Acceptance

- [ ] new UTF-8 file atomic create PASS。
- [ ] existing UTF-8 file atomic overwrite PASS。
- [ ] overwrite 不再对 live target truncate。
- [ ] invalid UTF-8 existing target → `UNSUPPORTED_CONTENT` 且 unchanged。
- [ ] missing parent 不自动创建。
- [ ] directory target fail unchanged。
- [ ] outside relative path fail closed。
- [ ] outside absolute path fail closed。
- [ ] target-created race 不被静默 clobber。
- [ ] exact UTF-8 bytes preserved。
- [ ] 不自动添加 newline。
- [ ] 不进行 Unicode normalization。
- [ ] `bytesWritten` 使用 UTF-8 byte length。

## 7. `edit` Production Acceptance

- [ ] exact unique replacement PASS。
- [ ] 0-match fail without mutation。
- [ ] multi-match fail without mutation。
- [ ] invalid UTF-8 fail without mutation。
- [ ] replacement 可增大/缩小/置空。
- [ ] multibyte UTF-8 replacement PASS。
- [ ] live target 不再使用 `fs::write(target, replacement)` 直接覆盖。
- [ ] prepare 后 target bytes 改变 → `TARGET_CHANGED`。
- [ ] `TARGET_CHANGED` 不覆盖 concurrent actor 内容。
- [ ] target 删除/替换 race fail closed。
- [ ] commit failure 不产生 partial edit。

## 8. Integration Acceptance

根据 `INTEGRATION_TEST_PLAN.md`：

- [ ] Integration result = PASS。
- [ ] INT-01 ～ INT-18 已执行并有 evidence。
- [ ] atomic write create/overwrite PASS。
- [ ] atomic edit PASS。
- [ ] TempWrite/TempFlush/Commit failure preservation PASS。
- [ ] failed create no named target PASS。
- [ ] concurrent edit `TARGET_CHANGED` PASS。
- [ ] UTF-8 boundary PASS。
- [ ] Workspace boundary PASS。
- [ ] M0 write/edit/read/glob regression PASS。
- [ ] Activity/Audit confidentiality PASS。

若 Windows symlink/junction fixture 环境不可用：

- [ ] 已明确记录 `ENVIRONMENT_SKIP` 原因。
- [ ] 其它 Windows boundary/atomic tests 仍 PASS。

Integration evidence reference:

```text

```

## 9. External ChatGPT Regression Acceptance

根据 `E2E_TEST_PLAN.md`：

- [ ] E2E-01 External write create PASS。
- [ ] E2E-02 External write overwrite PASS。
- [ ] E2E-03 External exact edit PASS。
- [ ] E2E-04 ambiguous edit无 mutation。
- [ ] E2E-05 outside mutation仍被 Runtime 拒绝。
- [ ] E2E-06 read regression PASS。
- [ ] filesystem observation 已确认，不只看 ChatGPT 文本回复。
- [ ] Runtime Activity/Audit evidence 已确认。

Optional：

- [ ] E2E-07 UTF-8 external smoke PASS / NOT RUN。

E2E evidence reference:

```text

```

## 10. Error / Audit Review

- [ ] `TARGET_CHANGED` external error semantics稳定且 sanitized。
- [ ] staging/flush/commit OS details没有直接泄露给 MCP caller。
- [ ] temp absolute path不进入 MCP error。
- [ ] temp filename/path不进入 Activity/Audit。
- [ ] raw `content` 不进入 Activity/Audit。
- [ ] raw `oldText` / `newText` 不进入 Activity/Audit。
- [ ] outside absolute path不被不必要暴露。
- [ ] success / target-changed / I/O failure / Workspace rejection 可从 evidence 区分。

## 11. Security Boundary Review

人工确认 production hardening 没有通过 atomic helper 新建旁路：

- [ ] WorkspaceGuard 仍是 path authorization authority。
- [ ] atomic helper 不是 authorization authority。
- [ ] atomic helper 只操作已经批准的 target/context。
- [ ] commit 前重新执行/要求 Workspace revalidation。
- [ ] temp 不能 staging 到 Workspace 外 system temp。
- [ ] symlink/junction escape 继续 fail closed。
- [ ] sibling-prefix escape 继续 fail closed。
- [ ] Windows special/root-relative path继续 fail closed。
- [ ] Policy evaluation 仍发生在 ExecutionPipeline。
- [ ] MCP adapter 不直接调用 atomic primitive 绕过 Pipeline。

## 12. Scope / Claim Review

确认 M1 没有超范围或过度承诺：

- [ ] 未声称支持 power-loss durability guarantee。
- [ ] 未声称 parent-directory durable fsync guarantee。
- [ ] 未声称支持 multi-file transaction。
- [ ] 未声称提供 full OS-level sandbox。
- [ ] 未实现 Tool Toggle。
- [ ] 未实现 User Policy Editor。
- [ ] 未实现 Approval。
- [ ] 未实现 Bash。
- [ ] 未实现 i18n。
- [ ] 未引入新的 protocol major。
- [ ] 未修改 M0 历史 acceptance evidence 以适配 M1。

## 13. Compatibility Review

- [ ] MCP tool names仍为 `read_file`, `glob`, `write`, `edit`。
- [ ] write input fields仍为 `path`, `content`。
- [ ] edit input fields仍为 `path`, `oldText`, `newText`。
- [ ] write success core fields兼容 M0。
- [ ] edit success core fields兼容 M0。
- [ ] tool annotations兼容 M0。
- [ ] ToolRegistry order/capabilities兼容 M0。
- [ ] M0 External golden path仍可运行。

## 14. Failure Classification

若验收失败，必须选择真实原因：

```text
REJECTED — IMPLEMENTATION FAILURE
REJECTED — ATOMICITY / FAILURE-SAFETY FAILURE
REJECTED — INTEGRATION FAILURE
REJECTED — SECURITY BOUNDARY FAILURE
REJECTED — M0 REGRESSION
REJECTED — EXTERNAL REGRESSION
BLOCKED — TEST ENVIRONMENT
```

不要使用 `BLOCKED_BY_EXTERNAL_CAPABILITY` 作为 M1 的常规状态；M0 已经冻结证明 External write/edit capability 存在。只有出现新的、可证明的产品侧能力变化时，才单独记录外部依赖变化。

## 15. Final Decision

只允许选择一个：

```text
[ ] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — ATOMICITY / FAILURE-SAFETY FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — M0 REGRESSION
[ ] REJECTED — EXTERNAL REGRESSION
[ ] BLOCKED — TEST ENVIRONMENT
```

Reviewer conclusion:

```text

```

## 16. Freeze Condition

标记 `M1 — ACCEPTED / FROZEN` 前，Reviewer 必须确认：

- [ ] T1–T4 development evidence完整。
- [ ] atomic failure-safety evidence完整。
- [ ] Windows atomic replace evidence完整。
- [ ] Runtime Integration PASS。
- [ ] External regression smoke PASS。
- [ ] M0 frozen contract无回归。
- [ ] 没有未解决的 M1 blocking defect。
- [ ] 没有未解决的 security boundary regression。

满足后才能进入：

```text
M2 — Tool Enable / Disable
```
