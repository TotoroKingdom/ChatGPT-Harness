# Task：M6-A — Policy Engine

开始正式实现 Roadmap M6 的第一个子阶段：

M6-A — Policy Engine

本任务只建立 Runtime 内部真正可用、可测试、确定性的 Policy Engine。

不要一次完成整个 M6。
不要实现 Request State Machine。
不要实现 Audit Event。
不要实现 Approval Manager / Approval UI。
不要进入 M7。
不要修改 tunnel-client。
不要新增 bash。
不要修改 read_file / glob 的既有语义。

---

# 0. 当前基线

当前已经完成并提交：

- M0 UI ↔ Runtime Contract
- M1 UI Skeleton
- M2 Config / Lifecycle UI
- M3 Rust Runtime Skeleton
- M4 Workspace Security Boundary
- M5 Read-Only MCP Runtime
- M5 MCP Component State UI Corrective

当前 Normal Mode：

read_file
glob

MCP endpoint 已真实运行于 Runtime 内。

Tunnel integration 仍属于 M7。

---

# 1. 开始前必须读取

先检查当前仓库 HEAD、Git 状态以及以下文件：

AGENTS.md

docs-ai/PRODUCT.md
docs-ai/ARCHITECTURE.md
docs-ai/Roadmap.md

docs/IPC.md
docs/CONFIG.md

protocol/src/v1/common.rs
protocol/src/v1/policies.rs
protocol/src/v1/tools.rs
protocol/src/v1/config.rs
protocol/src/v1/runtime.rs

runtime/src/server.rs
runtime/src/state.rs
runtime/src/tools/**
runtime/src/mcp/**
runtime/src/workspace/**
runtime/src/events.rs

以及 M5 最终实现和测试。

先理解当前真正的 Tool execution path，
不要凭空建立第二条执行链。

---

# 2. 最高约束

优先级：

PRODUCT.md
> ARCHITECTURE.md
> Roadmap.md
> M0 Contract
> 当前 M4/M5 安全边界
> 本任务

禁止重新设计已经冻结的 V1 架构。

尤其不要修改现有 canonical：

PolicyDecision

当前 protocol 已有：

Deny
Ask
Allow

必须直接复用。

不要定义：

RuntimePolicyDecision
InternalDecision
PolicyAction

等表达同一领域概念的第二套 enum，
除非只是纯内部、明显不同语义且有充分必要性。

---

# 3. M6-A 目标

建立一个 Runtime-owned Policy Engine：

Tool Request / Policy Context
↓
Policy Engine
↓
matched rules
↓
effective PolicyDecision

决策优先级必须是：

DENY > ASK > ALLOW

即：

只要存在任一适用 DENY
→ effective = DENY

否则存在任一适用 ASK
→ effective = ASK

否则存在适用 ALLOW
→ effective = ALLOW

如果：

没有任何适用 policy
或 policy engine 无法确定安全决策

必须：

fail closed

即：

effective = DENY

不要默认 ALLOW。

---

# 4. 本阶段只做 Engine，不改变现有 Tool execution

这是重要边界。

M6-A 先实现：

Policy model adaptation
Policy matching
Policy precedence
default/system rules
effective decision
tests

本阶段原则上：

不要把 Policy Engine 半接入 MCP executor。

原因：

完整强制执行链将在 M6-B 统一接入。

不能形成：

某些入口经过 Policy
某些入口仍绕过 Policy

这种危险的半完成状态。

因此 M6-A 完成后：

read_file / glob 当前真实执行行为不应发生变化。

但是 Policy Engine 必须已经是 production-quality Rust module，
M6-B 可以直接接入。

如果你认为必须做极小 wiring 才能避免未来结构返工，
可以做纯依赖注入/初始化，
但不得宣称 Policy 已 enforcement。

---

# 5. 建议模块边界

优先按当前 Runtime 结构新增类似：

runtime/src/policy/
mod.rs
engine.rs

如果当前代码结构证明其他命名更合适，可以调整，
但必须保持 Policy Engine 独立于：

MCP transport
Tool executor
WorkspaceGuard implementation
Tauri UI

依赖方向应类似：

MCP / future request pipeline
↓
PolicyEngine
↓
PolicyRule / PolicyDecision

不能：

PolicyEngine
↓
MCP HTTP implementation

Policy Engine 不得依赖 transport。

---

# 6. Policy evaluation input

不要直接拿任意 serde_json::Value 做核心 policy 判断。

定义一个最小、明确的内部 evaluation context。

至少应能表达：

- tool name
- security mode
- logical scope / requested capability class
- workspace relationship（如确实需要）

但不要在 M6-A 发明过度复杂 DSL。

当前只有：

read_file
glob

所以保持最小。

建议类似概念：

PolicyContext {
tool_name
security_mode
scope
}

具体 Rust 类型由现有代码决定。

不要把 raw secret、完整 command、任意敏感 arguments
放进 PolicyContext。

后面 M8 bash 再扩展高风险 context。

---

# 7. Scope 处理

当前 M0 `PolicyRule.scope` 是 String。

不要为了 M6-A 擅自修改 protocol 把它改成 enum。

在 Runtime 内部可以对已知 V1 scope 做明确解析/匹配，
但必须：

- deterministic
- unknown scope fail closed
- malformed scope fail closed
- 不因为未知 scope 自动忽略后继续 ALLOW

至少检查当前已有 mock / product / architecture 中使用的 scope：

active_workspace
outside_active_workspace
developer_test_capabilities

以及仓库当前真实定义。

不要凭感觉创造大量新 scope。

如果最终发现当前 M0 scope contract 客观不足，
停止并报告，不要直接扩大协议。

---

# 8. System policy baseline

根据当前 V1 设计建立最小 system policy set。

Normal Mode 的核心事实必须保持：

read_file
glob

属于：

active_workspace
read-only

有效决策应为：

ALLOW

而 workspace 外访问：

不得因为 Policy Engine 的存在而削弱 M4 WorkspaceGuard。

WorkspaceGuard 仍是独立硬安全边界。

如果 policy 层表达：

outside_active_workspace → DENY

可以保留作为 defense-in-depth / effective policy 表达，

但绝不能把：

WorkspaceGuard DENY

改成依赖 Policy Engine 才生效。

即：

WorkspaceGuard 是 hard security boundary
Policy 是 execution authorization layer

不要合并两者。

---

# 9. ASK 语义

M6-A 必须正确支持：

ASK

但本阶段：

ASK 只是 Policy Engine 的有效决策结果。

不要实现：

- Approval Manager
- pending approval storage
- Allow Once
- Deny button
- expiry
- approval UI

这些属于后续 M6 状态机准备和 M8 真正批准流程。

因此测试中可以验证：

rules = [ALLOW, ASK]
→ ASK

rules = [ALLOW, ASK, DENY]
→ DENY

但不要为了测试 ASK 去新增 bash。

---

# 10. Rule applicability

Engine 必须明确区分：

enabled = false
→ 不参与匹配

enabled = true
→ 才可能参与

必须测试：

disabled DENY
不能覆盖 enabled ALLOW

disabled ASK
不能覆盖 enabled ALLOW

同时保持 source：

System
User

当前优先级核心仍然是：

decision severity

而不是：

User > System
或
System > User

除非 PRODUCT/ARCHITECTURE 已明确规定其他规则。

不要自行创造 source precedence。

---

# 11. Determinism

同一：

PolicyContext
+
同一有效 rule set

无论 rule 输入顺序如何：

effective decision 必须一致。

必须增加 permutation / reordered rule 测试，例如：

[ALLOW, ASK, DENY]
[DENY, ALLOW, ASK]
[ASK, DENY, ALLOW]

全部：

DENY

Policy Engine 不能依赖“第一条命中”。

---

# 12. Evaluation result

不要只返回裸 PolicyDecision，如果后续 M6-B/C 明显需要知道命中的依据。

建议返回一个小型内部结果，例如概念：

PolicyEvaluation {
decision
matched_rule_ids
reason / decisive_rule
}

但：

- 不修改 M0 wire model
- 不把内部结果直接暴露给 UI
- 不加入敏感 arguments
- matched rule 顺序必须 deterministic

具体设计以最小够用为准。

M6-C Audit 后续应可以复用这个结果，
不要到时重新计算 Policy。

---

# 13. Fail closed

必须显式测试以下情况：

no rules
→ DENY

only disabled rules
→ DENY

unknown scope
→ DENY

unsupported tool / unknown capability
→ DENY

invalid policy context
→ DENY 或构造阶段拒绝

engine internal invalid state
→ 不得回退 ALLOW

安全原则：

任何无法证明允许的请求
都不能被解释为允许。

---

# 14. Tool Registry 边界

当前 Tool Registry 已存在。

M6-A 不要让 Policy Engine 自己维护第二份 Tool Catalog。

应保持：

ToolRegistry
→ 工具是否 implemented / exposed / enabled

PolicyEngine
→ 请求授权决策

两者职责不同。

后续 M6-D 才计算：

effective Tool Catalog

不要现在重写 Tools 页面或 tools.list。

---

# 15. 不要做这些

本任务禁止：

- 修改 MCP HTTP transport
- 修改 read_file implementation
- 修改 glob implementation
- 修改 WorkspaceGuard / PathResolver
- 增加 shell/bash
- 增加 write/edit
- 增加 database
- 实现 Approval Manager
- 实现 Request State Machine
- 实现 Audit pipeline
- 修改 UI
- 修改 tunnel-client
- 实现 M7
- 升级依赖
- 重构整个 runtime
- 修改 M0 contract（除非发现客观 blocker，先停止报告）
- git commit

---

# 16. 测试要求

Policy Engine unit tests 至少覆盖：

1.
ALLOW
→ ALLOW

2.
ASK
→ ASK

3.
DENY
→ DENY

4.
ALLOW + ASK
→ ASK

5.
ALLOW + DENY
→ DENY

6.
ASK + DENY
→ DENY

7.
ALLOW + ASK + DENY
→ DENY

8.
rule order changes
→ same effective result

9.
disabled DENY + enabled ALLOW
→ ALLOW

10.
disabled ASK + enabled ALLOW
→ ALLOW

11.
no applicable rules
→ DENY

12.
unknown tool
→ DENY

13.
unknown / unsupported scope
→ DENY

14.
Normal Mode read_file in active workspace
→ ALLOW

15.
Normal Mode glob in active workspace
→ ALLOW

16.
outside workspace policy context
→ DENY

注意：

这里的 outside workspace policy test
不能替代现有 WorkspaceGuard security tests。

原有 M4/M5 workspace escape tests 必须继续全部通过。

---

# 17. Regression

必须确保：

M5 的真实 MCP tests 全部继续通过：

tools/list

read_file
glob

MCP lifecycle
WorkspaceGuard
HTTP transport security
shutdown

因为 M6-A 不应改变现有 execution semantics。

---

# 18. 验证

执行：

cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

如果仓库当前已有其他 Rust lint/check，
按现有规范执行。

最后：

git diff --check

本任务原则上不需要 UI 修改，
所以如果完全没有碰 UI，
不需要为了形式无意义跑长期 Tauri 人工启动。

但如果任何共享 model / frontend generated artifact 被意外影响，
必须运行对应 UI：

typecheck
test
build

---

# 19. 完成标准

M6-A 完成时必须能够明确证明：

Policy Engine exists
PolicyDecision uses canonical M0 type
DENY > ASK > ALLOW
order independent
disabled rules ignored
unknown/no-match fail closed
Normal read_file/glob evaluate ALLOW
outside/unknown evaluate DENY
M5 execution behavior unchanged
all existing tests pass

---

# 20. 最终报告

最后只汇报：

## Implementation

新增/修改文件。

## Policy semantics

简述：

DENY > ASK > ALLOW
fail closed
scope matching
disabled rule behavior

## M6 boundary

明确说明：

Policy Engine 已完成，
但尚未接入统一 Tool execution pipeline。

M6-B 将负责 enforcement / Request State Machine wiring。

不要声称：

M6 complete

## Validation

cargo fmt:
cargo check:
cargo test:
git diff --check:

列出测试数量。

## Blockers

如无：

None.

不要提交 commit。