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

如果当前 tunnel-client 无法满足所需的安全凭据传递能力，则升级或替换 Tunnel client，使该链路能够真实工作。

建立能够区分以下状态的真实连接信号：

```text
Tunnel Process Running
        ≠
Tunnel Connected
        ≠
MCP Reachable
```

只有真实外部连接建立后，才能进入 Harness Ready。

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

Start Harness 必须检查完整 Harness 启动条件，而不仅是 Runtime 是否可启动。

停止顺序为：

```text
Stop Harness
    ↓
Tunnel Stop
    ↓
Runtime Stop
```

Runtime、Tunnel 任一启动失败、退出或崩溃时，Harness 必须退出 Ready 状态，并收敛到明确、可恢复的生命周期状态，不留下错误的 Ready 状态或失控子进程。

Tauri 成为 Runtime 与 Tunnel 生命周期的统一 owner。

UI 展示真实的：

```text
Runtime
MCP
Tunnel
Harness
```

状态，不再以 Runtime readiness 推断 Tunnel 或外部 MCP readiness。

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

验证 ChatGPT 可以通过真实 Tunnel：

* 发现 Runtime MCP
* 获取有效工具
* 调用 `read_file`
* 调用 `glob`
* 经过 WorkspaceGuard、Policy 和 ExecutionPipeline
* 在 UI 中看到对应的真实 Activity 和状态变化

完成 Runtime 与 Tunnel 的安装分发，使安装版不依赖源码目录或开发环境中的二进制文件。

在干净安装环境中验证：

```text
Install
   ↓
Configure
   ↓
Start Harness
   ↓
ChatGPT Connect
   ↓
Tool Call
   ↓
Activity
   ↓
Stop Harness
```

开发版和安装版必须完成相同的产品链路。

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
UI 显示真实状态与 Activity
        ↓
Stop Harness
```

并且：

* `Harness Ready` 表示真实外部 MCP 链路已经可用，而不是仅表示 Runtime 或 Tunnel 进程存在。
* Runtime 或 Tunnel 发生失败时，UI 能反映真实状态，Harness 不会保持错误的 Ready。
* API Key 不进入普通配置、Runtime、日志或 Activity。
* 安装版能够独立完成同样的完整流程。

在这条主链完全跑通之前，不扩展 Bash、数据库工具、写文件、Git 写操作等高权限能力。
