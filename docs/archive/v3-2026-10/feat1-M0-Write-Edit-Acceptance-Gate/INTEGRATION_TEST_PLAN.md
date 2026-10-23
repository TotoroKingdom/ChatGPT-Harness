# M0 Integration Test Plan — Write/Edit Capability

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M0 — Write/Edit Capability Acceptance Gate  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: Runtime + MCP local integration only; this document does **not** prove External ChatGPT acceptance.

## 1. Objective

验证 T1–T5 组合后，`write` / `edit` 已通过现有 Runtime enforcement chain 成为真实 MCP capability，而不是若干彼此孤立的单元实现。

Integration 重点验证：

```text
MCP schema / routing
→ ExecutionPipeline
→ Workspace validation
→ Policy
→ ExecutionPermit
→ write/edit executor
→ Activity/Audit
→ filesystem result
```

Integration PASS 不能替代真实 ChatGPT + Tunnel E2E。

## 2. Entry Criteria

执行本计划前必须满足：

- T1–T5 开发完成。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- Runtime 可用临时 Workspace 初始化并进入 Ready。
- 测试不得依赖生产 Tunnel、真实 ChatGPT 或用户真实 Workspace。

若开发任务未完成，本计划状态为 `BLOCKED_BY_IMPLEMENTATION`，不得通过降低断言继续测试。

## 3. Test Environment

使用独立临时目录构造：

```text
base/
├ workspace/
│  ├ existing.txt
│  ├ duplicate.txt
│  └ nested/
└ outside/
   └ outside.txt
```

建议初始内容：

```text
workspace/existing.txt = "hello from ChatGPT\n"
workspace/duplicate.txt = "same same"
outside/outside.txt = "must not change"
```

Runtime 使用 Normal Mode、现有 M0 minimal policy baseline，并完成 acknowledgement 后再执行 Ready-path 测试。

每个 case 必须在测试后检查 filesystem 实际状态，不只检查 MCP 返回值。

## 4. Required Integration Cases

### INT-01 — Ready catalog exposes the four M0 tools

步骤：

1. 初始化并 acknowledge Runtime。
2. 通过 MCP `tools/list` 获取 catalog。
3. 检查顺序与 metadata。

期望：

```text
read_file
 glob
 write
 edit
```

- `write` / `edit` 均存在。
- `write` / `edit`: `readOnly=false`, `destructive=true`, `openWorld=false`。
- 不出现 `bash` 或其他 V3 未实现工具。

### INT-02 — Runtime NotReady prevents mutation

步骤：

1. Runtime initialize，但不 acknowledge。
2. 调用 MCP `write` 创建 `not-ready.txt`。

期望：

- 返回 `RUNTIME_NOT_READY`。
- 文件不存在。
- 不出现 execution success evidence。

### INT-03 — MCP write creates a new workspace file

调用：

```json
{"path":"created.txt","content":"hello from ChatGPT"}
```

期望：

- MCP success。
- `workspace/created.txt` 真实存在。
- 内容完全等于 `hello from ChatGPT`。
- structured result 至少包含相对 `normalizedPath`、正确 `bytesWritten`、`created=true`、`overwritten=false`。
- 返回值不泄露 Workspace 绝对路径。
- Activity/Audit 能关联该 `write` request 并看到 Workspace validation、Policy ALLOW、ExecutionCompleted。

### INT-04 — MCP write overwrites an existing regular file

调用 `write` 覆盖 `existing.txt`。

期望：

- 文件完整内容等于新 content。
- structured result：`created=false`, `overwritten=true`。
- 同一次请求只有一次成功执行 lifecycle。

说明：M0 只验证 full overwrite capability；atomic replacement 属于 M1，不在本 case 中声明。

### INT-05 — MCP edit performs one exact replacement

前置：

```text
existing.txt = hello from ChatGPT
```

调用：

```json
{
  "path":"existing.txt",
  "oldText":"hello from ChatGPT",
  "newText":"hello from V3"
}
```

期望：

- MCP success。
- 文件最终内容为 `hello from V3`。
- structured result `replacements=1`。
- 没有 fuzzy / approximate behavior。
- Activity/Audit 显示成功执行。

### INT-06 — Edit zero-match fails without mutation

前置记录目标文件完整 bytes。

调用不存在的 `oldText`。

期望：

- 返回 `EDIT_MATCH_NOT_FOUND`。
- 文件 bytes 与调用前完全相同。
- 不出现 ExecutionCompleted success evidence。

### INT-07 — Edit multiple-match fails without mutation

前置：

```text
duplicate.txt = same same
```

调用 `oldText="same"`。

期望：

- 返回 `EDIT_MATCH_NOT_UNIQUE`。
- 文件 bytes 完全不变。
- 不进行 first-match guessing。

### INT-08 — Outside-workspace write is rejected before execution

至少分别测试：

- 相对 escape：`../outside/escape.txt`
- outside absolute path

期望：

- 返回 `WORKSPACE_VIOLATION`。
- Workspace 外没有新文件/变化。
- error payload 不包含 outside absolute path 或 Workspace root。
- Audit 存在 Workspace validation failure。
- 对该 request 不得出现 `ExecutionStarted` / `ExecutionCompleted`。

### INT-09 — Write target requires an existing approved parent

调用：

```json
{"path":"missing-parent/file.txt","content":"x"}
```

其中 `missing-parent/` 不存在。

期望：

- 调用失败。
- Runtime 不递归创建目录。
- Workspace 中不产生部分文件。

### INT-10 — Existing-path semantics remain fail closed

在同一个 Runtime snapshot 中验证：

- `read_file` missing target 仍失败。
- `edit` missing target 仍失败。
- `glob` existing-base semantics不因 WriteTarget 支持被弱化。

目的：证明 `approve_write_target()` 没有改变现有 `WorkspaceGuard::approve()` 的 security semantics。

### INT-11 — Existing read/glob regression

验证至少：

- `read_file` 正常读取 existing UTF-8 file。
- `glob` 正常匹配 Workspace 内文件。
- Workspace 外 read/glob 继续被拒绝。

期望：V2 已验收 read-only capability 无回归。

### INT-12 — Malformed MCP arguments fail without mutation

至少覆盖：

- write 缺 `path`
- write 缺 `content`
- write 非 string path/content
- edit 缺任一字段
- edit `oldText=""`
- unknown fields（若 schema/serde contract 禁止）

期望：

- 返回 `INVALID_ARGUMENTS` 或 SPEC 对应固定错误。
- filesystem 无变化。

### INT-13 — Unsupported edit content fails safely

对 invalid UTF-8 existing file 调用 `edit`。

期望：

- 返回 `UNSUPPORTED_CONTENT`。
- 原文件 bytes 不变。
- error sanitized。

### INT-14 — Audit payload does not contain mutation content

执行含唯一 marker 的 write/edit，例如：

```text
M0_SECRET_SENTINEL_DO_NOT_AUDIT
```

检查 Activity/Audit projection。

期望：

- 可观察 request ID、tool、workspace validation、policy decision、outcome。
- 不包含 raw `content`、`oldText`、`newText`。
- 不包含 Workspace 外绝对路径。

## 5. Windows Boundary Cases

在 Windows 可创建相应 fixture 时，至少验证：

- unsupported special path form 继续被拒绝。
- root-relative path 继续被拒绝。
- existing outside symlink/junction target 被拒绝。
- missing write target 的 parent 如果经 symlink/junction 解析到 Workspace 外，则被拒绝。

如果 CI/测试账户无法创建 symlink/junction，记录为 `ENVIRONMENT_SKIP`，但 Windows path-form tests 仍必须执行；不得把环境跳过记为安全 case PASS。

## 6. Evidence Required

每个 case 记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED / ENVIRONMENT_SKIP
MCP request category
MCP structured result/error code
Filesystem before/after assertion
Relevant Activity/Audit assertion
Test command / test name
```

不得记录真实 secret、完整用户目录或不必要的绝对路径。

## 7. Integration Exit Criteria

Integration Test Plan 只有在以下全部满足时为 `PASS`：

- INT-01 ～ INT-14 全部 PASS；允许明确标注的 Windows symlink/junction `ENVIRONMENT_SKIP`，但不得掩盖其它 boundary failure。
- write create 与 overwrite 均产生正确 filesystem result。
- exact edit 成功；0-match / multi-match 均无 mutation。
- outside Workspace mutation 始终被 Runtime 拒绝。
- existing read/glob semantics 无回归。
- Activity/Audit evidence 存在且不泄露 mutation content。
- 完整 workspace regression test suite PASS。

Integration PASS 后仍必须执行 `E2E_TEST_PLAN.md` 和 `MANUAL_ACCEPTANCE_CHECKLIST.md`。

## 8. Non-Claims

本计划通过只能声明：

> M0 Runtime/MCP local integration is verified.

不得声明：

```text
External ChatGPT PASS
M0 ACCEPTED / FROZEN
V3 write capability externally available
```
