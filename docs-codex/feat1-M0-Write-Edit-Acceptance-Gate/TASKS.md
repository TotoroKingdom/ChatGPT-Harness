# M0 TASKS — Write/Edit External Acceptance Gate

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> 每个 Task 必须包含实现 + Unit Test 或代码级自动化验证。  
> Integration Test、E2E Test、Manual Acceptance 不属于本文件。

## Dependency

```text
T1 Capability / Registry / Policy
        ↓
T2 WorkspaceTarget / WriteTarget Guard
        ↓
   ┌────┴────┐
   ↓         ↓
T3 Write    T4 Edit
   └────┬────┘
        ↓
T5 MCP / Errors / Evidence
```

### T1 — WorkspaceWrite Capability / Registry / Policy

**Files:** `runtime/src/tools/registry.rs`, `effective.rs`, `runtime/src/policy/engine.rs`, tests。

实现：
- 增加 `ToolCapability::WorkspaceWrite`。
- Registry 增加 `write` / `edit`，顺序固定 `[read_file, glob, write, edit]`。
- descriptor：implemented/enabled/exposed=true，Ready，risk Medium，placeholder ALLOW，capability WorkspaceWrite。
- PolicyEngine 允许 Normal/DeveloperTest + active workspace 的 WorkspaceWrite；invalid/unsupported 继续 DENY。
- 不实现 User Policy/Tool Default config。

Unit Tests：stable order、capability、unavailable state、WorkspaceWrite allow、invalid fail closed、Ready/NotReady effective catalog。

Done：相关 unit tests PASS，且没有第二份 authoritative tool registry。

### T2 — WriteTarget Workspace Validation

**Files:** `runtime/src/workspace/guard.rs`, `mod.rs`, `runtime/src/execution/executor.rs`, `pipeline.rs`, tests。

实现：
- 引入 `WorkspaceTarget::ExistingPath | WriteTarget`。
- Pipeline 按 kind 调用 `guard.approve()` 或 `approve_write_target()`。
- 不得修改 `approve()` missing-target fail-closed 行为。
- `approve_write_target()` 要求 existing parent directory；canonical parent/target 留在 Workspace；missing target 使用 canonical parent + safe final component。
- 保持 Windows unsupported path form 拒绝。

Unit Tests：new inside、existing inside、missing parent、relative/absolute outside、sibling prefix、outside symlink/junction（可用时）、existing approve missing 仍失败、read/glob regression。

Done：Workspace boundary 无回归。

### T3 — Runtime `write`

**Files:** new `runtime/src/tools/write.rs`; modify tools mod、executor/pipeline/state。

实现：
- `WriteRequest { path, content }`。
- `WriteOutput { normalized_path, bytes_written, created, overwritten }`。
- `WriteError` + `ExecutionError::Write`。
- ToolExecutor target=WriteTarget；mutation 前 revalidate。
- create/full overwrite；parent missing/outside/special path/non-file fail。
- 返回 workspace-relative normalized path。
- RuntimeCore typed seam 必须复用 Pipeline。
- 不实现 M1 atomic primitive。

Unit Tests：create、exact content/bytes、overwrite flags、nested parent、missing parent、outside relative/absolute、directory target、revalidation。

Done：Workspace 外无 mutation，permit 不能旁路。

### T4 — Runtime `edit`

**Files:** new `runtime/src/tools/edit.rs`; modify tools mod、executor/pipeline/state。

实现：
- `EditRequest { path, old_text, new_text }`。
- `EditOutput { normalized_path, replacements, bytes_written }`。
- `EditError` + `ExecutionError::Edit`。
- target=ExistingPath；old_text empty invalid；mutation 前 revalidate。
- regular UTF-8 file；exact match 0 fail / 1 replace / >1 fail。
- 不 fuzzy/regex/first-match guessing；M1 再做 atomic replacement。
- RuntimeCore seam 必须复用 Pipeline。

Unit Tests：one-match、0-match unchanged、multi-match unchanged、outside、missing、directory、invalid UTF-8、revalidation。

Done：ambiguous edit 绝不修改文件。

### T5 — MCP Adapters / Errors / Evidence

**Files:** new `runtime/src/mcp/write.rs`, `edit.rs`; modify `mcp/mod.rs`, `service.rs`, `errors.rs`，按需更新 execution/activity/audit tests。

实现：
- write schema：`path`, `content`。
- edit schema：`path`, `oldText`, `newText`。
- adapter 只做 deserialize → typed request → `context.execute()` → result/error mapping。
- `McpService` 增加 write/edit routing 与 tool builders。
- annotations：read_only=false, destructive=true, open_world=false。
- error codes至少：INVALID_ARGUMENTS、WORKSPACE_VIOLATION、TARGET_NOT_FOUND、TARGET_NOT_FILE、EDIT_MATCH_NOT_FOUND、EDIT_MATCH_NOT_UNIQUE、UNSUPPORTED_CONTENT、IO_FAILURE、RUNTIME_NOT_READY、POLICY_DENIED、EXECUTION_FAILED。
- sanitized errors 不返回 outside path/workspace root。
- write/edit 自然进入现有 Pipeline audit sink；禁止 adapter 自建第二套 execution truth。

Unit/code-level tests：tool builders、schemas、annotations、routing、structured success、malformed args、outside write、edit match errors、unknown tool unchanged、audit lifecycle、outside write无 ExecutionStarted。

Done：执行并通过：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

T5 完成只代表开发实现完成。随后必须单独生成：Integration Test Plan、E2E Test Plan、Manual Acceptance Checklist；External ChatGPT gate 未通过前不得将 M0 标记 `ACCEPTED / FROZEN`。

## Development Completion Checklist

- [ ] T1 PASS
- [ ] T2 PASS
- [ ] T3 PASS
- [ ] T4 PASS
- [ ] T5 PASS
- [ ] cargo fmt/check/test PASS
- [ ] git diff --check PASS
- [ ] TASKS 未声称 Integration/E2E/Manual Acceptance PASS
