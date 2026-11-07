# ChatGPT-Harness Wiki

> 本 Wiki 描述 **当前 main / HEAD 的实现事实**。当前源码版本为 **0.1.3**。若 Wiki 与源码、测试、manifest 或构建脚本冲突，以后者为准。
>
> V4 的开发与验收文档已归档；`docs/archive/v4-2026-11/` 中的 PRD / Architecture / Roadmap / Milestone 文档描述目标、计划与历史状态，**不等于已经实现或已经验收**。`docs-codex/` 是下一版本文档工作区。

## 1. 当前产品是什么

ChatGPT-Harness 是一个面向 Windows 的本地 MCP Harness。它把 ChatGPT 的远程工具调用连接到本机 Runtime，并在 Runtime 内执行 Workspace 边界、Policy、Approval、工具执行和 Activity/Audit/History 记录。

当前主链路：

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↓
argument validation
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
Approval（需要时）
   ↓
Tool executor
   ↓
Audit / Activity
   ↓
History / Metrics / Trace（本地 SQLite，按配置）
```

桌面应用由 React UI + Tauri Host 组成。Tauri 负责配置、Secret、系统托盘、通知、自动启动策略、Runtime/Tunnel 进程生命周期和 Harness 编排；它不是每一次 MCP tool call 的代理。

## 2. 当前已实现能力

| 能力 | 当前状态 |
| --- | --- |
| Runtime / MCP lifecycle | 已实现 |
| Workspace confinement | 已实现 |
| `read_file` | 已实现 |
| `glob` | 已实现 |
| `grep` | 已实现，仅工作区文本搜索 |
| `write` | 已实现，原子整文件写入 |
| `edit` | 已实现，唯一精确匹配后原子替换 |
| `bash` | 已实现，Developer/Test mode 下可用 |
| Tool enable / disable | 已实现 |
| Effective tool projection | 已实现 |
| Policy：ALLOW / ASK / DENY | 已实现 |
| Tool default policy | 已实现 |
| User Policy Rule | 已实现，支持 Tool / BashCommand matcher |
| Runtime system guardrail | 已实现，只读且不能被用户规则削弱 |
| Approval | 已实现：Allow once / Allow for session / Deny |
| Session Grant | 已实现，Runtime generation 内存级、按精确 scope 匹配 |
| Activity / Audit | 已实现，安全投影、bounded、Runtime 内存态 |
| SQLite History | 已实现，跨重启持久化、可分页查询、可清空 |
| Metrics / Trace | 已实现，基于 History 的只读聚合与精确请求追踪 |
| Bash 风险分析与受控执行 | 已实现 |
| Runtime diagnostics / control reads | 已实现 |
| 启动阶段耗时观测 | 已实现 |
| 系统托盘 / 关闭到托盘 | 已实现 |
| 审批 / 连接通知 | 已实现，默认启用 |
| 应用启动后自动启动 Harness | 已实现，默认关闭；不是 Windows 登录自启 |
| 单实例运行 | 已实现（Windows） |
| Config persistence / migration | 已实现，当前 schema version = 3 |
| SecretStore | 已实现，Windows Credential Manager |
| Tunnel launch / readiness / cleanup | 已实现 |
| 中文 / 英文 UI | 已实现 |
| dark / light / system theme | 已实现 |
| Portable / NSIS 打包链路 | 已实现 |

## 3. 当前工具目录与默认值

Runtime 的稳定工具目录：

```text
read_file
glob
grep
write
edit
bash
```

新配置默认值：

| Tool | 默认 enabled | 默认 Policy |
| --- | ---: | --- |
| read_file | true | ALLOW |
| glob | true | ALLOW |
| grep | true | ALLOW |
| write | true | ALLOW |
| edit | true | ALLOW |
| bash | false | ASK |

升级配置的差异：由 v1/v2 升级、或缺少 `grep` 字段的旧 v3 配置，会把 `grep` 显式补为 **disabled**，避免静默扩大工具面；用户可在 Tools 页面手动启用。

工具是否真正暴露给 MCP，不只看“是否实现”，而是由 Runtime 的 effective catalog 共同决定：

```text
implemented
+ enabled
+ security mode eligible
+ runtime readiness
+ policy result
= MCP exposure
```

只有 `Available` 或 `AvailableWithApproval` 的工具进入 MCP `tools/list`；disabled、DENY、mode 不允许或 Runtime unavailable 的工具不会暴露。

## 4. Policy 与 Approval

Policy 的授权事实由 Runtime 计算，UI 只编辑配置并展示 Runtime 返回的 effective projection。

当前 Policy 配置包含：

- Tool Defaults：每个内置工具的 ALLOW / ASK / DENY；
- User Rules：支持 tool matcher；
- Bash User Rules：支持 `executable + argvPrefix` 的 typed matcher；
- System Guardrail：Runtime 固定规则，例如 Workspace 外访问 DENY。

无效 Policy snapshot、无效 context 或不支持的 capability 均 fail closed。

当最终决策为 ASK 时，Runtime 创建 Approval Request 并暂停**当前精确调用**。用户可选择：

- Allow once；
- Allow for session；
- Deny。

Approval 默认 TTL 为 5 分钟。Allow for session 创建的是 Runtime-generation-local、memory-only、精确 scope grant，不是“整个工具永久放行”。

## 5. Bash 的额外边界

Bash 是 `DeveloperShell` capability：

- 仅 Developer/Test mode 可用；
- 默认 disabled；
- Runtime 每个 generation 重新验证 Git Bash executable；
- 命令最大 8192 bytes；
- 执行 timeout 30s；
- stdout + stderr 总捕获上限 256 KiB；
- 最大并发 1；
- 高风险/无法可靠分类的命令至少提升到 ASK；
- Session Grant 只用于可安全形成精确 normalized scope 的命令；
- Runtime 负责 cancellation、timeout 与进程树清理。

Bash 工作目录固定在 active Workspace，但 **Bash 不是 OS filesystem sandbox**。安全边界依赖 Security Mode、Policy、Approval 和受控执行，不应把 cwd 误解为系统级沙箱。

## 6. 桌面应用职责

### React UI

当前主要页面：

- Overview：Harness 状态、Start / Stop / Restart、诊断结果与最近一次启动阶段耗时；
- Tools：工具 desired/effective 状态、Policy default、Bash readiness；
- Policies：Tool Defaults、User Rules、System Rules；
- Approvals：处理 pending approval；
- Activity：实时 Activity、History、Metrics 与请求追踪；
- Settings：Workspace、Tunnel、Proxy、Security、通知、自动启动、History、语言、主题、诊断与重置。

### Tauri Host

Tauri 负责：

- ConfigStore；
- SecretStore；
- RuntimeSupervisor；
- TunnelProcessManager；
- HarnessController；
- DesktopLifecycleCoordinator；
- 系统托盘与窗口关闭行为；
- Approval / Connection notifications；
- 应用启动后的 Harness 自动启动；
- 单实例；
- local / connection diagnostics；
- startup benchmark capture；
- Tauri commands / events；
- 打包与本机分发资源协调。

### Runtime

Runtime 负责：

- WorkspaceGuard；
- ToolRegistry；
- PolicyEngine；
- ApprovalManager / SessionGrantStore；
- ExecutionPipeline；
- read/glob/grep/write/edit；
- Bash analyzer / runtime；
- Audit / Activity；
- Diagnostics；
- MCP HTTP server；
- 将脱敏生命周期证据写入 evidence store。

## 7. History 与可观测性

- Activity 是 Runtime lifetime 内的实时有界视图，容量 256。
- History 使用本地 SQLite：应用数据目录下的 `history/history.sqlite3`，默认启用，默认保留 30 天、最多 256 MiB。
- History evidence schema 当前为 **1**；配置 schema 与 evidence schema 相互独立。
- Metrics 与 Trace 是对 History 数据的只读投影；History 停用时不可用。
- Trace 关联使用 `(runtime_id, request_id)` 精确匹配，不做时间戳猜测。
- History 保留脱敏参数、生命周期、策略/审批结果和有界执行证据；不保存原始 API Key、原始命令输出或完整敏感参数。

## 8. 两条通信平面

### Desktop Control Plane

```text
React UI
   ↓ Tauri invoke / events
Tauri Host
   ↓ stdin/stdout JSONL control RPC
Runtime
```

### MCP Data Plane

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP (127.0.0.1:<ephemeral>/mcp)
   ↓
Runtime authorization + execution
```

Tauri 不位于单次工具调用的数据路径中。

## 9. Harness 生命周期

启动：

```text
eligibility
→ Runtime start
→ Runtime Ready
→ mcp.endpoint.get + loopback validation
→ MCP Ready
→ SecretStore controlled read
→ Tunnel launch
→ Tunnel readiness / target evidence
→ Harness Ready
```

停止：

```text
Harness stopping
→ Tunnel / descendants cleanup
→ Runtime cleanup
→ endpoint / session clear
→ Harness stopped
```

Runtime/Tunnel watcher 都带实例身份，旧 generation/session 的 callback 不应污染新会话。

桌面生命周期补充：

- 关闭主窗口默认进入托盘，首次关闭先展示后台运行说明；
- 托盘可显示状态与 pending approval 数，并提供 Start / Stop / Quit；
- `autoStartHarnessOnAppLaunch` 只在应用启动时触发一次 Harness Start，不是登录自启；
- 单实例插件确保重复启动回到现有窗口。

## 10. Repository Map

```text
ChatGPT-Harness/
├── protocol/                 control/config/policy/approval/activity/history contracts
├── runtime/                  MCP + authorization + tool execution
│   └── evidence-store/       SQLite History / Metrics / Trace storage
├── ui/src/                   React UI
├── ui/src-tauri/src/         Tauri host + lifecycle + desktop + config + secrets
├── ui/scripts/               packaging/distribution scripts
├── tunnel-client-*/          bundled Tunnel payload
├── wiki/                     current implementation map
├── docs-codex/               next-version documentation workspace
└── docs/archive/             historical documents
```

## 11. Wiki 导航

- [ARCHITECTURE.md](./ARCHITECTURE.md)：当前进程、授权与生命周期架构。
- [CODEBASE-MAP.md](./CODEBASE-MAP.md)：源码目录、模块职责与关键入口。
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)：当前能力矩阵与默认安全行为。
- [MCP-AND-TUNNEL.md](./MCP-AND-TUNNEL.md)：MCP 暴露、调用链、Approval 与 Tunnel。
- [V2 历史验收记录](../docs/archive/v2-2026-09/V2-ACCEPTANCE.md)：**已归档**，不是当前能力说明。
- [V4 PRD](../docs/archive/v4-2026-11/ChatGPT-Harness_V4_PRD.md)
- [V4 Architecture](../docs/archive/v4-2026-11/ChatGPT-Harness_V4_ARCHITECTURE.md)
- [V4 Roadmap](../docs/archive/v4-2026-11/ChatGPT-Harness_V4_ROADMAP.md)

## 12. Source-of-Truth Rule

```text
源码 / Tests / Manifests / Build Scripts
        ↓
wiki/**
        ↓
docs-codex/**（下一版本工作区）
        ↓
docs/archive/**（历史）
```

Wiki 的职责是帮助人理解当前代码，不冻结某个旧 Milestone，也不把未来 Roadmap 当成已发布功能。
