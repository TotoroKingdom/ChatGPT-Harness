# MCP and Tunnel Integration

> 本文描述 ChatGPT-Harness 当前 MCP 与 Tunnel 的真实集成状态、责任边界和连接断点。
>
> 本文以当前源码为事实来源。未来开发方向以 `docs-codex/ROADMAP.md` 为准。

## 1. Purpose

ChatGPT-Harness 的目标外部数据链是：

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↓
ExecutionPipeline
   ↓
Workspace / Policy / Tools
```

其中：

- Runtime 提供本机 MCP Server。
- Tunnel 把外部 ChatGPT 连接转发到本机 Runtime MCP。
- Tauri 负责本机 Runtime 与 Tunnel 的生命周期、配置和 Secret。
- React UI 展示状态并触发生命周期操作。

最关键的架构规则是：

```text
ChatGPT ⇄ Tunnel ⇄ Runtime MCP
```

而不是：

```text
ChatGPT ⇄ Tunnel ⇄ Tauri ⇄ Runtime
```

Tauri 是 orchestration layer，不是 MCP tool-call proxy。

---

## 2. Current State

当前链路已经完成到：

```text
UI
 ↓
Tauri
 ↓
Runtime
 ↓
MCP HTTP
```

但没有继续完成：

```text
MCP HTTP
 ↕
Tunnel
 ↕
ChatGPT
```

因此当前产品状态可以概括为：

```text
Runtime MCP             Implemented
Loopback HTTP           Implemented
Tunnel process manager  Implemented / Internal
Tunnel orchestration    Not Implemented
ChatGPT reachability    Not Implemented
```

---

## 3. Runtime MCP

Runtime 初始化时会创建 `McpRuntimeService`。

它负责：

```text
start
bind endpoint
store endpoint URL
shutdown
join service thread
```

当前 MCP endpoint 由 Runtime 绑定在：

```text
127.0.0.1:<ephemeral-port>/mcp
```

其中端口由操作系统动态分配。

Runtime 启动成功后，`McpRuntimeService` 保存实际 endpoint URL：

```text
McpRuntimeService
└── endpoint_url
```

这说明：

```text
Runtime 已经知道 MCP 在哪里
```

但当前这个信息仍然主要停留在 Runtime 内部。

---

## 4. Runtime MCP Lifecycle

简化生命周期：

```text
Runtime initialize
    ↓
build McpExecutionContext
    ↓
McpRuntimeService.start
    ↓
start MCP HTTP thread
    ↓
bind 127.0.0.1:0
    ↓
obtain actual URL
    ↓
store endpoint_url
    ↓
Runtime Ready
```

停止：

```text
Runtime stop / process exit
    ↓
McpRuntimeService.shutdown
    ↓
signal MCP thread
    ↓
stop HTTP endpoint
    ↓
join thread
```

因此 MCP Server 的生命周期属于 Runtime 生命周期。

---

## 5. MCP Tool Path

当前 MCP Server 可以暴露 Runtime 的有效工具。

当前主要工具：

```text
read_file
glob
```

调用路径：

```text
ChatGPT / MCP Client
        ↓
MCP HTTP
        ↓
McpService
        ↓
MCP Tool Adapter
        ↓
McpExecutionContext
        ↓
ExecutionPipeline
        ↓
WorkspaceGuard
        ↓
PolicyEngine
        ↓
Tool Executor
        ↓
Audit / Activity
```

所以真正的安全执行逻辑全部位于 Runtime 内。

Tunnel 不负责：

- Workspace enforcement
- Policy evaluation
- Tool implementation
- Audit logic

Tunnel 只负责连接和转发。

---

## 6. Tunnel Process Manager

Tauri 当前已经构造 `TunnelProcessManager`。

它已经具备基础进程管理能力：

```text
TunnelProcessManager
├── start
├── stop
├── stop_instance
├── restart
├── status
├── process ownership
├── instance_id
├── exit watcher
├── graceful termination
└── forced kill fallback
```

Tunnel instance 当前会记录：

```text
instance_id
pid
tunnel_id
runtime_endpoint_ref
started_at
executable_path
```

这说明 Tunnel lifecycle 基础设施已经存在。

但是：

> `TunnelProcessManager` 存在，不代表 Tunnel 已进入产品启动链。

---

## 7. Current Tunnel Spawn Behavior

当前 production spawn 的本质仍然是：

```text
Command::new(tunnel-client)
```

然后配置：

```text
stdin  → null
stdout → null
stderr → null
```

当前正常 production spawn 尚未真正消费：

```text
runtime endpoint
tunnel id
API key
```

也没有形成完整的 tunnel-client launch contract。

因此：

```text
TunnelLaunch 中有字段
```

不等于：

```text
这些字段已经传给 tunnel-client
```

---

## 8. Current Product Wiring

Tauri application setup 当前会创建：

```text
RuntimeSupervisor
TunnelProcessManager
```

但是正常 Tauri command surface 主要注册的是：

```text
runtime_start
runtime_stop
runtime_status
...
```

没有完整的：

```text
tunnel_start
tunnel_stop
tunnel_status
```

产品级 lifecycle command。

当前：

```text
runtime_start
```

只启动 Runtime。

它不会自动：

```text
启动 Tunnel
```

所以真实链路仍然是：

```text
Start Harness
    ↓
Runtime
    ↓
Runtime MCP
```

而不是：

```text
Start Harness
    ↓
Runtime
    ↓
Tunnel
    ↓
Harness Ready
```

---

## 9. Current Connection Break

当前最关键的连接断点位于：

```text
Runtime
   ↓
MCP endpoint URL
   ↓
[not surfaced through product orchestration]
   ↓
Tauri
   ↓
[not delivered to Tunnel launch]
   ↓
Tunnel
```

更完整地看：

```text
Runtime MCP
└── http://127.0.0.1:<random>/mcp
             │
             │ Runtime knows URL
             │
             X
             │ no complete product handoff
             ↓
         Tauri Host
             │
             X
             │ no complete launch contract
             ↓
      Tunnel Client
             │
             X
             ↓
          ChatGPT
```

这就是当前 V2 `Connect` 阶段最核心的问题。

---

## 10. Configuration and Secrets

当前 UI/Tauri 已经具备保存本机配置和 Secret 的基础能力。

相关信息包括：

```text
Workspace
Tunnel Client path
Tunnel ID
API Key reference / secret
```

但需要区分：

```text
配置能够保存
```

和：

```text
Tunnel 启动时真正消费这些配置
```

当前后者尚未闭合。

尤其：

```text
SecretStore 能保存 API Key
```

不等于：

```text
TunnelProcessManager 已经把 API Key 安全交给 tunnel-client
```

---

## 11. Required Connect Boundary

V2 Connect 的目标不是重新设计 MCP，也不是重写 Tunnel manager。

需要闭合的是这一条边界：

```text
Runtime MCP endpoint
        ↓
Tauri orchestration
        ↓
Tunnel launch configuration
        ↓
Tunnel process
```

Connect 完成后至少应该能够形成：

```text
Runtime starts
    ↓
MCP endpoint becomes available
    ↓
Tauri obtains endpoint
    ↓
Tauri resolves Tunnel config + Secret
    ↓
Tauri starts Tunnel
    ↓
Tunnel targets Runtime MCP
```

---

## 12. Endpoint Ownership

MCP endpoint 的 source of truth 应仍然属于 Runtime。

原因：

```text
Runtime 创建 endpoint
Runtime 决定端口
Runtime 控制 endpoint 生命周期
```

Tauri 不应该自己猜测 MCP URL。

正确关系：

```text
Runtime
   │
   │ reports actual endpoint
   ↓
Tauri
```

而不是：

```text
Tauri
   │
   │ assumes port
   ↓
Runtime
```

尤其当前端口是 ephemeral port，不能依赖固定端口约定。

---

## 13. Tunnel Ownership

Tunnel process 的生命周期 owner 应属于 Tauri。

正确关系：

```text
Tauri
   ↓
TunnelProcessManager
   ↓
Tunnel process
```

Runtime 不应该负责启动 tunnel-client。

原因是 Tunnel 依赖：

- 本机 executable
- user configuration
- API Key / Secret
- application lifecycle
- desktop-level restart/stop behavior

这些都属于 Tauri Host 的本机 orchestration responsibility。

---

## 14. Target Startup Sequence

目标启动顺序：

```text
User: Start Harness
        ↓
Tauri loads config
        ↓
Tauri validates local setup
        ↓
Tauri starts Runtime
        ↓
Runtime initializes
        ↓
Runtime starts MCP HTTP
        ↓
Runtime reports actual MCP endpoint
        ↓
Tauri resolves Tunnel Client
        ↓
Tauri reads Tunnel ID
        ↓
Tauri obtains API Key from SecretStore
        ↓
Tauri starts Tunnel
        ↓
Tunnel reaches Running / Ready
        ↓
Harness Ready
```

这个顺序保证：

> Tunnel 不会在 Runtime MCP endpoint 尚未存在时提前启动。

---

## 15. Target Shutdown Sequence

目标停止顺序：

```text
User: Stop Harness
        ↓
Tauri stops Tunnel
        ↓
Tunnel exits
        ↓
Tauri stops Runtime
        ↓
Runtime stops MCP endpoint
        ↓
Harness Stopped
```

Tunnel 应先于 Runtime 停止。

原因是：

```text
Tunnel depends on Runtime MCP
```

如果先停止 Runtime，会造成 Tunnel 短暂指向已经不存在的本机 endpoint。

---

## 16. Failure Semantics

统一 lifecycle 后，需要明确几个基本失败场景。

### Runtime start fails

```text
Runtime start
   ↓ fail
Tunnel must not start
Harness = error / stopped
```

### Runtime MCP endpoint fails

```text
Runtime process exists
   ↓
MCP bind fails
   ↓
Runtime initialization fails
   ↓
Tunnel must not start
```

### Tunnel start fails

```text
Runtime Ready
   ↓
Tunnel start fails
```

这时不能宣称：

```text
Harness Ready
```

Runtime 可以继续存在还是回滚停止，应由 orchestration policy 明确定义。

### Tunnel crashes

```text
Runtime Ready
Tunnel Failed
```

UI 必须能够区分：

```text
Runtime Ready
MCP Local Ready
Tunnel Failed
Harness Not Ready
```

不能把 Runtime 状态直接投影成 Tunnel 状态。

---

## 17. Readiness Model

需要区分至少四个概念：

```text
Runtime Process Running
Runtime Ready
MCP Local Ready
Tunnel Running / Ready
```

最终 Harness Ready 应表达：

```text
Runtime Ready
AND
MCP endpoint available
AND
Tunnel usable
```

不能继续使用：

```text
Runtime Ready
⇒ MCP Ready
⇒ Tunnel Ready
```

这种隐含推导。

---

## 18. External Reachability

即使：

```text
Tunnel process = Running
```

也不一定能够证明：

```text
ChatGPT can reach Runtime MCP
```

因此状态语义应该尽量区分：

```text
process running
local endpoint ready
external reachability verified
```

V2 Validate 阶段才负责证明完整链：

```text
ChatGPT
   ↓
Tunnel
   ↓
Runtime MCP
   ↓
read_file / glob
```

---

## 19. V2 Connection Scope

按照当前 Roadmap，V2 分为：

```text
Connect
   ↓
Orchestrate
   ↓
Validate
```

本文主要对应 `Connect`。

Connect 应聚焦：

```text
Runtime MCP endpoint
        ↕
Tunnel
```

而不是同时扩展：

```text
Bash
Database tools
Write tools
Git write
Approval UX redesign
```

先让最小 read-only MCP 产品链工作。

---

## 20. What Connect Should Not Redesign

Connect 阶段原则上不需要重写这些已存在模块：

```text
WorkspaceGuard
PolicyEngine
ExecutionPipeline
read_file
glob
ActivityStore
RuntimeSupervisor
TunnelProcessManager lifecycle core
```

除非连接过程中发现明确阻塞性缺陷。

Connect 的核心工作应该围绕：

```text
handoff + launch + state
```

---

## 21. Minimal Connection Contract

从架构角度看，连接至少需要三个输入：

```text
Tunnel executable
Tunnel ID
Runtime MCP endpoint
```

以及认证信息：

```text
API Key / credential
```

这些输入由不同 owner 提供：

| Value | Source of Truth |
| --- | --- |
| Tunnel executable | Tauri local config / discovery |
| Tunnel ID | Tauri local config |
| Runtime MCP endpoint | Runtime |
| API Key | Tauri SecretStore |

Tauri 的作用是汇合这些信息并创建实际 Tunnel launch。

---

## 22. Security Rules

### API Key

API Key 不应写入：

```text
Git
public config
logs
activity payload
diagnostics output
```

如果 tunnel-client 支持比命令行参数更安全的 credential delivery 方式，应优先使用不会轻易暴露在进程列表中的方式。

具体机制应以实际 tunnel-client 支持的接口为准。

### MCP Endpoint

Runtime MCP 当前绑定 loopback：

```text
127.0.0.1
```

这意味着 MCP 本身保持本机入口。

外部暴露职责属于 Tunnel。

不应为了连接 Tunnel 而直接把 Runtime MCP 改成：

```text
0.0.0.0
```

除非未来有明确重新设计和安全评审。

---

## 23. Current vs Target

### Current

```text
                  ┌─────────────┐
                  │   ChatGPT   │
                  └──────X──────┘
                         │
                  ┌──────X──────┐
                  │   Tunnel    │
                  │ manager only│
                  └──────X──────┘
                         │
                 endpoint handoff
                     missing
                         │
┌────────────────────────┴─────────────────┐
│ Runtime                                  │
│                                          │
│ MCP HTTP                                 │
│ 127.0.0.1:<ephemeral>/mcp                │
│      ↓                                   │
│ ExecutionPipeline                        │
│      ↓                                   │
│ Workspace / Policy / Tools               │
└──────────────────────────────────────────┘
```

### Target

```text
                  ┌─────────────┐
                  │   ChatGPT   │
                  └──────┬──────┘
                         ↕
                  ┌──────┴──────┐
                  │   Tunnel    │
                  └──────┬──────┘
                         ↕
┌────────────────────────┴─────────────────┐
│ Runtime                                  │
│                                          │
│ MCP HTTP                                 │
│      ↓                                   │
│ ExecutionPipeline                        │
│      ↓                                   │
│ Workspace / Policy / Tools               │
└──────────────────────────────────────────┘

Tauri Host:
- owns Runtime lifecycle
- obtains MCP endpoint
- owns Tunnel lifecycle
- supplies local configuration
- supplies credential securely
- exposes real state to UI
```

---

## 24. Definition of Connected

`Connect` 可以认为完成，当且仅当至少满足：

```text
1. Runtime 启动后，Tauri 能得到实际 MCP endpoint。
2. Tauri 能使用真实 Tunnel executable 启动 Tunnel。
3. Tunnel 启动参数真正包含所需 Tunnel ID。
4. Tunnel 真正指向 Runtime MCP endpoint。
5. Tunnel 真正获得所需 credential。
6. Tunnel status 来自真实 manager，而不是 UI mock/projection。
7. Runtime stop 前能够停止 Tunnel。
```

这仍然不等于 V2 全部完成。

之后还需要：

```text
Orchestrate
Validate
```

---

## 25. Definition of External E2E

完整外部 E2E 至少应证明：

```text
ChatGPT
   ↓
Tunnel
   ↓
Runtime MCP
   ↓
tools/list
   ↓
read_file / glob
   ↓
WorkspaceGuard
   ↓
Policy / Execution
   ↓
Activity
   ↓
UI observable
```

同时需要验证：

```text
Stop Harness
```

能够正确关闭：

```text
Tunnel
Runtime
MCP endpoint
```

---

## 26. Source-of-Truth Locations

| Question | Source |
| --- | --- |
| Runtime MCP lifecycle | `runtime/src/mcp/runtime.rs` |
| MCP HTTP endpoint | `runtime/src/mcp/http.rs` |
| MCP service/tool adapter | `runtime/src/mcp/service.rs` |
| Runtime initialization | `runtime/src/server.rs` |
| Runtime state | `runtime/src/state.rs` |
| Tunnel process lifecycle | `ui/src-tauri/src/tunnel/mod.rs` |
| Tauri command surface | `ui/src-tauri/src/commands/mod.rs` |
| Tauri app registration | `ui/src-tauri/src/lib.rs` |
| Config persistence | `ui/src-tauri/src/config/mod.rs` |
| Secret storage | `ui/src-tauri/src/secrets/mod.rs` |
| Real UI adapter | `ui/src/services/harness/local-client.ts` |
| Current implementation status | `wiki/IMPLEMENTATION-MATRIX.md` |
| Overall architecture | `wiki/ARCHITECTURE.md` |
| Future V2 path | `docs-codex/ROADMAP.md` |

---

## 27. Related Documents

Wiki entry point:

- [`README.md`](./README.md)

Architecture:

- [`ARCHITECTURE.md`](./ARCHITECTURE.md)

Detailed code map:

- [`CODEBASE-MAP.md`](./CODEBASE-MAP.md)

Implementation status:

- [`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md)

Future V2 path:

- [`../docs-codex/ROADMAP.md`](../docs-codex/ROADMAP.md)
