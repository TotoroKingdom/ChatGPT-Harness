# ChatGPT-Harness Architecture

> 本文描述当前源码中的架构边界与进程关系。V2 已于 2026-09-27 完成外部产品链验收。

## 1. Architecture Summary

ChatGPT-Harness 由四个主要运行角色组成：

```text
React UI
   ↓
Tauri Host
   ├── Runtime process
   └── Tunnel process

ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
```

理解本项目的关键是区分 **Control Plane** 和 **MCP Data Plane**。

### Control Plane

```text
React UI
   ↓ Tauri IPC
Tauri HarnessController
   ↓ JSONL control RPC
Runtime
```

Tauri 负责本机配置、Secret、进程 lifecycle 和状态编排。

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
WorkspaceGuard
   ↓
PolicyEngine
   ↓
Tool Executor
```

Tauri 不位于每一次 tool call 的数据路径中。

## 2. Process Boundaries

### React UI

React UI 运行在 Tauri WebView 中，负责：

- 配置 Workspace、Tunnel ID 和连接设置。
- 短暂接收 API Key 输入并提交给 Tauri。
- 展示 Harness / Runtime / MCP / Tunnel 状态。
- 发起 Start / Stop / Restart。
- 展示 Tools、Policies、Activity 和 Diagnostics。

React 不直接 spawn Runtime 或 Tunnel，也不持久化 raw API key。

### Tauri Host

Tauri 是本机 orchestration layer，主要拥有：

```text
HarnessController
ConfigStore
SecretStore
RuntimeSupervisor
TunnelProcessManager
Tauri Commands / Events
```

`HarnessController` 是产品 lifecycle 的唯一 owner。

### Runtime

Runtime 是独立 Rust child process，内部拥有：

```text
RuntimeCore
├── Workspace / WorkspaceGuard
├── ToolRegistry
├── PolicyEngine
├── ExecutionPipeline
├── Audit / Activity
├── Diagnostics
└── MCP Service
```

Tauri 与 Runtime 使用 stdin/stdout JSONL control RPC。

### Runtime MCP

MCP Server 运行在 Runtime process 内，不是单独 OS process：

```text
Runtime Process
├── Control Server (stdin/stdout JSONL)
└── MCP HTTP Server
     └── 127.0.0.1:<ephemeral>/mcp
```

Runtime 创建并拥有 endpoint。Tauri 通过 `mcp.endpoint.get` 获取当前 active Runtime instance 的 endpoint，并只在内部用于 Tunnel launch。

### Tunnel

Tunnel Client 是由 Tauri 管理的独立进程。`TunnelProcessManager` 负责 launch、readiness、watcher、graceful stop 和 forced cleanup。

Windows 下每个 managed Tunnel session 使用独立 Job Object 管理 `tunnel-client.exe` 与其 `cloudflared.exe` descendants。

## 3. Harness Lifecycle

### Start

```text
harness_start_eligibility
        ↓
validate config / workspace / security / distribution / secret
        ↓
Harness = starting
        ↓
RuntimeSupervisor.start
        ↓
Runtime Ready + MCP endpoint
        ↓
mcp.endpoint.get + loopback validation
        ↓
SecretStore.get
        ↓
TunnelProcessManager.start
        ↓
health discovery + /readyz
        ↓
Tunnel Ready
        ↓
Harness Ready
```

`Harness Ready` 只在 Runtime、MCP、Tunnel 同时 Ready 且属于同一 active session 时成立。

### Stop

```text
Harness = stopping
        ↓
Tunnel / cloudflared cleanup
        ↓
Runtime cleanup
        ↓
session + endpoint clear
        ↓
Harness = stopped
```

### Restart

`restart` 是完整 stop 后重新加载 config/secret，再执行完整 start；不会在旧 session 上做局部复用。

## 4. State and Convergence

产品状态：

```text
lifecycle: stopped | starting | ready | degraded | error | stopping
runtime:   stopped | starting | ready | degraded | error | stopping
mcp:       stopped | starting | ready | degraded | error | stopping
tunnel:    stopped | starting | ready | degraded | error | stopping
```

`HarnessController` 使用 operation lock 串行生命周期命令。

Runtime watcher 携带 Runtime generation；Tunnel watcher 携带 Tunnel instance id。旧 session 的 stale callback 不得改变新 session 状态。

Runtime crash 会使 MCP/Harness 离开 Ready，并停止关联 Tunnel。Tunnel crash 会使 Harness 进入 Error；Runtime 可以保留用于诊断，直到显式 stop/restart。

## 5. MCP Tool Execution

当前 Runtime MCP 暴露两个只读工具：

```text
read_file
glob
```

调用链：

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
   ↓
Argument validation
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
ExecutionPermit
   ↓
Tool executor
   ↓
Audit / Activity
```

Tool executor 不负责自己重建 Workspace 或 Policy 边界。

## 6. Workspace and Policy Boundary

`WorkspaceGuard` 是文件系统访问边界的 source of truth。它处理 canonical root、relative/absolute resolution、workspace escape rejection 以及 Windows 路径相关边界。

`PolicyEngine` 在 Runtime 内执行 `ALLOW / DENY / ASK`。当前 V2 只读系统规则允许受控 Workspace read，并对非法/未知 policy context fail closed。

`ASK` 状态模型存在，但完整 approval/resume workflow 不属于 V2。

## 7. Secret Boundary

Raw API key 的预期路径只有：

```text
Settings input
   ↓
Tauri command
   ↓
SecretStore
   ↓
Windows Credential Manager
   ↓ controlled read during Tunnel launch
child environment: HARNESS_TUNNEL_API_KEY
```

Raw secret 不应进入 AppConfig、Runtime config、Runtime、HarnessStatus、Activity、Diagnostics、普通日志或 Tunnel argv。

## 8. Tunnel Readiness

Tunnel process existence 不等于 Ready。

Ready 需要 authoritative evidence，包括：

- control-plane connection；
- configured Runtime MCP target reachability；
- tunnel-client `/readyz` readiness。

因此：

```text
PID exists
≠ Tunnel Ready

Runtime Ready
≠ Harness Ready
```

## 9. Activity and Observability

Lifecycle event 与 execution activity 是不同数据来源。

Execution activity 由：

```text
ExecutionPipeline
   ↓
Audit
   ↓
ActivityStore
   ↓
control-read / UI Activity
```

Lifecycle status 由 Tauri/Harness event bridge 更新。UI 应展示真实 HarnessStatus，而不是从 Runtime readiness 推断 Tunnel 状态。

## 10. Distribution

当前 distribution 包含：

```text
chatgpt-harness-ui.exe
runtime/chatgpt-harness-runtime.exe
tunnel/tunnel-client.exe
tunnel/cloudflared.exe
distribution.json
```

仓库同时保留 NSIS 与 portable Windows 分发路径。安装/便携版都必须从打包资源解析 Runtime/Tunnel，而不能依赖开发机 PATH。

## 11. Ownership Model

| Component | Owner | Responsibility |
| --- | --- | --- |
| React UI | Tauri App | User interaction and presentation |
| Harness lifecycle | HarnessController | Product-level state and ordering |
| ConfigStore | Tauri Host | Local non-secret config |
| SecretStore | Tauri Host | Local credentials |
| Runtime process | RuntimeSupervisor | Runtime child lifecycle |
| Tunnel process | TunnelProcessManager | Tunnel tree lifecycle/readiness |
| MCP HTTP | Runtime | Local MCP transport |
| Workspace | Runtime | Filesystem boundary |
| Policy | Runtime | Execution authorization |
| Tools | Runtime | Capability implementation |
| Audit / Activity | Runtime | Execution observability |

## 12. Source-of-Truth Locations

| Question | Source |
| --- | --- |
| Control protocol | `protocol/src/**` |
| Runtime control server | `runtime/src/server.rs` |
| Runtime core | `runtime/src/state.rs` |
| Runtime MCP lifecycle | `runtime/src/mcp/runtime.rs` |
| MCP HTTP | `runtime/src/mcp/http.rs` |
| Execution | `runtime/src/execution/pipeline.rs` |
| Workspace boundary | `runtime/src/workspace/**` |
| Policy | `runtime/src/policy/**` |
| Harness lifecycle | `ui/src-tauri/src/harness.rs` |
| Runtime supervision | `ui/src-tauri/src/runtime/**` |
| Tunnel lifecycle | `ui/src-tauri/src/tunnel/**` |
| Secret storage | `ui/src-tauri/src/secrets/**` |
| React adapter | `ui/src/services/harness/local-client.ts` |
| UI orchestration | `ui/src/app/providers/HarnessDataProvider.tsx` |

## 13. Architecture Invariants

```text
Tauri manages processes; Runtime executes tools.
UI displays state; Runtime owns execution truth.
Control Plane manages lifecycle; MCP Data Plane carries tool calls.
Runtime creates MCP endpoint; Tauri does not guess it.
Tunnel owns external connectivity; Runtime MCP stays loopback.
Raw API keys do not cross into Runtime.
Harness Ready requires Runtime + MCP + Tunnel readiness.
```
