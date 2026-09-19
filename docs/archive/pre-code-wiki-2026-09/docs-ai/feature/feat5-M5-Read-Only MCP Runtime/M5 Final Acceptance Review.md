执行 M5 Final Acceptance Review。

目标：

对 M5 — Read-Only MCP Runtime 做最终验收。

本任务原则上：

- 不新增功能
- 不重构
- 不进入 M6
- 不修改 UI
- 不修改 tunnel-client
- 不修改 control protocol

只有发现真实 blocker 时，才提出最小 corrective。

最高约束：

- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- `docs/IPC.md`
- `docs/CONFIG.md`

当前已完成：

- M4 Workspace Security Boundary
- M5-A Tool Registry
- M5-B read_file
- M5-C glob
- M5-D1 MCP Service Adapter
- M5-D2 Streamable HTTP Endpoint
- M5-D3 Runtime MCP Lifecycle Integration

---

# 1. 验收目标

最终真实链路必须成立：

```text
Runtime initialize
        ↓
WorkspaceGuard initialized
        ↓
MCP HTTP endpoint started
        ↓
initialized acknowledgement
        ↓
Runtime Ready
        ↓
real MCP HTTP client
        ↓
tools/list
        ↓
read_file / glob
        ↓
WorkspaceGuard
        ↓
filesystem
```

停止链路：

```text
runtime.stop / stdin EOF
        ↓
MCP execution disabled
        ↓
HTTP endpoint graceful shutdown
        ↓
background thread joined
        ↓
Runtime exits
```

---

# 2. M5 Tool Catalog

确认 Normal Mode 对 MCP 暴露：

```text
read_file
glob
```

且只能有这两个。

确认不存在：

- bash
- tree
- list_files
- find_files
- git
- database
- write
- edit
- diagnostics
- runtime.status
- control protocol methods

确认：

```text
ToolRegistry exposed catalog
==
MCP exposed catalog
==
RuntimeState.exposed_tool_catalog
```

如果顺序有定义：

```text
read_file
glob
```

---

# 3. read_file final acceptance

使用真实临时 workspace，经完整 Runtime lifecycle + HTTP MCP 调用。

覆盖：

## 成功

- UTF-8 file
- Unicode
- empty file
- CRLF
- nested file
- workspace 内绝对路径

## offset / limit

确认：

- offset = 0-based line offset
- limit = 最大返回行数
- 连续分页无重复/跳行
- nextOffset 正确

## maxReadBytes

确认：

- 来自 initialized RuntimeConfigSnapshot
- content bytes 永远不超过 maxReadBytes
- byte-limit truncation 正确
- 超长单行返回 read limit error
- 不使用整文件 read_to_string/read_to_end

## unsupported

- invalid UTF-8 rejected
- NUL content rejected
- directory rejected
- missing target rejected

---

# 4. glob final acceptance

通过真实 MCP HTTP 调用覆盖：

- `*.rs`
- `src/*.rs`
- `src/**/*.rs`
- `**/*.md`
- `?`
- nested basePath
- empty result
- file / directory result
- deterministic ordering

确认：

```text
MAX_GLOB_RESULTS = 1000
MAX_GLOB_DIAGNOSTICS = 100
```

或当前实现中的等价具名限制仍然有效。

确认：

- 不先收集完整 workspace tree
- diagnostics bounded
- result bounded
- truncated 状态正确

---

# 5. Workspace Security final acceptance

这是 M5 最重要的验收项。

read_file 和 glob 都必须证明：

```text
MCP request
↓
existing executor
↓
existing WorkspaceGuard
```

不存在 MCP bypass。

至少覆盖：

- `../` escape
- absolute outside workspace
- sibling-prefix attack
- symlink → outside
- junction → outside
- Windows root-relative path
- UNC / device / unsupported path forms

合法：

- workspace 内普通文件
- workspace 内 symlink/junction target

非法路径：

不得访问 workspace 外内容。

---

# 6. glob traversal security

确认 walker 对 filesystem candidate：

```text
candidate
↓
WorkspaceGuard
↓
metadata/read_dir/return
```

而不是：

```text
read_dir
↓
WorkspaceGuard
```

确认：

- outside junction 不被进入
- outside symlink 不被进入
- cycle 不无限递归
- logical path 与 canonical security path 分离

---

# 7. Runtime lifecycle

覆盖完整状态：

```text
WaitingInitialize
↓
initialize
↓
InitializedPendingAck
↓
initialized
↓
Ready
↓
Stopping
```

MCP readiness 必须：

```text
WaitingInitialize       false
InitializedPendingAck   false
Ready                   true
Stopping                false
```

不能出现 Phase 与 MCP readiness 不一致。

---

# 8. Pre-ack behavior

真实 HTTP MCP endpoint 在 initialize 后已经存在。

但是：

```text
initialize
↓
未发送 initialized
↓
tools/call read_file / glob
```

必须：

```text
runtime_not_ready
```

并确认 filesystem executor没有执行。

---

# 9. Post-ack behavior

发送：

```text
initialized
```

后：

- read_file 成功
- glob 成功
- RuntimeState Ready
- readiness gate true

重复 initialized：

- 不启动第二 endpoint
- URL不变
- 不创建额外 thread
- 不重复 RuntimeStarted lifecycle副作用

---

# 10. Initialize rollback

确认：

```text
Workspace init success
↓
MCP endpoint startup failure
```

导致完整 rollback：

- config = None
- events = None
- WorkspaceGuard = None
- readiness = false
- phase = WaitingInitialize

同时：

- runtime_id保持
- started_at保持
- 下一次合法 initialize可以成功

---

# 11. Invalid initialize retry

覆盖：

```text
invalid workspace initialize
↓
error
↓
valid initialize
↓
success
```

确认第一次失败：

- 无 HTTP listener
- 无 leaked MCP thread
- 无 partial Runtime state

---

# 12. MCP HTTP transport acceptance

确认：

绑定：

```text
127.0.0.1:0
```

实际 URL：

```text
http://127.0.0.1:<ephemeral>/mcp
```

确认：

- 非固定端口
- 两个 endpoint可同时启动
- 不绑定 `0.0.0.0`
- 不绑定公网/LAN

仅：

```text
/mcp
```

是 MCP endpoint。

其它 route正常 404/405。

---

# 13. Transport security

确认：

Host：

- loopback Host允许
- malicious Host拒绝

Origin：

- 无 Origin允许
- 合法 loopback Origin允许
- 非 loopback Origin返回 403

request body：

- rmcp request-size limit仍存在
- oversized body拒绝

不得关闭 SDK security来通过测试。

---

# 14. MCP protocol/error semantics

确认：

工具执行失败，例如：

- workspace violation
- unsupported content
- invalid glob pattern

表现为：

```text
MCP tool failure
```

而不是：

```text
JSON-RPC internal failure
```

未知 tool：

保持标准 MCP routing/unknown-tool failure。

Malformed JSON：

属于协议层错误。

两种错误类别不能混淆。

---

# 15. Error sanitization

检查所有 MCP error mapping。

不得泄露：

- outside canonical path
- host-specific敏感绝对路径
- raw OS error Debug
- `WorkspaceError` Debug dump
- Rust backtrace/internal representation

允许：

- stable error category
- sanitized message

---

# 16. Runtime shutdown

真实流程：

```text
initialize
initialized
successful MCP call
runtime.stop
```

确认：

- readiness先变 false
- MCP endpoint graceful shutdown
- background thread join
- port释放
- stop response正常
- Runtime退出

stop response返回后，不应该还有可工作的 MCP listener。

---

# 17. EOF cleanup

模拟 control stdin EOF。

确认：

- readiness false
- HTTP endpoint停止
- background thread退出
- port释放
- control run loop正常返回

不得留下后台 thread。

---

# 18. Dependency audit

检查：

```text
cargo tree
```

确认生产 dependencies没有主动启用：

- rmcp client
- MCP stdio transport
- child process transport
- auth
- reqwest client
- tokio/full

允许：

- rmcp server
- macros
- transport-streamable-http-server
- server transport必要的 Tower/HTTP依赖
- axum
- 最小 Tokio features

---

# 19. Source-of-truth audit

确认没有出现第二套：

- Workspace boundary
- Tool catalog
- Runtime config
- read_file implementation
- glob implementation
- MCP readiness lifecycle

必须仍然是：

```text
WorkspaceGuard
ToolRegistry
RuntimeConfigSnapshot
read_file executor
glob executor
Runtime Phase
```

各自唯一真源。

---

# 20. Architecture boundary audit

确认 M5 没有提前进入：

- Policy Engine
- Audit pipeline
- Approval
- bash
- tunnel-client management
- UI lifecycle redesign

M5 当前 pipeline允许仍然是：

```text
MCP
→ validation
→ existing file capability
→ WorkspaceGuard/executor
```

M6 再增加：

```text
Policy
Audit
Request State Machine
```

不要为了“架构完整”提前补 M6。

---

# 21. Full validation

必须执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

以及所有已有：

- protocol generated consistency tests
- MCP HTTP E2E
- Windows workspace tests

全部通过。

不接受：

- ignored security tests
- 临时 skip
- 删除 assertion
- “只在本机失败所以忽略”

如果是权限环境无法创建 symlink：

可以保持项目已有明确 skip策略，
但 junction等可执行覆盖必须继续运行。

---

# 22. 不允许顺带修改

本次 review原则上不改功能。

禁止新增：

- MCP URL protocol field
- tunnel integration
- health route
- metrics
- diagnostics route
- Policy
- Audit
- bash
- UI

如果 review没有发现 blocker：

不要制造代码改动。

---

# 23. 如果发现问题

只分类为：

## BLOCKER

会导致：

- workspace escape
- MCP bypass
- lifecycle leak
- catalog违规
- protocol错误
- M5验收无法成立

提出最小 corrective，停止进入 M6。

## NON-BLOCKING

例如：

- warning
- naming
- test helper duplication
- comment/style

记录即可，不在 M5范围扩大重构。

---

# 24. 最终只汇报

1. M5 总体结论：PASS / BLOCKED
2. Tool Catalog 验收
3. read_file 验收
4. glob 验收
5. Workspace security验收
6. MCP HTTP transport验收
7. lifecycle/readiness验收
8. initialize rollback/retry验收
9. error sanitization验收
10. shutdown/EOF cleanup验收
11. dependency audit
12. source-of-truth audit
13. validation结果
14. BLOCKER（如有）
15. NON-BLOCKING issues（如有）
16. 如果 PASS，明确声明：

`M5 — Read-Only MCP Runtime accepted.`

17. 同时明确声明仍未完成：

- M6 Policy / State Machine / Audit
- M7 tunnel-client Integration
- Supervisor 获取 ephemeral MCP endpoint URL 的正式 contract