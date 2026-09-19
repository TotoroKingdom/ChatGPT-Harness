实施 M5-D2：Loopback Streamable HTTP MCP Endpoint。

## 0. 最高约束

最高约束：

- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- 已冻结 `protocol/`
- M4 Workspace Security Boundary
- M5-A Tool Registry
- M5-B read_file
- M5-C glob
- 已完成并通过全部 validation 的 M5-D1 MCP Service Adapter

不要重新设计 V1。

---

# 1. 本任务目标

把已经完成的：

```text
McpService
    ↓
read_file / glob adapters
    ↓
existing Runtime executors
```

挂到真实的：

```text
http://127.0.0.1:<ephemeral-port>/mcp
```

Streamable HTTP MCP endpoint。

完成后必须可以由真实 MCP HTTP client：

```text
initialize / discover as SDK requires
        ↓
tools/list
        ↓
tools/call read_file
tools/call glob
```

真实走过 HTTP transport。

---

# 2. 本任务边界非常重要

本任务只实现：

```text
MCP Service
    ↓
Streamable HTTP transport
    ↓
loopback TCP listener
```

不要实现：

- tunnel-client process integration
- UI
- Supervisor
- Runtime control protocol redesign
- M6 Policy
- Audit
- Approval
- bash
- 新 tools
- OAuth
- TLS
- public network binding
- fixed public port
- MCP stdio

尤其：

**不要把 MCP 接到现有 stdin/stdout。**

现有 stdin/stdout control protocol保持不变。

---

# 3. 暂时不要自动接入 Runtime main 生命周期

这是本任务的重要边界。

当前 Runtime binary/control server 是已有同步生命周期。

M5-D2 不要为了 HTTP endpoint：

- 把整个 Runtime 改成 async
- 把 `server.rs` 大规模重写成 Tokio
- 把 `RuntimeCore` 包成全局 `Arc<Mutex<_>>`
- 改 M0 initialize contract
- 改 Runtime control protocol

本阶段实现一个独立：

```rust
McpHttpEndpoint
```

或等价组件。

它接受已经构造成功的：

```rust
McpExecutionContext
```

或 M5-D1 已有 service factory/context。

测试和未来 Runtime lifecycle 可以显式：

```text
start endpoint
↓
obtain local URL
↓
use MCP
↓
shutdown endpoint
```

M5-D3 再负责把这个 handle 接入真实 Runtime process lifecycle。

---

# 4. rmcp feature

当前：

```text
rmcp = 3.2.0
features = ["server", "macros"]
```

本任务增加：

```text
transport-streamable-http-server
```

保持：

- server
- macros

不要启用：

- client
- transport-io
- transport-child-process
- streamable-http-client
- reqwest client
- auth
- elicitation

注意：

`transport-streamable-http-server` 自身带来的：

- server-side-http
- tower
- HTTP body utilities

属于正常依赖。

不要把这些误认为“不必要 transport”。

---

# 5. HTTP framework

优先采用 rmcp 官方兼容路径：

```text
StreamableHttpService
        ↓
axum Router
```

如果仓库目前没有 Axum：

允许增加：

```text
axum 0.8
```

只启用实际所需功能。

允许增加/调整 Tokio features，用于：

- async runtime in tests/component
- net
- task spawning
- synchronization/graceful shutdown

不要使用：

```text
tokio = { features = ["full"] }
```

除非当前仓库本来就在使用 full。

只启用需要的 features。

---

# 6. 推荐模块

建议：

```text
runtime/src/mcp/
├─ context.rs
├─ errors.rs
├─ read_file.rs
├─ glob.rs
├─ service.rs
├─ http.rs       ← 新增
└─ mod.rs
```

也可以命名：

```text
transport.rs
http_server.rs
```

但 transport 与 tool adapter 必须分离。

---

# 7. McpHttpEndpoint API

建议形成类似：

```rust
pub struct McpHttpEndpointHandle {
    ...
}
```

以及：

```rust
pub async fn start_mcp_http_endpoint(
    context: McpExecutionContext,
) -> Result<McpHttpEndpointHandle, McpHttpError>
```

Handle 至少能够提供：

```rust
local_addr()
url()
shutdown().await
```

其中：

```text
url()
```

返回：

```text
http://127.0.0.1:<actual-port>/mcp
```

不要返回：

```text
localhost
```

作为 canonical endpoint URL。

使用明确的：

```text
127.0.0.1
```

---

# 8. 必须使用 ephemeral port

绑定：

```text
127.0.0.1:0
```

让 OS 分配实际端口。

然后：

```rust
listener.local_addr()
```

得到实际端口。

禁止：

- 3000
- 3001
- 8000
- 8080
- 任意硬编码 port

作为 Runtime 默认 MCP port。

M7 以后使用 handle 返回的实际 endpoint URL。

---

# 9. 必须 loopback only

必须显式绑定：

```text
127.0.0.1
```

禁止：

```text
0.0.0.0
[::]
```

本阶段甚至不要开放 LAN bind 配置项。

V1 本地 Runtime MCP endpoint只服务本机 tunnel-client。

---

# 10. 路由

只挂：

```text
/mcp
```

MCP endpoint。

其它普通路径：

```text
/
 /foo
 /health
 /tools
```

不得自动映射到 MCP。

应返回正常 HTTP 404/405。

不要顺手加：

- health endpoint
- metrics endpoint
- debug endpoint
- Web UI

这些属于后续 lifecycle/diagnostics。

---

# 11. StreamableHttpService

使用 `rmcp 3.2.0` 官方：

```text
StreamableHttpService
```

不要自己手写：

- MCP HTTP framing
- SSE framing
- protocol version parsing
- session header handling
- initialize wire handling

这些由 SDK transport负责。

M5-D1 已完成的 `McpService` 仍是 handler/service authority。

HTTP transport只负责承载它。

---

# 12. Protocol/session compatibility

不要自行实现或强制新的 MCP lifecycle。

`rmcp 3.2.0` 已同时处理：

- 当前 2026-07-28 stateless protocol
- legacy protocol compatibility

本阶段优先保持 SDK 默认兼容行为。

不要主动设置：

```text
stateless_protocol_metadata_required = true
```

也不要为了追新协议关闭 legacy compatibility，

除非现有 Tunnel/MCP acceptance test证明这是必须的。

原因：

本项目目标是兼容真实 tunnel-client，而不是只兼容最新版测试 client。

---

# 13. json_response

不要为了简化测试强制修改 transport语义。

优先使用 `StreamableHttpServerConfig` 的兼容默认行为。

如果你确认：

```text
json_response
```

只影响响应表现而不破坏旧 client兼容，可以保持 SDK default。

不要因为测试方便自行锁定成只返回 JSON。

MCP client必须能够正常处理 SDK transport真实行为。

---

# 14. Host / DNS rebinding 防护

不能关闭 SDK 的默认 Host validation。

`rmcp 3.2.0` 默认只允许 loopback Host。

保持：

```text
localhost
127.0.0.1
::1
```

或更严格的 loopback范围。

禁止：

```rust
disable_allowed_hosts()
```

也不要加入：

```text
0.0.0.0
*
```

MCP specification 要求本地 Streamable HTTP server 防范 DNS rebinding，并推荐只绑定 localhost。

---

# 15. Origin validation

MCP specification要求对存在的 Origin 进行验证。

同时 tunnel-client 这种非浏览器 client通常不会发送 Origin。

因此采用兼容策略：

- 无 `Origin` header：允许
- 存在 `Origin` header：只允许明确 loopback origin
- 非 loopback Origin：403

不要因为 tunnel-client 没 Origin 就彻底关闭 Origin安全测试。

允许的浏览器 Origin至少限定为当前实际 endpoint 对应的 loopback origin。

例如实际端口为 P：

```text
http://127.0.0.1:P
```

如需要兼容 localhost，可明确加入对应 loopback origin。

不要允许任意：

```text
http://evil.example
https://evil.example
null
*
```

---

# 16. HTTP request size

检查：

```text
StreamableHttpServerConfig.max_request_body_bytes
```

的默认值。

本阶段不要新增 Runtime config field，

但必须保证：

- request body有明确上限
- 不关闭 SDK 已有 request body limit

如果 SDK 默认值合理：

保持默认并写测试确认巨大 body被拒绝。

如果默认值明显不合理：

使用模块内具名 constant设置保守上限。

不要增加 M0 Config Contract。

---

# 17. Graceful shutdown

必须支持显式 graceful shutdown。

Handle：

```rust
shutdown().await
```

调用后：

- listener停止接受新 connection
- rmcp transport cancellation被触发
- server task退出
- port最终释放
- 不留 detached task

优先复用：

```text
StreamableHttpServerConfig.cancellation_token
```

及 Axum/Tokio graceful shutdown机制。

不要只：

```rust
task.abort()
```

作为正常 shutdown path。

异常清理可以有 abort fallback，但正常关闭必须 graceful。

---

# 18. Drop 行为

不要依赖 async Drop。

Handle Drop 至少不能导致后台 endpoint永久泄漏。

如果无法在 Drop 内 await：

允许：

- Drop 触发 cancellation
- 正式资源 join由 `shutdown().await` 完成

测试和生产调用必须显式 shutdown。

不要让 Drop silently block无限时间。

---

# 19. Endpoint startup errors

增加集中式：

```rust
McpHttpError
```

至少区分：

- BindFailed
- LocalAddressFailed
- ServeFailed
- StartupFailed
- ShutdownFailed（如实际需要）

不要只返回：

```text
Box<dyn Error>
```

到 Runtime authority层。

错误可以保留 internal source，

但未来对 control protocol 映射时不应泄露 raw OS debug。

---

# 20. 不得改变 M5-D1 tool语义

HTTP接入后：

read_file 仍然必须调用：

```text
existing MCP adapter
→ existing Runtime read_file executor
→ existing WorkspaceGuard
```

glob 同理。

禁止在 HTTP层：

- 读文件
- walk目录
- canonicalize
- 改 tool arguments
- 改 result metadata
- 改 error category

---

# 21. Tool Catalog

HTTP client执行：

```text
tools/list
```

必须且只能得到：

```text
read_file
glob
```

并继续与 ToolRegistry exposed catalog一致。

HTTP transport不能自行创建第二套工具表。

---

# 22. HTTP E2E 测试

这一步必须真正走 TCP/HTTP。

不要只直接调用 service handler。

测试流程：

```text
temporary workspace
↓
RuntimeCore initialize
↓
McpExecutionContext
↓
start_mcp_http_endpoint
↓
actual 127.0.0.1:<port>/mcp
↓
real MCP HTTP client
↓
tools/list / tools/call
```

如果为了测试客户端需要 `rmcp` client feature：

**优先不要把 client feature加到生产 dependencies。**

可以：

- 放到 `[dev-dependencies]`
- 仅测试启用对应 client transport feature

但先检查 Cargo feature统一解析是否会让生产 build也实际启用。

如果会污染最终 rmcp feature集合，

则使用轻量 HTTP integration client构造标准 MCP请求。

优先选择不会扩大生产 dependency surface 的方式。

---

# 23. E2E：tools/list

真实 HTTP MCP client：

```text
tools/list
```

验证只有：

```text
read_file
glob
```

不得有：

- bash
- runtime.status
- diagnostics
- 其它 control methods

---

# 24. E2E：read_file

workspace：

```text
workspace/
  README.md
```

HTTP MCP调用：

```text
read_file
```

验证：

- content正确
- structured result存在
- offset/limit正确
- metadata正确

然后调用：

```text
../outside.txt
```

必须：

- 调用失败
- tool error category正确
- 不泄露 outside canonical path

---

# 25. E2E：glob

HTTP MCP调用：

```text
glob("**/*.rs")
```

验证：

- matches正确
- ordering正确
- structured result正确
- diagnostics字段存在

workspace escape/basePath非法时：

- tool failure
- 不越过 WorkspaceGuard

---

# 26. malformed HTTP / MCP request

至少测试：

- 非 JSON body
- MCP malformed arguments
- unknown tool
- 非 MCP route
- 错误 HTTP method

要求：

- server不 panic
- endpoint仍可处理后续正常请求

---

# 27. Host security tests

至少：

正常：

```text
Host: 127.0.0.1:<port>
```

允许。

恶意 Host，例如：

```text
evil.example
```

应拒绝。

不要关闭 SDK Host validation来让测试方便。

---

# 28. Origin security tests

至少：

无 Origin：

```text
allowed
```

合法 loopback Origin：

```text
allowed
```

非 loopback Origin：

```text
Origin: https://evil.example
```

必须：

```text
403
```

---

# 29. Port tests

验证：

```text
start endpoint A → port A
start endpoint B → port B
```

可以同时启动。

不要因为固定端口产生冲突。

验证 URL：

```text
http://127.0.0.1:<actual>/mcp
```

---

# 30. Shutdown tests

至少：

```text
start
↓
successful MCP request
↓
shutdown
↓
server task exits
```

然后确认该 endpoint不能继续接受请求。

如测试环境稳定允许：

确认端口可重新 bind。

不要写 timing-sensitive 长 sleep。

使用合理 timeout。

---

# 31. 不接 tunnel-client

本阶段不要运行：

```text
tunnel-client
```

不要读取：

- tunnel ID
- API key
- MCP_SERVER_URL

M7 才负责 tunnel process。

M5-D2 的 client只是本地 acceptance client。

---

# 32. 不修改 control protocol

不要给：

```text
InitializeResult
RuntimeState
```

添加：

```text
mcpUrl
mcpPort
```

这一步不解决 Runtime process endpoint discovery。

那是下一小步 M5-D3 / 后续 lifecycle integration需要单独决策的边界。

发现确实必须修改 M0 protocol才能继续时：

停止并汇报，不自行修改。

---

# 33. Dependency validation

完成后检查：

```text
cargo tree
```

明确汇报：

直接 rmcp features：

```text
server
macros
transport-streamable-http-server
```

以及：

新增直接 dependencies：

- axum（如使用）
- tokio feature变化
- 其它确实必要项

确认没有主动启用：

- rmcp client
- stdio
- child-process
- reqwest client
- auth

---

# 34. Validation

必须执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

还要执行新增 HTTP integration tests。

全部必须真实通过。

---

# 35. 完成后只汇报

1. 修改文件
2. `rmcp` features变化
3. 新增 dependencies
4. `McpHttpEndpoint` API
5. bind address / ephemeral port实现
6. `/mcp` route实现
7. Host / Origin防护
8. request body limit
9. graceful shutdown机制
10. HTTP E2E：
   - tools/list
   - read_file
   - glob
   - workspace violation
11. malformed request tests
12. concurrent endpoint / ephemeral port tests
13. shutdown tests
14. `cargo tree`结论
15. validation
16. 明确声明：
   - 未修改 control protocol
   - 未自动接入 Runtime main lifecycle
   - 未修改 tunnel-client
   - 未进入 M6

如果为了完成 D2 必须：

- 重构整个 Runtime main为 async
- 修改 M0 protocol
- 关闭 Host/Origin安全防护
- 放宽 WorkspaceGuard
- 把 MCP塞进 stdin/stdout

则停止继续实现，只汇报冲突和最小 corrective建议。