# ChatGPT-Harness Architecture

> 本文描述当前 main / HEAD 的架构事实，基于 0.1.3 源码。V4 开发文档已归档；归档中的规划、阶段状态与验收结论不等于当前实现或发布状态。

## 1. 总体架构

ChatGPT-Harness 由四个核心运行单元组成：

```text
React UI (WebView)
        ↕ Tauri IPC
Tauri Host
   ├── Runtime child process
   └── Tunnel child process

ChatGPT ↔ Tunnel ↔ Runtime MCP
                    ↓
        Authorization / Execution
```

关键原则：

- Tauri 是本机 desktop lifecycle owner；
- Runtime 是工具授权与执行 authority；
- Tunnel 是 transport，不是 authorization authority；
- React UI 表达用户意图并展示观测状态，不重算 Runtime Policy。

## 2. 进程与职责边界

### React UI

位于 `ui/src/`，负责：

- Overview / Tools / Policies / Approvals / Activity / Settings；
- Activity 内的 Live / History / Metrics 视图；
- 诊断结果与启动阶段耗时展示；
- config draft 与保存；
- desired/effective 状态展示；
- i18n；
- dark/light/system theme；
- 调用 Tauri command 与消费状态/事件。

UI 不直接启动 Runtime/Tunnel，也不负责工具授权。

### Tauri Host

位于 `ui/src-tauri/src/`，主要拥有：

- ConfigStore；
- SecretStore；
- RuntimeSupervisor；
- TunnelProcessManager；
- HarnessController；
- DesktopLifecycleCoordinator；
- 系统托盘、窗口关闭与单实例；
- Approval / Connection notifications；
- 应用启动后的 Harness 自动启动；
- diagnostics 与 startup benchmark capture；
- Tauri command surface；
- desktop/process cleanup。

Tauri 与 Runtime 使用 stdin/stdout JSONL control RPC。

### Runtime

位于 `runtime/src/`，拥有安全与执行核心：

```text
WorkspaceGuard
ToolRegistry
EffectiveToolCatalog
PolicyEngine
ApprovalManager
SessionGrantStore
ExecutionPipeline
Tools
Bash analyzer/runtime
Audit / Activity
Diagnostics
MCP Service
Evidence Store（History / Metrics / Trace）
```

### Runtime MCP

MCP Server 运行在 Runtime process 内，不是独立进程：

```text
Runtime
├── Control Server      stdin/stdout JSONL
└── MCP HTTP Server     127.0.0.1:<ephemeral>/mcp
```

Tauri 通过 `mcp.endpoint.get` 获取 endpoint，并只把它用于当前 Runtime generation 的 Tunnel orchestration。

### Tunnel

Tunnel Client 是 Tauri 管理的独立 child process。它负责把远程 ChatGPT 流量转发到 Runtime MCP，并暴露自身权威 readiness/health 信息。

Tunnel 不执行：

- WorkspaceGuard；
- Policy；
- Approval；
- tool implementation；
- Activity authorization decision。

### Desktop integration

`ui/src-tauri/src/desktop/**` 负责桌面生命周期与系统集成：

- 托盘菜单展示 Harness 状态、pending approval 数，并提供 Show / Start / Stop / Quit；
- 关闭主窗口默认隐藏到托盘，首次关闭先显示后台运行说明；
- 审批通知与连接状态通知可分别关闭，默认启用；
- `autoStartHarnessOnAppLaunch` 在应用启动后最多触发一次 Harness Start，默认关闭；
- Windows 单实例插件避免重复进程；
- 诊断与启动阶段耗时只读取当前 Host/Runtime 状态，不代替 Runtime 授权。

## 3. Desktop Control Plane

```text
React UI
   ↓ invoke / events
Tauri commands
   ↓
HarnessController / RuntimeSupervisor
   ↓ JSONL
Runtime Control Server
```

主要 control contracts 位于 `protocol/src/v1/`：

- config；
- runtime；
- tools；
- policies；
- approvals；
- activity；
- diagnostics；
- control_read；
- `mcp.endpoint.get`。

Config 持久化 schema 当前为 **3**；这与产品版本号、Wire Protocol v1 和 Milestone 编号相互独立。

## 4. MCP Data Plane

```text
ChatGPT
   ↓
Tunnel
   ↓
Runtime MCP
   ↓
McpService
   ↓
argument validation
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
Approval / SessionGrant（需要时）
   ↓
ExecutionPermit
   ↓
Tool executor / BashRuntime
   ↓
AuditEmitter
   ↓
Activity projection
```

### MCP 暴露规则

Runtime 的 declared tool catalog 固定为：

```text
read_file
glob
grep
write
edit
bash
```

但 `tools/list` 只返回 effective availability 为：

- `Available`
- `AvailableWithApproval`

的工具。

以下状态不会暴露：

- disabled；
- security mode 不允许；
- Runtime/tool readiness 不满足；
- DENY；
- invalid policy/context。

## 5. Workspace Boundary

Workspace 是 Runtime 的安全边界，不由 UI 或 Tunnel 代替。

文件工具都必须经过 WorkspaceGuard。`grep` 只读扫描 Workspace，结果受匹配数、输出字节和 cancellation 限制。写入类工具还会在 mutation path 中进行目标校验/再校验，避免路径逃逸和 TOCTOU 类型的目标变化问题。

当前文件 mutation：

- `write`：UTF-8 full-file create/overwrite，经 shared atomic mutation path；
- `edit`：UTF-8 文本中 `old_text` 必须唯一精确匹配，再通过 shared atomic mutation path 写回。

`edit` 不会自动选择第一处匹配；零匹配或多匹配都失败。

## 6. Policy Architecture

持久化配置包含：

```text
ToolPolicyDefaults
UserPolicyRuleConfig[]
```

Typed matcher：

- `Tool { tool }`
- `BashCommand { executable, argvPrefix }`

Runtime 启动一个 generation 时编译 immutable Policy snapshot。Runtime 同时拥有不可编辑的 system rules。

关键不变量：

- Workspace 外访问由 system guardrail DENY；
- 用户规则不能降低固定系统边界；
- invalid snapshot/context fail closed；
- unknown/unavailable capability fail closed；
- UI 不复制授权算法，只消费 Runtime effective snapshot。

默认 Tool Policy：

```text
read_file = ALLOW
glob      = ALLOW
grep      = ALLOW
write     = ALLOW
edit      = ALLOW
bash      = ASK
```

## 7. Approval 与 Session Grant

ASK 不是“返回错误后让用户重试”的简单模型。当前 Runtime 会：

```text
policy = ASK
→ 创建 ApprovalRequest
→ exact invocation 等待
→ UI 通过 approvals.resolve 给出 decision
→ exact waiter 被唤醒
→ 继续执行或拒绝
```

支持：

- `AllowOnce`
- `AllowForSession`
- `Deny`

Approval 默认 TTL：5 分钟。

Session Grant：

- 仅内存；
- 绑定当前 Runtime generation；
- 绑定 tool；
- 绑定 exact normalized scope；
- Runtime 重启/generation replacement 后不会成为跨代永久权限。

Bash 的 unclassified high-risk 请求不能获得 AllowForSession。

## 8. Bash Architecture

Bash capability = `DeveloperShell`。

### Availability

- 只允许 SecurityMode::DeveloperTest；
- Tool 默认 disabled；
- Runtime 每代重新发现/验证 Git Bash executable；
- readiness 由 Runtime 决定，不信任 UI picker。

### Analyzer

`runtime/src/bash/analyzer.rs` 生成用于 Policy/Approval 的 bounded logical facts，不把 raw command 直接当作 matcher。

高风险特征包括 command substitution、backticks、eval、nested shell、inline interpreter code、redirection、dynamic expansion、malformed quoting、unsupported grammar 等。

无法可靠分类的命令至少获得 ASK floor。

### Runtime hard limits

生产固定限制：

- command：8192 bytes；
- timeout：30s；
- stdout + stderr：256 KiB；
- concurrency：1。

这些限制不是 MCP request 可覆盖的参数。

Bash cwd 固定为 active Workspace，但 Bash 不是 OS 级 filesystem sandbox。

## 9. Audit / Activity / History

Audit event 是 Runtime-owned 的非 secret 事件模型；Activity 是其面向 UI 的安全投影。

重要边界：

- 不持久化 raw tool arguments；
- 不持久化凭据、环境变量、原始物理路径、raw process output；
- Bash 只暴露 sanitized/bounded evidence；
- Activity capacity 当前为 256；
- Activity 是 Runtime lifetime 内的 bounded observation，不是 durable audit database；
- History 通过 `runtime/evidence-store/` 写入本地 SQLite，默认启用，默认保留 30 天 / 256 MiB；
- Metrics / Trace 是 History 的只读投影，History 停用时不可用；
- Trace 使用 `(runtime_id, request_id)` 精确关联，不做时间戳回退。

## 10. Config 与 Secret

AppConfig 包括：

```text
workspace
tunnel
security
ui
tools
bash
policies
desktop
history
```

Runtime 启动时得到 non-secret、immutable `RuntimeConfigSnapshot`。

当前持久化 config schema 为 **3**；History evidence schema 为 **1**，两者相互独立。`desktop` 与 `history` 是 Host-only 配置，不进入 Runtime authorization snapshot。

原始 Tunnel API Key 由 SecretStore 管理；AppConfig 只保存 opaque `SecretReference`。Secret 不应进入 Runtime snapshot、IPC response、Activity、diagnostics 或普通日志。

## 11. Harness Lifecycle

### Start

```text
validate eligibility
→ allocate active session
→ start Runtime
→ Runtime ready
→ mcp.endpoint.get + endpoint validation
→ MCP ready
→ controlled SecretStore read
→ launch Tunnel
→ authoritative readiness / target evidence
→ Harness ready
```

### Stop

```text
mark stopping
→ stop Tunnel and owned descendants
→ stop Runtime
→ clear endpoint/session
→ Harness stopped
```

### Convergence

- Runtime crash：MCP/Harness 离开 Ready，关联 Tunnel 被收敛；
- Tunnel crash/readiness loss：Harness 离开 Ready；
- callback 带 generation/session/instance identity；
- stale callback 不得改变新 session。

若配置启用 `autoStartHarnessOnAppLaunch`，Host 会在桌面初始化后尝试一次 Harness Start；冲突或失败不会自动重试。

## 12. Source-of-Truth Locations

| 问题 | 代码位置 |
| --- | --- |
| Protocol / config | `protocol/src/v1/**` |
| Runtime core | `runtime/src/state.rs`, `runtime/src/server.rs` |
| MCP | `runtime/src/mcp/**` |
| Workspace | `runtime/src/workspace/**` |
| Tools | `runtime/src/tools/**` |
| Policy | `runtime/src/policy/**` |
| Approval | `runtime/src/approval.rs` |
| Session Grant | `runtime/src/session_grant.rs` |
| Bash | `runtime/src/bash/**` |
| Audit / Activity | `runtime/src/audit/**`, `runtime/src/activity/**` |
| Runtime supervision | `ui/src-tauri/src/runtime/**` |
| Tunnel | `ui/src-tauri/src/tunnel/**` |
| Harness orchestration | `ui/src-tauri/src/harness.rs` |
| Desktop lifecycle / tray / notifications | `ui/src-tauri/src/desktop/**` |
| Diagnostics / startup benchmark | `ui/src-tauri/src/diagnostics.rs`, `ui/src-tauri/src/startup_benchmark.rs` |
| History / Metrics / Trace | `runtime/evidence-store/**`, `ui/src-tauri/src/history.rs` |
| Config / Secret | `ui/src-tauri/src/config/**`, `ui/src-tauri/src/secrets/**` |
| React pages | `ui/src/pages/**` |
| Theme | `ui/src/theme/**` |

## 13. Related

- [README.md](./README.md)
- [CODEBASE-MAP.md](./CODEBASE-MAP.md)
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)
- [MCP-AND-TUNNEL.md](./MCP-AND-TUNNEL.md)
