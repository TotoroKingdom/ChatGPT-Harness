# ChatGPT-Harness

**让 ChatGPT 安全、可控地使用你的本地工作区。**

ChatGPT-Harness 是一款面向 Windows 的桌面应用。它通过 MCP（Model Context Protocol）和 Tunnel 将 ChatGPT 的工具调用连接到本机，让 ChatGPT 在你指定的工作区内读取文件、编辑内容或执行受控命令，同时由本地 Runtime 负责权限校验、操作审批和活动记录。

> **当前源码版本：0.1.3** · **支持平台：Windows** · **V4 正在开发中**（下方的功能介绍以当前代码已经具备的能力为准，并非 V4 全部功能已发布）

## 主要功能

| 功能 | 你可以做什么 |
| --- | --- |
| 本地文件操作 | 使用 `read_file`、`glob`、`write`、`edit` 在指定工作区内读取、查找和修改文件 |
| 命令执行 | 通过受控的 `bash` 工具运行命令 |
| 工具管理 | 按需启用或停用工具，查看当前生效状态 |
| 权限策略 | 使用 Policy 规则控制工具行为，按规则允许、拒绝或要求审批 |
| 人工审批 | 在 Approvals 页面处理需要确认的操作 |
| 活动记录 | 在 Activity 页面查看执行结果和相关事件，辅助排查问题 |
| 连接管理 | 在桌面应用中启动、停止 Harness，查看 Runtime、MCP、Tunnel 的状态 |
| 界面体验 | 提供中文/英文界面及深色、浅色外观设置 |

**安全边界：** 工具调用必须经过 Runtime 的工作区约束和权限检查；Tunnel 负责通信转发，不代替 Runtime 做授权。涉及文件修改和命令执行时，请先检查工具及 Policy 配置，并对重要文件做好备份。

## 工作原理

```text
ChatGPT
   ↕
Tunnel（远程连接）
   ↕
本机 Runtime MCP
   ↓
工作区校验 → 权限策略 / 人工审批
   ↓
文件工具 / 命令执行
   ↓
Activity / Audit 记录
```

Windows 桌面应用由 React UI 和 Tauri Host 组成，负责配置管理、状态展示以及 Runtime / Tunnel 进程的启动和停止。实际的工具执行与安全校验发生在 Runtime 中。

## 快速开始

1. **准备应用。** 使用项目构建的 Windows Portable 便携版或 NSIS 安装包，也可以按照下文自行从源码运行。
2. **配置工作区。** 打开应用，在 **Settings（设置）** 中选择允许访问的本地 Workspace 目录。
3. **配置连接。** 在 Settings 中填写所需的 Tunnel ID、API Key；网络环境需要时配置 Control Plane HTTP Proxy。请勿把密钥粘贴到日志或公开 Issue。
4. **检查权限。** 在 **Tools（工具）** 中选择开放的工具，在 **Policies（策略）** 中设置允许、拒绝和审批规则。
5. **启动 Harness。** 前往 **Overview（概览）**，点击 Start，检查 Runtime、MCP 与 Tunnel 的实际就绪状态。
6. **连接 ChatGPT 并验证。** 按所用 ChatGPT 连接方式配置对应的 Tunnel/MCP 入口，先尝试工作区内的只读 `read_file` 或 `glob`；需要授权的操作在 **Approvals（审批）** 中处理，结果可在 **Activity（活动）** 中查看。

> 仅启动本地 Runtime 并不等于外部 ChatGPT 已连接成功。若连接失败，请分别检查 Workspace、Tunnel 配置、凭据、代理以及界面显示的就绪状态。

## 界面导览

- **Overview（概览）**：启动或停止 Harness，查看组件运行与连接状态。
- **Tools（工具）**：管理哪些工具能够被使用。
- **Policies（策略）**：配置不同工具和场景的权限规则。
- **Approvals（审批）**：查看、处理需要人工确认的请求。
- **Activity（活动）**：查看最近的操作及执行证据。
- **Settings（设置）**：工作区、连接凭据、代理、语言、外观及其他配置。

## 从源码运行

开发环境为 Windows，需要 Node.js / npm、Rust 工具链，以及 Tauri 2 所需的 Windows 构建依赖。

```powershell
# 在仓库根目录
cargo check --workspace
cargo test --workspace

# 启动桌面应用
cd ui
npm install
npm run tauri:dev
```

前端检查与构建：

```powershell
cd ui
npm run typecheck
npm test
npm run build
```

Windows 打包（在 `ui/` 目录运行）：

```powershell
npm run portable:windows  # 便携版
npm run tauri:build       # NSIS 安装包
```

打包需要相关 Runtime/Tunnel 分发资源和 Windows 构建环境，具体以仓库构建脚本为准。

## 版本进展与开发文档

当前 0.1.3 代码提供了本地 Harness 的基础能力；**V4 仍处于迭代开发阶段**。托盘与后台运行、通知、自动启动、连接诊断、结构化追踪、评测能力等新功能以实际里程碑验收结果为准，不能因为列入规划就视为已经发布。

想了解后续规划或参与开发，可以查阅：

- [V4 产品需求（PRD）](docs-codex/ChatGPT-Harness_V4_PRD.md)
- [V4 架构设计](docs-codex/ChatGPT-Harness_V4_ARCHITECTURE.md)
- [V4 开发路线图](docs-codex/ChatGPT-Harness_V4_ROADMAP.md)
- [开发 Agent 规则](AGENTS.md)
- [代码与历史技术说明（Wiki）](wiki/README.md)（部分历史描述可能滞后，以当前源码为准）

**说明：** 开发文档中的计划、测试用例和验收状态不等同于用户可用的已发布功能。
