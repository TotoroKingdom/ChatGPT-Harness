# Task：M6-C1 — Audit Event Model + Audit Event Emission

开始实现 M6-C 的第一阶段：

Audit Event Model + Pipeline Event Emission。

---

# 当前状态

已完成：

- M6-A Policy Engine
- M6-B Request State Machine + Enforcement Pipeline

当前 Runtime 已有：

- RequestLifecycle
- RequestContext
- ExecutionPipeline
- ExecutionPermit
- WorkspaceGuard boundary validation
- Policy evaluation
- lifecycle hook seam

本任务只建立：

结构化 Audit Event

以及：

ExecutionPipeline 中的事件产生点。

---

# 非目标

本任务不要：

- 实现完整 Audit Storage
- 实现 UI Activity 页面
- 实现 Audit 查询 API
- 实现数据库存储
- 实现远程日志上传
- 实现 telemetry
- 修改 MCP protocol
- 修改 UI
- 修改 Tool schema
- 修改 WorkspaceGuard
- 修改 PolicyEngine

不要进入 M6-D。

---

# 1. 设计目标

建立 Runtime 内部统一 Audit Event 模型。

目标：

任何一次 Tool execution 都能够回答：

- request id
- tool name
- lifecycle 状态
- policy decision
- workspace validation result
- execution result
- timestamp

但不能泄漏：

- API Key
- Secret
- raw credentials
- 完整敏感 arguments

---

# 2. 模块边界

新增独立 Runtime 模块。

推荐：

runtime/src/audit/

例如：

audit/
mod.rs
event.rs
redaction.rs (如果本阶段需要)

具体命名可调整。

职责：

Audit Event definition

不要把 Audit 逻辑塞入：

- policy/
- execution/
- tools/

---

# 3. Audit Event Model

设计结构化事件。

建议包含：

AuditEvent

至少：

- event_id
- request_id
- timestamp
- event_type
- tool_name
- lifecycle_state
- policy_decision
- outcome

具体字段根据现有 protocol/style 调整。

要求：

serde serializable

便于未来：

- UI
- JSONL
- storage

消费。

---

# 4. Event Type

不要定义大量实现细节事件。

当前 M6-C1 最小集合：

RequestReceived

WorkspaceValidated

PolicyEvaluated

ExecutionStarted

ExecutionCompleted

ExecutionFailed

RequestDenied

ApprovalRequired

即可。

不要加入：

PolicyRuleMatched
PathCanonicalized
FilesystemOpened

这些属于内部 debug。

---

# 5. Pipeline 集成

利用 M6-B 已存在 hook seam。

不要重新设计 pipeline。

必须在以下节点产生 event：

## RequestReceived

进入 pipeline 时。

---

## WorkspaceValidated

WorkspaceGuard validation 完成后。

包含：

success/failure

但不要记录：

完整路径。

---

## PolicyEvaluated

PolicyEngine 返回后。

记录：

ALLOW / ASK / DENY

可以记录：

matched rule ids

如果现有结果已有。

不要重新计算 policy。

---

## ExecutionStarted

executor 调用前。

---

## ExecutionCompleted

executor 成功返回后。

---

## ExecutionFailed

executor 返回错误后。

---

## RequestDenied

Policy DENY。

---

## ApprovalRequired

Policy ASK。

---

# 6. Redaction 原则

本阶段必须设计边界。

禁止：

AuditEvent 直接存：

- raw MCP arguments
- raw tool input payload
- API key
- credential
- environment variables

如果未来需要记录 input：

必须经过：

Redaction layer。

---

# 7. 与 Policy 的关系

Audit 只记录：

Policy decision result。

不要让：

AuditEvent

重新调用：

PolicyEngine。

禁止：

Pipeline:

PolicyEngine
↓
Audit
↓
PolicyEngine again


单次请求：

一次 policy evaluation。

Audit 使用结果。

---

# 8. 与 WorkspaceGuard 的关系

Audit 可以记录：

workspace validation:

passed / denied

但是：

不要记录完整 canonical path。

例如：

允许：

workspace_id = "active"

不允许：

D:\Users\xxx\secret-project\file.txt


---

# 9. Event ordering

必须保证：

同一个 request_id：

event 顺序稳定。

例如：

正确：

Received
WorkspaceValidated
PolicyEvaluated
ExecutionStarted
Completed


DENY：

Received
WorkspaceValidated
PolicyEvaluated
Denied


ASK：

Received
WorkspaceValidated
PolicyEvaluated
ApprovalRequired


不要出现：

Completed
之后又 Received

---

# 10. Tests

新增测试：

## Event generation

ALLOW:

产生：

Received
WorkspaceValidated
PolicyEvaluated
ExecutionStarted
ExecutionCompleted


DENY:

产生：

Received
WorkspaceValidated
PolicyEvaluated
RequestDenied


ASK:

产生：

Received
WorkspaceValidated
PolicyEvaluated
ApprovalRequired


---

## Ordering

同一 request_id：

event sequence 正确。


---

## Redaction

测试：

输入包含：

secret
api_key
credential

AuditEvent 不包含原文。


---

## Regression

保持：

cargo test --workspace

M6-B tests

M5 MCP tests

全部通过。

---

# 11. Documentation

新增：

docs/AUDIT.md

说明：

- Audit Event purpose
- Event types
- Data classification
- Redaction boundary
- Future storage direction

不要写：

已经完成 storage。

---

# 12. 验证

执行：

cargo fmt --all -- --check

cargo check --workspace

cargo test --workspace

git diff --check


---

# 13. 完成汇报

输出：

## Implementation

新增/修改文件。

## Audit Model

说明：

event schema

## Pipeline Integration

说明：

哪些 lifecycle 节点产生事件。


## Security Boundary

说明：

哪些数据明确不记录。


## M6 boundary

明确：

Audit Event 完成。

Storage/API/UI 未完成。


## Validation

fmt:
check:
test:
diff:

## Blockers

None 或具体问题。

不要提交 commit。