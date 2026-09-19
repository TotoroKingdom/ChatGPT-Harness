# V2 Roadmap

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

Tauri 负责本机 Runtime 与 Tunnel 的生命周期和状态编排。

实际 MCP 数据流为：

```text
ChatGPT ⇄ Tunnel ⇄ Runtime MCP
```

## V2 Path

### 1. Connect

打通：

```text
Tunnel ⇄ Runtime MCP
```

让 Tauri 能够获得 Runtime MCP endpoint，并使用本机配置与 Secret 启动和管理 Tunnel。

### 2. Orchestrate

形成统一 Harness 生命周期：

```text
Start Harness
    ↓
Runtime Ready
    ↓
Tunnel Ready
    ↓
Harness Ready
```

以及：

```text
Stop Harness
    ↓
Tunnel Stop
    ↓
Runtime Stop
```

UI 展示真实 Runtime / MCP / Tunnel 状态。

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

并确保安装版能够完成相同流程。

## Definition of Done

V2 完成时，用户可以：

```text
配置 Workspace / Tunnel ID / API Key
        ↓
Start Harness
        ↓
ChatGPT 连接成功
        ↓
调用 read_file / glob
        ↓
Runtime 安全执行
        ↓
UI 显示真实状态与 Activity
        ↓
Stop Harness
```

在这条主链完全跑通之前，不扩展 Bash、数据库工具、写文件、Git 写操作等高权限能力。
