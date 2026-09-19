实施 M5-D1：MCP Service Adapter。

## 0. 最高约束

以以下文档和既有实现为最高约束：

- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- `docs/IPC.md`
- `docs/CONFIG.md`
- `protocol/`
- 已完成的 M4 Workspace Security Boundary
- 已完成的 M5-A Tool Registry
- 已完成的 M5-B read_file Runtime Executor
- 已完成的 M5-C glob Runtime Executor

不要重新设计已经冻结的 V1 架构。

---

# 1. 当前状态

Runtime 当前已经具备：

- 唯一 `WorkspaceGuard`
- `ToolRegistry`
- `read_file` executor
- `glob` executor

Normal Mode Tool Catalog 必须且只能包含：

- `read_file`
- `glob`

当前尚未完成：

- MCP service
- MCP endpoint
- Streamable HTTP
- tunnel-client integration
- M6 Policy / State Machine / Audit

本任务只实现：

**MCP Service Adapter**

也就是：

```text
MCP tool definition
        ↓
argument validation / mapping
        ↓
existing Runtime capability
        ↓
result / error mapping
```

---

# 2. 本任务明确禁止

不要实现：

- HTTP listener
- Streamable HTTP transport
- SSE
- MCP stdio transport
- TCP bind
- tunnel-client
- Supervisor 改造
- UI 改造
- M6 Policy Engine
- Audit
- Approval
- Request State Machine
- bash
- 新的文件 executor
- 新的 glob implementation
- 新 Tool

尤其禁止：

把 MCP JSON-RPC 接入现有 Runtime stdin/stdout。

当前 stdin/stdout 已经承担 UI ↔ Runtime control protocol。

MCP 和 control protocol 必须保持独立。

---

# 3. 开始前先做安全回归检查

先检查 M5-C 对以下文件的修改：

```text
runtime/src/workspace/guard.rs
```

确认：

1. `WorkspaceGuard` 仍然是唯一 workspace security authority。
2. 没有公开 `PathResolver`。
3. 没有公开 `canonical_root` 给普通 tool/MCP 层使用。
4. 没有放宽 workspace escape。
5. symlink / junction fail-closed 行为仍成立。
6. glob/read_file 没有形成第二套 workspace boundary implementation。

如果发现 M5-C 改动破坏上述任一约束：

**停止 M5-D1，实现不要继续。**

只汇报冲突与建议 corrective。

---

# 4. MCP SDK

优先使用官方 Rust MCP SDK：

```text
rmcp
```

要求：

- 使用 crates.io 正式稳定版本。
- 不使用 Git branch dependency。
- 只启用本任务需要的最小 features。
- 不启用 HTTP transport。
- 不启用 MCP stdio transport。
- 不启用 MCP client。
- 不引入大型 Web framework。

如果当前 `rmcp` API 已有明显版本变化：

先阅读该版本的公开 API / crate docs，再实现。

不要根据旧示例盲写 API。

---

# 5. 推荐模块结构

建议新增：

```text
runtime/src/mcp/
├─ mod.rs
├─ service.rs
├─ context.rs
├─ errors.rs
├─ read_file.rs
└─ glob.rs
```

可根据现有代码风格调整。

但职责必须清晰。

## MCP module 只负责

- MCP tool metadata/schema
- MCP argument deserialize
- 参数转换
- 调用 existing Runtime executor
- Runtime error → MCP tool error
- Runtime output → MCP structured result

## MCP module 禁止

- `fs::read`
- `File::open`
- `read_dir`
- `canonicalize`
- 自己做 workspace boundary 判断
- 自己实现 glob
- 自己重新解析 symlink/junction security

---

# 6. MCP Execution Context

不要直接把整个：

```rust
Arc<Mutex<RuntimeCore>>
```

作为所有 MCP Tool 的执行对象。

避免：

- MCP 并发调用全部锁住 RuntimeCore
- MCP transport 与 Runtime 生命周期高度耦合
- 后续 M6 pipeline 很难插入

建立一个最小 execution context。

概念上例如：

```rust
McpExecutionContext
```

它只包含执行 `read_file / glob` 所需要的 Runtime initialized state。

要求：

- 必须复用 Runtime initialization 后产生的 workspace security state。
- 不根据 MCP request 创建新的 `Workspace`。
- 不根据 MCP request 创建新的 `WorkspaceGuard`。
- `read_file` 的 `max_read_bytes` 必须来自 initialized Runtime config snapshot。
- 不形成第二份独立 Runtime config source of truth。

如果当前 `WorkspaceGuard` 不支持安全共享：

允许做最小结构调整，例如内部使用 `Arc`。

但：

- 不改变 WorkspaceGuard security semantics。
- 不公开 `PathResolver`。
- 不公开 canonical root。
- 不允许 caller 自己完成 boundary checking。

---

# 7. RuntimeCore integration

RuntimeCore 应提供明确方式生成 MCP execution context。

例如概念：

```rust
RuntimeCore::mcp_context()
```

或等价方法。

要求：

Runtime 未完成 initialize：

```text
MCP context unavailable
```

Runtime initialize 成功：

```text
MCP context references initialized workspace/security state
```

不要允许一个未初始化 Runtime 构造可执行 MCP service。

---

# 8. MCP Tool Catalog

MCP service 对外必须且只能暴露：

```text
read_file
glob
```

不得暴露：

- `bash`
- `tree`
- `list_files`
- `find_files`
- `git`
- database
- write/edit
- `runtime.status`
- `runtime.stop`
- `tools.list`
- `policies.list`
- `activity.*`
- `diagnostics.run`

UI control protocol 方法绝不能自动变成 MCP tools。

---

# 9. ToolRegistry 必须仍然是真源

当前 Runtime 已经有 `ToolRegistry`。

MCP service 不得形成另一份独立 catalog source of truth。

允许 MCP SDK 因宏或静态注册要求，在代码中声明：

```text
read_file
glob
```

但必须建立一致性校验：

```text
MCP exposed tool names
==
ToolRegistry exposed tool names
```

测试必须防止以后出现：

```text
Registry: read_file + glob

MCP: read_file + glob + bash
```

或：

```text
Registry: read_file + glob

MCP: read_file
```

---

# 10. read_file MCP Tool Contract

MCP Tool name：

```text
read_file
```

description：

说明：

- 读取 active workspace 内 UTF-8 text file
- read-only
- 支持 line offset / limit

输入 schema：

```json
{
  "path": "string",
  "offset": "optional non-negative integer",
  "limit": "optional non-negative integer"
}
```

语义必须严格复用已经完成的 Runtime executor：

- `offset` = 0-based line offset
- `limit` = maximum returned lines
- `max_read_bytes` 不从 MCP 参数读取
- `max_read_bytes` 来自 Runtime initialized config

不要改变现有 read_file executor contract。

如果 Runtime executor 对：

```text
limit = 0
```

有既定行为：

MCP schema 和 adapter 必须保持一致。

---

# 11. glob MCP Tool Contract

MCP Tool name：

```text
glob
```

输入：

```json
{
  "pattern": "string",
  "basePath": "optional string"
}
```

MCP wire 使用：

```text
basePath
```

Runtime internal model仍可使用：

```text
base_path
```

Adapter 完成 camelCase → internal model 映射。

pattern 与 basePath 的所有实际语义必须复用现有 `glob` executor。

不要在 MCP 层重新：

- 拒绝 `..`
- canonicalize
- 判断 symlink
- walk filesystem

这些仍由现有 executor / WorkspaceGuard 负责。

MCP 层只做 schema-level argument validation。

---

# 12. MCP Tool Result

不要只返回自然语言字符串。

优先同时提供：

- structured result
- 简短 text content

这样兼容不同 MCP client。

---

## read_file structured result

至少包含：

```text
normalizedPath
content
offset
linesRead
bytesRead
truncated
nextOffset
```

camelCase。

数据直接来源于现有 Runtime `ReadFileOutput`。

不要重新计算 metadata。

---

## glob structured result

至少包含：

```text
matches
truncated
diagnostics
diagnosticsTruncated
```

每个 match：

```text
path
kind
```

所有路径必须是现有 glob executor 已返回的 logical workspace-relative path。

不要输出：

- canonical outside path
- Windows device path
- raw filesystem internals

---

# 13. Error Mapping

建立集中式：

```text
Runtime execution error
        ↓
MCP Tool Error
```

不要在 read_file handler / glob handler 各自拼 free-form error。

例如：

```text
runtime/src/mcp/errors.rs
```

---

## read_file 至少区分

- invalid arguments
- workspace violation
- target not found
- target not file
- unsupported content
- read limit exceeded
- IO failure
- Runtime not initialized

---

## glob 至少区分

- invalid arguments
- invalid pattern
- workspace violation
- invalid/missing base path
- target not directory
- IO failure
- Runtime not initialized

---

# 14. 不泄露敏感 filesystem 信息

MCP error 不得直接返回：

```rust
format!("{error:?}")
```

尤其不能泄露：

- workspace 外 canonical target
- UNC/device path
- host-specific absolute path
- raw OS internals

可以给客户端：

- stable machine-readable category
- sanitized message

内部 Runtime error 仍可保留完整类型。

---

# 15. Tool failure 与 JSON-RPC failure 分离

非常重要：

MCP request 能被正常解析：

```text
tools/call read_file
```

但 `read_file` 自身失败：

例如 workspace escape。

这应是：

```text
MCP tool execution error/result
```

而不是：

```text
JSON-RPC internal error
```

不要因为 filesystem error 就破坏整个 MCP protocol session。

使用 `rmcp` 当前推荐的 tool error 表达方式。

---

# 16. Tool annotations

如果当前 rmcp Tool metadata 支持 MCP annotations：

`read_file` 和 `glob`：

```text
readOnlyHint = true
destructiveHint = false
```

如果支持其他 hint：

只声明能够严格证明的属性。

不要声称：

- sandboxed
- network isolated
- fully safe

WorkspaceGuard 是 workspace file boundary，不是 OS sandbox。

---

# 17. MCP Server Capabilities

本阶段 service 只声明真正支持的能力：

```text
tools
```

不要主动声明：

- resources
- prompts
- logging
- sampling
- elicitation
- subscriptions
- tasks

Server identity 使用稳定产品名称。

例如：

```text
chatgpt-harness
```

不要使用 runtime UUID 作为 MCP server implementation name。

---

# 18. 不实现 transport

完成本任务后：

应该能够：

- 构造 MCP service
- 枚举 tools
- 模拟/直接调用 tool handler
- 验证 argument / result / error mapping

但：

不应该存在任何：

```rust
TcpListener
```

不应该：

```text
bind 127.0.0.1
bind 0.0.0.0
```

不应该启动 HTTP。

不应该读取 `MCP_SERVER_URL`。

---

# 19. 测试要求

至少增加以下测试。

## A. Catalog

验证：

```text
MCP tools == ["read_file", "glob"]
```

稳定顺序如果 SDK 能保证则测试；
如果 SDK 明确不保证 tool ordering，则测试集合一致性。

验证不存在：

```text
bash
tree
list_files
git
database
write
edit
runtime.status
diagnostics.run
```

---

## B. Registry consistency

验证：

```text
MCP exposed tool names
==
ToolRegistry exposed names
```

---

## C. read_file success

构造真实临时 workspace：

- 写 UTF-8 file
- initialize Runtime
- 创建 MCP context/service
- MCP adapter 调用 read_file

验证：

- content
- normalizedPath
- offset
- limit
- linesRead
- bytesRead
- truncated
- nextOffset

均来自 executor 正确结果。

---

## D. read_file security

至少：

```text
../outside.txt
```

不能读取。

workspace 外 absolute path不能读取。

验证 MCP error：

- 是 tool error
- category 正确
- 不泄露 outside canonical path

---

## E. read_file unsupported content

测试：

- invalid UTF-8
- NUL content

确认 error mapping 正确。

---

## F. glob success

测试：

```text
**/*.rs
```

以及：

```text
basePath
```

验证 structured result：

- matches
- kind
- truncated
- diagnostics
- diagnosticsTruncated

---

## G. glob security

验证：

- `../` base path rejected
- outside absolute base rejected
- glob executor 的 workspace security 不被 MCP adapter 绕过

---

## H. malformed arguments

测试：

- missing read_file.path
- negative offset
- wrong type
- missing glob.pattern
- wrong basePath type

要求：

- 不 panic
- 不调用 executor
- 返回 MCP argument/tool error

---

## I. unknown tool

调用：

```text
unknown_tool
```

必须：

- 不调用任何 Runtime executor
- 返回标准 MCP unknown tool failure

---

## J. Runtime initialization

Runtime 未 initialize：

不能正常获得 executable MCP context，

或者 service 调用必须明确：

```text
RuntimeNotInitialized
```

Runtime successful initialize 后才允许正常 tool execution。

---

# 20. Dependency 审查

如果新增 `rmcp`：

检查：

```text
cargo tree
```

确认：

- 没启用 HTTP transport
- 没启用 client
- 没启用无关大 features

不要为了方便直接：

```text
features = ["full"]
```

除非 rmcp 当前版本没有更细粒度方案，并在汇报中说明。

---

# 21. 验证命令

执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

如 protocol/UI 没修改：

不要无意义修改 generated files。

如 Cargo.lock 因 rmcp 正常变化：

允许提交。

---

# 22. 完成条件

本任务完成后必须达到：

```text
MCP Service
    ↓
read_file adapter ──→ existing read_file executor
    ↓
glob adapter      ──→ existing glob executor
```

且：

```text
WorkspaceGuard
ToolRegistry
Runtime config
```

仍然只有一份 security/source-of-truth。

---

# 23. 完成后只汇报以下内容

1. `guard.rs` 安全回归检查结论
2. 修改文件
3. `rmcp` 版本
4. `rmcp` enabled features
5. MCP module 结构
6. MCP execution context 结构
7. `read_file` MCP schema
8. `glob` MCP schema
9. Runtime result → MCP result mapping
10. Runtime error → MCP error mapping
11. ToolRegistry ↔ MCP catalog 一致性机制
12. tests
13. validation
14. dependency tree 是否引入非必要 transport/client features
15. 明确声明：
    - 未监听 HTTP
    - 未实现 Streamable HTTP
    - 未使用 MCP stdio
    - 未修改 tunnel-client
    - 未进入 M6

如果执行过程中发现：

- 必须修改冻结 control protocol
- 必须让 MCP 和 UI 共用 stdin/stdout
- 必须放宽 WorkspaceGuard
- rmcp 当前 API 与本任务假设存在根本冲突

停止继续扩张实现，只汇报具体冲突和最小 corrective 建议。