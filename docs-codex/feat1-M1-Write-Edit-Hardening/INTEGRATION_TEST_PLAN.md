# M1 Integration Test Plan — Write/Edit Production Hardening

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M1 — Write/Edit Production Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Scope: Runtime + MCP local integration; validates production hardening, not External ChatGPT capability discovery.

## 1. Objective

验证 T1–T4 组合后，`write` / `edit` 已从 M0 的非原子 mutation 升级为正式的 atomic text-file mutation path，并且没有破坏 M0 已冻结的 Runtime enforcement chain。

Integration 必须同时验证：

```text
MCP request
→ ExecutionPipeline
→ WorkspaceGuard
→ Policy
→ ExecutionPermit
→ write/edit executor
→ shared atomic file primitive
→ Activity/Audit
→ filesystem result
```

重点不是再次证明“工具能调用”，而是证明：

- success 只产生完整最终文件。
- staging / flush / commit failure 不破坏旧文件。
- failed create 不留下正式 partial target。
- concurrent edit 不静默覆盖其它 actor 修改。
- UTF-8 text boundary 生效。
- M0 write/edit/read/glob contract 无回归。

## 2. Entry Criteria

执行本计划前必须满足：

- T1–T4 开发完成。
- Windows existing-target atomic replace 自动化测试已存在。
- atomic fault injection unit tests 已存在。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- `git diff --check` PASS。
- Runtime 可使用临时 Workspace 初始化并进入 Ready。

若开发任务未完成，状态为：

```text
BLOCKED_BY_IMPLEMENTATION
```

不得通过降低断言继续验收。

## 3. Test Environment

使用专用临时目录，例如：

```text
base/
├ workspace/
│  ├ existing.txt
│  ├ edit.txt
│  ├ duplicate.txt
│  ├ utf8.txt
│  ├ binary.bin
│  └ nested/
└ outside/
   └ outside.txt
```

建议 fixture：

```text
existing.txt  = old content
edit.txt      = hello from ChatGPT
multiple.txt  = same same
utf8.txt      = 你好, ChatGPT 👋
binary.bin    = invalid UTF-8 bytes
outside.txt   = must not change
```

所有 filesystem assertion 必须直接验证最终 bytes，而不只检查 MCP success/error。

## 4. Required Integration Cases

### INT-01 — Ready MCP contract remains M0-compatible

通过 Ready Runtime 的 `tools/list` 验证：

```text
read_file
glob
write
edit
```

期望：

- 工具顺序不变。
- write/edit schema field names 不变。
- write/edit annotations 不变：`readOnly=false`, `destructive=true`, `openWorld=false`。
- 不出现 M2+ 工具状态/config 行为。

### INT-02 — Atomic write create

调用：

```json
{"path":"created.txt","content":"hello from M1"}
```

期望：

- success。
- target 最终 bytes 完全等于 UTF-8 `hello from M1`。
- `created=true`, `overwritten=false`。
- Workspace 内无 atomic temp residue。
- Activity/Audit 为成功 lifecycle。

### INT-03 — Atomic write overwrite

前置：

```text
existing.txt = old content
```

调用 write full overwrite。

期望：

- success。
- 最终文件只包含完整新内容。
- `created=false`, `overwritten=true`。
- 不存在 target truncate 后 partial/new-old mixed state 的最终结果。
- 无 temp residue。

### INT-04 — Write preserves exact UTF-8 byte semantics

分别测试：

- multibyte UTF-8。
- empty content。
- content without trailing newline。
- content with CRLF / LF supplied by caller。
- UTF-8 BOM supplied explicitly in String content where representable。

期望：

- Harness 不自动修改 newline。
- 不自动追加 newline。
- 不进行 Unicode normalization。
- `bytesWritten` 等于最终 UTF-8 bytes length。

### INT-05 — Existing binary overwrite is rejected

对 invalid UTF-8 `binary.bin` 调用 write overwrite。

期望：

- `UNSUPPORTED_CONTENT`。
- 原 bytes 完全不变。
- 不残留 temp。
- Audit 不包含原 binary/content。

### INT-06 — Atomic exact edit success

前置：

```text
edit.txt = hello from ChatGPT
```

调用：

```json
{
  "path":"edit.txt",
  "oldText":"hello from ChatGPT",
  "newText":"hello from V3"
}
```

期望：

- success。
- 最终 bytes 完全等于 `hello from V3`。
- `replacements=1`。
- 无 temp residue。

### INT-07 — Edit 0-match / multi-match remain non-mutating

分别执行：

```text
0 match
>1 match
```

期望：

- `EDIT_MATCH_NOT_FOUND` / `EDIT_MATCH_NOT_UNIQUE`。
- 原文件 bytes 完全不变。
- 无 temp residue。

### INT-08 — Edit invalid UTF-8 remains non-mutating

对 invalid UTF-8 file 调用 edit。

期望：

- `UNSUPPORTED_CONTENT`。
- 原 bytes 完全不变。
- 无 temp residue。

### INT-09 — Concurrent edit source change returns TARGET_CHANGED

必须使用稳定的 test hook / synchronization seam 构造：

```text
Runtime reads original
→ replacement prepared/staged
→ external test actor changes target
→ Runtime pre-commit check
```

期望：

- 返回 `TARGET_CHANGED`。
- Runtime 不提交旧 replacement。
- 最终 target 保留 external actor 的内容。
- staged temp 被 cleanup。
- Activity/Audit 可区分 target-changed rejection。

不得通过 race + sleep 作为唯一验证方式。

### INT-10 — Edit target removed/replaced before commit fails closed

分别模拟：

- source 在 prepare 后被删除。
- source 在 prepare 后被换成另一个 filesystem object / 不同 bytes。

期望：

- invocation fail closed。
- 不把旧 replacement 提交到新的对象上。
- 不出现错误对象被覆盖。
- temp cleanup。

### INT-11 — Atomic failure preserves existing target

通过 production test seam 分别注入：

```text
TempWrite failure
TempFlush failure
Commit failure
```

对 existing target 执行 write 和/或 edit 集成路径。

期望：

- MCP 返回 sanitized `IO_FAILURE` / `EXECUTION_FAILED`（按实现稳定映射）。
- existing target bytes 与调用前完全相同。
- 无成功 execution evidence。
- 无 temp residue（cleanup failure 专门 case 除外）。

### INT-12 — Failed create leaves no named target

对 missing target 注入 pre-commit failure。

期望：

- 正式 target 不存在。
- 不存在 partial `created.txt`。
- staging temp 被 cleanup。

### INT-13 — Target-created race does not silently clobber

构造：

```text
write observes target missing
→ stages temp
→ external actor creates target
→ write commit
```

期望：

- Runtime 不静默覆盖 external actor 创建的文件。
- 返回确定 failure 或 SPEC 允许的明确 branch behavior。
- 最终内容必须与所声明 branch 一致且可解释。

### INT-14 — Workspace boundary survives atomic staging

至少覆盖：

- relative outside write。
- absolute outside write。
- outside edit。
- sibling-prefix escape。
- missing parent。
- Windows unsupported/root-relative path form。

期望：

- 全部 fail closed。
- Workspace 外无 mutation。
- atomic temp 不得出现在 Workspace 外 staging directory。
- error sanitized。

### INT-15 — Symlink/junction boundary revalidation

Windows 环境允许时，构造 target/parent 在 authorization 后发生 symlink/junction-sensitive 变化。

期望：

- pre-commit revalidation 能阻止 Workspace escape。
- Workspace 外无 mutation。

若环境无法创建 fixture，可标为 `ENVIRONMENT_SKIP`，但必须记录原因，且其它 Windows boundary tests 仍需 PASS。

### INT-16 — M0 write/edit MCP regression

重新执行 M0 核心 local golden paths：

- write create。
- write overwrite。
- edit exact replacement。
- outside write reject。
- outside edit reject。
- malformed args。
- error sanitization。

期望：M1 hardening 不改变外部 core contract。

### INT-17 — read_file / glob regression

验证：

- read_file normal path。
- glob normal path。
- outside read/glob rejection。
- existing-target WorkspaceGuard semantics 未被 atomic helper 改写。

### INT-18 — Audit confidentiality and failure categories

使用唯一 sentinel content 执行 success、TARGET_CHANGED、commit failure。

期望 Activity/Audit：

- 能关联 request/tool/policy/outcome。
- 能区分 success / target changed / atomic I/O failure。
- 不包含 raw `content` / `oldText` / `newText`。
- 不包含 temp filename/absolute temp path。
- 不包含不必要 Workspace root / outside absolute path。

## 5. Fault-Injection Requirement

M1 的 failure safety 不允许仅靠代码 review 推断。

Integration harness 必须能稳定触发至少：

```text
TempWrite
TempFlush
Commit
Concurrent target change
```

fault seam 必须：

- 仅 Runtime internal/test 可用。
- production default path 无 fault behavior。
- 不成为 MCP/Tauri 用户可控接口。
- 不复制第二套 atomic production implementation。

## 6. Windows Atomic Replace Verification

因为产品是 Windows-first，必须至少有一项运行在 Windows 上、真实触发 existing-target replace 的 integration/automated evidence。

验证不能只证明：

```text
mock commit called
```

而要证明：

```text
existing target old bytes
→ replace operation
→ existing target new bytes
```

并确认实现不是：

```text
delete target
→ rename temp
```

如果测试只能从外部可观察行为证明 atomic replace，而无法直接观察瞬时窗口，则还必须配合源码/primitive-level test evidence确认使用的 Windows API/primitive 语义。

## 7. Evidence Required

每个 INT case 记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED / ENVIRONMENT_SKIP
Test command / test name
MCP result/error code
Filesystem before/after bytes assertion
Temp residue assertion
Activity/Audit assertion where applicable
```

对 fault case 还应记录 injected phase，但不得记录真实 temp absolute path。

## 8. Integration Exit Criteria

M1 Integration 只有在以下全部满足时为 `PASS`：

- INT-01 ～ INT-18 PASS；仅允许明确记录的 symlink/junction `ENVIRONMENT_SKIP`。
- Windows existing-target atomic replace 有真实自动化 evidence。
- atomic write create/overwrite PASS。
- atomic edit PASS。
- TempWrite/TempFlush/Commit failure 均证明 existing target unchanged。
- failed create 不留下正式 target。
- concurrent edit 返回 `TARGET_CHANGED` 且不覆盖 external modification。
- UTF-8 boundary PASS。
- Workspace boundary PASS。
- M0 write/edit/read/glob regression PASS。
- Activity/Audit 无 mutation content/temp-path leakage。

Integration PASS 后仍需执行 `E2E_TEST_PLAN.md` regression smoke 和 `MANUAL_ACCEPTANCE_CHECKLIST.md`，但 M1 不重新定义 M0 External capability gate。

## 9. Non-Claims

M1 Integration PASS 不代表：

```text
power-loss durable fsync guarantee
multi-file transaction guarantee
OS sandbox guarantee
M2 Tool Toggle complete
M3 Policy complete
V3 ACCEPTED / FROZEN
```
