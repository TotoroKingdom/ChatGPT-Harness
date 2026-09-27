# V2 Roadmap

> **Status: COMPLETED / ACCEPTED — 2026-09-27**  
> 本文件保留为 V2 的历史执行路线，不再代表当前待办。当前实现状态以 `wiki/**` 与源码为准；最终验收见 `wiki/ACCEPTANCE.md`。

## Goal

完成 ChatGPT-Harness 第一条真正可用的产品链路：

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↕
Tools / Policy / Execution
   ↓
Activity / Diagnostics
   ↓
Tauri
   ↓
UI
```

Tauri 负责本机 Runtime 与 Tunnel 的生命周期、连接和状态编排。

实际 MCP 数据流为：

```text
ChatGPT ⇄ Tunnel ⇄ Runtime MCP
```

V2 的核心目标不是继续扩展 Runtime 能力，而是把现有 Runtime、MCP、Tunnel、Tauri 和 UI 连接成一条真实可用、可验证、可安装的产品链路。

## V2 Path

### 1. Connect

打通：

```text
Tunnel ⇄ Runtime MCP
```

完成 Runtime MCP endpoint 向 Tauri 的可靠交付，并让 Tunnel 使用真实 Runtime endpoint 建立连接。

完成 Tunnel 启动所需的配置与凭据传递能力，包括 Tunnel ID、Runtime MCP endpoint 和 API Key；凭据不得进入普通配置、日志、Activity 或 Runtime。

建立能够区分以下状态的真实连接信号：

```text
Tunnel Process Running
        ≠
Tunnel Connected
        ≠
MCP Reachable
```

只有真实外部连接建立后，才能进入 Harness Ready。

**Result: completed.**

### 2. Orchestrate

形成统一 Harness 生命周期：

```text
Start Harness
    ↓
Runtime Ready
    ↓
Tunnel Connected
    ↓
MCP Verified
    ↓
Harness Ready
```

停止顺序：

```text
Stop Harness
    ↓
Tunnel Stop
    ↓
Runtime Stop
```

Runtime、Tunnel 任一启动失败、退出或崩溃时，Harness 必须退出 Ready 状态，并收敛到明确、可恢复的 lifecycle state。

Tauri 成为 Runtime 与 Tunnel 生命周期的统一 owner，UI 展示真实 Runtime / MCP / Tunnel / Harness 状态。

**Result: completed.**

### 3. Validate

完成真实外部链路：

```text
ChatGPT
   ↓
Tunnel
   ↓
Runtime MCP
   ↓
read_file / glob
   ↓
WorkspaceGuard
   ↓
Policy / Execution
   ↓
Activity
   ↓
UI
```

验证真实 ChatGPT 能够发现并调用 Runtime MCP 工具，并完成 packaged product acceptance。

**Result: accepted on 2026-09-27.**

## Definition of Done

V2 完成时，用户可以：

```text
配置 Workspace / Tunnel ID / API Key
        ↓
Start Harness
        ↓
Runtime Ready
        ↓
Tunnel Connected
        ↓
ChatGPT 连接成功
        ↓
调用 read_file / glob
        ↓
Runtime 安全执行
        ↓
UI / Activity 提供真实状态与执行证据
        ↓
Stop Harness
```

并且：

- `Harness Ready` 表示真实 Runtime + MCP + Tunnel readiness，而不是单一进程存在。
- Runtime 或 Tunnel 失败时，Harness 不保持错误的 Ready。
- Raw API Key 不进入普通配置、Runtime、日志或 Activity。
- Windows packaged distribution 能携带 Runtime/Tunnel 所需 payload。

这些目标已经完成并进入 V2 freeze。后续高权限能力属于新的版本范围。
