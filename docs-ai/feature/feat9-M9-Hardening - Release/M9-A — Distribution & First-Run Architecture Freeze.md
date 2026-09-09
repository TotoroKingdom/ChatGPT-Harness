# Task: M9-A — Distribution & First-Run Architecture Freeze

目标：

为 ChatGPT-Harness 的 Windows 本地分发、安装和首次启动冻结架构。

本任务只调查现有仓库并输出架构文档。
不要实现 installer。
不要修改 Runtime 功能。

---

# 项目当前状态

已完成：

- M0-M5 Runtime foundation
- M6 Security Runtime
- M8 Local Runtime Productization
- 真实 Tauri Desktop 本地闭环验收 PASS

M7-C Credential Transport：

BLOCKED

因此 M9 不解决 Tunnel external connectivity。

---

# 首先审查现有仓库

必须检查：

- workspace Cargo.toml
- runtime Cargo.toml
- ui/package.json
- ui/src-tauri/Cargo.toml
- tauri.conf.*
- RuntimeSupervisor
- Runtime executable discovery / launch logic
- config persistence
- SecretStore
- .gitignore
- 当前 build scripts
- 当前 Runtime binary 输出位置
- tunnel-client 当前仓库布局

不要根据假设设计。

---

# M9 核心问题

回答以下问题并冻结决策。

## 1. 安装拓扑

Windows 安装后应形成什么结构？

例如概念上：

ChatGPT Harness/
├─ chatgpt-harness-ui.exe
├─ runtime/
│  └─ chatgpt-harness-runtime.exe
└─ ...

必须决定：

Runtime 是：

A. Tauri sidecar
B. bundled resource
C. 独立安装 binary
D. 其他

分析各方案：

- Tauri 支持度
- executable discovery
- upgrade
- process lifecycle
- security
- dev/prod parity

给出 V1 决策。

---

## 2. Runtime Discovery

当前开发环境可能依赖 target/debug、target/release 或仓库相对路径。

正式安装后禁止依赖：

- Git repository root
- cargo target directory
- developer machine absolute path
- D:\ChatGPT-Harness
- current working directory 偶然值

冻结：

Development discovery

与

Installed discovery

两个模式。

RuntimeSupervisor 必须最终能够明确知道它启动的是哪个 Runtime binary。

---

## 3. Local Data Layout

冻结 Windows 本机数据目录。

区分：

Application binaries

与：

User data


需要定义：

- AppConfig
- local state
- logs（若当前产品有）
- future policy config
- secrets
- transient runtime metadata

明确哪些：

可进入 Git

哪些：

绝不能进入 Git

哪些：

由 Windows Credential Manager 保存。

不要迁移 secret 到普通文件。

---

## 4. First Run

定义首次启动流程。

例如：

App launch
↓
Detect local config
↓
Create default non-secret config
↓
Select Workspace
↓
Validate Runtime
↓
Optional API Key configuration
↓
Ready for local Runtime Start

注意：

M7-C BLOCKED。

API Key 可以继续保存于 SecretStore，
但不能暗示已经可以用于 tunnel-client。

首次启动不能因为 Tunnel unavailable 而阻止 Local Runtime 使用。

---

## 5. Config Portability

当前项目目标之一：

代码可分享，
机器配置不可进入 Git。

检查当前是否仍可能存在：

- absolute path
- runtime path
- workspace path
- tunnel-client path
- local config

被错误写入 repo tracked files。

冻结：

example/default config

vs

machine-local config

模型。

---

## 6. Upgrade Boundary

V1 至少说明：

升级 UI 时：

Runtime binary 如何保持版本一致？

必须避免：

UI version A
+
Runtime version B

被静默组合。

设计最小 compatibility/version check。

不要现在实现复杂 updater。

---

## 7. tunnel-client

因为 M7-C BLOCKED：

M9 不要求 tunnel-client 可运行。

但是必须决定 distribution 中：

- 是否暂时打包 tunnel-client
- 是否不打包
- 是否作为 optional external dependency

基于当前安全边界给出推荐。

不要因为“已经有 binary”就默认打包。

---

# 安全要求

安装/首次启动设计不得削弱：

- Workspace boundary
- Policy boundary
- SecretStore boundary
- Runtime / Tunnel separation
- M7-C Strict Security Boundary

禁止：

- plaintext API key
- secret env fallback
- secret file
- argv secret

---

# 输出文档

新增：

docs/DISTRIBUTION.md

包含：

1. Current repository/build audit
2. Installed topology
3. Runtime packaging decision
4. Runtime discovery
5. Local data layout
6. First-run flow
7. Config portability
8. Version compatibility
9. Tunnel packaging decision
10. Security boundary
11. M9-B implementation scope

---

# 重要原则

本任务由你执行调查，
但不要自行改变产品架构。

如果发现 PRODUCT / ARCHITECTURE / Roadmap
与上述目标冲突：

停止并报告冲突，
不要擅自修改最高级文档。

---

# 禁止

不要：

- 修改 Runtime execution logic
- 修改 MCP
- 修改 Policy
- 修改 Tunnel
- 修改 SecretStore security model
- 实现 installer
- 实现 auto updater
- 新增 UI 页面
- commit

---

# Validation

若只修改 docs：

git diff --check

报告当前 git status。

不要 commit。