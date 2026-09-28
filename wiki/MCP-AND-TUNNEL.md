# MCP and Tunnel Integration

> 本文描述当前 MCP、Tunnel 和 Tauri Harness lifecycle 的真实集成状态。V2 已完成真实链路验收并进入 Frozen；V3 计划不在本文中提前实现。

~~~yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN
~~~

## 1. Current Product Chain

~~~text
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
Policy
   ↓
read_file / glob
   ↓
Audit / Activity
   ↓
UI
~~~

Tauri 是 lifecycle / configuration / Secret orchestration layer，不是 MCP tool-call proxy：

~~~text
ChatGPT ⇄ Tunnel ⇄ Runtime MCP
~~~

## 2. Runtime MCP Endpoint

Runtime 初始化时创建 McpRuntimeService，并在 127.0.0.1:<ephemeral>/mcp 启动 Streamable HTTP endpoint。Runtime 持有实际 URL，Tauri 在 Runtime initialize / initialized 完成后调用：

~~~text
mcp.endpoint.get
~~~

Tauri 对 endpoint 做 loopback、HTTP 和 /mcp path 校验，将它绑定到当前 Runtime generation，仅供内部 Tunnel orchestration 使用；它不进入 AppConfig、RuntimeState、React state 或 HarnessStatus。Runtime stop、crash 或 generation replacement 时，endpoint 会清除。

## 3. Tunnel Launch

Tauri 的 TunnelProcessManager 使用已验证的 bundled tunnel-client 0.0.13 contract，语义上包含：

~~~text
run
--control-plane.tunnel-id <id>
--control-plane.api-key env:HARNESS_TUNNEL_API_KEY
--mcp.server-url url=<Runtime endpoint>,channel=main
--cloudflared.path <bundled companion>
--health.listen-addr 127.0.0.1:0
--health.url-file <temporary loopback health artifact>
~~~

Tunnel ID 与 Runtime MCP endpoint 来自 Harness orchestration；API Key 从 SecretStore 受控读取，只通过 child-scoped secret reference 注入。原始 key 不进入 argv、配置文件、Runtime、IPC response、Activity、diagnostics 或普通日志。

## 4. Tunnel Readiness

Tunnel process state 与 product readiness 分离：

~~~text
spawn / PID
    ≠
process running
    ≠
Tunnel Ready
    ≠
Harness Ready
~~~

Tunnel Ready 需要同时具备：

1. managed Tunnel process 仍属于当前 instance；
2. control-plane connection 已建立；
3. configured Runtime MCP target 可达；
4. tunnel-client loopback health / /readyz 权威检查通过。

health URL 必须是 loopback，discovery artifact 不含 secret，并在 startup failure、stop、restart 和可行的 crash cleanup 路径中删除。

## 5. Harness Lifecycle

### Start

~~~text
harness_start_eligibility
        ↓
Runtime start
        ↓
Runtime Ready
        ↓
mcp.endpoint.get
        ↓
MCP Ready
        ↓
SecretStore controlled read
        ↓
Tunnel launch
        ↓
health discovery / target readiness
        ↓
Tunnel Ready
        ↓
Harness Ready
~~~

Start 的每一步都由 Tauri HarnessController 串行化。任一阶段失败都会阻止 Harness Ready，并清理本 session 已启动的 Runtime、Tunnel 和临时 health artifact。

### Stop

~~~text
Harness stopping
        ↓
Tunnel / cloudflared descendants stop
        ↓
Runtime stop
        ↓
MCP endpoint shutdown
        ↓
endpoint / session clear
        ↓
Harness stopped
~~~

Windows 使用每 managed Tunnel session 一个 Job Object，graceful timeout 后清理 owned descendants；不按 unrelated process name 终止进程。

### Crash convergence

- Runtime crash：MCP / Harness 离开 Ready，关联 Tunnel 被停止，旧 endpoint 清除。
- Tunnel crash：Tunnel 与 Harness 进入 Error；Runtime 可保留作诊断，显式 Restart Harness 才重建完整链路。
- Runtime generation 或 Tunnel instance 不匹配的 watcher callback 会被忽略。

## 6. Tool Execution Path

外部 ChatGPT 的真实工具调用进入 Runtime 后仍使用 V2 原有安全执行链：

~~~text
MCP request
   ↓
McpService / adapter
   ↓
ExecutionPipeline
   ↓
argument validation
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
read_file / glob executor
   ↓
Audit / Activity
~~~

Tunnel 只负责连接和转发，不负责 Workspace enforcement、Policy evaluation、tool implementation 或 Audit。

## 7. V2 Acceptance Boundary

V2 核心能力：

- read_file
- glob
- Workspace confinement
- Policy / Audit
- Runtime MCP
- Tunnel
- Tauri lifecycle
- Tool exposure
- External ChatGPT E2E

External ChatGPT E2E 已完成 **manual acceptance（人工验收）**。这不是自动化测试通过的替代说法；自动化验证仍只声明各自实际覆盖的组件、合同、分发和安全边界。

V2 不包含 write_file、edit_file、Bash / shell、database tools、Git write 或新的 approval/resume workflow；这些属于 V3 Roadmap 的后续规划。

## 8. Source-of-Truth Locations

| Question | Source |
| --- | --- |
| Runtime MCP lifecycle | runtime/src/mcp/**, runtime/src/server.rs |
| MCP endpoint handoff | protocol/src/v1/methods.rs, ui/src-tauri/src/runtime/mod.rs |
| Runtime execution | runtime/src/execution/**, runtime/src/workspace/**, runtime/src/policy/** |
| Tunnel launch / readiness | ui/src-tauri/src/tunnel/** |
| Harness lifecycle | ui/src-tauri/src/harness.rs, ui/src-tauri/src/commands/** |
| Real UI adapter | ui/src/services/harness/local-client.ts |
| V2 acceptance | wiki/V2-ACCEPTANCE.md |
| V3 plan | docs-codex/v3/ROADMAP.md |

## 9. Related Documents

- [README.md](./README.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [CODEBASE-MAP.md](./CODEBASE-MAP.md)
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)
- [V2-ACCEPTANCE.md](./V2-ACCEPTANCE.md)
