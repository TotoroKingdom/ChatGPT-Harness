# ChatGPT-Harness Wiki

> 本 Wiki 描述 **当前 main / HEAD 的实现事实**。当前源码版本为 **0.1.3**。若 Wiki 与源码、测试、manifest 或构建脚本冲突，以后者为准。
>
> V4 正在开发中；`docs-codex/` 中的 PRD / Architecture / Roadmap / Milestone 文档描述目标与计划，**不等于已经实现**。

## 1. 当前产品是什么

ChatGPT-Harness 是一个面向 Windows 的本地 MCP Harness。它把 ChatGPT 的远程工具调用连接到本机 Runtime，并在 Runtime 内执行 Workspace 边界、Policy、Approval、工具执行和 Activity/Audit 记录。

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
```

桌面应用由 React UI + Tauri Host 组成。Tauri 负责配置、Secret、Runtime/Tunnel 进程生命周期和 Harness 编排；它不是每一次 MCP tool call 的代理。

## 2. 当前已实现能力

| 能力 | 当前状态 |
| --- | --- |
| Runtime / MCP lifecycle | 已实现 |
| Workspace confinement | 已实现 |
| `read_file` | 已实现 |
| `glob` | 已实现 |
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
| Bash 风险分析与受控执行 | 已实现 |
| Runtime diagnostics / control reads | 已实现 |
| Config persistence / migration | 已实现，当前 schema version = 2 |
| SecretStore | 已实现 |
| Tunnel launch / readiness / cleanup | 已实现 |
| 中文 / 英文 UI | 已实现 |
| dark / light / system theme | 已实现 |
| Portable / NSIS 打包链路 | 已实现 |

## 3. 当前工具目录与默认值

Runtime 的稳定工具目录：

```text
read_file
glob
write
edit
bash
```

默认配置：

| Tool | 默认 enabled | 默认 Policy |
| --- | ---: | --- |
| read_file | true | ALLOW |
| glob | true | ALLOW |
| write | true | ALLOW |
| edit | true | ALLOW |
| bash | false | ASK |

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

- Overview：Harness 状态与 Start / Stop / Restart；
- Tools：工具 desired/effective 状态、Policy default、Bash readiness；
- Policies：Tool Defaults、User Rules、System Rules；
- Approvals：处理 pending approval；
- Activity：查看执行、Policy、Approval 和结果证据；
- Settings：Workspace、Tunnel、Proxy、Git Bash、语言、主题等。

### Tauri Host

Tauri 负责：

- ConfigStore；
- SecretStore；
- RuntimeSupervisor；
- TunnelProcessManager；
- HarnessController；
- Tauri commands / events；
- 打包与本机分发资源协调。

### Runtime

Runtime 负责：

- WorkspaceGuard；
- ToolRegistry；
- PolicyEngine；
- ApprovalManager / SessionGrantStore；
- ExecutionPipeline；
- read/glob/write/edit；
- Bash analyzer / runtime；
- Audit / Activity；
- Diagnostics；
- MCP HTTP server。

## 7. 两条通信平面

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

## 8. Harness 生命周期

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

## 9. Repository Map

```text
ChatGPT-Harness/
├── protocol/                 control/config/policy/approval/activity contracts
├── runtime/                  MCP + authorization + tool execution
├── ui/src/                   React UI
├── ui/src-tauri/src/         Tauri host + lifecycle + config + secrets
├── ui/scripts/               packaging/distribution scripts
├── tunnel-client-*/          bundled Tunnel payload
├── wiki/                     current implementation map
├── docs-codex/               active V4 product/milestone documents
└── docs/archive/             historical documents
```

## 10. Wiki 导航

- [ARCHITECTURE.md](./ARCHITECTURE.md)：当前进程、授权与生命周期架构。
- [CODEBASE-MAP.md](./CODEBASE-MAP.md)：源码目录、模块职责与关键入口。
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)：当前能力矩阵与默认安全行为。
- [MCP-AND-TUNNEL.md](./MCP-AND-TUNNEL.md)：MCP 暴露、调用链、Approval 与 Tunnel。
- [V2-ACCEPTANCE.md](./V2-ACCEPTANCE.md)：**历史** V2 只读版本验收记录，不是当前能力说明。
- [V4 PRD](../docs-codex/ChatGPT-Harness_V4_PRD.md)
- [V4 Architecture](../docs-codex/ChatGPT-Harness_V4_ARCHITECTURE.md)
- [V4 Roadmap](../docs-codex/ChatGPT-Harness_V4_ROADMAP.md)

## 11. Source-of-Truth Rule

```text
源码 / Tests / Manifests / Build Scripts
        ↓
wiki/**
        ↓
active docs-codex/**（目标与计划）
        ↓
docs/archive/**（历史）
```

Wiki 的职责是帮助人理解当前代码，不冻结某个旧 Milestone，也不把未来 Roadmap 当成已发布功能。
