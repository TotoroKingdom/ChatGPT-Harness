# M0 PLAN — Write/Edit External Acceptance Gate

> Input: `SPEC.md`  
> Goal: 用最小实现改动完成 Runtime write/edit capability。  
> Rule: PLAN 只决定如何实现 SPEC；Integration/E2E/Manual Acceptance 另行生成独立文档。

## 1. Strategy

沿现有 read/glob vertical slice 扩展：

```text
Capability / Registry / Policy
→ Workspace target semantics
→ write executor
→ edit executor
→ MCP adapters / errors / evidence
```

不新增执行入口，不改 Tauri/Tunnel ownership，不引入 V3 AppConfig。

## 2. Phase A — Capability / Registry / Policy

- `ToolCapability` 增加 `WorkspaceWrite`。
- Registry 增加 `write` / `edit`，稳定顺序 `read_file, glob, write, edit`。
- 新工具 descriptor：implemented/enabled/exposed/ready；risk Medium；placeholder policy ALLOW。
- PolicyEngine 增加 WorkspaceWrite active-workspace eligibility，invalid/unsupported 继续 fail closed。
- 更新 registry/effective/policy unit tests。

Exit：Ready effective catalog 可得到四个 Available tools；NotReady 仍不可执行。

## 3. Phase B — Workspace Target Semantics

- 引入 `WorkspaceTarget::ExistingPath | WriteTarget`。
- ToolExecutor 返回 typed target；Pipeline 按 kind 选择 guard validation。
- 保留 `WorkspaceGuard::approve()` existing-only 语义。
- 新增 `ApprovedWriteTarget` / `approve_write_target(...)`：existing parent、canonical parent in Workspace、existing target in Workspace、missing target 使用 canonical parent + safe final component。
- 增加 outside、missing-parent、symlink/junction、Windows special path 单元测试。

Exit：安全支持 missing write target，existing path boundary 无回归。

## 4. Phase C — Runtime `write`

- 新建 `runtime/src/tools/write.rs`。
- 定义 `WriteRequest/WriteOutput/WriteError`。
- ToolExecutor 使用 WriteTarget；mutation 前 revalidate。
- 支持 create/full overwrite；existing non-file fail；返回相对 normalized path、bytes、created/overwritten。
- `ExecutionError` 增加 Write；RuntimeCore typed seam 复用同一 Pipeline。
- Unit tests 覆盖 create/overwrite/outside/missing-parent/non-file/revalidation。

Exit：typed Runtime write 只能经 Pipeline + permit 执行。

## 5. Phase D — Runtime `edit`

- 新建 `runtime/src/tools/edit.rs`。
- 定义 `EditRequest/EditOutput/EditError`。
- ExistingPath；old_text 非空；UTF-8 regular file；mutation 前 revalidate。
- exact match 0 fail / 1 replace / >1 fail；不 fuzzy/guess。
- `ExecutionError` 增加 Edit；RuntimeCore typed seam 复用同一 Pipeline。
- Unit tests 覆盖 success/0-match/multi-match/outside/missing/not-file/invalid UTF-8/revalidation。

Exit：typed Runtime edit deterministic 且无 authorization bypass。

## 6. Phase E — MCP / Errors / Evidence

- 新建 `runtime/src/mcp/write.rs`、`edit.rs`。
- `McpService` 增加 routing 与 tool builders。
- schemas：write(path, content)，edit(path, oldText, newText)。
- annotations：readOnly=false, destructive=true, openWorld=false。
- `mcp/errors.rs` 增加 Write/Edit 映射与固定 sanitized codes。
- 确认 adapters 只 deserialize/map，执行继续走 `McpExecutionContext → ExecutionPipeline`。
- unit/code-level tests覆盖 schema、annotations、routing、structured success、sanitized failure、audit lifecycle。

Exit：M0 production MCP vertical slice 完整，不存在 MCP→filesystem 旁路。

## 7. Development Completion Check

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

随后单独生成并执行 `Integration Test Plan`、`E2E Test Plan`、`Manual Acceptance Checklist`。External ChatGPT hard gate 未通过前不得进入 M1。

## 8. Change Discipline

- 不引入 Tool Toggle / Approval / Bash / i18n。
- 不顺手实现 M1 atomic hardening。
- 不改变 existing read/glob Workspace semantics。
- 不把 Unit/local test 解释为 External PASS。
- 若必须改变 process ownership、wire protocol major、immutable snapshot model，停止并先做 Architecture amendment。
