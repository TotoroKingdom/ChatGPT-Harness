# ChatGPT-Harness Architecture

> 本文描述当前源码中的架构边界与进程关系。
> 它解释“谁负责什么、谁调用谁、哪些组件位于数据路径中”，但不作为未来开发计划。
> 当前实现状态请看 [`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md)。

## 1. Architecture Summary

ChatGPT-Harness 由四个主要运行角色组成：

```text
React UI
   ↓
Tauri Host
   ↓
Runtime
   ↕
Tunnel
   ↕
ChatGPT
```

但这张图只表达组件关系，不能理解为所有请求都会按这个顺序流动。

项目实际上存在两个不同的通信平面。

### Control Plane

```text
React UI
   ↓
Tauri Host
   ↓
Runtime
```

### MCP Data Plane

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

这是理解 ChatGPT-Harness 的核心。

---

## 2. Process Boundaries

### React UI

React UI 运行在 Tauri WebView 中，主要负责：

- 展示 Runtime / MCP / Tunnel 状态
- 保存和读取用户配置
- 选择 Workspace
- 配置 Tunnel Client
- 展示 Tools / Policies / Activity / Diagnostics
- 发起 Start / Stop 等用户操作

React UI 不直接启动 Runtime，也不直接启动 Tunnel，而是通过 Tauri IPC 调用 Rust Host。

```text
React
   ↓ invoke(...)
Tauri Command
```

### Tauri Host

Tauri Host 是桌面应用的本机 orchestration layer。

它负责：

```text
ConfigStore
SecretStore
RuntimeSupervisor
TunnelProcessManager
Tauri Commands
Event Bridge
```

它拥有或管理：

- 本机配置
- API Key 等 Secret
- Runtime child process
- Tunnel child process
- UI ↔ Runtime control RPC
- Runtime lifecycle event → UI event

Tauri 的关键定位是：

> **Local lifecycle owner / orchestrator**

而不是 MCP tool execution engine。

### Runtime

Runtime 是独立 Rust child process，由 `RuntimeSupervisor` 启动并监管。

Tauri 与 Runtime 之间通过 stdin/stdout 传输 JSONL control RPC。

Runtime 内部拥有真正的执行能力：

```text
RuntimeCore
├── Workspace
├── WorkspaceGuard
├── ToolRegistry
├── PolicyEngine
├── ExecutionPipeline
├── Audit
├── Activity
├── Diagnostics
└── MCP Service
```

### Runtime MCP

MCP Server 不是独立进程。

它运行在 Runtime process 内。当前 Runtime 初始化时会启动 MCP Streamable HTTP endpoint：

```text
Runtime Process
├── Control Server
│    └── stdin/stdout JSONL
│
└── MCP HTTP Server
     └── 127.0.0.1:<ephemeral>/mcp
```

MCP HTTP service 使用 Runtime 内部独立线程和 Tokio runtime，但仍属于同一个 OS process。

因此正确理解是：

```text
Runtime
   └── MCP
```

而不是：

```text
Runtime
   ↓
MCP Process
```

### Tunnel

Tunnel Client 是独立进程。

Tauri 当前已经拥有 `TunnelProcessManager`，其内部实现了：

- start
- stop
- restart
- process ownership
- instance id
- exit watcher
- graceful stop
- forced-stop fallback

但它目前仍属于内部能力，没有进入正常 Harness lifecycle。

所以当前真实启动链仍然是：

```text
Start Harness
   ↓
Runtime
```

而不是：

```text
Start Harness
   ↓
Runtime
   ↓
Tunnel
```

---

## 3. Control Plane

Control Plane 用于管理 Runtime，而不是传输 ChatGPT 的 MCP tool calls。

```text
React UI
   ↓
Tauri Host
   ↓
Runtime Control Server
```

### UI → Tauri

React 使用 Tauri `invoke(command, args)` 调用 Rust command。

例如：

```text
runtime_start
runtime_stop
runtime_status
runtime_tools_list
runtime_get_diagnostics
runtime_get_activity_snapshot
```

### Tauri → Runtime

Tauri 的 `RuntimeSupervisor`：

1. 启动 Runtime child process
2. 保存 stdin/stdout handle
3. 给 RPC request 分配 request id
4. 写入一行 JSON
5. 等待对应 response
6. 处理 Runtime notification

简化流程：

```text
Tauri
  │
  │ request
  ↓
Runtime stdin
  │
  │ JSONL RPC
  ↓
Runtime dispatch
  │
  │ response
  ↓
Runtime stdout
  │
  ↓
Tauri
```

---

## 4. Runtime Initialization

当前 Runtime 启动链路：

```text
UI
 ↓
runtime_start
 ↓
Tauri
 ↓
RuntimeSupervisor.start
 ↓
spawn Runtime
 ↓
initialize
 ↓
RuntimeCore.initialize
 ↓
start MCP HTTP
 ↓
initialized
 ↓
Runtime Ready
```

初始化成功意味着：

- Runtime Core 已初始化
- Workspace 已建立
- ExecutionPipeline 已建立
- MCP loopback endpoint 已绑定
- Runtime 可以进入 Ready

但它不意味着：

```text
Tunnel Ready
ChatGPT Reachable
```

---

## 5. MCP Data Plane

MCP Data Plane 是 ChatGPT 实际调用工具时应该走的链路。

目标路径：

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↓
ExecutionPipeline
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
Tool
   ↓
Audit / Activity
```

这里最重要的一点是：

> **Tauri 不位于每一次 MCP tool call 的执行路径中。**

Tauri 负责启动、停止和协调组件。

真正的 MCP request 目标路径应该是：

```text
ChatGPT ⇄ Tunnel ⇄ Runtime MCP
```

而不是：

```text
ChatGPT ⇄ Tunnel ⇄ Tauri ⇄ Runtime
```

---

## 6. Tool Execution Path

当前 Runtime MCP 暴露：

```text
read_file
glob
```

一次 MCP tool call 会进入：

```text
MCP request
   ↓
McpService
   ↓
MCP adapter
   ↓
McpExecutionContext
   ↓
ExecutionPipeline
```

ExecutionPipeline 内部执行：

```text
Runtime readiness
   ↓
Argument validation
   ↓
Workspace validation
   ↓
Policy evaluation
   ↓
Tool execution
   ↓
Audit emission
   ↓
ActivityStore
```

因此 Workspace 与 Policy enforcement 位于 Runtime 内部，不依赖 UI 或 Tauri。

---

## 7. Workspace Security Boundary

Workspace 是 Runtime 的安全边界之一。

工具不会直接接受任意系统路径然后执行。

执行路径经过：

```text
Tool Request
   ↓
WorkspaceGuard
   ↓
Canonical Workspace Root
   ↓
Target Validation
   ↓
Tool Executor
```

当前 Workspace 实现负责处理：

- root canonicalization
- relative path resolution
- workspace escape rejection
- Windows 特殊路径形式
- symlink / junction 相关边界

所以安全判断的 source of truth 在：

```text
runtime/src/workspace/**
```

而不是 UI。

---

## 8. Policy Boundary

Policy evaluation 同样属于 Runtime。

```text
ExecutionPipeline
   ↓
PolicyEngine
   ↓
ALLOW / DENY / ASK
```

因此：

```text
PoliciesPage 显示什么
```

和：

```text
PolicyEngine 实际是否执行规则
```

不是同一个问题。

当前 Runtime PolicyEngine 已参与 execution pipeline。

UI policy listing 是否完整，应单独判断。

---

## 9. Activity and Observability

当前项目中存在两类不同的 activity 数据。

### Lifecycle Activity

来源：

```text
EventBus
```

主要描述 Runtime started/stopped 等 lifecycle event。

路径：

```text
Runtime
 ↓ notification
Tauri
 ↓ event
UI
```

### Execution Activity

来源：

```text
ExecutionPipeline
 ↓
Audit
 ↓
ActivityStore
```

UI 当前通过 snapshot 读取：

```text
ActivityPage
 ↓
control.activity.snapshot
 ↓
ActivityStore
```

因此当前不能把：

```text
activity.event
```

和：

```text
tool execution audit
```

理解为同一条 live stream。

---

## 10. Current Architecture

当前真实产品链路：

```text
┌───────────────────────────────┐
│       Tauri Desktop App       │
│                               │
│  React UI                     │
│      │                        │
│      ↓                        │
│  Tauri Host                   │
│      │                        │
│      │ spawn + JSONL RPC      │
└──────┼────────────────────────┘
       ↓
┌───────────────────────────────┐
│        Runtime Process        │
│                               │
│  Control Server               │
│                               │
│  RuntimeCore                  │
│      │                        │
│      └── MCP HTTP             │
│          127.0.0.1:<port>/mcp │
└───────────────────────────────┘

             X
             │
             │ not wired
             ↓

       Tunnel Process

             X
             │
             ↓

          ChatGPT
```

当前的关键断点：

```text
Runtime MCP endpoint
        ↓
     [missing]
        ↓
Tunnel launch configuration
        ↓
Tunnel
        ↓
ChatGPT
```

---

## 11. Target Architecture

目标架构：

```text
                        ┌──────────────┐
                        │   ChatGPT    │
                        └──────┬───────┘
                               ↕
                        ┌──────┴───────┐
                        │    Tunnel    │
                        └──────┬───────┘
                               ↕
┌──────────────────────────────┼────────────────────┐
│ Runtime Process              │                    │
│                              ↓                    │
│                        MCP HTTP Server             │
│                              │                    │
│                              ↓                    │
│                       ExecutionPipeline            │
│                         │     │     │              │
│                         ↓     ↓     ↓              │
│                    Workspace Policy Tools          │
│                              │                    │
│                              ↓                    │
│                       Activity / Audit             │
└───────────────────────────────────────────────────┘


┌───────────────────────────────────────────────────┐
│ Tauri Desktop App                                 │
│                                                   │
│ React UI                                          │
│    ↓                                              │
│ Tauri Host                                        │
│    ├── Runtime lifecycle                          │
│    ├── Tunnel lifecycle                           │
│    ├── Config                                     │
│    ├── Secrets                                    │
│    └── Observability                              │
└───────────────────────────────────────────────────┘
```

Tauri 对 Runtime 和 Tunnel 做 orchestration：

```text
Start Harness
    ↓
Start Runtime
    ↓
Runtime Ready
    ↓
Obtain MCP Endpoint
    ↓
Start Tunnel
    ↓
Tunnel Ready
    ↓
Harness Ready
```

停止顺序：

```text
Stop Harness
    ↓
Stop Tunnel
    ↓
Stop Runtime
```

---

## 12. Ownership Model

| Component | Owner | Responsibility |
| --- | --- | --- |
| React UI | Tauri App | User interaction and presentation |
| ConfigStore | Tauri Host | Local configuration |
| SecretStore | Tauri Host | Local credentials |
| Runtime process | RuntimeSupervisor | Runtime lifecycle |
| Tunnel process | TunnelProcessManager | Tunnel lifecycle |
| Workspace | Runtime | Filesystem boundary |
| Tools | Runtime | Capability implementation |
| Policy | Runtime | Execution authorization |
| ExecutionPipeline | Runtime | Safe execution |
| MCP HTTP | Runtime | MCP transport |
| Audit / Activity | Runtime | Execution observability |

---

## 13. Source-of-Truth Locations

| Architecture Question | Source |
| --- | --- |
| Control protocol | `protocol/src/**` |
| Runtime control server | `runtime/src/server.rs` |
| Runtime core ownership | `runtime/src/state.rs` |
| Runtime MCP lifecycle | `runtime/src/mcp/runtime.rs` |
| MCP HTTP transport | `runtime/src/mcp/http.rs` |
| Tool execution | `runtime/src/execution/pipeline.rs` |
| Workspace boundary | `runtime/src/workspace/**` |
| Policy | `runtime/src/policy/**` |
| Tauri app setup | `ui/src-tauri/src/lib.rs` |
| Runtime supervision | `ui/src-tauri/src/runtime/**` |
| Tunnel lifecycle | `ui/src-tauri/src/tunnel/**` |
| Tauri commands | `ui/src-tauri/src/commands/**` |
| React → Tauri adapter | `ui/src/services/harness/local-client.ts` |
| UI orchestration | `ui/src/app/providers/HarnessDataProvider.tsx` |

---

## 14. Architecture Rules of Thumb

When reading or modifying the codebase, keep these rules in mind:

```text
Tauri manages processes.
Runtime executes tools.

UI displays state.
Runtime owns execution truth.

Control Plane manages Runtime.
MCP Data Plane carries tool calls.

Runtime Ready
≠ Tunnel Ready

MCP endpoint bound
≠ ChatGPT reachable

Tunnel manager exists
≠ Tunnel orchestration exists
```

---

## 15. Related Documents

For repository and call-chain details:

- [`CODEBASE-MAP.md`](./CODEBASE-MAP.md)

For implementation status:

- [`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md)

For Wiki entry point:

- [`README.md`](./README.md)

For future development direction:

- [`../docs-codex/ROADMAP.md`](../docs-codex/ROADMAP.md)
