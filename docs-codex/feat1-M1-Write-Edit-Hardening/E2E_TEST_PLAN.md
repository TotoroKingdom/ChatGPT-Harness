# M1 E2E Test Plan — External Regression Smoke

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M1 — Write/Edit Production Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Purpose: External ChatGPT regression smoke only. This plan does not re-prove product-side write/edit capability discovery.

## 1. Objective

M0 已经证明真实：

```text
External ChatGPT
→ Tunnel
→ Runtime MCP
→ write / edit
→ Workspace filesystem
```

M1 不重复做 capability gate，而是确认 atomic hardening 后没有破坏已经验收的真实 External ChatGPT 使用路径。

External smoke 只验证用户可观察行为：

- write 仍可创建文件。
- write 仍可 full overwrite。
- edit 仍可 exact replacement。
- Workspace 外 mutation 仍被拒绝。
- Runtime Activity/Audit 仍有对应 evidence。

Atomic failure injection、TARGET_CHANGED race、temp cleanup 等生产化内部语义由 `INTEGRATION_TEST_PLAN.md` 证明，不要求通过真实 ChatGPT 人工制造故障。

## 2. Preconditions

开始 E2E 前必须：

- M1 Integration result = PASS。
- 使用包含 M1 实现的明确 build / commit。
- Harness Runtime Ready。
- Tunnel 已连接当前 Runtime MCP endpoint。
- External ChatGPT 当前连接的是本次验收 Harness，而非旧实例。
- 使用专用 M1 E2E Workspace。

建议 Workspace 初始为空或只包含可丢弃 fixture。

## 3. Evidence Rule

每个 External case 必须至少有两类 evidence：

```text
External ChatGPT session observation
+
filesystem / Runtime Activity-Audit observation
```

不能只依据 ChatGPT 文本回复判断成功。

以下均不能替代本计划：

- MCP Inspector。
- local HTTP request。
- loopback MCP client。
- mock。
- Unit/Integration test。

但这些已经在 M1 Integration 中承担其自己的覆盖范围。

## 4. E2E Cases

### E2E-01 — External write create regression

请求 External ChatGPT 创建：

```text
m1-e2e.txt
```

内容必须完全为：

```text
hello from M1
```

期望：

- ChatGPT 实际发起 Harness `write` tool call。
- Workspace 中真实创建 `m1-e2e.txt`。
- 文件 bytes 对应精确 UTF-8 `hello from M1`。
- 无额外 newline/文本变化。
- Activity/Audit 有对应成功 request evidence。

### E2E-02 — External write full-overwrite regression

前置：E2E-01 已创建文件。

请求 ChatGPT 将 `m1-e2e.txt` 完整覆盖为：

```text
hello from M1 overwrite
```

期望：

- 实际调用 `write`。
- 最终文件完整等于新内容。
- 不残留旧内容。
- Activity/Audit 有成功 evidence。

本 case 只验证 External observable overwrite；atomic failure safety 已由 Integration 负责。

### E2E-03 — External exact edit regression

请求 ChatGPT 精确替换：

```text
hello from M1 overwrite
```

为：

```text
hello from M1 hardened
```

期望：

- 实际调用 `edit`。
- 最终文件完全为 `hello from M1 hardened`。
- 只发生一次预期 replacement。
- Activity/Audit 有成功 evidence。

### E2E-04 — External edit ambiguous match remains safe

先通过正常方式准备一个专用文件：

```text
m1-duplicate.txt = same same
```

请求 ChatGPT 把 `same` 替换为 `changed`。

期望：

- Runtime 返回 edit multi-match failure。
- 文件仍为 `same same`。
- ChatGPT 不应通过 Harness 自动猜测 first match。
- Activity/Audit 有 failure evidence。

如果 ChatGPT 在失败后主动提出其它操作，不属于 Harness 自动 edit 的成功；Reviewer 只验证本次工具调用没有 mutation。

### E2E-05 — External Workspace boundary regression

请求 External ChatGPT 尝试写入 Workspace 外路径。

路径选择应确保请求能到达 Runtime，但不会危害真实数据；使用专门的相邻测试目录。

期望：

- ChatGPT 实际发起 mutation tool call。
- Runtime 返回 `WORKSPACE_VIOLATION` 或当前稳定对应 error。
- Workspace 外目标没有创建/修改。
- error 不泄露不必要绝对路径。
- Activity/Audit 为 Workspace rejection，无成功 execution evidence。

### E2E-06 — External read regression after hardened mutation

请求 ChatGPT 使用 `read_file` 读取最终 `m1-e2e.txt`。

期望：

```text
hello from M1 hardened
```

目的：确认 M1 mutation hardening 没有破坏已有 read path，且真实文件最终状态可由同一 Harness 链路读取。

## 5. Optional External UTF-8 Smoke

建议但不是额外 capability gate：

请求创建或覆盖：

```text
m1-utf8.txt
```

内容：

```text
你好，M1 👋
```

验证 filesystem UTF-8 bytes 和 External read-back 一致。

如果执行，记录为：

```text
E2E-07 — PASS / FAIL
```

## 6. Cases Deliberately Not Tested Through External ChatGPT

以下 M1 acceptance requirement 不应通过人工 External ChatGPT 制造 flaky/risky race：

- TempWrite failure。
- TempFlush failure。
- atomic Commit failure。
- target-created race。
- edit TARGET_CHANGED synchronization race。
- symlink/junction TOCTOU fixture。
- temp cleanup failure。

这些必须由确定性的 Runtime Integration / unit test seam 验证。

External E2E 不扩大其覆盖范围。

## 7. Required Evidence

每个 E2E case 记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED
External ChatGPT user request
Observed tool invoked
Sanitized tool result/error
Filesystem observation
Activity/Audit observation
Build / commit identity
Reviewer / date
```

不要在 evidence 文档中记录 API Key 或其它 secret。

## 8. Exit Criteria

M1 External regression smoke PASS 要求：

- E2E-01 write create PASS。
- E2E-02 write overwrite PASS。
- E2E-03 exact edit PASS。
- E2E-04 ambiguous edit无 mutation。
- E2E-05 outside mutation仍 fail closed。
- E2E-06 read regression PASS。
- 对应 filesystem 与 Activity/Audit evidence 已人工核实。

E2E-07 UTF-8 smoke 为建议项；若执行则必须记录真实结果，不能失败后忽略。

## 9. Failure Interpretation

如果 Integration PASS，但 External regression 失败：

- 先区分 Tunnel/session/stale build/client cache 等环境问题与 Harness regression。
- 如果确认 M1 build 的 write/edit External path 回归，则 M1 不得 ACCEPTED。
- M0 历史 `ACCEPTED / FROZEN` 状态不因此被改写；应记录为 M1 regression defect。

M1 不再使用：

```text
BLOCKED_BY_EXTERNAL_CAPABILITY
```

作为默认结论，因为 M0 已证明该账号/产品链路具备 write/edit capability。只有有新的明确产品侧能力变化证据时，才单独记录 external dependency regression，不修改 M0 历史事实。

## 10. Non-Claims

本 External smoke 不证明：

```text
atomic fault safety
TARGET_CHANGED race protection
power-loss durability
multi-file transaction
M2+ capabilities
V3 final acceptance
```
