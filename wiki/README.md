# ChatGPT-Harness Wiki

> 本 Wiki 只描述 **当前实现事实**。V2 已于 **2026-09-27** 完成最终验收。

## 1. Current Status

ChatGPT-Harness 当前已经完成第一条真实产品链：

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
   ↓
Audit / Activity
```

V2 的 Connect、Orchestrate、Validate 三个阶段已经收口。真实外部 ChatGPT caller 已能够通过 Harness 调用受控 Workspace 中的只读工具。

当前状态不再是“Runtime MCP 本地可用但 Tunnel 未接入”。

## 2. Runtime Roles

项目有两个通信平面。

### Control Plane

```text
React UI
   ↓ invoke / events
Tauri HarnessController
   ├── RuntimeSupervisor
   ├── TunnelProcessManager
   ├── ConfigStore
   └── SecretStore
```

Tauri 是本机 lifecycle owner / orchestrator。

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
Workspace / Policy / Tools
```

Tauri 不代理每一次 MCP tool call。

## 3. Current Product Capabilities

| Capability | State |
| --- | --- |
| Runtime lifecycle | Implemented |
| Runtime MCP endpoint handoff | Implemented |
| Workspace/path security | Implemented |
| Policy + ExecutionPipeline | Implemented |
| `read_file` / `glob` | Implemented and externally accepted |
| Secret storage | Implemented |
| Tunnel launch/readiness | Implemented |
| Harness orchestration | Implemented |
| Runtime/Tunnel crash convergence | Implemented |
| Activity / diagnostics | Implemented |
| NSIS distribution | Implemented |
| Portable distribution | Implemented |
| External ChatGPT E2E | Accepted |
| Approval resume workflow | Not implemented; post-V2 |
| Write / Shell / DB / Git-write tools | Intentionally outside V2 |

## 4. Documentation Map

### [`ARCHITECTURE.md`](./ARCHITECTURE.md)

当前架构事实：进程边界、Control Plane、MCP Data Plane、Harness lifecycle、安全边界和 ownership。

### [`MCP-AND-TUNNEL.md`](./MCP-AND-TUNNEL.md)

当前 Runtime MCP endpoint handoff、Tunnel launch contract、readiness、Secret boundary 和 failure semantics。

### [`CODEBASE-MAP.md`](./CODEBASE-MAP.md)

源码目录职责、关键调用链和 source-of-truth 位置。

### [`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md)

功能是否真的实现，以及属于 automated verification 还是 manual external acceptance。

### [`ACCEPTANCE.md`](./ACCEPTANCE.md)

V2 最终验收记录和后续版本边界。

## 5. Documentation Classes

```text
Current source / tests / manifests
        ↓
wiki/**                    current implementation facts
        ↓
docs-codex/**              completed V2 planning/execution records
        ↓
docs/archive/**            older historical material
```

`docs-codex/**` 不再代表“下一步一定要做什么”；V2 已完成，它们现在用于保留设计与执行过程。

`docs/archive/**` 只用于历史追溯。

## 6. Source-of-Truth Rules

当文档和代码不一致时：

1. 当前源码、测试、manifest 优先。
2. `wiki/**` 用来解释当前实现。
3. `docs-codex/**` 只提供 V2 的计划/执行背景。
4. `docs/archive/**` 不参与当前状态判断。

不要因为某个 class、DTO、mock 或旧 roadmap 存在，就推断产品链仍未完成或某项能力已经存在。

## 7. Current Scope

V2 freeze 后保持最小、只读、安全产品链：

```text
read_file
glob
```

下一阶段如果扩展高权限能力，应先完成可恢复的 Approval workflow：

```text
Policy = ASK
   ↓
Pending approval
   ↓
User allow / deny
   ↓
Resume the original request
   ↓
Execute or reject
```

在这条链完成前，不建议直接加入 `write_file`、Git write、Shell 或数据库执行。
