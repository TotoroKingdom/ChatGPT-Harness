# ChatGPT-Harness Wiki

> 本 Wiki 描述 **当前代码事实**。  
> 当前实现状态以源码、manifest、测试与构建脚本为准；`docs-codex/**` 是未来开发计划，`docs/archive/**` 是历史资料，均不能反推当前实现。

## 1. Project Overview

ChatGPT-Harness 是一个本机运行的 MCP Harness。

它的目标产品链路是：

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↕
Tools / Policy / Execution
```

Tauri Desktop App 负责本机配置、Secret、Runtime/Tunnel 生命周期和状态展示。

当前项目采用 Rust + Tauri + React：

```text
React UI
   ↓ Tauri IPC
Tauri Host
   ↓ stdin/stdout JSON-RPC
Runtime
   ├── MCP HTTP
   ├── Workspace
   ├── Tools
   ├── Policy
   ├── Execution
   └── Activity / Diagnostics
```

## 2. Architecture at a Glance

项目中存在两条不同的通信链路。

### Control Plane

用于桌面 UI 控制和观察 Runtime：

```text
React UI
   ↓ Tauri invoke / events
Tauri Host
   ↓ stdin/stdout JSON-RPC
Runtime
```

Tauri Host 是 Runtime child process 的生命周期 owner，并负责：

- 本机配置保存
- Secret 保存
- Runtime discovery
- Runtime start / stop / status
- Control RPC 转发
- Runtime event bridge

### MCP Data Plane

目标 MCP 数据流是：

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
Policy
   ↓
Tools
```

Tauri 不位于每一次 MCP tool call 的数据链路中。

Tauri 的职责是启动、停止和协调 Runtime 与 Tunnel。

## 3. Current Implementation State

当前已经实现的主要能力：

| Capability | Current State |
| --- | --- |
| Runtime lifecycle | Implemented |
| Workspace initialization | Implemented |
| Workspace/path security | Implemented |
| `read_file` | Implemented inside Runtime MCP |
| `glob` | Implemented inside Runtime MCP |
| Policy evaluation | Implemented |
| Execution pipeline | Implemented |
| Execution activity snapshot | Implemented |
| Runtime diagnostics | Implemented |
| Config persistence | Implemented |
| Secret storage | Implemented |
| Runtime MCP HTTP endpoint | Implemented / Internal |
| Tunnel process manager | Implemented / Internal |
| Tunnel orchestration | Not Implemented |
| External ChatGPT reachability | Not Implemented |
| Approval resume workflow | Not Implemented |

The important distinction is:

```text
Runtime MCP works locally
        ≠
ChatGPT can already reach Runtime MCP
```

The Runtime currently starts an MCP Streamable HTTP endpoint on a loopback ephemeral port.

However, its endpoint URL is not yet surfaced through the product control path, and the Tunnel is not wired into the normal Harness lifecycle.

## 4. Current Product Boundary

The normal product startup path currently reaches:

```text
UI
 ↓
Tauri
 ↓
Runtime
 ↓
Runtime MCP
```

It does **not** yet continue through:

```text
Runtime MCP
 ↕
Tunnel
 ↕
ChatGPT
```

`TunnelProcessManager` already contains process lifecycle logic, but it is currently an internal implementation:

```text
TunnelProcessManager
├── start
├── stop
├── restart
├── process ownership
├── instance_id
├── exit watcher
└── forced-stop fallback
```

What is still missing is product orchestration:

```text
Runtime MCP endpoint
        ↓
Tauri
        ↓
Tunnel launch arguments / credentials
        ↓
Tunnel Ready
        ↓
ChatGPT
```

## 5. Current Tools

The Runtime currently exposes two read-only MCP tools:

```text
read_file
glob
```

Tool calls pass through the Runtime execution pipeline:

```text
MCP request
   ↓
ExecutionPipeline
   ↓
Argument validation
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
Tool executor
   ↓
Audit / Activity
```

Higher-risk capabilities such as Bash, database access, write-file operations and Git write operations are intentionally outside the current product-completion path.

## 6. Repository Map

```text
ChatGPT-Harness/
├── protocol/
│   └── Shared control protocol models, schemas and generated types
│
├── runtime/
│   └── Runtime process, MCP server, tools, policy and execution
│
├── ui/
│   ├── src/
│   │   └── React UI
│   └── src-tauri/
│       └── Tauri host, config, secrets and process supervision
│
├── tunnel-client-v0.0.13-windows-amd64/
│   └── Tunnel client payload currently present in the repository
│
├── wiki/
│   └── Current code-derived documentation
│
├── docs-codex/
│   └── Forward-looking development plans
│
└── docs/archive/
    └── Historical documentation
```

## 7. Documentation Guide

### Start here

This file is the entry point for rebuilding project context.

Use it to answer:

- What is ChatGPT-Harness?
- What are the main process boundaries?
- What works today?
- Where is the current product chain broken?
- Which document should I read next?

### `CODEBASE-MAP.md`

Read:

[`CODEBASE-MAP.md`](./CODEBASE-MAP.md)

Use it when you need to understand:

- repository structure
- process architecture
- control flow
- important call chains
- subsystem ownership
- source-of-truth code locations
- protocol/fixture drift
- test reality
- interpretation traps

This is the detailed code archaeology reference.

### `IMPLEMENTATION-MATRIX.md`

Read:

[`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md)

Use it when you need to answer:

> Is this capability actually implemented?

It distinguishes:

- `Implemented`
- `Implemented / Internal`
- `Partially Integrated`
- `Stub / Placeholder`
- `Not Implemented`
- `Not Applicable`
- `Unclear`

Do not infer product completion merely because a class, command, DTO or test exists.

### `docs-codex/ROADMAP.md`

Read:

[`../docs-codex/ROADMAP.md`](../docs-codex/ROADMAP.md)

This is **future development direction**, not evidence of current implementation.

The current V2 path is:

```text
Connect
   ↓
Orchestrate
   ↓
Validate
```

The V2 goal is to complete the first real external product chain:

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↕
Tools / Policy / Execution
```

### `docs/archive/**`

Historical documentation is preserved under:

```text
docs/archive/
```

Do not use archived documents to determine current implementation status unless historical context is explicitly required.

## 8. Source-of-Truth Rule

When documentation and code disagree, use this priority:

```text
Current source code / tests / manifests
        ↓
wiki/
        ↓
docs-codex/
        ↓
docs/archive/
```

Interpretation:

- **Source code** tells us what exists now.
- **Wiki** explains the current source-derived architecture and status.
- **docs-codex** describes what should be built next.
- **archive** records historical decisions and previous designs.

If current code evidence is insufficient, mark the fact as `Unclear` rather than filling the gap from old documentation.

## 9. Important Interpretation Traps

Avoid these assumptions:

```text
TunnelProcessManager exists
≠ Tunnel product integration is complete

Runtime MCP endpoint is Ready
≠ ChatGPT can reach it

MCP status is shown as Ready in UI
≠ external reachability is proven

Approval DTOs exist
≠ approval/resume workflow exists

Protocol fixtures pass tests
≠ Runtime wire output exactly matches those fixtures

MockClient supports a state
≠ the real production adapter supports it
```

## 10. Current Development Focus

The current development priority is not adding more tools.

The immediate goal is to complete the missing external connection:

```text
Runtime MCP
   ↕
Tunnel
   ↕
ChatGPT
```

Then unify lifecycle orchestration:

```text
Start Harness
    ↓
Runtime Ready
    ↓
Tunnel Ready
    ↓
Harness Ready
```

And shutdown:

```text
Stop Harness
    ↓
Tunnel Stop
    ↓
Runtime Stop
```

Only after this chain is validated end-to-end should the project expand into higher-permission capabilities.

---

For detailed current implementation facts, continue with:

1. [`CODEBASE-MAP.md`](./CODEBASE-MAP.md)
2. [`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md)

For future development direction, see:

- [`../docs-codex/ROADMAP.md`](../docs-codex/ROADMAP.md)
