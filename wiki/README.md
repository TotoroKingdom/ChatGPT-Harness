# ChatGPT-Harness Wiki

> 本 Wiki 描述 HEAD 的当前系统事实。当前实现状态以源码、Tests、Manifests 与构建脚本为准；V2 已冻结，V3 文档已归档且不等于当前实现。

~~~yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN
~~~

V2 的人工验收记录见 [V2-ACCEPTANCE.md](./V2-ACCEPTANCE.md)。V3 历史资料见 [V3 archive](../docs/archive/v3-2026-10/README.md)；下一版本开发入口见 [docs-codex/README.md](../docs-codex/README.md)。

## 1. Project Overview

ChatGPT-Harness 是一个本机运行的 MCP Harness。当前产品链路为：

~~~text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↓
WorkspaceGuard
   ↓
Policy / Execution
   ↓
Tools
   ↓
Audit / Activity
   ↓
UI
~~~

Tauri Desktop App 负责本机配置、Secret、Runtime/Tunnel 生命周期、连接编排和状态展示。

## 2. Current Implementation State

| Capability | Current State |
| --- | --- |
| Runtime lifecycle | Implemented |
| Workspace initialization and confinement | Implemented |
| read_file | Implemented |
| glob | Implemented |
| Policy evaluation | Implemented |
| Execution pipeline | Implemented |
| Audit / Activity | Implemented |
| Runtime diagnostics | Implemented |
| Config persistence | Implemented |
| Secret storage and controlled Tunnel launch read | Implemented |
| Runtime MCP HTTP endpoint | Implemented / loopback |
| Runtime → Tauri endpoint handoff | Implemented / internal |
| Tunnel launch and readiness | Implemented |
| Harness lifecycle | Implemented |
| Tool exposure | Implemented for the V2 read-only catalog |
| External ChatGPT E2E | Accepted / manual acceptance |
| Approval resume workflow | Not implemented; V3 Phase 3 |
| write_file, edit_file, bash | Not implemented; V3 planned |

需要区分：

~~~text
Runtime MCP 在本机可用
        ≠
外部 E2E 可以用 mock 或本地测试代替
~~~

当前真实产品路径已经包含 endpoint handoff、受控 Tunnel launch、Tunnel readiness 和 Harness lifecycle；外部 ChatGPT E2E 的状态单独记录为 manual acceptance，不冒充自动化测试。

## 3. Architecture at a Glance

项目存在两条通信平面。

### Control Plane

~~~text
React UI
   ↓ Tauri invoke / events
Tauri HarnessController
   ↓ stdin/stdout JSON-RPC
Runtime
~~~

Tauri Host 是 Runtime child process 与 Tunnel child process 的生命周期 owner，并负责配置、Secret、endpoint handoff、状态编排和清理。

### MCP Data Plane

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
Tools
~~~

Tauri 不位于每一次 MCP tool call 的执行路径中；它负责启动、停止和协调 Runtime 与 Tunnel。

## 4. Current Harness Lifecycle

启动顺序：

~~~text
eligibility
    ↓
Runtime start
    ↓
Runtime Ready
    ↓
mcp.endpoint.get + loopback validation
    ↓
MCP Ready
    ↓
controlled Tunnel launch
    ↓
Tunnel readiness (/readyz + target evidence)
    ↓
Harness Ready
~~~

停止顺序：

~~~text
Harness stopping
    ↓
Tunnel / cloudflared cleanup
    ↓
Runtime cleanup
    ↓
endpoint / session clear
    ↓
Harness stopped
~~~

Runtime、MCP、Tunnel 和 Harness 的状态由 Tauri 侧的 HarnessStatus 维护；Harness Ready 只在同一 session 中三个组件均 Ready 时成立。Runtime 或 Tunnel 崩溃会使 Harness 离开 Ready，过期 watcher callback 不得修改新 session。

## 5. Current Tools and Security Boundary

V2 当前向 MCP 暴露：

~~~text
read_file
glob
~~~

每次调用都经过：

~~~text
MCP request
   ↓
ExecutionPipeline
   ↓
argument validation
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
tool executor
   ↓
Audit / Activity
~~~

Runtime MCP 只绑定 127.0.0.1:<ephemeral>/mcp。Tunnel Ready 需要控制面连接、配置的 MCP target 可达和 tunnel-client 的权威健康检查；PID、spawn 成功或单独的监听端口都不能产生 Ready。原始 API Key 不进入 AppConfig、Runtime、IPC response、Activity、diagnostics、普通日志或 argv。

## 6. Repository Map

~~~text
ChatGPT-Harness/
├── protocol/                       shared control contracts
├── runtime/                        Runtime, MCP, tools, policy, execution
├── ui/src/                         React UI and harness adapters
├── ui/src-tauri/src/               Tauri host, lifecycle, config, secrets
├── tunnel-client-v0.0.13-*/        bundled Tunnel payload
├── wiki/                           current implementation facts
├── docs-codex/                     next-version documentation workspace
└── docs/archive/                   historical documentation
~~~

## 7. Documentation Guide

- [ARCHITECTURE.md](./ARCHITECTURE.md)：进程边界、Control Plane、MCP Data Plane 和生命周期所有权。
- [CODEBASE-MAP.md](./CODEBASE-MAP.md)：源码入口、模块关系、调用链和分发边界。
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)：当前实现状态、自动化验证与 manual acceptance 边界。
- [MCP-AND-TUNNEL.md](./MCP-AND-TUNNEL.md)：Runtime MCP、Tunnel launch/readiness 和安全边界。
- [V2-ACCEPTANCE.md](./V2-ACCEPTANCE.md)：V2 Accepted / Frozen 及人工验收说明。
- [../docs/archive/v3-2026-10/README.md](../docs/archive/v3-2026-10/README.md)：V3 历史归档；不是当前实现证据。
- [../docs-codex/README.md](../docs-codex/README.md)：下一版本开发入口。

## 8. Source-of-Truth Rule

~~~text
源码 / Tests / Manifests
        ↓
wiki/**
        ↓
docs-codex/**
        ↓
docs/archive/**
~~~

如果文档与代码不一致，优先使用源码、Tests 和 Manifests；Wiki 需要随 HEAD 当前事实更新。docs-codex/** 只作为下一版本规划工作区，docs/archive/** 只说明过去发生过什么。
