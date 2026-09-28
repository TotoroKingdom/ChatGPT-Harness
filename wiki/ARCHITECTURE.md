# ChatGPT-Harness Architecture

> 本文描述当前源码中的架构边界与进程关系。V2 已完成真实链路验收并冻结；V3 规划不属于当前架构事实。

~~~yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN
~~~

## 1. Architecture Summary

当前产品由 React UI、Tauri Host、Runtime、Tunnel 和外部 ChatGPT 连接组成，但请求执行与生命周期控制属于两条不同的平面：

~~~text
Control Plane:
React UI ↔ Tauri HarnessController ↔ Runtime control RPC

MCP Data Plane:
ChatGPT ↔ Tunnel ↔ Runtime MCP
                         ↓
              Workspace / Policy / Tools
~~~

Tauri 是本机 lifecycle owner / orchestrator，不是每一次 MCP tool call 的 proxy。

## 2. Process Boundaries

### React UI

React UI 运行在 Tauri WebView 中，负责配置输入、Start / Stop / Restart 操作、状态展示、Tools / Policies / Activity / Diagnostics 页面。它通过 Tauri IPC 调用 Rust Host，不直接启动 Runtime 或 Tunnel。

### Tauri Host

Tauri Host 管理：

~~~text
ConfigStore
SecretStore
RuntimeSupervisor
TunnelProcessManager
HarnessController
Tauri Commands / Event Bridge
~~~

HarnessController 是产品级生命周期 owner，负责 eligibility、统一状态、session / generation、rollback、crash convergence 和 cleanup。

### Runtime

Runtime 是由 RuntimeSupervisor 管理的 Rust child process，拥有：

~~~text
Workspace
WorkspaceGuard
ToolRegistry
PolicyEngine
ExecutionPipeline
Audit / Activity
Diagnostics
MCP Service
~~~

Tauri 与 Runtime 之间通过 stdin/stdout JSONL control RPC 通信。

### Runtime MCP

MCP Server 运行在 Runtime process 内，不是独立的 MCP process。Runtime 初始化时绑定 loopback ephemeral endpoint：

~~~text
Runtime
├── Control Server       stdin/stdout JSONL
└── MCP HTTP Server      127.0.0.1:<ephemeral>/mcp
~~~

Runtime 通过 mcp.endpoint.get 返回当前实例的 endpoint；Tauri 只在内部保存和校验该 URL，并在 Runtime stop/crash 时清除。

### Tunnel

Tunnel Client 是由 Tauri 管理的独立 child process。TunnelProcessManager 负责：

- bundled / development distribution resolution；
- Tunnel ID、Runtime MCP endpoint 和 secret-reference launch；
- loopback health URL discovery 与 /readyz readiness；
- process ownership、instance id、exit watcher；
- graceful stop、Windows Job Object forced cleanup；
- readiness loss 与 crash state。

Tunnel Ready 不是 PID、spawn 成功或监听端口的别名；它需要 control-plane connection、configured MCP target reachability 和权威 health check。

## 3. Control Plane

桌面控制链路为：

~~~text
React UI
   ↓ invoke / events
Tauri HarnessController
   ↓ JSONL RPC
Runtime Control Server
~~~

生命周期命令由 Tauri 统一持有：

~~~text
harness_start_eligibility
harness_start
harness_status
harness_stop
harness_restart
~~~

Runtime-specific tools、policy、activity、diagnostics 读取仍通过 Runtime control RPC 完成，但 UI 不再自己拼接完整 Harness 生命周期。

## 4. MCP Data Plane and Tool Execution

实际调用路径为：

~~~text
ChatGPT
   ↓
Tunnel
   ↓
Runtime MCP
   ↓
McpService / adapter
   ↓
ExecutionPipeline
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
read_file / glob executor
   ↓
Audit / Activity
~~~

Workspace 与 Policy enforcement 位于 Runtime 内部，不依赖 UI 或 Tauri。V2 不包含 write tools、Bash、database tools 或 Git write operations。

## 5. Harness Lifecycle

### Start

~~~text
validate eligibility
        ↓
allocate session / mark starting
        ↓
start Runtime
        ↓
Runtime Ready
        ↓
mcp.endpoint.get + loopback/path validation
        ↓
MCP Ready
        ↓
SecretStore controlled read
        ↓
launch Tunnel
        ↓
health discovery + /readyz + target evidence
        ↓
Tunnel Ready
        ↓
Harness Ready
~~~

Harness Ready 只有在同一 active session 中 Runtime、MCP、Tunnel 都 Ready 时成立。

### Stop

~~~text
mark stopping
        ↓
stop Tunnel / cloudflared descendants
        ↓
stop Runtime
        ↓
clear endpoint / session
        ↓
Harness stopped
~~~

### Failure convergence

- Runtime start 或 MCP bind 失败时，Tunnel 不会进入 Ready。
- Tunnel start / readiness 失败时，Harness 不会进入 Ready，并执行 rollback。
- Runtime crash 会使 MCP / Harness 离开 Ready，并停止关联 Tunnel。
- Tunnel crash 会使 Harness 进入 Error；Runtime 可保留用于诊断，但需要显式 Restart Harness。
- watcher callback 带 session / generation identity；stale callback 不得修改新 session。

## 6. Security Boundary

- Runtime MCP 只绑定 loopback。
- WorkspaceGuard 在 Runtime 中执行 canonicalization、escape rejection 和 Windows path boundary 检查。
- PolicyEngine 在 ExecutionPipeline 中执行 ALLOW / DENY / ASK 决策。
- API Key 只通过 Tauri SecretStore 和 child-scoped secret reference 进入 Tunnel launch；不进入 AppConfig、Runtime、IPC response、Activity、diagnostics、普通日志或 literal argv。
- Windows Tunnel child 使用每 session 的 Job Object；强制清理针对 owned descendants，不按进程名杀 unrelated process。

## 7. Current Architecture

~~~text
┌───────────────────────────────┐
│       Tauri Desktop App       │
│ React UI ↔ HarnessController  │
│ Config / Secrets / Lifecycle  │
└──────────────┬────────────────┘
               │
               ├── RuntimeSupervisor ── JSONL ── Runtime
               │                              └─ loopback MCP
               │
               └── TunnelProcessManager ── Tunnel Client
                                              └─ MCP target = Runtime

ChatGPT ↔ Tunnel ↔ Runtime MCP
                    ↓
             Workspace / Policy / Tools
                    ↓
                Audit / Activity → UI
~~~

这条链路的外部 ChatGPT E2E 已完成 manual acceptance；manual acceptance 不等同于自动化测试声明。

## 8. Source-of-Truth Locations

| Architecture question | Source |
| --- | --- |
| Control protocol | protocol/src/** |
| Runtime control server | runtime/src/server.rs |
| Runtime core and execution | runtime/src/state.rs, runtime/src/execution/** |
| Workspace boundary | runtime/src/workspace/** |
| Policy | runtime/src/policy/** |
| MCP lifecycle / HTTP | runtime/src/mcp/** |
| Runtime supervision / endpoint handoff | ui/src-tauri/src/runtime/** |
| Tunnel launch / readiness / cleanup | ui/src-tauri/src/tunnel/** |
| Harness lifecycle | ui/src-tauri/src/harness.rs, ui/src-tauri/src/commands/** |
| React adapter / provider | ui/src/services/harness/**, ui/src/app/providers/** |
| V2 acceptance | wiki/V2-ACCEPTANCE.md |
| V3 plan | docs-codex/v3/ROADMAP.md |

## 9. Related Documents

- [CODEBASE-MAP.md](./CODEBASE-MAP.md)
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)
- [MCP-AND-TUNNEL.md](./MCP-AND-TUNNEL.md)
- [V2-ACCEPTANCE.md](./V2-ACCEPTANCE.md)
- [../docs-codex/v3/ROADMAP.md](../docs-codex/v3/ROADMAP.md)
