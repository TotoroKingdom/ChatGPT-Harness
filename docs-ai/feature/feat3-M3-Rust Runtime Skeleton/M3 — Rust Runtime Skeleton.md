你现在要一次性完成 ChatGPT-Harness Roadmap **M3 — Rust Runtime Skeleton**。

这是实现任务。直接检查当前 M0/M1/M2 代码并完成 M3。

不要输出 spec / plan / task 分解。

不要重新设计 M0–M2。

# 1. 最高约束

先读取：

* `AGENTS.md`
* `docs-ai/PRODUCT.md`
* `docs-ai/ARCHITECTURE.md`
* `docs-ai/Roadmap.md`
* `docs/IPC.md`
* `docs/CONFIG.md`
* `protocol/`
* 当前完整 `ui/`

优先级：

```text
PRODUCT
> ARCHITECTURE
> Roadmap
> M0 frozen protocol
> M1/M2 architecture
> 本任务
```

M3 的核心原则：

```text
替换 Mock Runtime
!=
重构 UI
```

现有以下内容原则上不应重做：

```text
First Run
Settings
ConfigStore
SecretStore
页面布局
HarnessDataProvider
HarnessControlClient facade
M2 lifecycle UI
```

---

# 2. M3 最终目标

实现真正独立的 Rust Runtime：

```text
Tauri Backend
    │
    │ spawn
    ↓
chatgpt-harness-runtime.exe
    │
    ↕ stdin/stdout
    │ UTF-8 JSONL
    │ M0 Control Protocol
    ↓
Runtime Core
```

实际走通：

```text
Start Harness
→ Tauri spawn Runtime
→ initialize
← initialize result
→ initialized
→ runtime.status
→ Runtime Ready

Stop Harness
→ runtime.stop
→ graceful shutdown
→ process exits
→ UI recognizes Stopped
```

同时：

```text
强制结束 Runtime process
→ Tauri detects exit
→ UI recognizes Runtime crash/error
```

---

# 3. M3 不实现

绝对不要实现：

```text
MCP server
read_file
glob
WorkspaceGuard
PathResolver
Policy Engine
真实 Approval execution
bash
真实 tunnel-client process
MCP reachability
Git tools
Database tools
Agent loop
Skills
Memory
```

Tunnel 和 MCP 仍未真实接入。

不要为了让 Overview 显示全部 Ready 而假装真实 tunnel/MCP 已工作。

---

# 4. 新增 Runtime crate

在根目录新增：

```text
runtime/
├─ Cargo.toml
└─ src/
   ├─ main.rs
   ├─ server.rs
   ├─ state.rs
   ├─ events.rs
   └─ handlers/
```

按实际复杂度适当调整，不要过度拆分。

crate：

```text
chatgpt-harness-runtime
```

生成：

```text
chatgpt-harness-runtime.exe
```

加入根 Cargo workspace。

Runtime 必须直接依赖：

```text
harness-control-protocol
```

禁止复制 M0 protocol structs。

---

# 5. Runtime 是独立 executable

Runtime 必须能够脱离 Tauri 单独启动：

```text
chatgpt-harness-runtime.exe
```

启动后：

* stdin 接收 JSONL；
* stdout 只输出 Control Protocol JSONL；
* stderr 输出 tracing/logging；
* 不打开 GUI；
* 不监听 TCP；
* 不使用 Named Pipe；
* 不读取 Frontend；
* 不依赖 React/Tauri types。

stdout 绝不能出现：

```text
debug print
println!("started")
panic trace
log line
```

普通日志全部 stderr。

---

# 6. Config loading

Roadmap 中的 Runtime Config loading 必须遵守已经冻结的 M0。

Runtime **不得读取 persistent AppConfig**：

```text
%APPDATA%\...\config.json
```

Runtime 配置来源只能是：

```text
Tauri ConfigStore
→ RuntimeConfigSnapshot
→ initialize RPC
→ Runtime
```

因此：

```text
AppConfig authority = Tauri
RuntimeConfigSnapshot consumer = Runtime
```

Runtime initialize 成功后保存一份本进程 immutable/effective snapshot。

至少使用：

```text
configRevision
workspace
security
runtimeLimits
```

API Key 不得进入 Runtime。

Tunnel credential 不得进入 Runtime。

---

# 7. Runtime ID

每次 Runtime process 启动生成新的：

```text
runtimeId
```

使用 UUID。

必须满足：

```text
start process A → runtimeId A
stop
start process B → runtimeId B
A != B
```

不要使用 PID 作为 runtimeId。

---

# 8. RuntimeState

直接使用 M0 canonical：

```text
RuntimeState
```

不要定义第二套业务 RuntimeState。

初始化成功后至少正确报告：

```text
runtimeId
startedAt
lifecycle
readiness
configRevision
activeWorkspace
securityMode
exposedToolCatalog
activeRequests
pendingApprovals
runningExecutions
lastError
```

M3 因为还没有 Tool implementation：

```text
exposedToolCatalog = []
```

不要提前伪造：

```text
read_file
glob
```

它们属于 M5。

正常 initialized 后：

```text
lifecycle = ready
readiness = ready
```

停止过程中：

```text
lifecycle = stopping
```

---

# 9. Initialization state machine

严格实现 M0：

```text
PROCESS_STARTED
      ↓
WAITING_INITIALIZE
      ↓ initialize
INITIALIZED_PENDING_ACK
      ↓ initialized
READY
      ↓ runtime.stop
STOPPING
      ↓
EXIT
```

规则：

### initialize 前

除 `initialize` 外：

```text
NOT_INITIALIZED
```

### 重复 initialize

返回：

```text
ALREADY_INITIALIZED
```

### protocolVersion 不匹配

返回：

```text
PROTOCOL_VERSION_MISMATCH
```

并不得进入 Ready。

### initialized

必须在 initialize 成功后才接受。

---

# 10. JSONL Runtime server

实现稳定的 async read loop：

```text
stdin
→ read line
→ parse RpcMessage
→ dispatch
→ serialize
→ stdout
→ newline
→ flush
```

要求：

* 单个 malformed JSON 不 panic；
* 返回正确 Parse/Invalid Request error；
* EOF 触发 Runtime clean exit；
* stdout 写失败退出；
* 不允许一个坏 request 破坏整个 Runtime state；
* request ID 原样返回。

不要自己创建另一套 RPC envelope。

使用 M0：

```text
RpcRequest
RpcResponse
RpcNotification
RpcErrorResponse
```

---

# 11. Runtime handlers

M3 至少真实实现：

```text
initialize
initialized
runtime.status
runtime.stop
```

为了让现有五页在切换真实 Runtime 后仍然工作，同时为其它 M0 方法提供 contract-correct skeleton：

```text
tools.list
→ []

policies.list
→ []

activity.list
→ real Runtime lifecycle events

activity.subscribe
→ real subscription

approvals.list
→ []

approvals.resolve
→ APPROVAL_NOT_FOUND

diagnostics.run
→ Runtime skeleton diagnostics
```

这不是实现 Tool/Policy/Approval Engine。

只是让 frozen control contract 完整可调用。

---

# 12. Structured Event Bus

实现真正 Runtime-owned：

```text
Structured Event Bus
```

不要用 raw log 代替。

至少产生：

```text
runtime_started
runtime_stopped
```

如果有 Runtime-level失败，可按 M0 event types产生合适结构化事件。

每个事件必须使用：

```text
ActivityEvent
```

sequence 使用 M0 已修复的：

```text
DecimalU64 / decimal string wire representation
```

Runtime 内部维护：

```text
u64
```

wire 输出 decimal string。

---

# 13. Event history

M3 实现简单 bounded in-memory ring buffer。

大小使用：

```text
RuntimeConfigSnapshot.runtimeLimits.maxActivityEvents
```

不要实现 disk audit persistence。

那属于后续 milestone。

`activity.list` 必须支持：

```text
afterSequence
limit
```

并返回：

```text
items
latestSequence
```

---

# 14. activity.subscribe

严格实现 M0 subscription。

客户端调用：

```text
activity.subscribe
```

之后 Runtime 通过：

```text
activity.event
```

notification 推送新事件。

同一个 stdio connection 即当前唯一 client。

M3 不需要复杂 multi-client subscription manager。

Runtime process 只有一个 control connection：

```text
parent Tauri process
```

即可。

---

# 15. runtime.stop

收到：

```text
runtime.stop
```

时：

1. transition → stopping；
2. 产生 `runtime_stopped` event；
3. 返回：

```text
RuntimeStopResult {
  accepted: true
}
```

4. flush stdout；
5. cleanly exit process。

不要：

```text
process::exit()
```

直接跳过清理。

使用正常 shutdown signal/state，让主 event loop结束。

---

# 16. Tauri Runtime Process Supervisor

在：

```text
ui/src-tauri/
```

新增真正：

```text
RuntimeSupervisor
```

建议职责：

```text
spawn
handshake
RPC request correlation
notification routing
process observation
graceful stop
crash detection
process identity
```

不要把这些塞进 React。

不要把所有逻辑塞进一个 Tauri command。

---

# 17. Process spawning

使用 Rust process API，例如：

```text
tokio::process::Command
```

配置：

```text
stdin piped
stdout piped
stderr piped
kill_on_drop
CREATE_NO_WINDOW on Windows if appropriate
```

不要弹出 Runtime CMD window。

Runtime 是后台子进程。

---

# 18. Runtime executable discovery

M3 development 至少支持：

### 1

显式开发 override：

```text
CHATGPT_HARNESS_RUNTIME_PATH
```

仅用于 dev/test binary discovery，不是 AppConfig。

### 2

正常优先查找 Tauri executable sibling：

```text
<tauri-exe-directory>\chatgpt-harness-runtime.exe
```

开发 workspace 中，Tauri app 和 Runtime 应尽量构建到相同 target directory。

不要硬编码：

```text
D:\ChatGPT-Harness
```

不要把 runtime path 放入 AppConfig。

未来 packaging/bundled sidecar 属于 M9。

---

# 19. Dev build flow

保证：

```text
npm run tauri dev
```

之前或过程中 Runtime binary 会被构建。

可以调整 package script，使：

```text
cargo build -p chatgpt-harness-runtime
```

先执行，然后启动 Tauri dev。

目标是用户不用开第二个终端手动 build Runtime。

不要实现 release bundling。

---

# 20. Runtime RPC client

Tauri Backend 建立真正的：

```text
RuntimeRpcClient
```

它必须：

* stdin 写 Request；
* stdout 独立 reader task；
* request ID correlation；
* pending request map；
* response routing；
* notification routing；
* bounded request timeout；
* process exit时 reject 所有 pending requests。

不要：

```text
send request
then synchronously read next line
```

因为 notification 和 response 可以交错。

---

# 21. Handshake

`RuntimeSupervisor.start()`：

```text
load authoritative AppConfig
↓
construct RuntimeConfigSnapshot
↓
spawn runtime
↓
start stdout reader
↓
send initialize
↓
validate protocolVersion/result
↓
send initialized notification
↓
runtime.status
↓
mark runtime connected/ready
```

Frontend 不负责构造 RuntimeConfigSnapshot。

Frontend 不负责发送 initialize。

这全部属于 Tauri Backend。

---

# 22. Process Identity

不能只保存 PID。

Tauri supervisor 至少跟踪：

```text
pid
runtimeId
runtimeVersion
executablePath
startedAt
workspace
configRevision
```

可以新增 Tauri-local：

```text
RuntimeProcessIdentity
```

它不是 M0 RuntimeState，也不要硬塞进 Runtime wire model。

这是：

```text
process observed state
```

与：

```text
RuntimeState
```

保持分离。

---

# 23. Crash detection

spawn 后启动 process wait monitor。

如果 child 在没有正常 stop 的情况下退出：

```text
Runtime process exited unexpectedly
```

Supervisor 必须：

* 清理 stdin/client handle；
* 清理 pending requests；
* 标记 process absent；
* 保存 exit code/error summary；
* 通知 frontend；
* 不继续显示 Runtime Ready；
* 不把旧 RuntimeState 当活状态。

Frontend Overview 应显示 Runtime：

```text
Error / Disconnected
```

以及合适的人类可读信息。

不要 panic Tauri application。

---

# 24. Normal stop vs crash

Supervisor 必须区分：

```text
requested stop
```

和：

```text
unexpected exit
```

正常：

```text
runtime.stop
→ process exits
→ Stopped
```

异常：

```text
process exits without requested stop
→ Error / Crashed
```

不要把两者都显示成 Stopped。

---

# 25. Start conflict

如果已有真实 Runtime process alive：

再次：

```text
Start Harness
```

不得 spawn 第二个。

如果：

```text
active configRevision == desired configRevision
```

返回当前 active state / already active semantic。

如果：

```text
active configRevision != desired
```

继续使用 M2：

```text
Config Mismatch
Restart Required
```

不要偷偷启动第二个 Runtime。

---

# 26. Config mismatch

M2 已有逻辑必须继续成立：

```text
AppConfig revision = N + 1
RuntimeState revision = N
```

UI：

```text
Restart Required
```

Restart：

```text
runtime.stop
→ wait process exit
→ start new Runtime
→ initialize snapshot N+1
→ new RuntimeState revision N+1
```

然后 mismatch 消失。

---

# 27. Frontend Real Client

保留：

```text
HarnessControlClient
```

增加真正的 Tauri-backed implementation，例如：

```text
TauriHarnessControlClient
```

页面不能改成直接：

```ts
invoke(...)
```

数据流仍然：

```text
Page
→ HarnessDataProvider
→ HarnessControlClient
→ TauriHarnessControlClient
→ Tauri Backend
→ Runtime
```

---

# 28. Mock policy

正常 Tauri desktop execution 默认使用：

```text
TauriHarnessControlClient
```

不要默认再使用 Mock Runtime。

Mock 可以继续保留：

```text
?mock=ready
?mock=error
...
```

用于 frontend tests / development scenarios。

但 production/default Tauri flow 必须是真 Runtime。

不要删除仍有测试价值的 Mock。

---

# 29. Runtime/Tunnel/MCP 状态

M3 后：

### Runtime

必须是真实状态。

### Tunnel

仍未接入真实 process。

### MCP

仍未实现。

因此不要把：

```text
Tunnel = Ready
MCP = Ready
```

冒充真实结果。

建议明确表现为：

```text
Not integrated / Unavailable
```

或现有模型中最接近的非 Ready 状态，并给出：

```text
Available in later milestone
```

整体 Harness 在只有 Runtime Ready 时可以是：

```text
Degraded
```

而不是谎报：

```text
Ready
```

不要为此重做页面布局。

只调整状态语义。

---

# 30. Tools / Policies 页面

真实 Runtime M3：

```text
tools.list = []
policies.list = []
```

因此页面应自然进入现有：

```text
EmptyState
```

不要为了让页面“好看”继续从 Mock 混入工具。

M5 才实现：

```text
read_file
glob
```

---

# 31. Activity Page

M3 后 Activity 默认改为真实 Runtime events。

至少能看到：

```text
Runtime started
Runtime stopped
```

通过：

```text
activity.list
+
activity.subscribe
```

不要继续把 Mock activity 与真实 Runtime activity 混在一起。

---

# 32. Diagnostics

M2 Tauri local diagnostics 保留：

```text
Workspace
Tunnel ID
API Key
tunnel-client directory
security config
```

M3 可以增加 Runtime-side：

```text
Runtime control protocol initialized
Runtime config snapshot loaded
Runtime ready
```

但不要宣称：

```text
MCP reachable
Tunnel connected
WorkspaceGuard secure
```

因为尚未实现。

---

# 33. Tauri commands

具体命名可以根据当前代码，但至少实现清晰 adapter：

```text
runtime_start
runtime_status
runtime_stop
```

其它 Runtime control methods可通过统一 internal RuntimeRpcClient。

不要把 raw JSON RPC envelope暴露给 React。

---

# 34. stderr handling

Runtime stderr 必须被 Tauri 异步 drain。

否则 pipe buffer 满后可能阻塞 Runtime。

可以：

```text
stderr → tracing/debug log
```

但：

* 不进入 stdout；
* 不默认进入 Activity；
* 不包含 secret；
* 不直接 dump 到 UI。

---

# 35. Request timeout

Tauri `RuntimeRpcClient` 所有 request 必须有 bounded timeout。

例如：

```text
5s
```

初始化可以稍长。

超时：

* 返回结构化 Tauri control error；
* 不永久挂起；
* pending map清理。

不要实现复杂 retry framework。

---

# 36. Runtime exit on parent loss

stdio EOF：

```text
Tauri disappears
→ Runtime stdin EOF
```

Runtime 应结束。

同时 child process 设置：

```text
kill_on_drop
```

避免产生孤儿 Runtime。

M3 不实现 daemon/service 模式。

---

# 37. Tests — Runtime

至少覆盖：

### protocol state

```text
status before initialize → NOT_INITIALIZED
initialize → success
duplicate initialize → ALREADY_INITIALIZED
protocol mismatch → PROTOCOL_VERSION_MISMATCH
initialized before initialize → rejected
```

### status

initialize + initialized 后：

```text
runtime.status
```

返回正确 snapshot。

### stop

```text
runtime.stop
```

返回 accepted 并结束 server。

### malformed input

非法 JSON 不 panic。

### event bus

```text
sequence monotonic
bounded buffer
activity.list afterSequence
subscription notification
```

---

# 38. Tests — Supervisor

至少覆盖可单测部分：

```text
request id correlation
response routing
notification routing
timeout cleanup
unexpected EOF cleanup
```

如果环境允许，增加真正 process integration test：

```text
spawn chatgpt-harness-runtime
→ initialize
→ status
→ stop
→ exit success
```

强烈建议实现。

---

# 39. Crash integration test

必须验证真实异常退出。

可以：

```text
spawn Runtime
→ handshake
→ kill child process externally/from test
→ supervisor detects exit
→ observed state != Ready
→ marked unexpected/crashed
```

不要为测试新增 public：

```text
runtime.crash
```

RPC。

---

# 40. Frontend tests

现有 M1/M2 tests 必须继续通过。

新增最少覆盖：

```text
real-client adapter maps Tauri status
runtime disconnected
runtime crashed
restart required
empty tools/policies with real M3 runtime
```

Mock scenario tests继续存在。

---

# 41. 不要改视觉布局

M3 是 Runtime milestone。

禁止重新设计：

```text
First Run
Overview
Settings
Sidebar
Tools
Policies
Activity
```

最多增加必要状态文本：

```text
Runtime connected
Runtime crashed
Tunnel not integrated
MCP not integrated
```

不要做 UI redesign。

---

# 42. README / dev instructions

更新必要开发说明：

```text
how runtime binary is built
how Tauri discovers runtime
CHATGPT_HARNESS_RUNTIME_PATH
how to manually run runtime
how to test crash detection
```

保持简短。

不要新写大型架构文档。

---

# 43. 验证

必须实际执行：

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Protocol：

```text
cargo run -p harness-control-protocol --bin protocol-export -- generate-all
```

如果 M0 contract 未修改，生成结果不得产生意外 diff。

Frontend：

```text
cd ui
pnpm typecheck
pnpm test
pnpm build
```

Desktop：

```text
npm run tauri dev
```

---

# 44. GUI 人工验收

必须实际走：

```text
启动 app
→ 配置存在
→ Overview
→ Start Harness
```

确认：

```text
真正出现 chatgpt-harness-runtime.exe 子进程
```

然后：

```text
Runtime → Ready
runtimeId 有值
configRevision 与 saved config 一致
Workspace 与配置一致
```

Tools：

```text
Empty
```

Policies：

```text
Empty
```

Activity：

能看到真实：

```text
runtime_started
```

然后：

```text
Stop Harness
```

确认：

```text
runtime.stop
→ chatgpt-harness-runtime.exe 正常退出
→ UI Runtime = Stopped
```

---

# 45. Crash 人工验收

再次 Start。

然后使用 Task Manager / PowerShell / taskkill 强制结束：

```text
chatgpt-harness-runtime.exe
```

UI 必须在合理时间内：

```text
不再显示 Ready
```

并显示：

```text
Runtime crashed / disconnected
```

Tauri 主应用不得崩溃。

然后必须还能：

```text
Start Harness
```

重新生成新的 runtimeId。

---

# 46. Restart Required 人工验收

Start Runtime：

```text
active revision = N
```

Settings 修改并保存：

```text
saved revision = N+1
```

确认：

```text
Restart Required
```

点击 Restart：

```text
old Runtime graceful stop
→ process exits
→ new Runtime process
→ new runtimeId
→ configRevision N+1
→ Restart Required disappears
```

---

# 47. Definition of Done

只有全部满足才关闭 M3：

* 新增独立 Rust Runtime executable；
* Runtime 可以独立启动；
* Runtime 使用 M0 protocol crate；
* stdio JSONL server 工作；
* stdout 只包含 protocol；
* initialize / initialized 工作；
* RuntimeConfigSnapshot 真正被加载；
* Runtime ID 每次启动唯一；
* `runtime.status` 真实可用；
* `runtime.stop` graceful shutdown 可用；
* Structured Event Bus 可用；
* `activity.list/subscribe` 可用；
* Process identity 不只依赖 PID；
* Tauri RuntimeSupervisor 可 spawn真实 Runtime；
* Tauri 可进行 request/response correlation；
* Runtime stderr 被 drain；
* Runtime crash 可以识别；
* pending RPC 在 crash 后正确失败；
* Runtime 重启后产生新 runtimeId；
* Config Mismatch/Restart Required 继续工作；
* 默认 Tauri flow 使用真实 Runtime；
* Mock 仅保留测试/显式开发场景；
* Tools/Policies 不再混入 Mock 数据；
* Tunnel/MCP 不谎报真实 Ready；
* M1/M2 页面无需架构重构；
* Cargo tests/check 全通过；
* Frontend typecheck/tests/build 全通过；
* Tauri 实际 Start/Stop Runtime 成功；
* 人工 kill Runtime 后 UI 正确识别；
* 未实现任何 M4+ 功能。

完成后只汇报：

1. Runtime crate 主要结构；
2. stdio JSONL server 如何实现；
3. initialize / status / stop 如何实现；
4. Structured Event Bus 如何实现；
5. Tauri RuntimeSupervisor 如何实现；
6. Process Identity 包含什么；
7. crash detection 如何实现；
8. Mock Runtime 还保留在哪里、默认是否已切真实 Runtime；
9. Tunnel/MCP 在 M3 如何展示；
10. 自动化验证结果；
11. Start/Stop/Crash/Restart Required 人工验收结果；
12. 是否有未完成项或约束冲突。

不要进入 M4。
