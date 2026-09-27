# MCP and Tunnel Integration

> 本文描述当前已经完成的 Runtime MCP ↔ Tunnel 集成。V2 外部链路已于 2026-09-27 验收完成。

## 1. Current Data Path

真实 MCP 数据路径是：

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
read_file / glob
```

Tauri 只负责本机 orchestration，不代理每次 MCP request。

## 2. Endpoint Handoff

Runtime 初始化 MCP HTTP server：

```text
127.0.0.1:<ephemeral>/mcp
```

Runtime 是 endpoint 的 source of truth。当前 handoff 为：

```text
Runtime creates endpoint
        ↓
mcp.endpoint.get
        ↓
Tauri validates loopback endpoint
        ↓
Tauri keeps endpoint internal
        ↓
Tunnel launch target
```

Endpoint 不写入 AppConfig，不进入 React state 或 `HarnessStatus`，Runtime stop/crash 后会清除对应 handoff 状态。

## 3. Tunnel Launch Contract

当前 tunnel-client 0.0.13 launch contract：

```text
run
--control-plane.tunnel-id <id>
--control-plane.api-key env:HARNESS_TUNNEL_API_KEY
--mcp.server-url url=<Runtime endpoint>,channel=main
--cloudflared.path <bundled companion>
--health.listen-addr 127.0.0.1:0
--health.url-file <temporary loopback health artifact>
```

输入 owner：

| Value | Owner |
| --- | --- |
| Tunnel executable | Tauri distribution resolution |
| Tunnel ID | AppConfig |
| Runtime MCP endpoint | active Runtime instance |
| API Key | SecretStore |
| cloudflared | bundled distribution |

Tauri 汇合这些值并创建一次 managed Tunnel launch。

## 4. Secret Delivery

API key 不作为 literal CLI argument。

```text
SecretStore.get
   ↓
child environment
HARNESS_TUNNEL_API_KEY
   ↓
tunnel-client uses env: reference
```

Raw API key 不应进入：

- AppConfig；
- Runtime config / Runtime；
- IPC response；
- HarnessStatus；
- Activity / Diagnostics；
- 普通日志；
- process argv。

## 5. Readiness

Tunnel spawn 成功不代表外部连接可用。

当前 readiness 流程：

```text
spawn tunnel-client
        ↓
health URL-file discovery
        ↓
loopback health endpoint
        ↓
/readyz
        ↓
control-plane connected
AND configured MCP target reachable
        ↓
Tunnel Ready
```

只有 Runtime、MCP、Tunnel 都 Ready 时，`Harness Ready` 才成立。

## 6. Lifecycle Integration

### Start

```text
Harness start
   ↓
Runtime start
   ↓
Runtime MCP endpoint
   ↓
Secret + Tunnel config
   ↓
Tunnel start
   ↓
Tunnel readiness
   ↓
Harness Ready
```

### Stop

```text
Harness stop
   ↓
Tunnel/cloudflared stop
   ↓
Runtime stop
   ↓
endpoint/session clear
```

Tunnel 必须先于 Runtime 停止，因为 Tunnel 依赖 Runtime MCP target。

## 7. Process Ownership

Windows 上每个 managed Tunnel session 使用独立 Job Object。

目标是保证：

```text
tunnel-client.exe
└── cloudflared.exe
```

属于同一个 Harness-owned process tree。graceful stop 超时后终止 Job Object，而不是按进程名杀系统中的 unrelated process。

## 8. Failure Semantics

### Runtime failure

Runtime crash / unexpected stop：

```text
Runtime != Ready
MCP != Ready
Harness = Error
associated Tunnel is stopped
```

### Tunnel failure

Tunnel crash/readiness loss：

```text
Tunnel != Ready
Harness != Ready
```

Runtime 可以保留用于诊断，显式 restart 会重新建立完整 session。

### Stale callbacks

Runtime generation 和 Tunnel instance id 绑定 active session。旧 watcher callback 不得覆盖新 session 状态。

## 9. External Acceptance

V2 的外部连接已经完成最终验收。验收期间，真实 ChatGPT caller 能通过配置后的 Harness 访问当前 Workspace，并成功调用 exposed read-only MCP tools。

详细记录见 [`ACCEPTANCE.md`](./ACCEPTANCE.md)。

这类 manual external acceptance 与自动化测试仍保持区分：自动测试验证内部 contract，外部验收验证真实 ChatGPT/Tunnel/Runtime product chain。

## 10. Non-goals of V2

V2 不扩展：

```text
write_file
shell
Git write
database execution
full approval/resume
```

这些能力必须建立在当前连接链和更完整的 approval/security model 之上。

## 11. Integration Invariants

```text
Runtime owns MCP endpoint creation.
Tauri owns Tunnel lifecycle.
Tunnel targets the actual active Runtime endpoint.
Runtime MCP stays loopback-only.
Tunnel process existence is not readiness.
Raw secrets never enter Runtime.
ChatGPT tool calls do not proxy through Tauri.
```
