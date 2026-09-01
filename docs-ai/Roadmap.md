# ChatGPT-Harness V1 Roadmap

## M0 — 冻结 UI ↔ Runtime 通信协议

目标：先确定 UI 和 Runtime 的边界，避免后续互相牵制。

完成：

* 定义 IPC 协议
* 定义 Config Schema
* 定义 RuntimeState
* 定义 Tool / Policy / Activity / Approval / Diagnostics 数据模型
* 定义 Runtime 生命周期接口

核心接口：

* `runtime.status`
* `runtime.start`
* `runtime.stop`
* `tools.list`
* `policies.list`
* `activity.list / subscribe`
* `approvals.list / resolve`
* `diagnostics.run`

产物：

* `docs/IPC.md`
* `docs/CONFIG.md`
* 公共数据模型定义

验收：

* UI 和 Runtime 可以基于该协议独立开发
* 后续禁止 UI 自行定义另一套 Tool / Policy / Runtime 状态模型

---

## M1 — Tauri UI Skeleton

目标：先完成可运行的桌面控制面框架。

完成：

* Tauri 项目
* Overview
* Tools
* Policies
* Activity
* Settings
* 页面导航
* UI State Store
* IPC Client abstraction
* Runtime 未连接状态
* Error / Loading / Empty 状态

此阶段允许使用 Mock Runtime 数据。

要求：

* Mock 数据必须严格遵守 M0 IPC Contract
* Tools / Policies 等页面不能形成独立业务逻辑
* UI 只展示状态，不实现安全策略

验收：

* 五个页面完整可操作
* 所有数据均通过统一 IPC Client 获取
* 删除 Mock Runtime 后无需重构页面架构

---

## M2 — UI 配置与生命周期体验

目标：先把最终用户操作流程做完整。

完成：

* First Run Setup
* Workspace 选择
* Tunnel ID 配置
* API Key 输入
* tunnel-client 路径选择
* Developer/Test Mode 开关
* Start Harness
* Stop Harness
* Run Diagnostics
* Restart Required / Config Mismatch 状态

同时确定：

* 本地配置保存路径
* Secret Store abstraction
* Runtime / Tunnel / MCP 独立状态展示

验收：

用户可以完整走通：

`首次启动 → 配置 → Diagnostics → Start → Stop`

此时后端仍可由 Mock Runtime 提供。

---

## M3 — Rust Runtime Skeleton

目标：建立真正独立运行的 Runtime。

完成：

* Rust Runtime executable
* Config loading
* Runtime ID
* RuntimeState
* Structured Event Bus
* Local IPC Server
* Graceful Shutdown
* Process identity

Runtime 此时暂不实现 MCP Tool。

验收：

* UI 可以连接真实 Runtime
* `runtime.status` 可用
* Start / Stop 可用
* Runtime 崩溃后 UI 能识别
* Mock Runtime 可以开始移除

---

## M4 — Workspace Security Boundary

目标：实现整个 Runtime 的统一文件安全边界。

完成：

* `Workspace`
* `WorkspaceGuard`
* `PathResolver`

覆盖：

* 相对路径
* `..` traversal
* absolute path
* Windows drive path
* symlink
* junction
* canonicalization
* workspace escape

要求：

所有文件工具只能通过统一 `WorkspaceGuard` 访问文件系统。

验收：

* workspace 内访问允许
* workspace 外访问拒绝
* symlink / junction 无法逃逸

---

## M5 — Read-Only MCP Runtime

目标：完成 V1 Normal Mode 核心能力。

实现：

* MCP Protocol
* Tool Registry
* ToolRequest
* ToolResult
* `read_file`
* `glob`

Normal Mode Tool Catalog 必须只有：

* `read_file`
* `glob`

不增加：

* tree
* list_files
* git
* database
* write
* shell

验收：

ChatGPT / MCP Client 可以真实调用：

`read_file`
`glob`

且只能访问 Workspace 内文件。

---

## M6 — Policy / State Machine / Audit

目标：完成 Harness 核心安全执行链。

统一 Pipeline：

`MCP Request`
→ Validation
→ Workspace Guard
→ Policy Engine
→ Executor
→ ToolResult
→ Audit Event

完成：

* `DENY / ASK / ALLOW`
* `DENY > ASK > ALLOW`
* Request State Machine
* Structured Error
* Audit Event
* Secret Redaction
* Runtime effective Tool Catalog

验收：

* 所有执行请求经过 Policy Engine
* 不存在绕过 Policy 的 Executor 调用
* Activity 页面开始使用真实 Runtime Event
* Tools / Policies 页面使用 Runtime 真实状态

---

## M7 — tunnel-client Integration

目标：真正打通：

`ChatGPT → tunnel-client → Harness Runtime`

完成：

* tunnel-client discovery
* version detection
* executable validation
* Tunnel Process Manager
* Tunnel State
* MCP readiness
* Harness lifecycle orchestrator

Start Harness：

`Validate Config`
→ Start Runtime
→ Runtime Ready
→ Start tunnel-client
→ Tunnel Healthy
→ Verify MCP
→ Verify Tool Catalog
→ Ready

验收：

* Runtime / Tunnel / MCP 状态独立
* 任一组件失败都不能显示 Ready
* Workspace / Runtime 配置不一致显示 Restart Required
* 不仅依赖 PID 判断进程身份

---

## M8 — Developer/Test Mode

目标：在 Normal Mode 稳定后加入高风险测试能力。

实现：

* `bash`
* `CommandExecutor`
* ASK Policy
* Approval Manager
* Approval UI
* Allow Once
* Deny
* Expire
* Cancel

同时实现：

* timeout
* cancellation
* stdout / stderr
* output limit
* exit code
* cwd control
* process-tree cleanup
* hard DENY

验收：

Normal Mode：

`read_file + glob`

Developer/Test Mode：

`read_file + glob + bash`

其中：

`bash → ASK → Allow Once / Deny`

Hard DENY 永远不能通过人工批准绕过。

---

## M9 — Hardening & Release

目标：完成可交付 V1。

完成：

* crash recovery
* stale process detection
* config/runtime mismatch
* secret redaction verification
* symlink/junction tests
* structured diagnostics
* release packaging
* clean-machine acceptance test

最终验收流程：

`安装 / 解压`
→ 启动 UI
→ 选择 Workspace
→ 输入 Tunnel ID
→ 输入 API Key
→ 检测 tunnel-client
→ Diagnostics
→ Start Harness
→ Ready
→ ChatGPT 调用 `read_file / glob`
→ Workspace 外访问被拒绝
→ Stop Harness

V1 到此结束。

---

# 推荐 Codex 执行顺序

1. IPC / Config Contract
2. Tauri UI Skeleton
3. UI First Run / Settings / Lifecycle
4. Rust Runtime Skeleton
5. WorkspaceGuard
6. `read_file + glob + MCP`
7. Policy + State Machine + Audit
8. tunnel-client
9. Developer/Test `bash + Approval`
10. Hardening + Release

## V1 边界

在 M9 完成之前，不增加：

* write / edit
* Git Tools
* Database Tools
* Skills
* Agent Loop
* Memory
* SubAgent
* Background Tasks
* Plugin System

核心原则：

**先定协议 → UI 先行 → Runtime 实现协议 → Read-Only 链路跑通 → tunnel 接入 → 最后 bash → 最后发布。**
