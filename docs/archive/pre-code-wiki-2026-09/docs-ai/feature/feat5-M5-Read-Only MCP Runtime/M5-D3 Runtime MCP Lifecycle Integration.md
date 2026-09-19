实施 M5-D3：Runtime MCP Lifecycle Integration。

## 0. 最高约束

遵守：

- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- `docs/IPC.md`
- 冻结的 `protocol/`
- M3 Runtime lifecycle
- M4 Workspace Security Boundary
- M5-A Tool Registry
- M5-B read_file
- M5-C glob
- M5-D1 MCP Service Adapter
- M5-D2 Loopback Streamable HTTP Endpoint

本任务只负责：

```text
Runtime control lifecycle
        ↓
MCP HTTP endpoint lifecycle
```

不要实现 tunnel-client。
不要进入 M6。
不要修改 UI。
不要修改 control protocol schema。

---

# 1. 目标状态

当前：

```text
Runtime process
├─ stdin/stdout control protocol
└─ MCP HTTP endpoint 尚未自动启动
```

完成后：

```text
Runtime process
│
├─ stdin/stdout
│    └─ existing control protocol
│
└─ background MCP HTTP service
     └─ 127.0.0.1:<ephemeral>/mcp
```

两个协议必须完全独立。

---

# 2. 不要把整个 Runtime 改成 async

当前 `server.rs` control loop 是同步 stdin/stdout。

禁止为了 MCP HTTP：

- 把整个 `main.rs/server.rs` 重写成 async
- 把 control protocol 改成 Tokio stdin/stdout
- 把 RuntimeCore 全局包成 `Arc<Mutex<RuntimeCore>>`
- 大规模重写已经稳定的 M3 lifecycle

采用：

**同步 control loop + 独立 MCP background thread + thread 内专用 Tokio runtime**

---

# 3. 新增 Runtime MCP service manager

建议新增类似：

```rust
McpRuntimeService
```

可放：

```text
runtime/src/mcp/runtime.rs
```

或：

```text
runtime/src/runtime_services.rs
```

职责：

```text
start(context)
↓
spawn dedicated OS thread
↓
create Tokio runtime inside thread
↓
start_mcp_http_endpoint(context)
↓
return startup result to control thread
```

以及：

```text
shutdown()
↓
signal async thread
↓
McpHttpEndpointHandle::shutdown().await
↓
join OS thread
```

---

# 4. 推荐同步 API

control server 不应该处理 async Handle。

提供同步 facade，例如概念上：

```rust
pub struct McpRuntimeService {
    ...
}

impl McpRuntimeService {
    pub fn start(
        &mut self,
        context: McpExecutionContext,
    ) -> Result<(), McpRuntimeServiceError>;

    pub fn endpoint_url(&self) -> Option<&str>;

    pub fn shutdown(
        &mut self,
    ) -> Result<(), McpRuntimeServiceError>;
}
```

`endpoint_url()` 本阶段：

- crate-private/internal 即可
- 用于测试和未来 M7 integration

不要加入 M0 protocol。

---

# 5. Background thread

MCP HTTP server运行在独立 OS thread。

thread 内创建专用 Tokio runtime。

不要使用全局 runtime。

优先最小 Tokio runtime：

- IO enabled
- time enabled（仅 SDK/timeout确实需要）
- 不启用 `tokio/full`

允许 current-thread runtime，只要 rmcp/Axum 测试证明工作正常。

如果 SDK 确实要求 multi-thread runtime：

使用最小 multi-thread配置并说明原因。

---

# 6. Startup handshake

后台线程启动 endpoint 后，必须把 startup result同步传回 control thread。

可以使用：

- `std::sync::mpsc`
- Tokio oneshot + 同步包装
- 等价小型机制

要求：

`McpRuntimeService::start()` 返回 `Ok` 时：

```text
listener 已成功 bind
actual port 已知
/mcp server task 已建立
endpoint URL 已确定
```

不能：

```text
spawn thread
→ 立即返回 Ok
→ thread稍后才发现 bind失败
```

否则 initialize 会虚假成功。

---

# 7. Endpoint 启动时机

在处理 control protocol：

```text
initialize
```

时：

```text
validate InitializeParams
↓
RuntimeCore::initialize(config)
↓
construct initialized McpExecutionContext
↓
start McpRuntimeService
↓
只有二者都成功
↓
返回 InitializeResult
```

目标：

成功的 `initialize` 响应意味着：

- Workspace 初始化成功
- Runtime security state存在
- MCP HTTP listener已经成功启动

---

# 8. MCP startup failure 必须回滚 initialize

非常重要。

如果：

```text
RuntimeCore::initialize
```

已经成功，但：

```text
McpRuntimeService::start
```

失败，

则本次 initialize 整体必须视为失败。

不能留下：

- config
- events
- WorkspaceGuard
- MCP context
- InitializedPendingAck phase

增加一个最小 crate-private rollback API，或等价实现。

例如概念：

```rust
RuntimeCore::rollback_initialize()
```

要求回到与新建 Runtime 相同的未初始化业务状态：

```text
phase = WaitingInitialize
config = None
events = None
workspace_guard = None
MCP readiness = false
```

`runtime_id` / process `started_at` 可保持进程身份不变。

不要通过重新构造整个 RuntimeCore 改 runtime_id。

---

# 9. initialize MCP startup error 映射

MCP endpoint bind/start失败不是：

```text
INVALID_PARAMS
```

因为用户 workspace/config参数未必有错。

应返回 control protocol已有的内部错误类别，例如：

```text
JSON-RPC INTERNAL_ERROR
MachineErrorCode::InternalError
```

或仓库当前等价的稳定错误分类。

message 必须脱敏，例如：

```text
Failed to start Runtime MCP endpoint.
```

不要把：

- raw OS bind error
- socket内部信息
- Rust Debug dump

直接发给 UI。

内部可以保留 source error。

---

# 10. initialized acknowledgement 与 MCP readiness

这里冻结一个重要行为：

**HTTP listener 可以在 initialize 成功后存在，但 MCP tools 在收到 control protocol `initialized` acknowledgement 之前不能执行。**

也就是说：

```text
initialize response success
↓
HTTP endpoint 已存在
↓
Runtime phase = InitializedPendingAck
↓
MCP tool call => RuntimeNotReady
↓
control notification: initialized
↓
Runtime phase = Ready
↓
MCP tool execution allowed
```

这样 MCP capability 与 Runtime readiness保持一致。

---

# 11. 增加共享 readiness gate

不要让 MCP service读取/锁住整个 RuntimeCore。

在 initialized execution context 中加入一个非常小的共享 readiness gate，例如：

```rust
Arc<AtomicBool>
```

或等价结构。

初始：

```text
false
```

`RuntimeCore::acknowledge()` 成功进入 Ready时：

```text
true
```

Runtime停止时：

```text
false
```

MCP tool adapter调用现有 executor前：

```text
ensure_runtime_ready()
```

如果 false：

返回已有/新增稳定的 MCP：

```text
runtime_not_ready
```

tool error。

不要执行 filesystem executor。

---

# 12. readiness gate 不得成为第二套 Phase

`Phase` 仍然是 Runtime lifecycle authority。

Atomic readiness 只是供跨线程 MCP execution读取的 projection。

要求：

```text
Phase::Ready
→ readiness gate = true
```

其他 phase：

```text
false
```

不要允许独立代码随意修改 readiness。

只有 Runtime lifecycle方法负责更新。

---

# 13. D1 测试语义同步

M5-D1 原有直接 service测试如果：

```text
initialize
→ 未 acknowledge
→ 直接 tool call
```

需要更新。

正确流程：

```text
initialize
→ acknowledge
→ MCP tool call
```

并新增：

```text
initialize
→ 未 acknowledge
→ MCP tool call
→ RuntimeNotReady
```

不要删除原有 security assertions。

---

# 14. initialized notification

当前：

```text
initialized
```

notification触发：

```rust
RuntimeCore::acknowledge()
```

保持这一 contract。

acknowledge成功后：

- Phase → Ready
- MCP readiness gate → true
- RuntimeStarted event保持原语义

重复 `initialized`：

- 不重复创建 endpoint
- 不重复启动线程
- 不产生第二个 port
- 不改变 endpoint URL

---

# 15. runtime.stop

处理：

```text
runtime.stop
```

时必须：

```text
MCP readiness = false
↓
RuntimeCore stop transition
↓
gracefully shutdown MCP endpoint
↓
join MCP background thread
↓
write RuntimeStopResult
↓
control loop exit
```

可以根据现有事件语义微调 stop event顺序，

但必须确保：

**返回 stop response / process退出前，MCP endpoint已经停止。**

不要正常路径只调用：

```rust
thread.abort()
```

---

# 16. stdin EOF

当前 control stdin EOF 会使 Runtime process结束。

D3 后：

```text
stdin EOF
↓
MCP readiness=false
↓
graceful MCP shutdown
↓
join background thread
↓
return Ok
```

不能因为 UI/control process断开，让 MCP listener残留后台。

---

# 17. unexpected control-loop error

对于：

- stdin read IO error
- stdout fatal IO error
- run loop异常退出

保证 `McpRuntimeService` Drop至少触发 cancellation。

优先在正常 error返回路径显式 shutdown。

Drop 是最后防线，不是正常生命周期实现。

---

# 18. McpRuntimeService Drop

Drop：

- 触发 cancellation/shutdown signal
- 不无限阻塞
- 不 panic

正式 join应由：

```text
shutdown()
```

完成。

如果 thread已异常结束：

shutdown应安全返回结构化错误或已停止状态，

不得 deadlock。

---

# 19. 重复 start / shutdown

定义明确行为：

重复 `start()`：

如果已运行：

返回：

```text
AlreadyRunning
```

或等价结构化错误。

不要再启动第二个 endpoint。

重复 `shutdown()`：

最好 idempotent：

```text
Ok(())
```

如果已经停止。

避免 stop cleanup复杂化。

---

# 20. Runtime 服务状态 ownership

control server应拥有：

```text
RuntimeCore
McpRuntimeService
```

二者生命周期同属于一个 Runtime process。

不要把 `McpRuntimeService` 放进：

```text
ToolRegistry
WorkspaceGuard
McpService
```

这些层级。

---

# 21. endpoint URL

D3 内部保存实际：

```text
http://127.0.0.1:<port>/mcp
```

但本任务：

**不要把 URL 加到：**

- `InitializeResult`
- `RuntimeState`
- config schema
- Activity contract
- protocol generated files

原因：

endpoint discovery 是 M7 tunnel lifecycle需要单独冻结的 process/control contract。

D3 只保证 Runtime内部能够获得 URL。

---

# 22. 明确记录后续 M7 unresolved item

代码注释不要写 TODO essay。

但完成汇报必须指出：

```text
M7 仍需要决定 tunnel supervisor 如何获得 Runtime MCP endpoint URL。
```

不要在 D3 偷偷通过：

- stderr文本
- 临时文件
- activity event metadata
- environment variable回写

建立隐式 side channel。

---

# 23. 测试：initialize 生命周期

至少：

### valid initialize

```text
initialize
→ response success
→ MCP endpoint已监听
→ phase InitializedPendingAck
```

### invalid workspace

```text
initialize fails
→ MCP endpoint未启动
→ RuntimeCore保持 WaitingInitialize
```

---

# 24. 测试：pre-ack MCP

流程：

```text
initialize success
↓
获得内部 endpoint URL
↓
真实 HTTP MCP call
```

在未收到：

```text
initialized
```

前：

`tools/call read_file/glob`

必须返回：

```text
runtime_not_ready
```

且 executor未执行。

---

# 25. 测试：post-ack MCP

```text
initialize
↓
initialized notification
↓
MCP read_file
MCP glob
```

真实 HTTP调用成功。

验证：

- read_file正确
- glob正确
- WorkspaceGuard仍生效

---

# 26. 测试：stop lifecycle

流程：

```text
initialize
↓
initialized
↓
successful MCP request
↓
runtime.stop
```

验证：

- stop response正常
- endpoint无法继续连接
- background MCP thread退出
- port释放

不要使用长时间 sleep。

使用 bounded timeout / join。

---

# 27. 测试：EOF cleanup

启动 Runtime host/control loop：

```text
initialize
initialized
```

随后关闭 stdin / 输入结束。

验证：

- MCP endpoint关闭
- background thread退出
- run loop返回

---

# 28. 测试：duplicate initialized

```text
initialize
↓
initialized
↓
record endpoint URL
↓
initialized again
```

验证：

- URL相同
- 未启动第二 endpoint
- 无额外 MCP thread

---

# 29. 测试：retry invalid initialize

保持 M4/M5已有语义：

```text
invalid initialize
↓
valid initialize
```

第二次必须成功。

invalid initialize期间：

- 无 MCP listener
- 无 leaked thread

---

# 30. 测试：endpoint startup rollback

必须覆盖：

```text
core initialize succeeds
MCP endpoint startup fails
```

然后确认：

```text
core.config == None
core.events == None
workspace_guard == None
phase == WaitingInitialize
```

并且下一次 initialize可以重试。

不要为了制造 bind failure引入大型 dependency injection框架。

允许使用一个非常小的：

- local starter abstraction
- cfg(test) fake starter
- test-only injected start function

但不要把测试 hook暴露为公共 API。

---

# 31. 测试：readiness gate

至少测试：

```text
WaitingInitialize => false
InitializedPendingAck => false
Ready => true
Stopping => false
```

不能出现：

```text
Phase != Ready
但 MCP readiness=true
```

---

# 32. 保持 D2 transport security

Lifecycle integration不得改变：

- `127.0.0.1:0`
- `/mcp`
- Host validation
- Origin validation
- 4 MiB request limit
- graceful HTTP shutdown

不要为了 thread integration关闭任何 transport security。

---

# 33. 不进入 M6

本任务不要添加：

```text
MCP Request
→ Policy Engine
```

当前仍然是：

```text
MCP Request
→ MCP adapter
→ existing executor
```

M6 才插入统一：

```text
Validation
→ Workspace Guard
→ Policy
→ Executor
→ Audit
```

不要提前重构。

---

# 34. 不接 tunnel-client

不要：

- 启动 tunnel-client
- 添加 tunnel config
- 读取 API key
- 读取 tunnel ID
- 修改 Tauri Supervisor
- 增加 MCP URL control-protocol field

这些属于 M7。

---

# 35. Validation

执行并全部通过：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

另外确认：

```text
cargo tree
```

没有因为 D3新增：

- rmcp client
- stdio transport
- tokio/full
- 无关 HTTP client

---

# 36. 完成条件

最终 Runtime lifecycle应为：

```text
process start
↓
WaitingInitialize
↓
initialize
├─ workspace/security init
└─ MCP HTTP endpoint start
↓
InitializedPendingAck
↓
initialized notification
↓
MCP execution enabled
↓
Ready
↓
runtime.stop / EOF
↓
MCP execution disabled
↓
MCP HTTP graceful shutdown
↓
background thread joined
↓
process exits
```

---

# 37. 完成后只汇报

1. 修改文件
2. Runtime/MCP lifecycle ownership结构
3. background thread + Tokio runtime结构
4. endpoint startup handshake
5. initialize failure rollback
6. readiness gate实现
7. initialized acknowledgement行为
8. runtime.stop cleanup
9. stdin EOF cleanup
10. endpoint URL内部保存方式
11. lifecycle tests
12. startup failure/retry tests
13. HTTP E2E regression结果
14. cargo tree结论
15. validation
16. 明确声明：
    - 未修改 control protocol
    - endpoint URL尚未暴露给 UI/Supervisor
    - 未修改 tunnel-client
    - 未进入 M6
17. 明确记录：
    - M7 尚需冻结 Supervisor 获取 Runtime MCP endpoint URL 的方式

如果实现过程中发现必须：

- 修改 M0 protocol
- 把 RuntimeCore全局 Mutex化
- 把 control loop整体 async化
- 放宽 WorkspaceGuard
- 用 stderr/临时文件偷偷传 endpoint URL

则停止扩大实现，汇报冲突和最小 corrective建议。