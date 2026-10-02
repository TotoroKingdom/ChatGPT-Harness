# M3 Integration Test Plan — Configurable Policy

> Status: ACCEPTED / FROZEN
> Milestone: M3 — Configurable Policy  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: Protocol / ConfigStore / Runtime policy compilation / ExecutionPipeline / Control read / UI data integration. This is not Local Black-box E2E and does not require External ChatGPT.

## 1. Objective

验证 M3 各开发 Task 组合后形成一致的 configurable policy system，而不是若干孤立 unit behavior。

Integration 重点验证：

```text
Persisted Desired Policy
→ ConfigStore / configRevision
→ RuntimeConfigSnapshot.policies
→ Runtime policy compile
→ immutable PolicyEngine snapshot
→ ExecutionPipeline
→ effective decision + matched rule IDs + reason
→ Control/UI effective observation
```

核心不变量：

- System Guardrails 由 Runtime owns，用户不可覆盖。
- Tool Default 仅在没有显式匹配 User Rule 时 fallback。
- 多个匹配 User Rules 按 `DENY > ASK > ALLOW` 聚合。
- 任一 enabled invalid rule 使整个 policy snapshot fail closed。
- Runtime 是 effective policy authority；UI 不自行重算。
- Policy config 只在下一 Runtime generation 生效。

## 2. Entry Criteria

执行本计划前必须满足：

- T1–T5 development complete。
- repository Unit/code-level tests PASS。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- frontend tests/typecheck/build PASS（按仓库实际命令）。
- `git diff --check` PASS。

若实现尚未完成：

```text
Integration = BLOCKED_BY_IMPLEMENTATION
```

不得通过降低断言获得 PASS。

## 3. Integration Ownership Boundary

Integration 可以：

- 使用真实 ConfigStore / temporary config.json。
- 真实 serialize/deserialize Protocol DTO。
- 实例化 RuntimeCore / ToolRegistry / PolicyEngine / ExecutionPipeline。
- 通过真实 Control read adapter读取 effective policy projection。
- 用 React/provider/component integration测试 Policies UI desired/effective组合。

Integration 不应：

- 用 mock 替代需要验证的 ConfigStore revision semantics。
- 用单个 `PolicyEngine::evaluate()` unit test替代跨 snapshot/pipeline evidence。
- 声称已验证真实 Runtime executable/loopback MCP；这些属于 E2E。
- 依赖 External ChatGPT、Tunnel、浏览器人工操作。

## 4. Required Integration Cases

### INT-01 — Legacy schema v1 without `policies` loads defaults

Fixture：M2-valid schema v1 JSON，没有 `policies`。

期望：

```text
read_file default = ALLOW
glob default      = ALLOW
write default     = ALLOW
edit default      = ALLOW
bash default      = ASK
userRules         = []
schemaVersion     = 1
```

M2 `tools` 和其它 config字段保持原值。

### INT-02 — Policy config persistence + semantic revision

使用真实 ConfigStore：

1. 保存 defaults。
2. `write` default ALLOW → DENY。
3. reload。
4. 再保存相同 config。
5. create 一个 user rule。
6. disable该 rule。
7. delete该 rule。

期望每个 semantic change正确 `configRevision + 1`，重复保存相同语义 revision不变。

### INT-03 — User Rule CRUD persistence

针对一条 ToolMatcher rule依次验证：

```text
create
edit name/reason
deision change
enable/disable
matcher tool change
delete
```

期望 save/reload 完整保持，且 persisted user rule不能伪造 `source=system` / `editable=false`。

### INT-04 — Malformed policy JSON fails closed

至少覆盖：

- invalid decision value。
- malformed matcher object。
- wrong field type。
- unknown matcher discriminator。

期望 ConfigStore load/save失败，不静默默认 ALLOW，不破坏已存在有效 config。

### INT-05 — RuntimeConfigSnapshot carries policy config

通过真实 Tauri snapshot construction：

- tool defaults精确复制。
- userRules精确复制。
- configRevision正确。
- host-only data/secrets不进入 Runtime snapshot。

### INT-06 — Runtime compiles immutable generation snapshot

Generation A：default policy + no rules。

运行中保存 Generation B desired：`write default DENY`。

期望 Runtime A仍以原 snapshot执行 ALLOW；active revision保持 A；latest revision变化后 `restartRequired=true`。

重建 Runtime B后才应用 DENY。

### INT-07 — Tool Default fallback

对无显式匹配 User Rule的当前 implemented tools分别设置：

```text
ALLOW
ASK
DENY
```

期望 evaluation来自 Tool Default，`matchedRuleIds=[]`，reason明确为 tool default。

### INT-08 — Explicit ALLOW overrides default ASK

设置：

```text
write default = ASK
user rule: tool=write, decision=ALLOW
```

期望：

```text
effective = ALLOW
matchedRuleIds = [user rule]
```

证明 Tool Default不参与 severity competition。

### INT-09 — Explicit ALLOW overrides default DENY when no system guardrail denies

设置：

```text
write default = DENY
matching user rule = ALLOW
```

期望 User Rule胜出为 ALLOW，前提是请求通过硬 System Guardrails。

该 case只证明 Tool Default是 fallback；不得用于声称 User Rule可以覆盖 System Guardrail。

### INT-10 — User Rule severity is DENY > ASK > ALLOW

同一个 tool创建三条匹配 rules，打乱持久化顺序。

期望：

```text
ALLOW only → ALLOW
ALLOW + ASK → ASK
ALLOW + ASK + DENY → DENY
```

规则 vector顺序不改变结果。

### INT-11 — Matched rule IDs deterministic

多条同 tool User Rules使用不同 IDs并改变输入顺序。

期望 `matchedRuleIds`按冻结的稳定顺序输出，不依赖 config数组顺序。

### INT-12 — System Guardrail cannot be overridden

使用真实 Workspace boundary / unsupported capability fixture，并配置 user ALLOW。

期望：

- Workspace 外请求仍被 WorkspaceGuard拒绝。
- unsupported capability仍 fail closed。
- User ALLOW不能产生 ExecutionPermit。

### INT-13 — Duplicate enabled rule ID invalidates whole snapshot

两个 enabled User Rules使用同一 ID，其中一个 ALLOW。

期望：

```text
policy snapshot invalid
effective decision = DENY
reason = invalid policy snapshot
no executor side effect
```

不得 last-write-wins。

### INT-14 — Enabled invalid matcher invalidates whole snapshot

至少构造：

- unknown tool matcher。
- unsupported matcher type。
- semantically invalid required field。

同时存在另一条 valid ALLOW rule。

期望整个 snapshot fail closed，不跳过坏 rule寻找 ALLOW。

### INT-15 — Disabled invalid semantic rule does not participate

若 persisted DTO本身可合法反序列化，但 rule disabled且 matcher semantic无法编译，验证 frozen SPEC所允许行为：disabled rule不影响 active compiled set。

同时证明重新 enable后 snapshot立即变 invalid（在下一 generation compile时）。

### INT-16 — DENY execution path has evidence and no side effect

对 `write` 配置 matching DENY User Rule。

经过真实 ExecutionPipeline调用。

期望：

- `POLICY_DENIED`。
- filesystem unchanged。
- policy evidence包含 decision=DENY、matched IDs、reason。
- 不出现 ExecutionStarted/Completed success。

### INT-17 — ASK execution path is M3 fail-closed transition

对 `write` 配置 matching ASK rule。

期望：

- policy evaluation = ASK。
- matched IDs/reason存在。
- pipeline进入 AwaitingApproval / emit ApprovalRequired evidence。
- 返回 `APPROVAL_REQUIRED`。
- filesystem unchanged。
- M3不创建真正 pending approval/resume flow。

### INT-18 — ALLOW execution path remains functional

对 read/write/edit分别使用 Tool Default或User ALLOW。

期望通过 existing WorkspaceGuard / ExecutionPermit正常执行，M1 atomic semantics不受影响。

### INT-19 — Disabled Tool still fails before normal policy execution

M2 tool config中 `write OFF`，同时设置 `write User ALLOW`。

期望：

- tool仍 `TOOL_DISABLED`。
- 不因 Policy ALLOW重新启用 capability。
- filesystem unchanged。

### INT-20 — Effective tool/policy projection is Runtime-owned

Control read/effective projection至少验证：

- per-tool effective decision。
- reason。
- policy snapshot validity。
- matched IDs或可追踪 evidence（按最终 DTO）。
- System Rules read-only projection。

UI consumption不应自行重跑 PolicyEngine。

### INT-21 — Policies UI desired/effective mismatch

Fixture：

```text
Runtime A: write default ALLOW, revision N
Persisted desired: write default DENY, revision N+1
```

期望 UI：

```text
Desired = DENY
Effective = ALLOW
Restart Required = visible
```

refresh Runtime observation不得覆盖 Desired。

### INT-22 — Policies UI restart convergence

切换 effective observation为 Runtime B revision N+1 / DENY。

期望 desired/effective收敛，Restart Required消失。

### INT-23 — System Rules cannot be edited in UI

期望 System rows：

- no edit action。
- no enable toggle。
- no delete action。
- source/read-only状态明确。

User rows具备 CRUD controls。

### INT-24 — UI save failure does not fake persisted policy

模拟 control save failure。

期望：

- 明确 error。
- desired state rollback或re-fetch真实 persisted config。
- 不伪造 revision increment/restart-required。

### INT-25 — M0–M2 regression

默认 Policy config下验证：

- M2 Tool Toggle ON/OFF / stale-call semantics不变。
- read/glob normal + Workspace boundary。
- M1 write/edit atomic normal path + boundary。
- Activity/Audit confidentiality。

## 5. Evidence Format

每个 case记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED
Test command / test name
Config revision + relevant policy config
Effective decision
Matched rule IDs
Decision reason
Execution/error result
Filesystem/Activity assertion when applicable
```

不得记录 secrets/raw mutation content。

## 6. Integration Exit Criteria

M3 Integration只有以下全部满足才为 PASS：

- INT-01 ～ INT-25 PASS。
- policy config persistence/CRUD/revision PASS。
- immutable snapshot/restart convergence PASS。
- Tool Default fallback语义 PASS。
- explicit User Rule severity PASS。
- System Guardrail不可覆盖 PASS。
- invalid enabled rule whole-snapshot fail-closed PASS。
- matched IDs + reason deterministic evidence PASS。
- ASK no-execution transition PASS。
- Runtime effective projection + UI desired/effective/restart PASS。
- M0–M2 regression PASS。

随后已完成 `E2E_TEST_PLAN.md` 的真实本地黑盒验证，并完成 Manual Acceptance。

## 7. Final Result / Evidence Retention

```text
Integration result: PASS
Cases: INT-01–INT-25 all PASS (`m3_config_integration_cases` + `m3_runtime_integration_cases` evidence)
Evidence: retained from the existing M3 Integration run; no Integration Test was re-executed during final closeout.
Implementation commit: b49c7a8031edcfe6fdf39acd8b7fd43be34bb08b
Test/evidence commit: 67b2fb81d5d1f3351d713512358493b0b2f62a85
Manual Acceptance: PASS (confirmed by Human Reviewer on 2026-10-02)
```

The existing Integration PASS evidence remains the source for this closeout. The observed `read_file=ASK` behavior difference from Manual Acceptance is recorded as a non-blocking observation; it does not change the M3 result and is not fixed in this closeout.

## 8. Non-Claims

Integration PASS不代表：

```text
M4 Approval lifecycle implemented
ASK request can be approved/resumed
Bash command-specific policy implemented
schemaVersion v1→v2 migration complete
External ChatGPT acceptance complete
M4 Approval lifecycle started or implemented
```

The Integration PASS result did not by itself establish Manual Acceptance; the subsequent human gate passed and the milestone is now recorded as `M3 — ACCEPTED / FROZEN`.
