# Task：M7-A — Tunnel Integration Design Freeze

开始 M7 阶段设计。

本任务只做架构分析和设计冻结。

禁止写代码。

---

# 当前状态

已完成：

M0:
UI ↔ Runtime protocol

M1-M5:
Runtime core + MCP tools

M6:
Runtime security layer

当前 Runtime 已具备：

- ToolRegistry
- EffectiveToolCatalog
- ExecutionPipeline
- PolicyEngine
- WorkspaceGuard
- AuditEvent
- MCP exposure boundary

---

# M7 目标

建立：

ChatGPT
|
v
Tunnel Client
|
v
Runtime MCP Service


使外部 ChatGPT 可以安全访问本机 Runtime。

---

# 非目标

本任务不要：

- 创建 tunnel-client
- 修改 Runtime execution pipeline
- 修改 MCP schema
- 修改 Policy semantics
- 修改 UI
- 写安装脚本
- 写配置迁移代码

---

# 需要分析并冻结以下问题

## 1. Tunnel 生命周期模型

设计：

谁负责启动 tunnel-client？

候选：

A.

Launcher:

UI
|
启动 tunnel-client


B.

Runtime:

Runtime
|
spawn tunnel-client


C.

独立 service manager


分析优缺点。

给出 V1 推荐方案。


---

## 2. Runtime 与 tunnel-client 边界

明确：

Runtime 负责什么？

Tunnel client 负责什么？


禁止职责混合。


例如：

Runtime:

- MCP server
- tools
- security


Tunnel:

- external connectivity
- authentication transport
- reconnect


---

## 3. API Key 管理

设计：

API Key 生命周期：

- 输入
- 保存
- 使用
- 删除


明确：

禁止：

- git
- logs
- audit event
- Runtime source config


需要决定：

V1 使用：

- OS credential store
- local encrypted config
- plain local config


说明理由。


---

## 4. Tunnel 配置模型

设计配置：

例如：

TunnelConfig

包含：

- tunnel id
- api key reference
- endpoint
- runtime binding


不要包含：

secret 明文。


---

## 5. 多实例问题

分析：

是否允许：

多个 Runtime

绑定：

同一个 tunnel-id


是否允许：

多个 tunnel-client

连接同一个 Runtime。


给出限制。


---

## 6. Runtime 状态模型

设计：

Tunnel 状态是否进入 RuntimeState。


例如：

Disconnected

Starting

Connected

Failed


说明：

哪些状态属于 Runtime。

哪些属于 Launcher。


---

## 7. Workspace 切换影响

当前已有：

WorkspaceGuard

分析：

切换 workspace 时：

是否需要：

restart tunnel-client？

refresh catalog？

restart runtime？


给 V1 行为。


---

## 8. Shutdown 顺序

冻结：

关闭顺序：

例如：

ChatGPT connection

↓

Tunnel

↓

Runtime

↓

MCP tools


避免：

runtime 已关闭

tunnel 仍连接。


---

## 9. Failure Recovery

设计：

以下情况：

Tunnel crash

Runtime crash

API key invalid

Network failure


V1 行为。


---

# 输出文档

新增：

docs/TUNNEL.md


内容：

1. Architecture
2. Component responsibility
3. Lifecycle
4. Configuration model
5. Security boundary
6. Failure handling
7. V1 limitations


---

# 验收要求

只输出：

## Architecture Decision

## Component Boundary

## Lifecycle

## Config Model

## Security Model

## Open Questions


不要修改代码。

不要 commit。