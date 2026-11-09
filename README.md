# ChatGPT-Harness

**让 ChatGPT 安全、可控地使用你的本地工作区。**

ChatGPT-Harness 是一款面向 Windows 的桌面应用。它通过 MCP（Model Context Protocol，模型上下文协议）和 Tunnel 将 ChatGPT 的工具调用连接到本机，让 ChatGPT 在你指定的 Workspace（工作区）内读取、搜索、编辑文件或执行受控命令，同时由本地 Runtime（运行时）负责权限校验、人工审批和审计记录。

> **当前源码版本：0.1.3** · **支持平台：Windows** · **当前行为以 `main` / HEAD 的源码、测试和构建脚本为准**

## 主要功能

| 能力 | 你可以做什么 |
| --- | --- |
| 文件读取与搜索 | 使用 `read_file`、`glob`、`grep` 读取文件、匹配路径和搜索工作区文本 |
| 文件修改 | 使用 `write`、`edit` 创建或覆盖文件，或进行唯一精确匹配的文本替换 |
| 受控命令 | 在 Developer/Test 模式下使用 `bash` 执行受控 Git Bash 命令 |
| 工具管理 | 按需启用或停用工具，查看 Runtime 返回的 effective（实际生效）状态 |
| 权限策略 | 使用 Policy 规则控制工具行为，按规则允许、拒绝或要求审批 |
| 人工审批 | 在 Approvals 页面处理 Allow once、Allow for session 或 Deny |
| 活动与历史 | 查看实时 Activity；使用本地 SQLite History 查询历史请求、指标和请求追踪 |
| 桌面与后台 | 关闭窗口后隐藏到系统托盘；接收审批/连接通知；Windows 单实例运行 |
| 启动与诊断 | 可选择应用启动后自动启动 Harness；运行一键诊断并查看启动阶段耗时 |
| 连接管理 | 在桌面应用中启动、停止、重启 Harness，查看 Runtime、MCP、Tunnel 状态 |
| 界面体验 | 提供中文/英文界面，以及深色、浅色和跟随系统外观 |

**安全边界：** 工具调用必须经过 Runtime 的 Workspace 约束和权限检查；Tunnel 只负责通信转发，不代替 Runtime 做授权。`bash` 的工作目录固定在 Workspace，但它不是操作系统级文件系统沙箱。涉及文件修改和命令执行时，请先检查工具及 Policy 配置，并对重要文件做好备份。

## 工具与默认状态

| 工具 | 能力 | 新配置默认值 |
| --- | --- | --- |
| `read_file` | 读取工作区文件 | 启用，Policy = ALLOW |
| `glob` | 按模式匹配工作区路径 | 启用，Policy = ALLOW |
| `grep` | 在工作区内进行文本/正则搜索 | 启用，Policy = ALLOW |

> `glob` 和 `grep` 默认遵循 `.gitignore`、嵌套 `.gitignore`、`.ignore` 与 Git 排除规则，并在遍历阶段剪枝被忽略的目录（不会硬编码 `target`、`node_modules` 等名称）。显式指定被忽略目录作为 `basePath`、`scope`、固定目录前缀或 `include` 前缀时，仍可搜索该目录。可在 **Settings → 搜索工具** 分别设置 5–120 秒的 glob/grep 超时，重新启动 Harness 后生效。
| `write` | 原子创建或覆盖 UTF-8 文件 | 启用，Policy = ALLOW |
| `edit` | 唯一精确匹配后原子替换 | 启用，Policy = ALLOW |
| `bash` | 受控 Git Bash 命令 | 关闭，Policy = ASK；仅 Developer/Test 模式可用 |

> Bash 可在 **Policies（策略）** 中显式开启“Bash 信任模式”。该模式把 Bash 默认值从 ASK 放宽为 ALLOW，但只自动允许已分类且非破坏性的命令；删除、覆盖、磁盘/Git 破坏性操作、未分类命令和显式 DENY 仍会询问或拒绝。信任模式不是文件系统沙箱，包管理器脚本、任意程序内部行为和未被静态识别的操作仍可能执行。

> 从旧版本升级的配置会保留 `grep` 关闭状态，避免在用户不知情时扩大工具面；需要时可在 **Tools（工具）** 中手动启用。

工具是否真正暴露给 ChatGPT，由 Runtime 的 effective catalog 决定：工具必须已实现、已启用、Security Mode 允许、Runtime readiness 满足且 Policy 不为 DENY。`tools/list` 只返回 `Available` 或 `AvailableWithApproval` 的工具。

## 工作原理

```text
ChatGPT
   ↕
Tunnel（远程连接）
   ↕
本机 Runtime MCP
   ↓
参数校验 → 工作区校验 → 权限策略 → 人工审批（按需）
   ↓
文件工具 / grep / 受控命令
   ↓
Activity / History / Metrics / Trace（本地记录）
```

Windows 桌面应用由 React UI 和 Tauri Host 组成，负责配置管理、系统托盘、通知、状态展示以及 Runtime / Tunnel 进程的启动和停止。实际的工具执行与安全校验发生在 Runtime 中。

## 快速开始

1. **准备应用。** 使用项目构建的 Windows Portable 便携版或 NSIS 安装包，也可以按照下文自行从源码运行。
2. **配置工作区。** 打开应用，在 **Settings（设置）** 中选择允许访问的本地 Workspace 目录。
3. **配置连接。** 在 Settings 中填写所需的 Tunnel ID、API Key；网络环境需要时配置 Control Plane HTTP Proxy。请勿把密钥粘贴到日志或公开 Issue。
4. **检查权限。** 在 **Tools（工具）** 中选择开放的工具，在 **Policies（策略）** 中设置允许、拒绝和审批规则。
5. **启动 Harness。** 前往 **Overview（概览）**，点击 Start，检查 Runtime、MCP 与 Tunnel 的实际就绪状态。
6. **连接 ChatGPT 并验证。** 按所用 ChatGPT 连接方式配置对应的 Tunnel/MCP 入口，先尝试工作区内的只读 `read_file`、`glob` 或 `grep`；需要授权的操作在 **Approvals（审批）** 中处理，结果可在 **Activity（活动）** 中查看。

关闭主窗口时，应用会先说明后台运行行为，随后隐藏到系统托盘；需要完全退出时，请使用托盘菜单中的 **Quit**。

> 仅启动本地 Runtime 并不等于外部 ChatGPT 已连接成功。若连接失败，请分别检查 Workspace、Tunnel 配置、凭据、代理以及界面显示的就绪状态。

## 界面导览

- **Overview（概览）**：启动、停止或重启 Harness，查看组件状态、诊断结果和最近一次启动各阶段耗时。
- **Tools（工具）**：管理工具 desired/effective 状态、Policy 默认值以及 Bash readiness。
- **Policies（策略）**：配置 Tool Defaults、User Rules 和只读的 System Rules。
- **Approvals（审批）**：查看并处理需要人工确认的请求。
- **Activity（活动）**：查看实时事件；切换到 History 查询持久化历史，切换到 Metrics 查看调用指标和请求追踪。
- **Settings（设置）**：工作区、连接凭据、代理、安全模式、通知、自动启动、历史保留、搜索工具超时、语言、外观、诊断和重置。

## 本地数据与隐私

- **History（历史）**默认启用，保存在应用数据目录下的 `history/history.sqlite3`，默认保留 30 天、最多占用 256 MiB。可在 Settings 中停用、调整保留期或清空。
- History 保存的是经过裁剪和脱敏的请求生命周期证据，不保存原始 API Key、原始命令输出或完整敏感参数。
- Tunnel API Key 由 Windows Credential Manager 中的 SecretStore 管理，配置文件只保存 opaque SecretReference（不透明引用）。
- Activity 是 Runtime 生命周期内的有界实时视图；History 才是跨重启的持久化记录。两者用途不同。

## 从源码运行

开发环境为 Windows，需要 Node.js 20+、Rust 工具链，以及 Tauri 2 所需的 Windows 构建依赖。

```powershell
# 首次构建 Tauri crate 前，生成 Runtime/Tunnel 分发资源
cd ui
npm install
npm run distribution:stage
cd ..

# 在仓库根目录
cargo check --workspace
cargo test --workspace

# 启动桌面应用
cd ui
npm run tauri:dev
```

若 `target-exe/distribution` 尚不存在，Tauri build script 会因缺少 `distribution.json` 和 Runtime/Tunnel 资源而失败；此时先执行一次 `npm run distribution:stage`。

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

当前 0.1.3 代码提供上述本地 Harness 能力。V4 的开发、测试与验收文档已整体归档；归档不改变各 Milestone 的原始结论，也不代表 V4 已验收、发布或冻结。

文档入口：

- [V4 历史归档](docs/archive/v4-2026-11/README.md)
- [历史文档索引](docs/archive/README.md)
- [下一版本文档工作区](docs-codex/README.md)
- [开发 Agent 规则](AGENTS.md)
- [当前实现 Wiki](wiki/README.md)

**说明：** 开发文档中的计划、测试用例和验收状态不等同于用户可用的已发布功能；本地自动化测试通过也不等于外部 ChatGPT 连接或人工验收已完成。
