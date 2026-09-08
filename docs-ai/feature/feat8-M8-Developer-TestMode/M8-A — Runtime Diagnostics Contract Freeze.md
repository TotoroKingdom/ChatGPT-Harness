# Task: M8-A — Runtime Diagnostics Contract Freeze

目标：

设计并实现 Runtime Diagnostics Snapshot。

这是 M8 的第一步。

---

## 当前状态

已完成：

- M5 MCP Runtime
- M6 Security Runtime Layer
- M7 Tunnel Architecture Freeze
- M7 Tunnel Process Manager

M7-C Credential Transport:

BLOCKED

禁止重新打开 tunnel credential 设计。

---

# 本任务目标

建立统一诊断模型：

Runtime → Diagnostics → Launcher/UI

用于：

- 启动状态展示
- 故障定位
- Activity 页面基础
- 后续 UI 产品化

---

# 设计原则

Diagnostics 不是 RuntimeState。

RuntimeState:

- runtime 内部运行状态

Diagnostics:

- 面向控制平面的安全观察快照

禁止：

- 修改 MCP schema
- 修改 tool schema
- 修改 Policy semantics
- 修改 WorkspaceGuard
- 修改 Tunnel credential boundary

---

# 需要设计的数据模型

建议包含：

## Runtime Diagnostics

例如：

- runtime identity
- readiness
- initialized modules
- workspace status
- policy status
- effective tool summary
- active request summary
- last error summary

---

## Security Diagnostics

包含：

- workspace validation status
- policy engine loaded
- audit enabled
- execution pipeline state

禁止包含：

- API Key
- Secret
- Token
- credential
- absolute sensitive paths

---

## Tool Diagnostics

展示：

Declared Tool:

↓

Effective Tool:

状态：

- available
- unavailable by policy
- unavailable by runtime
- disabled

---

## Tunnel Diagnostics

只保留观察模型：

例如：

- configured
- process state
- connection state
- blocked reason

不要实现连接。

M7-C BLOCKED 必须保持。

---

# 输出要求

新增：

docs/DIAGNOSTICS.md

描述：

1. Diagnostics boundary
2. RuntimeState vs Diagnostics 区别
3. Data ownership
4. Security redaction rules
5. Future UI consumption

---

# 代码范围

允许：

runtime:

新增 diagnostics module

ui:

如果需要，增加只读读取接口

禁止：

- UI 页面
- React component
- tunnel implementation
- credential code

---

# 验证

执行：

cargo fmt --all -- --check

cargo check --workspace

cargo test --workspace

git diff --check

不要 commit。