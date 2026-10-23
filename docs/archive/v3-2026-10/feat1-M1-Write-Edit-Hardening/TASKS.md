# M1 TASKS — Write/Edit Production Hardening

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Baseline: M0 — ACCEPTED / FROZEN  
> 每个 Task 必须包含实现 + Unit Test 或代码级自动化验证。  
> Integration Test、E2E Test、Manual Acceptance 不属于本文件。

## Dependency

```text
T1 Atomic File Primitive
        ↓
   ┌────┴────┐
   ↓         ↓
T2 Write    T3 Edit
   └────┬────┘
        ↓
T4 Errors / Evidence / Regression
```

## T1 — Shared Atomic File Mutation Primitive

**Primary files:** new shared Runtime module such as `runtime/src/tools/atomic_file.rs`; `runtime/src/tools/mod.rs`; `runtime/Cargo.toml` only if required for Windows atomic replace.

### Implement

新增 Runtime-internal shared atomic mutation primitive，至少支持：

```text
CreateNew
ReplaceExisting
```

必须实现：

- temp file 创建在 approved target 的同一 parent directory。
- unique/unpredictable temp suffix。
- exclusive create (`create_new` 或等价语义)。
- `write_all` 完整写入 prepared bytes。
- flush temp file before commit。
- commit 前执行调用方提供的 target revalidation。
- create 使用 atomic rename/create semantics。
- replace existing target 使用 Windows 上真实 atomic replace semantics。
- 禁止 `delete target → rename temp`。
- success 后无 temp residue。
- failure 后 best-effort cleanup。
- cleanup failure 不覆盖 primary error。
- internal phase-aware error 至少区分：TempCreate / TempWrite / TempFlush / Commit / Cleanup。

如果标准库无法满足 Windows replace-existing 语义，可以：

- 加最小 dependency；或
- 实现最小 Windows API wrapper。

不得引入大型 filesystem abstraction。

### Test seam

提供最小、仅 Runtime-internal/test 可用的 fault injection seam，能稳定模拟：

- temp create failure。
- temp write failure。
- flush failure。
- commit failure。

不要通过磁盘写满、随机锁文件等 flaky 环境手段作为核心单测。

### Unit Tests

至少覆盖：

- create-new success，最终 bytes 完整且 temp 消失。
- replace-existing success，最终 bytes 完整且 temp 消失。
- temp create failure：existing target unchanged。
- temp write failure：existing target unchanged。
- temp flush failure：existing target unchanged。
- commit failure：existing target unchanged。
- failed create：正式 target 不存在。
- cleanup 执行且不会覆盖 primary failure。
- temp 与 target 位于同一 directory。
- exclusive temp creation 不覆盖预先存在的同名文件。

Windows-specific replace test 必须真实覆盖 existing-target replacement；不能只测 helper mock。

### Done

- write/edit 可以复用同一 primitive，不需要各复制一套 temp/commit logic。
- unit tests PASS。
- 没有修改 WorkspaceGuard / Policy ownership。

---

## T2 — Production Harden `write`

**Primary files:** `runtime/src/tools/write.rs`; shared atomic module；按需更新 execution/state tests。

### Implement

把当前 live-target：

```text
OpenOptions(create + truncate)
→ write_all(target)
```

替换为：

```text
validate arguments
→ approve_write_target
→ inspect target state
→ existing target UTF-8 validation
→ prepare exact UTF-8 content bytes
→ atomic primitive
```

#### Create path

- target missing 时 parent 必须已存在且已批准。
- 不创建 parent directories。
- temp 完成后 atomic commit 为正式 target。
- commit 前 target/parent revalidation。
- 如果 prepare 后 target 被其它 actor 创建，不得静默覆盖；必须返回确定失败或经过明确的 overwrite branch re-evaluation。

#### Overwrite path

- target existing 时必须是 regular file。
- existing bytes 必须 valid UTF-8；否则 `UNSUPPORTED_CONTENT`。
- 不直接 truncate target。
- atomic replace。

#### Output compatibility

保持：

```text
normalized_path
bytes_written
created
overwritten
```

`bytes_written = content.as_bytes().len()`。

不得：

- 改换行。
- 自动追加 newline。
- 自动插入/移除 BOM。
- Unicode normalize。

### Unit Tests

至少覆盖：

- new UTF-8 file atomic create。
- overwrite existing UTF-8 file atomic replace。
- multibyte UTF-8 `bytes_written` 正确。
- empty content 合法并生成 0-byte text file。
- existing invalid UTF-8 target → `UNSUPPORTED_CONTENT` 且 unchanged。
- directory target fail unchanged。
- missing parent fail，无目录/文件 residue。
- outside relative/absolute reject unchanged。
- target-created race 不能被静默 clobber。
- temp write/flush/commit fault 均保持 existing target 原 bytes。
- failure 不残留 temp file。

### Done

- production `write` 不再对 live target 使用 truncate。
- M0 write contract 保持兼容。
- tests PASS。

---

## T3 — Production Harden `edit`

**Primary files:** `runtime/src/tools/edit.rs`; shared atomic module；按需更新 execution/state tests。

### Implement

保留 M0 exact-match contract：

```text
0 match  → MatchNotFound
1 match  → replace
>1 match → MatchNotUnique
```

mutation 改为：

```text
approve existing path
→ read original bytes
→ validate regular file + UTF-8
→ exact unique match
→ build replacement bytes in memory
→ stage temp + flush
→ pre-commit revalidate target
→ compare current bytes with original bytes
→ changed ? TARGET_CHANGED : atomic replace
```

不得继续：

```text
fs::write(target, replacement)
```

### Concurrent Modification Contract

如果在 original read 与 commit 之间 target bytes 已变化：

```text
TARGET_CHANGED
```

并保证本 invocation 不覆盖当前 target。

最低要求比较完整 bytes。若实现选择更强 identity/version check 可以，但不能弱于完整 bytes change detection。

### Unit Tests

至少覆盖：

- unique exact replacement success。
- multibyte UTF-8 replacement。
- replacement 可增大/缩小/置空。
- 0 match unchanged。
- multiple matches unchanged。
- invalid UTF-8 unchanged。
- missing file fail。
- directory fail。
- outside target reject。
- target 在 prepare 后被修改 → `TARGET_CHANGED`，并保留并发 actor 的内容。
- target 在 prepare 后被删除/替换 → fail closed，不把旧 replacement 写到错误对象。
- temp write/flush/commit failure 保持目标未被本 invocation 部分修改。
- success/failure 无 temp residue。

### Done

- edit 的 live target 只在 atomic commit 时发生最终替换。
- concurrent modification 不会被静默覆盖。
- exact-match semantics 无回归。
- tests PASS。

---

## T4 — Errors / Evidence / M0 Regression

**Primary files:** `runtime/src/mcp/errors.rs`, `runtime/src/mcp/write.rs`, `runtime/src/mcp/edit.rs`, Activity/Audit tests，按需更新 shared execution errors。

### Implement

新增或冻结稳定外部错误：

```text
TARGET_CHANGED
```

保持既有：

```text
INVALID_ARGUMENTS
WORKSPACE_VIOLATION
TARGET_NOT_FOUND
TARGET_NOT_FILE
UNSUPPORTED_CONTENT
EDIT_MATCH_NOT_FOUND
EDIT_MATCH_NOT_UNIQUE
IO_FAILURE
RUNTIME_NOT_READY
POLICY_DENIED
EXECUTION_FAILED
```

内部 atomic phase errors 映射为 sanitized external error；不要把：

- temp absolute path。
- Workspace root。
- raw OS message containing unsafe path。
- content / oldText / newText。

暴露给 MCP caller。

Activity/Audit 至少能够从 outcome/category 区分：

- success。
- target changed。
- I/O/commit failure。
- Workspace rejection。

不需要把 temp filename 记入 audit。

### Regression Tests

必须证明 M0 frozen contract 未回归：

- ToolRegistry 仍为 `read_file, glob, write, edit`。
- MCP schemas/input field names 不变。
- tool annotations 不变。
- write structured result core fields 不变。
- edit structured result core fields 不变。
- read_file normal + boundary tests PASS。
- glob normal + boundary tests PASS。
- write outside boundary fail closed。
- edit outside boundary fail closed。
- `WorkspaceGuard::approve()` existing-only semantics 保持。
- `ExecutionPermit` 仍只能由 Pipeline 构造。
- MCP adapter 不直接执行 filesystem mutation。
- raw mutation content 不进入 Activity/Audit。

### Development Completion

执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

全部 PASS。

### Done

T1–T4 全部完成只代表 M1 开发完成。

随后必须独立生成/执行：

```text
Integration Test Plan
E2E regression plan/smoke（如 M1 验收需要）
Manual Acceptance Checklist
```

不得仅凭 unit tests 标记 M1 `ACCEPTED / FROZEN`。

---

## Development Completion Checklist

- [ ] T1 Shared Atomic File Primitive PASS
- [ ] T2 Production `write` PASS
- [ ] T3 Production `edit` PASS
- [ ] T4 Errors / Evidence / M0 Regression PASS
- [ ] Windows existing-target atomic replace automated coverage PASS
- [ ] atomic fault-path tests PASS
- [ ] concurrent edit target-change test PASS
- [ ] cargo fmt/check/test PASS
- [ ] git diff --check PASS
- [ ] TASKS 未声称 Integration/E2E/Manual Acceptance PASS
