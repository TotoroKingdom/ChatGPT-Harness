# M0 SPEC — Write/Edit External Acceptance Gate

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M0 — Write/Edit Capability Acceptance Gate  
> Baseline: repository `main` at generation time  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`

## 1. Objective

M0 只验证一个 blocking capability：真实 External ChatGPT 能否通过现有 `ChatGPT → Tunnel → Runtime MCP` 链路发现并调用 `write` / `edit`，最终在 Workspace 产生准确文件变化。M0 通过前不得进入 M1+。

所有调用必须沿最终架构路径：

```text
MCP Adapter → ExecutionPipeline → WorkspaceGuard → PolicyEngine → ExecutionPermit → Executor → Activity/Audit
```

禁止 MCP handler 直接调用文件系统。

## 2. Current Code Baseline

当前 `main` 已有：

- `runtime/src/tools/registry.rs`：`ToolRegistry` 是工具定义/capability 单一事实源；目前只有 `read_file`、`glob`，`ToolCapability` 只有 `WorkspaceRead`。
- `runtime/src/execution/*`：统一 Pipeline 执行 readiness、validation、Workspace、Policy、permit、executor；`ExecutionPermit` 只由 Pipeline 创建。
- `ToolExecutor` 当前只有单一 `workspace_target() -> &Path`，Pipeline 固定调用 `WorkspaceGuard::approve()`。
- `WorkspaceGuard::approve()` 只允许 existing target；missing target fail closed，并保护 Windows path form、symlink/junction、sibling-prefix 与 Workspace escape。
- `PolicyEngine` 当前是 V2 minimal immutable rules；active workspace read 为 ALLOW，invalid rule/context/capability fail closed。
- `runtime/src/mcp/*` 当前只暴露 `read_file` / `glob`，且通过同一 `McpExecutionContext` 回到 Pipeline。
- Pipeline 已连接现有 Activity/Audit sink。

因此 M0 是沿现有 vertical slice 扩展，不新建旁路或平行子系统。

## 3. Scope

M0 实现：

1. `ToolCapability::WorkspaceWrite`。
2. Registry 注册 `write` / `edit`。
3. typed `write` / `edit` request、output、error、executor。
4. 独立 write-target Workspace validation。
5. Execution layer 最小 typed target semantics。
6. WorkspaceWrite minimal policy eligibility。
7. MCP schema、annotations、routing、structured/sanitized result。
8. write/edit 进入现有 Activity/Audit lifecycle。

M0 不实现：Tool Toggle、V3 AppConfig、User Policy、Approval、Session Grant、bash、i18n、`tools/list_changed`、write/edit UI、Tauri 新接口、完整 atomic replace hardening。Atomic mutation 与 production hardening 属于 M1。

## 4. Tool Contracts

### 4.1 `write`

Input：

```json
{"path":"relative/path.txt","content":"full text content"}
```

语义：

- `path` 非空；`content` 为 string。
- 可创建 missing file，也可 full overwrite existing regular file。
- parent directory 必须已存在；不递归创建目录。
- target / parent 必须保持在 Workspace 内。
- existing non-file 失败。
- mutation 前重新验证 write target。

Success 至少返回：

```json
{"normalizedPath":"relative/path.txt","bytesWritten":18,"created":true,"overwritten":false}
```

不得无必要暴露绝对 Workspace root。

### 4.2 `edit`

Input：

```json
{"path":"relative/path.txt","oldText":"hello from ChatGPT","newText":"hello from V3"}
```

语义：

- target 必须是 Workspace 内 existing regular UTF-8 text file。
- `oldText` 非空。
- exact match count：0 → fail；1 → replace；>1 → fail。
- 不 fuzzy、不 approximate、不 silent first-match。
- mutation 前重新验证 existing target。

Success 至少返回：

```json
{"normalizedPath":"relative/path.txt","replacements":1,"bytesWritten":13}
```

M0 可使用直接整文件 rewrite；M1 升级 atomic replacement。

## 5. Workspace Target Contract

Execution layer 引入最小 typed semantics：

```text
WorkspaceTarget::ExistingPath
WorkspaceTarget::WriteTarget
```

映射：`read_file/edit/glob → ExistingPath`，`write → WriteTarget`。

不得把现有 `WorkspaceGuard::approve()` 改成“missing 也成功”。新增 `approve_write_target(...)`，至少执行：

```text
validate path form
→ resolve existing parent
→ canonicalize parent
→ parent inside Workspace
→ if target exists: canonical target inside Workspace
→ if target missing: canonical parent + safe final component
→ approved write target
```

现有 read/edit/glob boundary 语义必须保持不变。

## 6. Registry / Capability / Policy

Registry stable order：

```text
read_file, glob, write, edit
```

`write` / `edit`：implemented/enabled/exposed=true，readiness=Ready，risk=Medium，capability=WorkspaceWrite。M0 仍是固定启用；M2 才接 AppConfig。

PolicyEngine 必须识别 `WorkspaceWrite`。在 Normal / DeveloperTest + active workspace 且通过 WorkspaceGuard 时，M0 minimal system baseline 允许 write/edit；invalid context/rule/capability 继续 fail closed。M0 不引入 User Policy/Tool Default/Approval。

## 7. Execution Contract

write/edit 必须执行：

```text
Argument validation
→ Runtime readiness
→ Workspace validation
→ Policy evaluation
→ ALLOW
→ ExecutionPermit
→ executor
→ Activity/Audit
```

Tool Adapter、Runtime helper 均不能自行构造 permit。

## 8. MCP Contract

Ready + ALLOW 时 `tools/list` 包含：

```text
read_file, glob, write, edit
```

Annotations：

```text
write/edit: readOnly=false, destructive=true, openWorld=false
```

具体 `rmcp 3.2.0` builder API 按当前依赖实现，语义必须等价。

错误至少可区分：

```text
INVALID_ARGUMENTS
WORKSPACE_VIOLATION
TARGET_NOT_FOUND
TARGET_NOT_FILE
EDIT_MATCH_NOT_FOUND
EDIT_MATCH_NOT_UNIQUE
UNSUPPORTED_CONTENT
IO_FAILURE
RUNTIME_NOT_READY
POLICY_DENIED
EXECUTION_FAILED
```

所有 MCP error 必须 sanitized，不泄露 Workspace 外绝对路径。

## 9. Activity / Audit

沿现有 Pipeline evidence 记录 write/edit 的 request ID、tool、workspace validation、policy evaluation、execution lifecycle/outcome。不得把 raw `content` / `oldText` / `newText` 或外部绝对路径写入 audit。

Boundary rejection 必须产生 Workspace validation failure evidence，且不得出现执行成功 evidence。

## 10. Expected Code Touchpoints

```text
runtime/src/tools/registry.rs
runtime/src/tools/effective.rs
runtime/src/tools/mod.rs
runtime/src/tools/write.rs        # new
runtime/src/tools/edit.rs         # new
runtime/src/workspace/guard.rs
runtime/src/workspace/mod.rs
runtime/src/execution/executor.rs
runtime/src/execution/pipeline.rs
runtime/src/execution/tests.rs
runtime/src/policy/engine.rs
runtime/src/policy/tests.rs
runtime/src/mcp/mod.rs
runtime/src/mcp/service.rs
runtime/src/mcp/errors.rs
runtime/src/mcp/write.rs          # new
runtime/src/mcp/edit.rs           # new
runtime/src/state.rs
```

Protocol major、Tauri/UI、AppConfig 不应因 M0 修改；若实施发现必须改变 process ownership、wire protocol major 或 immutable Runtime snapshot，应先做 Architecture amendment。

## 11. Development Verification Requirements

每个 Task 自带 unit/code-level tests。最终至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

这些不是 Integration/E2E/Manual Acceptance。

## 12. Post-TASK Verification Documents

按仓库流程，`SPEC / PLAN / TASKS` 冻结后必须另行生成：

```text
Integration Test Plan
E2E Test Plan
Manual Acceptance Checklist
```

至少覆盖：Runtime/MCP write create、edit exact replacement、outside-workspace rejection、read/glob regression、Activity/Audit，以及真实 External ChatGPT + Tunnel write/edit。

如果 ChatGPT 当前产品侧无法调用 write/modify MCP action：

```text
M0 = BLOCKED_BY_EXTERNAL_CAPABILITY
```

Unit/Integration、MCP Inspector、loopback、mock 均不能替代 External PASS。

## 13. Final Acceptance Criteria

M0 最终只有在独立测试/验收文档确认以下事实后可 `ACCEPTED / FROZEN`：

- Registry/effective catalog 正确提供 write/edit。
- write/edit 均走统一 ExecutionPipeline。
- existing WorkspaceGuard 语义未削弱。
- missing write target 使用独立 write-target validation。
- Workspace 外 mutation 被拒绝。
- Runtime Integration write/edit/boundary PASS。
- Existing read/glob regression PASS。
- Activity/Audit evidence verified。
- External ChatGPT write PASS。
- External ChatGPT edit PASS。
- External ChatGPT boundary rejection PASS。

## 14. Open Questions

```text
0
```

M0 implementation 不应再做产品或顶层架构决策，只允许在本 SPEC 边界内选择局部 Rust 代码组织方式。
