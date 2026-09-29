# M1 SPEC — Write/Edit Production Hardening

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M1 — Write/Edit Production Hardening  
> Baseline: M0 — ACCEPTED / FROZEN  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, `../feat1-M0-Write-Edit-Acceptance-Gate/*`

## 1. Objective

M1 将 M0 已经通过 External ChatGPT 验收的 `write` / `edit` 从“可用链路能力”升级为正式 V3 文件修改能力。

M1 不重新设计执行链路。所有调用继续沿 M0 冻结路径：

```text
MCP Adapter
→ ExecutionPipeline
→ WorkspaceGuard
→ PolicyEngine
→ ExecutionPermit
→ Executor
→ Activity/Audit
```

M1 的核心改动是把当前非原子文件 mutation：

```text
write: OpenOptions + truncate + write_all
edit:  fs::write(full replacement)
```

升级为共同的受控 atomic file mutation primitive。

## 2. Baseline Facts

M0 已验收并冻结：

- External ChatGPT 能发现并调用 `write` / `edit`。
- `write` 支持 create / full overwrite。
- `edit` 支持 exact single-match replacement。
- `WorkspaceTarget::WriteTarget` 与 existing-path boundary 已生效。
- Workspace 外 mutation fail closed。
- write/edit 均经过统一 ExecutionPipeline / Policy / Audit。
- `ExecutionPermit` 不可由 MCP adapter 绕过。

当前需要 M1 hardening 的事实：

- `write` 对 existing file 使用 truncate 后直接写入；中途失败可能损坏旧内容。
- `edit` 计算 replacement 后直接整文件覆盖；中途失败可能损坏旧内容。
- `write` 的 String input 保证新内容是 UTF-8，但 existing target 尚未形成正式 text-only boundary。
- atomic commit failure、temporary file cleanup、concurrent edit target change 尚未冻结为正式语义。

## 3. Scope

M1 实现：

1. Runtime 内共享 atomic text-file replacement primitive。
2. `write` create / overwrite 使用 atomic commit。
3. `edit` exact replacement 使用 atomic commit。
4. 正式 UTF-8 text boundary。
5. edit concurrent modification protection。
6. atomic failure cleanup。
7. 明确且 sanitized 的 Runtime/MCP error semantics。
8. 对 atomic path、failure path、Workspace boundary 和 M0 regression 的自动化验证。

M1 不实现：

- Tool enable / disable。
- AppConfig v2。
- configurable User Policy。
- ASK / Approval。
- Bash。
- i18n。
- multi-file transaction。
- filesystem snapshot / rollback system。
- full OS-level sandbox。
- power-loss durability guarantee。
- same-generation config hot reload。

## 4. Atomicity Contract

### 4.1 Required mutation sequence

正式 `write` / `edit` mutation 必须遵循：

```text
validate request
→ Workspace authorization
→ prepare final UTF-8 bytes
→ create unique temporary file in target directory
→ write all bytes to temporary file
→ flush temporary file
→ revalidate target / parent boundary
→ atomic commit (rename / replace)
→ best-effort cleanup on failure
```

Temporary file 必须与 target 位于同一已批准目录，以避免跨 filesystem move 破坏 atomic rename/replace 语义。

### 4.2 Observable guarantee

M1 的 atomicity guarantee 是单个目标文件的 operation-level guarantee：

- commit 成功后，目标表现为完整新内容。
- commit 前失败时，existing target 保持完整旧内容。
- create 失败时，不得留下“成功命名的部分目标文件”。
- 不允许正式实现继续使用 `truncate target → write target`。

M1 不声明：

- 断电后数据一定落盘。
- parent-directory metadata 已执行 durable fsync。
- 多个文件共同事务提交。
- 任意恶意本机进程竞争下具备 handle-level sandbox guarantee。

### 4.3 Platform implementation

Windows-first 实现必须使用能够满足 existing-target atomic replacement 的同-volume primitive。

实现层可选择：

- 标准库 primitive（仅当已证明其 Windows overwrite/replace 语义满足本 SPEC）；或
- 最小 Windows platform API wrapper / dependency。

禁止因为 API 方便而退回：

```text
delete target
→ rename temp
```

因为该序列存在 target temporarily missing window，不满足 M1 replace contract。

## 5. Temporary File Contract

Atomic mutation helper 必须：

- 在 approved target 的 parent 内创建 temp。
- 使用不可预测/唯一 suffix。
- 使用 `create_new` 或等价 exclusive-create，不能覆盖已有 temp file。
- temp filename 不成为 MCP/API contract。
- temp 内容写完并 flush 后才允许 commit。
- failure/cancellation 后 best-effort 删除本 invocation 创建的 temp。
- cleanup failure 不得覆盖主要 execution error，但可进入内部 diagnostic/evidence。
- success 后不得残留 temp file。

不得使用 Workspace 外系统 temp directory 作为 atomic replace staging area。

## 6. `write` Production Contract

Input 保持 M0 schema：

```json
{"path":"relative/path.txt","content":"full text content"}
```

### 6.1 Create

若 target missing：

- parent 必须已存在并通过 `approve_write_target()`。
- 将 UTF-8 content 写入 same-directory temp。
- commit 为 atomic create/rename。
- 不递归创建目录。
- 若 commit 前 target 被其它进程创建，必须 fail closed 或使用明确、可测试的 overwrite path；不能静默绕过 revalidation。

### 6.2 Full overwrite

若 target existing：

- target 必须是 regular file。
- target 必须属于 text boundary（见第 8 节）。
- 不直接 truncate target。
- temp 完整写入后 atomic replace existing target。
- success result 继续报告：

```text
normalizedPath
bytesWritten
created=false
overwritten=true
```

### 6.3 Exact byte semantics

`content: String` 编码为 UTF-8 bytes 后原样写入：

- 不自动修改换行符。
- 不自动追加 trailing newline。
- 不自动添加或移除 BOM。
- 不进行 Unicode normalization。

`bytesWritten` 是最终 UTF-8 byte length，不是 Unicode scalar/character count。

## 7. `edit` Production Contract

Input 保持 M0 schema：

```json
{"path":"relative/path.txt","oldText":"old","newText":"new"}
```

继续冻结：

```text
match count = 0  → fail
match count = 1  → replace
match count > 1  → fail
```

不 fuzzy / regex / approximate / silent first-match。

### 7.1 Atomic edit

`edit` 必须：

1. authorize existing target。
2. 读取完整 original bytes。
3. 验证 UTF-8。
4. 验证唯一 exact match。
5. 在内存生成 replacement bytes。
6. 写入 same-directory temp 并 flush。
7. commit 前重新验证 target。
8. atomic replace。

不得把 replacement bytes 直接写回已打开的 target file。

### 7.2 Concurrent modification protection

`edit` 是基于读取到的 original content 做精确修改，因此不得无条件覆盖在 prepare 与 commit 之间已经变化的文件。

commit 前必须验证 target 仍对应本次 edit 所读取的版本。实现可采用可靠的内容/identity check；最低要求是：

```text
current bytes != original bytes
→ fail TARGET_CHANGED
→ 不 commit replacement
```

这样可以避免把其它进程的并发修改静默覆盖掉。

`write` 是显式 full overwrite，不要求同样的 original-content compare，但仍必须在 commit 前重新通过 Workspace target boundary validation。

## 8. UTF-8 Text Boundary

`write` / `edit` 都是文本工具，不是 arbitrary binary writer。

正式边界：

### New file write

`content: String` → valid UTF-8，允许创建。

### Existing file write overwrite

existing regular file 必须能被读取为 valid UTF-8；否则：

```text
UNSUPPORTED_CONTENT
```

且原文件不变。

### Edit

existing regular file 必须 valid UTF-8；否则：

```text
UNSUPPORTED_CONTENT
```

且原文件不变。

UTF-8 BOM 本身属于合法 UTF-8 bytes；Harness 不自动插入/移除 BOM。

## 9. Workspace / Revalidation Contract

M1 不修改 M0 冻结的 Workspace authorization model：

- existing `WorkspaceGuard::approve()` 继续 existing-only。
- `approve_write_target()` 继续负责 create/overwrite target authorization。
- temporary file 只能创建在已经批准的 target parent 内。
- commit 前必须重新验证 target/parent boundary。
- outside/sibling-prefix/unsupported Windows path form/symlink-junction escape 继续 fail closed。

Atomic helper 不是新的 authorization layer；它只能消费已授权 mutation context，并在 commit 前调用/要求 revalidation。

## 10. Error Semantics

保留既有 MCP stable codes，并增加 M1 必需区分。

至少覆盖：

```text
INVALID_ARGUMENTS
WORKSPACE_VIOLATION
TARGET_NOT_FOUND
TARGET_NOT_FILE
UNSUPPORTED_CONTENT
EDIT_MATCH_NOT_FOUND
EDIT_MATCH_NOT_UNIQUE
TARGET_CHANGED
IO_FAILURE
RUNTIME_NOT_READY
POLICY_DENIED
EXECUTION_FAILED
```

内部 I/O error 必须区分阶段，至少可诊断：

```text
TempCreate
TempWrite
TempFlush
Commit
Cleanup
ReadSource
```

MCP 不需要把 OS path / raw OS error 全量返回给调用方；外部仍使用 sanitized code + safe message。

`Cleanup` failure 不得把已经成功 commit 的 operation 改报失败；如果主 operation 已失败，cleanup failure 也不能覆盖主错误。

## 11. Activity / Audit

沿现有 evidence model。

M1 至少应能区分：

- execution success / failure。
- sanitized failure category。
- atomic preparation/commit failure（无需暴露 temp path）。
- `TARGET_CHANGED` concurrency rejection。

不得记录：

- raw `content`。
- raw `oldText` / `newText`。
- temp absolute path。
- Workspace 外绝对路径。

## 12. Expected Code Touchpoints

优先集中在 Runtime：

```text
runtime/src/tools/write.rs
runtime/src/tools/edit.rs
runtime/src/tools/mod.rs
runtime/src/execution/*                # only if shared mutation context needs additive seam
runtime/src/workspace/*                # only for revalidation helpers; do not weaken guard
runtime/src/mcp/errors.rs
runtime/src/mcp/write.rs
runtime/src/mcp/edit.rs
runtime/src/state.rs                    # only if typed seams/errors require additive change
runtime/Cargo.toml                      # only if Windows atomic replace requires dependency
```

建议新增共享模块，例如：

```text
runtime/src/tools/atomic_file.rs
```

命名可由实现选择，但 write/edit 不应各自复制一套 atomic temp/commit logic。

Protocol major、Tauri、React UI、Tunnel、AppConfig 不属于 M1 正常 touchpoint。

## 13. Compatibility

M1 必须保持 M0 external contract 稳定：

- MCP tool names 不变。
- input field names 不变。
- success result core fields 不变。
- tool annotations 不变。
- ToolRegistry order/capability 不变。
- ExecutionPipeline authorization path 不变。
- M0 write/edit/read/glob golden paths继续通过。

允许新增 sanitized error code `TARGET_CHANGED`，属于 additive behavior。

## 14. Development Verification Requirements

每个 Task 自带 unit/code-level tests。

最终开发检查至少：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

M1 还必须专门具备 fault-path tests，证明：

- temp write/flush/commit failure 不损坏 existing target。
- failed create 不留下正式 target。
- temp cleanup 正常。
- concurrent edit target change 不被静默覆盖。

这些仍不是独立 Runtime Integration Test Plan。

## 15. Post-TASK Verification Documents

按仓库流程，`SPEC / PLAN / TASKS` 冻结后另行生成：

```text
Integration Test Plan
E2E Test Plan（仅在 M1 需要 external smoke 时；不重复 M0 capability gate）
Manual Acceptance Checklist
```

M1 Roadmap 的强制 Exit Criteria 是 automated normal/error/boundary coverage + Runtime Integration。

因此 M1 最终至少必须完成独立 Integration 验证；External ChatGPT 可做 regression smoke，但不重新作为“产品 capability 是否存在”的 blocking proof。

## 16. Final Acceptance Criteria

M1 可标记 `ACCEPTED / FROZEN` 前必须证明：

- write create 使用 atomic staging + commit。
- write overwrite 不再 truncate live target。
- edit 使用 atomic replacement。
- atomic failure 不破坏 existing target。
- failed create 不留下 partial named target。
- write/edit 正式限制在 UTF-8 text boundary。
- edit 0/multi-match 保持无 mutation。
- concurrent edit source change 被拒绝，不静默 clobber。
- temp file success/failure cleanup verified。
- Workspace boundary 无回归。
- Runtime/MCP errors sanitized 且可区分关键 failure。
- M0 read/glob/write/edit regression PASS。
- Runtime Integration PASS。

## 17. Open Questions

```text
0
```

M1 implementation 可以在标准库与最小 Windows primitive wrapper 之间选择具体实现，但不得降低本 SPEC 的 observable atomicity、Workspace boundary 或 failure-safety contract。
