# CODEBASE MAP

> 面向当前 main / HEAD（0.1.3）的代码导航。此页按“谁拥有 authority”组织，而不是按旧 Milestone 组织。

## 1. Repository Structure

| 区域 | 当前职责 | Authority / 边界 |
| --- | --- | --- |
| `protocol/` | Control/config/tools/policy/approval/activity/diagnostics contracts、schema、TS 生成物 | 定义 contract，不执行授权 |
| `runtime/` | MCP、Workspace、Tools、Policy、Approval、Bash、Execution、Audit/Activity/History | 工具授权与执行 authority |
| `runtime/evidence-store/` | SQLite History、Metrics、Trace、schema/迁移/保留期 | 本地证据存储，不参与授权 |
| `ui/src/` | React UI、路由、页面、Provider、i18n、theme | 展示与用户意图，不是授权 authority |
| `ui/src-tauri/src/` | Config、Secret、Runtime/Tunnel 生命周期、Harness orchestration | Desktop/process lifecycle authority |
| `ui/scripts/` | distribution staging、Portable/NSIS packaging/verification | 分发链路 |
| `tunnel-client-*/` | bundled Tunnel payload | transport payload |
| `wiki/` | 当前代码地图 | 必须跟随 HEAD |
| `docs-codex/` | 下一版本文档工作区 | 规划材料，不是当前实现证据 |
| `docs/archive/` | 历史资料，包含 V4 产品与 milestone 文档 | 非当前事实来源 |

## 2. Runtime 入口

### 核心初始化

- `runtime/src/main.rs`：binary entry；
- `runtime/src/server.rs`：control server / initialize wiring；
- `runtime/src/state.rs`：RuntimeCore、generation 状态与模块组合。

Runtime 启动后同时存在：

```text
stdin/stdout JSONL control server
+
loopback MCP HTTP server
```

## 3. Tool System

### Registry

`runtime/src/tools/registry.rs`

稳定内置目录：

```text
read_file   WorkspaceRead
glob        WorkspaceRead
grep        WorkspaceRead
write       WorkspaceWrite
edit        WorkspaceWrite
bash        DeveloperShell
```

### Effective Catalog

`runtime/src/tools/effective.rs`

综合判断：

- implemented；
- enabled；
- exposure；
- security mode；
- Runtime/tool readiness；
- Policy result。

输出 availability：

```text
available
available_with_approval
unavailable_by_mode
unavailable_by_policy
unavailable_by_runtime
unavailable_disabled
```

### File Tools

- `runtime/src/tools/read_file.rs`
- `runtime/src/tools/glob.rs`
- `runtime/src/tools/grep.rs`
- `runtime/src/tools/write.rs`
- `runtime/src/tools/edit.rs`
- `runtime/src/tools/atomic_file.rs`

`write` 与 `edit` 共用 atomic mutation 基础设施。

## 4. Workspace

`runtime/src/workspace/**`

主要负责：

- active Workspace canonicalization；
- Windows path 形式与边界校验；
- path escape rejection；
- read/write target approval；
- mutation 目标再验证；
- 返回逻辑相对路径，而不是把内部物理路径当成对外事实。

WorkspaceGuard 是 Runtime authority。

## 5. Policy

`runtime/src/policy/engine.rs`

输入：

- `ToolPolicyDefaults`
- typed User Rules
- Runtime-owned System Rules
- SecurityMode
- Tool capability
- Bash analyzer facts（Bash 请求）

重要语义：

- invalid snapshot/context fail closed；
- system workspace boundary 不可由用户规则降低；
- BashCommand matcher 使用 normalized executable + argvPrefix；
- Runtime 生成 effective snapshot 给 UI 使用。

Protocol：

- `protocol/src/v1/config.rs`
- `protocol/src/v1/policies.rs`

## 6. Approval

### Runtime Manager

`runtime/src/approval.rs`

职责：

- 注册 pending request；
- 5 分钟 TTL；
- list / pending_count；
- resolve；
- 唤醒 exact invocation；
- expiry / cancellation。

Decision：

```text
allow_once
allow_for_session
deny
```

### Session Grant

`runtime/src/session_grant.rs`

特征：

- memory-only；
- Runtime-generation-local；
- exact tool + normalized scope；
- 不等价于 tool-wide 永久 ALLOW。

### Protocol

`protocol/src/v1/approvals.rs`

包含安全展示用 sanitized arguments、Bash approval details、risk flags、TTL/status/decision。

## 7. Bash

目录：`runtime/src/bash/`

| 文件 | 职责 |
| --- | --- |
| `discovery.rs` | Git Bash executable discovery / validation |
| `analyzer.rs` | quote-aware command analysis / risk flags / normalized scope |
| `runtime.rs` | async process execution、timeout、concurrency、cancel |
| `output.rs` | stdout/stderr 总量限制 |
| `process_tree.rs` | owned process tree cleanup |

生产 limits：

```text
max command: 8192 bytes
timeout: 30s
stdout + stderr: 256 KiB
max concurrent bash: 1
```

Bash 只在 Developer/Test mode 可用。

## 8. MCP

目录：`runtime/src/mcp/`

- `runtime.rs`：MCP runtime lifecycle；
- `http.rs`：loopback HTTP / Streamable HTTP；
- `service.rs`：effective tool exposure + call dispatch；
- `read_file.rs` / `glob.rs` / `grep.rs` / `write.rs` / `edit.rs` / `bash.rs`：adapter。

关键行为：

```text
MCP tools/list
→ Runtime effective tools
→ 只暴露 Available / AvailableWithApproval
```

调用：

```text
MCP adapter
→ Runtime execution pipeline
→ Workspace
→ Policy
→ Approval/SessionGrant
→ executor
```

## 9. Audit / Activity / Diagnostics

### Audit

`runtime/src/audit/**`

只产生 bounded、non-secret evidence；不把 authorization authority 交给 observer。

### Activity

`runtime/src/activity/**`

- safe UI projection；
- capacity = 256；
- Runtime lifetime 内存态；
- category：Request / Workspace / Policy / Execution / Approval。

### History / Metrics / Trace

- `runtime/evidence-store/src/**`：SQLite schema、writer、query、retention、metrics、trace；
- `runtime/src/evidence/**`：Runtime 将 audit lifecycle 投影为 durable evidence；
- `protocol/src/v1/history.rs`：History / Metrics / Trace DTO；
- `ui/src-tauri/src/history.rs`：Host 侧只读查询与数据库路径管理。

History 默认写入 `<app data>/history/history.sqlite3`，默认保留 30 天 / 256 MiB。Metrics 与 Trace 依赖 History enabled，且 trace 关联只用 `(runtime_id, request_id)`。

### Control Reads

- `runtime/src/control_read.rs`
- `protocol/src/v1/control_read.rs`

包含 diagnostics、activity snapshot、effective tools 等 UI/本机观测面。

## 10. Tauri Host

### Harness

`ui/src-tauri/src/harness.rs`

产品级 Start / Stop / Restart、session/generation、rollback、crash/readiness convergence。

### Commands

`ui/src-tauri/src/commands/mod.rs`

包括：

- config / secret；
- workspace/tunnel/bash picker；
- runtime control；
- tools/policies/activity/approvals/diagnostics；
- history / metrics / trace；
- desktop preferences / quit；
- harness lifecycle。

### Desktop / Diagnostics

- `ui/src-tauri/src/desktop/**`：托盘、关闭到托盘、通知、单实例、自动启动编排；
- `ui/src-tauri/src/diagnostics.rs`：local / connection / runtime diagnostics；
- `ui/src-tauri/src/startup_benchmark.rs`：启动阶段耗时捕获；
- `ui/src-tauri/src/startup.rs`：启动相关 Host 状态。

### Runtime Supervisor

`ui/src-tauri/src/runtime/**`

- distribution resolution；
- child process；
- JSONL control；
- endpoint handoff；
- generation ownership。

### Tunnel Manager

`ui/src-tauri/src/tunnel/**`

- Tunnel distribution；
- launch；
- health/readiness；
- process ownership；
- cleanup。

### Config / Secrets

- `ui/src-tauri/src/config/**`
- `ui/src-tauri/src/secrets/**`

当前 persistent config schema version = 3。

AppConfig：

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

## 11. React UI

### Pages

- `ui/src/pages/overview/**`
- `ui/src/pages/tools/**`
- `ui/src/pages/policies/**`
- `ui/src/pages/approvals/**`
- `ui/src/pages/activity/**`
- `ui/src/pages/settings/**`
- `ui/src/pages/activity/HistoryPanel.tsx`
- `ui/src/pages/activity/MetricsPanel.tsx`
- `ui/src/components/status/DiagnosticsPanel.tsx`

### Harness data/service

`ui/src/services/harness/**` 与 Provider 负责对 Tauri/control data 做前端适配。

### Theme

`ui/src/theme/theme.ts`

Preference：

```text
system
light
dark
```

### i18n

当前 UI language wire values：

```text
en
zh-CN
```

语言属于 UI preference，不进入 RuntimeConfigSnapshot。

## 12. Distribution

`ui/scripts/`

主要脚本包括 staging、Portable build、NSIS build 与 bundle verification。

产品构建需要同时携带：

- desktop UI；
- Runtime；
- Tunnel client；
- Tunnel companion/resources；
- distribution metadata。

具体 payload 以当前 distribution manifest 和 scripts 为准，不应在 Wiki 固化旧包版本号。

本地首次构建 `ui/src-tauri` 前，如 `target-exe/distribution` 不存在，先运行 `npm run distribution:stage`；Tauri build script 需要其中的 `distribution.json` 与 Runtime/Tunnel 资源。

V4 的 milestone layout / delivery 脚本当前指向 `docs/archive/v4-2026-11/v4/milestones`，属于历史 V4 流程；开始下一版本前必须为新目录更新路径，不能把归档文档当作新版本交付契约。

## 13. 常见修改该看哪里

| 修改目标 | 首选入口 |
| --- | --- |
| 新增/修改 MCP 工具 | `runtime/src/tools/**`, `runtime/src/mcp/**`, protocol |
| 改 Workspace 安全规则 | `runtime/src/workspace/**` |
| 改 Policy | `protocol/src/v1/policies.rs`, `runtime/src/policy/**`, Policies UI |
| 改 Approval | `protocol/src/v1/approvals.rs`, `runtime/src/approval.rs`, Approvals UI |
| 改 Bash | `runtime/src/bash/**`, Tool/Policy/Approval UI |
| 改 History / Metrics / Trace | `runtime/evidence-store/**`, `protocol/src/v1/history.rs`, Activity UI |
| 改托盘 / 通知 / 自动启动 | `ui/src-tauri/src/desktop/**`, Settings UI |
| 改诊断 / 启动耗时 | `ui/src-tauri/src/diagnostics.rs`, `ui/src-tauri/src/startup_benchmark.rs`, DiagnosticsPanel |
| 改 Runtime 生命周期 | `runtime/src/server.rs`, `ui/src-tauri/src/runtime/**` |
| 改整条 Harness 编排 | `ui/src-tauri/src/harness.rs` |
| 改 Tunnel | `ui/src-tauri/src/tunnel/**` |
| 改配置 | protocol config + Tauri ConfigStore + Settings |
| 改页面/交互 | `ui/src/pages/**`, components, styles |
| 改语言/主题 | i18n + `ui/src/theme/**` |
