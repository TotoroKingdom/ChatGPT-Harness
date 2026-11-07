# MCP and Tunnel Integration

> 本文描述当前 main / HEAD（0.1.3）的 MCP、Approval、History 与 Tunnel 集成。旧 V2 “read-only tools only” 描述已经不适用于当前源码。MCP protocol revision 固定为 `2025-03-26`。

## 1. 当前真实链路

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↓
McpService
   ↓
ExecutionPipeline
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
Approval / SessionGrant（按需）
   ↓
read_file / glob / grep / write / edit / bash
   ↓
Audit / Activity
```

Tauri 是 lifecycle / config / secret orchestration layer，不是 MCP tool-call proxy。

## 2. Runtime MCP Endpoint

Runtime 初始化后在：

```text
127.0.0.1:<ephemeral>/mcp
```

启动 Streamable HTTP MCP endpoint。Runtime 使用 `rmcp 3.2.0`，MCP protocol revision 固定为 `2025-03-26`；兼容性身份清单位于 `fixtures/v4/m10-mcp/compatibility-v1.json`。

Tauri 通过：

```text
mcp.endpoint.get
```

取得当前 Runtime generation 的 endpoint，并进行 loopback/path 等校验。该 endpoint 用于 Tunnel target orchestration，不应被当作持久用户配置。

Runtime stop/crash/generation replacement 时，旧 endpoint 必须失效并被清理。

## 3. MCP Tool Exposure

Runtime declared catalog：

```text
read_file
glob
grep
write
edit
bash
```

MCP `tools/list` 不等于“列出 registry 的全部工具”。它读取 Runtime effective catalog，只保留：

```text
Available
AvailableWithApproval
```

因此：

- disabled tool 不暴露；
- Policy DENY 不暴露；
- Developer/Test mode 不满足时 Bash 不暴露；
- Bash executable unavailable 时 Bash 不暴露；
- Runtime not ready 不暴露。

这也是为什么 UI 的“已保存配置”与 ChatGPT 实际可发现工具之间可能暂时不同：Runtime generation 尚未 restart/converge 时，effective state 仍以当前运行实例为准。

## 4. Tool Call

文件工具与 Bash 最终都进入 Runtime authority：

```text
MCP arguments
→ adapter validation
→ Workspace / capability checks
→ Policy
→ Approval / SessionGrant
→ ExecutionPermit
→ executor
```

已实现工具：

| Tool | Capability | 关键行为 |
| --- | --- | --- |
| read_file | WorkspaceRead | bounded workspace file read |
| glob | WorkspaceRead | workspace path matching |
| grep | WorkspaceRead | bounded workspace text / regex search |
| write | WorkspaceWrite | atomic UTF-8 create/overwrite |
| edit | WorkspaceWrite | unique exact replacement + atomic mutation |
| bash | DeveloperShell | controlled Git Bash async execution |

## 5. ASK / Approval

当 effective Policy = ASK 时，工具仍可出现在 `tools/list`，但执行必须获得 Approval。

当前模型：

```text
MCP call
→ ASK
→ ApprovalRequest(Pending)
→ exact invocation waits
→ Tauri/UI approvals.resolve
→ AllowOnce / AllowForSession / Deny
→ exact invocation resumes or rejects
```

默认 Approval TTL = 5 分钟。

### Allow once

只授权当前 request。

### Allow for session

创建 memory-only Session Grant：

- 当前 Runtime generation；
- 当前 tool；
- exact normalized scope。

它不是“以后这个工具都允许”。

### Deny / Expired / Cancelled

调用不会继续执行受保护副作用，并产生对应 Activity/Audit evidence。

## 6. Bash 的 MCP 特性

Bash 是 async MCP execution：

- 仅 Developer/Test mode；
- 默认 disabled；
- Policy 默认 ASK；
- Runtime 校验 Git Bash readiness；
- command max 8192 bytes；
- timeout 30s；
- total captured stdout/stderr 256 KiB；
- concurrency 1；
- high-risk/unclassified command 至少 ASK；
- unclassified high-risk 不允许 Session Grant。

Bash 工作目录固定到 active Workspace，但并非 OS filesystem sandbox。

## 7. Tunnel Launch

TunnelProcessManager 由 Tauri 管理。其 launch 输入来自：

- Tunnel ID；
- 当前 Runtime MCP endpoint；
- SecretStore 中受控读取的 API Key；
- bundled/development Tunnel distribution；
- health/readiness 配置。

原始 API Key 不应进入：

- AppConfig plaintext；
- RuntimeConfigSnapshot；
- Runtime；
- React state；
- Activity；
- diagnostics；
-普通日志；
- literal command-line secret argument。

AppConfig 只保存 `SecretReference`。

## 8. Tunnel Readiness

必须区分：

```text
process spawned
≠ process alive
≠ Tunnel Ready
≠ Harness Ready
```

Tunnel Ready 需要当前 managed instance 的权威 health/readiness 与 target evidence，而不是只看 PID 或监听端口。

Harness Ready 还要求同一 active session 中 Runtime、MCP、Tunnel 状态已经收敛。

## 9. Harness Start / Stop

### Start

```text
harness_start_eligibility
→ Runtime start
→ Runtime Ready
→ mcp.endpoint.get
→ MCP Ready
→ SecretStore controlled read
→ Tunnel launch
→ Tunnel readiness / target evidence
→ Harness Ready
```

### Stop

```text
Harness stopping
→ Tunnel + owned descendants stop
→ Runtime stop
→ MCP endpoint shutdown
→ session/endpoint clear
→ Harness stopped
```

### Failure

- Runtime crash：旧 MCP endpoint 无效，Harness 离开 Ready，关联 Tunnel 需要收敛；
- Tunnel crash/readiness loss：Harness 离开 Ready；
- stale generation/session watcher callback 被忽略。

## 10. Activity / History / Trace Evidence

工具调用会产生 Runtime-owned audit lifecycle，再投影成 Activity。

当前 Activity 可表达：

- request received；
- workspace validation；
- policy evaluation；
- request denied；
- approval required/resolved/expired/cancelled；
- session grant matched；
- execution started/completed/failed；
- bounded duration/result evidence；
- Bash sanitized evidence。

Activity 不包含原始 Secret，也不应保存 raw Bash output/argv 作为授权证据。

History 与 Activity 的边界：

- Activity 是 Runtime lifetime 内的实时有界视图（容量 256）；
- History 写入本地 SQLite，默认启用，默认保留 30 天 / 256 MiB；
- Metrics 与 Trace 是 History 的只读投影，History 停用时不可用；
- Trace 只用 `(runtime_id, request_id)` 精确关联，不做时间戳回退；
- History 保存脱敏后的参数、生命周期、Policy/Approval 结果和有界执行证据。

## 11. Troubleshooting 模型

### ChatGPT 看不到某个工具

按顺序检查：

```text
Tool enabled?
→ current Runtime generation 已重启/收敛?
→ security mode eligible?
→ tool readiness ready?
→ Policy 是否 DENY?
→ Harness/Tunnel 是否连接到当前 Runtime?
```

### 工具能看到但执行停住

通常检查：

- effective Policy 是否 ASK；
- Approvals 页面是否存在 Pending；
- Approval 是否过期；
- Session Grant 是否 scope 不匹配。

### Bash 看不到

检查：

- `bash.enabled`；
- SecurityMode 是否 Developer/Test；
- Git Bash executable readiness；
- Policy；
- Runtime restart/convergence。

### History / Metrics / Trace 不可用

检查：

- History 是否在 Settings 中启用；
- History 数据库是否处于 healthy / busy / incompatible / corrupt 状态；
- 当前查询是否超出保留期；
- Metrics/Trace 是否需要先完成 evidence store migration。

## 12. Source-of-Truth

| 问题 | 代码 |
| --- | --- |
| MCP service | `runtime/src/mcp/service.rs` |
| MCP HTTP/lifecycle | `runtime/src/mcp/http.rs`, `runtime/src/mcp/runtime.rs` |
| MCP compatibility | `runtime/src/mcp/compatibility.rs`, `fixtures/v4/m10-mcp/compatibility-v1.json` |
| Tool registry/effective | `runtime/src/tools/registry.rs`, `effective.rs` |
| Policy | `runtime/src/policy/engine.rs` |
| Approval | `runtime/src/approval.rs` |
| Session Grant | `runtime/src/session_grant.rs` |
| Bash | `runtime/src/bash/**` |
| History / Metrics / Trace | `runtime/evidence-store/**`, `ui/src-tauri/src/history.rs` |
| Control methods | `protocol/src/v1/methods.rs` |
| Tauri Runtime | `ui/src-tauri/src/runtime/**` |
| Tunnel | `ui/src-tauri/src/tunnel/**` |
| Harness | `ui/src-tauri/src/harness.rs` |

## 13. Related

- [README.md](./README.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [CODEBASE-MAP.md](./CODEBASE-MAP.md)
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)
