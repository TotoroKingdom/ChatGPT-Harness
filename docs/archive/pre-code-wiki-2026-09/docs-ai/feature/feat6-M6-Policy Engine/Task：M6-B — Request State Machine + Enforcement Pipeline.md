# Task：M6-B — Request State Machine + Enforcement Pipeline

开始实现 Roadmap M6-B。

前置：

M6-A Policy Engine 已完成：

- PolicyDecision 使用 canonical M0 type
- DENY > ASK > ALLOW
- fail closed
- PolicyEngine 已独立存在
- 尚未接入 Tool execution

本任务目标：

建立 Runtime 内部统一 Tool Request 生命周期和 Enforcement Pipeline。

---

# 非目标

本任务不要：

- 实现 Audit Event 完整系统（M6-C）
- 实现 Approval UI
- 实现 Allow Once
- 实现 Approval Manager
- 实现 bash
- 实现 write/edit tools
- 实现 tunnel-client
- 实现 M7
- 修改 MCP protocol
- 修改 WorkspaceGuard 安全边界
- 修改 UI

ASK 只实现状态表达，不实现人工批准流程。

---

# 核心目标

当前未来所有 Tool execution 必须经过：

MCP Request
|
v
Request Context
|
v
Validation
|
v
Policy Evaluation
|
+------ DENY
|
+------ ASK
|
+------ ALLOW
|
v
WorkspaceGuard
|
v
Tool Executor
|
v
Tool Result


禁止：

MCP handler
|
v
Executor

这种绕过路径。

---

# 1. 开始前阅读

必须先检查：

docs-ai/PRODUCT.md
docs-ai/ARCHITECTURE.md
docs-ai/Roadmap.md
docs/POLICY.md

protocol/src/v1/

runtime/src/server.rs
runtime/src/state.rs
runtime/src/tools/**
runtime/src/mcp/**
runtime/src/workspace/**
runtime/src/policy/**

理解当前真实调用链。

不要假设架构。

---

# 2. 设计要求

建立 Runtime 内部 execution pipeline。

推荐结构：

runtime/src/execution/

例如：

execution/
mod.rs
pipeline.rs
context.rs
state.rs

具体命名可调整。

职责：

## RequestContext

表达一次 Tool 请求。

至少包含：

- request id
- tool name
- security context
- policy context

不要包含：

- API Key
- raw secrets
- 无限制 arguments dump

Arguments 是否需要进入 context，
必须谨慎。

如果未来 policy 需要判断参数，
设计可扩展接口。

但当前不要复制完整 MCP payload。

---

## Request State Machine

建立明确状态。

至少支持：

Received

↓

Validating

↓

PolicyEvaluating

↓

Denied

↓

AwaitingApproval

↓

Executing

↓

Completed

↓

Failed


如果当前 protocol 已有类似状态，
优先复用。

不要定义第二套 wire protocol state。

---

# 3. 状态转换必须受约束

禁止：

Completed → Executing

Denied → Completed

Failed → Executing

等非法跳转。

建议：

显式 transition function。

例如：

transition(current, event)
-> next

非法 transition 返回 error。

不要直接：

state = new_state

---

# 4. Policy Enforcement

Pipeline 中必须调用 M6-A PolicyEngine。

规则：

PolicyDecision::Deny

必须：

state = Denied

不能调用 executor。


PolicyDecision::Ask

必须：

state = AwaitingApproval

不能调用 executor。

本阶段：

AwaitingApproval 不自动继续。

不要实现：

auto approve


PolicyDecision::Allow

继续：

WorkspaceGuard
|
Executor


---

# 5. WorkspaceGuard 顺序

必须保持：

Policy
↓
WorkspaceGuard
↓
Executor


不要：

WorkspaceGuard
↓
Policy


原因：

Policy 是 authorization。

WorkspaceGuard 是 filesystem hard boundary。


必须保证：

Policy ALLOW
+
WorkspaceGuard DENY

最终：

DENY / Failed

不能执行。

---

# 6. Tool Executor 抽象

当前已有：

read_file
glob

不要重写。

目标：

以后新增：

bash
sql
redis
es

都必须接入同一个 pipeline。

因此建议抽象：

ToolExecutor trait/interface

类似：

execute(context)
-> ToolResult


但是：

不要为了抽象而大规模重构。

如果当前 read_file/glob 结构不适合，
先建立 adapter。

保持 M5 行为不变。

---

# 7. MCP Integration

本任务需要完成：

MCP tool call

进入 pipeline。


但是：

不要改变 MCP wire contract。

不要新增 MCP method。

不要修改 tools/list schema。

外部仍然看到：

read_file
glob


内部变为：

MCP request
↓
pipeline
↓
executor


---

# 8. ASK 语义边界

实现：

ASK
↓
AwaitingApproval


然后停止。


不要：

- sleep 等待
- CLI input
- UI popup
- approve command


M8 才实现真正 approval flow。

---

# 9. Error Handling

必须区分：

Policy Deny

和：

Execution Failed


例如：

DENY:

{
reason:
"blocked by policy"
}


Execution error:

{
reason:
"tool execution failed"
}


不要全部变成 generic error。

---

# 10. Pipeline Hooks

虽然 Audit 是 M6-C，

M6-B 必须预留 hook。

例如：

on_request_received

on_policy_decision

on_execution_started

on_completed


可以：

- trait
- callback
- internal event channel

任选。

但不要实现完整 Audit storage。

目标：

M6-C 不需要重新改 pipeline。

---

# 11. 测试要求

必须新增：

## State Machine tests

覆盖：

合法：

Received
→ Validating

Validating
→ PolicyEvaluating

PolicyEvaluating
→ Denied

PolicyEvaluating
→ AwaitingApproval

PolicyEvaluating
→ Executing

Executing
→ Completed

Executing
→ Failed


非法：

Completed
→ Executing

Denied
→ Executing

Failed
→ Completed


---

## Pipeline tests

至少：

1.

Policy DENY

结果：

Denied

executor 未调用。


2.

Policy ASK

结果：

AwaitingApproval

executor 未调用。


3.

Policy ALLOW

WorkspaceGuard allow

executor 调用。


4.

Policy ALLOW

WorkspaceGuard deny

executor 未调用。


5.

Executor error

state Failed。


6.

existing:

read_file

glob

行为保持一致。


---

# 12. Regression

必须保持：

cargo test --workspace

M5:

- MCP lifecycle
- tools/list
- read_file
- glob
- WorkspaceGuard

全部通过。

---

# 13. 注意

这是安全边界建设任务。

不要追求代码量。

优先：

正确边界

> 完整功能


如果发现当前架构无法无侵入接入：

停止。

报告：

- blocker
- 当前调用链
- 推荐调整

不要大规模重构。

---

# 14. 验证

执行：

cargo fmt --all -- --check

cargo check --workspace

cargo test --workspace

git diff --check


---

# 15. 完成汇报

输出：

## Implementation

新增/修改文件。

## Execution Pipeline

说明：

MCP request
如何进入 pipeline。


## State Machine

列出状态和非法 transition。


## Enforcement

说明：

DENY:
ASK:
ALLOW:

分别行为。


## M6 boundary

明确：

Audit 未完成（M6-C）
Approval 未完成（M8）

## Validation

fmt:
check:
test:
diff:

## Blockers

None 或具体问题。

不要提交 commit。