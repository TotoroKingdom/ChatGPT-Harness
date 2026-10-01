# M3 SPEC — Configurable Policy

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M3 — Configurable Policy  
> Baseline: M2 development complete  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, M0–M2 milestone docs

## 1. Objective

M3 将当前 Runtime 内部的只读 Policy 基础设施升级为用户可持久化配置、Runtime 可确定编译和执行、UI 可编辑并观察真实 effective decision 的 Policy 系统。

正式决策链：

```text
Capability Gate
      ↓
System Guardrails
      ↓
Explicit User Rules
      ↓
Tool Default (only when no explicit User Rule matches)
      ↓
Effective Decision: ALLOW / ASK / DENY
```

User Rule 聚合优先级：

```text
DENY > ASK > ALLOW
```

M3 保持 M2 的 immutable Runtime generation 模型；persistent policy config 不 hot reload。

## 2. Baseline Facts

当前代码已经具备：

- `PolicyDecision = ALLOW / ASK / DENY`。
- `PolicyRule` 基础 DTO，包含 source/editable/enabled 等字段。
- Runtime-owned `PolicyEngine` immutable rules snapshot。
- enabled invalid rule → `InvalidRuleSet` → DENY 的 fail-closed 基础行为。
- matched rule IDs evidence。
- `DENY > ASK > ALLOW` severity helper。
- fixed System Rules。
- ExecutionPipeline 在 PolicyEvaluation 后执行：DENY → reject；ASK → `ApprovalRequired`；ALLOW → executor。
- EffectiveToolCatalog 已能输出 decision/reason，但 ASK 当前仍属于 unavailable-by-policy 过渡状态。
- Policies UI 当前只是 Runtime rules 的只读展示。
- `AppConfig` / `RuntimeConfigSnapshot` 已在 M2 引入 immutable `tools`，但还没有 `policies`。

M3 应复用这些基础设施，不重新设计 WorkspaceGuard、ExecutionPermit 或 MCP adapter authorization。

## 3. Scope

M3 实现：

1. 持久化 Tool Default Policy。
2. 持久化 User Rules。
3. typed Policy matcher。
4. User Rule CRUD：创建、编辑、启用、禁用、删除。
5. Runtime 启动时验证并编译 immutable Policy snapshot。
6. System Guardrails 与 User Policy 分层，System Guardrails不可被覆盖。
7. 明确的 Tool Default fallback。
8. explicit User Rules 按 `DENY > ASK > ALLOW` 聚合。
9. 任一 enabled invalid User Rule使整个 Policy snapshot fail closed。
10. Runtime 输出 effective decision、matched rule IDs、decision reason。
11. Policy evidence进入 Runtime effective/control/activity/audit路径。
12. Policies UI 区分 desired config 与 current Runtime effective policy，并显示 Restart Required。
13. M0–M2 regression。

M3 不实现：

- pending Approval lifecycle。
- Allow once / Allow for session / Deny approval actions。
- ASK request async wait/resume。
- Bash executor。
- Bash command parser / command-specific matcher execution。
- arbitrary Boolean-expression policy language。
- persistent Policy hot reload。
- same-generation policy apply。
- full schemaVersion v1→v2 migration pipeline（M7）。

## 4. Config Contract

### 4.1 Transitional persisted model

继续沿用 M2 的 additive compatibility strategy：

```text
schemaVersion = 1
```

`AppConfig` 增加 backward-compatible `policies`：

```text
AppConfig
└ policies
   ├ toolDefaults
   │  ├ readFile = allow
   │  ├ glob     = allow
   │  ├ write    = allow
   │  ├ edit     = allow
   │  └ bash     = ask
   └ userRules[]
```

推荐强类型：

```text
PolicyConfig
ToolPolicyDefaults
UserPolicyRuleConfig
PolicyMatcherConfig
```

旧 schema v1 config 缺失 `policies` 时必须安全默认：

```text
read_file → ALLOW
glob      → ALLOW
write     → ALLOW
edit      → ALLOW
bash      → ASK
userRules → []
```

M3 不把 `CURRENT_SCHEMA_VERSION` 升为 2；正式 V1→V2 migration仍由 M7负责。

### 4.2 Persisted User Rule must not own System identity

持久配置中的 User Rule 不应允许调用方伪造：

```text
source = system
editable = false
```

因此建议 persisted `UserPolicyRuleConfig` 只保存用户可配置字段：

```text
id
name
enabled
decision
matcher
reason
```

Runtime/control projection再明确补充：

```text
source = user
editable = true
```

System Rules只由 Runtime代码创建，不进入 AppConfig `userRules`。

### 4.3 Config revision

任一 semantic policy change必须增加 `configRevision`：

- tool default变化。
- create/edit/delete rule。
- enabled toggle。
- matcher/decision/reason/name semantic change。

重复保存相同 semantic policy config：revision不变。

## 5. Tool Default Contract

每个 built-in tool有 fallback decision：

| Tool | Default |
| --- | --- |
| `read_file` | ALLOW |
| `glob` | ALLOW |
| `write` | ALLOW |
| `edit` | ALLOW |
| `bash` | ASK |

Tool Default 的关键语义：

> **只有当没有匹配的显式 enabled User Rule时才使用。**

Tool Default 不是一个始终参与 severity competition 的固定 rule。

例如：

```text
bash default = ASK
matching user rule = ALLOW
→ effective = ALLOW
```

不得得到 ASK。

M3 可以持久化/编辑 bash default，但不得因此注册或实现 bash executor。

## 6. Typed Matcher Contract

V3 Policy 不再以自由字符串 `scope` 作为 User Rule authoritative matcher。

M3 正式实现当前文件工具所需的：

```text
PolicyMatcher
└ ToolMatcher
   └ tool
```

建议 JSON/Rust tagged shape：

```json
{
  "type": "tool",
  "tool": "write"
}
```

或等价强类型 tagged enum。

M3 matcher目标只包括当前 Registry identity：

```text
read_file
glob
write
edit
```

`bash` 的 command-specific matcher语义属于 M5。

协议可以为未来 matcher extension保留 enum结构，但 M3 不需要实现 `BashCommandMatcher` execution/compilation。

### 6.1 Registry-derived identity

Policy matcher validation不能维护另一份 hardcoded tool-name authority。

Runtime compile时必须以 ToolRegistry built-in definitions验证 matcher tool identity。

允许 config中保留 bash Tool Default，因为它是已知未来 built-in config field；但 M3 User Rule 不应让未实现 bash masquerade as executable capability。

## 7. System Guardrails Contract

System Guardrails 由 Runtime代码 + fixed system rules拥有。

用户不能：

- 创建 system rule。
- 编辑 system rule。
- disable system rule。
- delete system rule。
- 使用 User Rule覆盖 System DENY。

System Guardrails至少继续包括：

- WorkspaceGuard path enforcement。
- invalid policy config fail closed。
- unsupported capability fail closed。
- Runtime readiness。
- stale disabled-tool rejection。
- future Bash mode guard（M5执行能力未实现）。

WorkspaceGuard / CapabilityGate是硬 enforcement，不因为 Policy UI/config存在而下沉成普通 user rule。

## 8. Policy Compilation Contract

Runtime generation初始化时：

```text
RuntimeConfigSnapshot.policies
      ↓
validate entire enabled User Rule set
      ↓
compile typed matchers
      ↓
compose fixed System Guardrails
      ↓
immutable PolicyEngine snapshot
```

### 8.1 Enabled invalid rule invalidates snapshot

任一 enabled rule存在以下问题时，整个 policy snapshot必须 fail closed：

- empty/invalid ID。
- duplicate ID。
- invalid/unknown matcher tool identity。
- malformed typed matcher。
- unsupported matcher type。
- invalid decision/config shape。
- any semantic condition Runtime无法可靠编译。

不得：

```text
skip bad rule
→ continue searching another ALLOW
```

Disabled invalid rule可不参与编译/evaluation，但其持久结构仍必须能被 typed config解析；真正 malformed JSON仍由 ConfigStore fail closed。

### 8.2 Fail-closed operational state

M3不要求因 semantic invalid User Rule直接让整个 Runtime process崩溃。

推荐并冻结语义：

```text
Policy snapshot = Invalid
→ Runtime control/diagnostics仍可观察
→ policy-controlled tools effective decision = DENY
→ tools/call rejected
→ reason = invalid policy snapshot
→ no executor side effect
```

Tauri/UI可以在保存前做相同 validation提升 UX，但不能替代 Runtime authority。

## 9. Policy Evaluation Algorithm

对一个通过 CapabilityGate/WorkspaceGuard的请求：

### Step 1 — System Guardrails

若任何 applicable System Guardrail产生 DENY：

```text
DENY
```

User Rule / Tool Default不得降低该结果。

### Step 2 — Explicit User Rules

筛选 enabled + matcher匹配当前 tool的 User Rules。

如果至少一个匹配：

```text
DENY > ASK > ALLOW
```

`matchedRuleIds` 必须包含所有 applicable matched User Rule IDs，使用确定顺序。

### Step 3 — Tool Default fallback

只有 explicit matched User Rules为空时：

```text
use configured tool default
```

Tool Default必须有明确 reason，但不伪造 user rule ID。

## 10. Effective Decision / Evidence Contract

每次有效 policy evaluation至少输出 typed evidence：

```text
PolicyEvaluation
├ effectiveDecision
├ matchedRuleIds
├ reason
└ source/fallback kind as needed
```

`reason` 至少可区分：

```text
system_guardrail
matched_user_rules
tool_default
invalid_policy_snapshot
invalid_context
unsupported_capability
```

不要只保存日志字符串。

### 10.1 Determinism

- matched IDs按稳定顺序输出。
- User Rule vector顺序不能改变 final decision。
- duplicate IDs使 snapshot invalid，而不是 last-write-wins。

## 11. ASK Boundary Before M4

M3必须支持：

```text
effective decision = ASK
matched evidence = complete
```

但 M3 **不实现真正 Approval lifecycle**。

当前 ExecutionPipeline的过渡语义可继续：

```text
ASK
→ AwaitingApproval state/evidence
→ return APPROVAL_REQUIRED
→ no executor execution
```

M3不创建 pending ApprovalManager request，不 async wait，不 resume原 request。

Architecture中的最终：

```text
ASK → exposed + execution requires Approval
```

在 M4 完成 Approval lifecycle时收口。

因此 M3 不应为了“完整 ASK UX”提前重构 async pipeline。

## 12. Runtime Snapshot Contract

`RuntimeConfigSnapshot` 增加：

```text
policies
```

只包含 non-secret：

- tool defaults。
- user rules。

Runtime generation启动后 immutable。

运行中保存 policy：

```text
Desired Policy changes
→ configRevision changes
→ current Runtime PolicyEngine unchanged
→ Restart Required
→ next generation compiles new snapshot
```

禁止 Runtime监听 config文件或通过 Tauri live mutate rules。

## 13. MCP Exposure Contract for M3

M3保持 M4 前的过渡边界：

- `ALLOW`：按当前 effective exposure正常可用。
- `DENY`：不暴露 / direct stale call仍由 Runtime重新评估并拒绝。
- `ASK`：Policy effective state必须真实显示 ASK；execution调用必须 fail closed为 `APPROVAL_REQUIRED`，不执行。

如果当前 MCP list层在 M3仍隐藏 ASK tool，可以保留该过渡行为；M4负责将 ASK升级为真正可审批、可恢复的 exposed capability。

关键安全要求：direct `tools/call`永远重新经过 CapabilityGate + WorkspaceGuard + PolicyEngine，不能信任之前的 `tools/list`。

## 14. Policy Control Surface

M3需要 Runtime-owned effective policy observation。

建议新增 additive Control read，例如：

```text
policies.getEffective
```

或扩展现有安全 read model，至少返回：

- system rules（read-only）。
- current generation compiled user rules projection。
- configured tool defaults for current generation。
- policy snapshot validity/status。
- per-tool effective decision。
- matched rule IDs/reason where meaningful。

现有 `policies.list` / UI provider如果仍存在，可以演进复用；不得让 UI自行执行 PolicyEngine算法。

## 15. Policies UI Contract

Policies页面从只读 Runtime rules页升级为：

### Desired configuration

来自 AppConfig：

- Tool Defaults editor。
- User Rules list。
- Create/Edit/Delete。
- Enable/Disable。

### Effective Runtime observation

来自 current Runtime：

- System Guardrails（只读）。
- effective policy snapshot status。
- per-tool effective decision。
- matched-rule / reason evidence。

### Restart semantics

running状态保存 policy后：

```text
Desired changes immediately after successful save
Effective stays current generation
Restart Required = activeRevision != latestRevision
```

UI不得把 saved desired rule冒充为当前 Runtime正在执行的 rule。

restart后 effective状态收敛到 desired，Restart Required消失。

## 16. UI CRUD Safety

System Rules：

```text
read-only
no toggle
no edit
no delete
```

User Rules：

```text
create
edit
enable/disable
delete
```

保存前UI/Tauri应提供可理解 validation错误，例如：

- duplicate ID。
- empty required field。
- unknown tool。
- unsupported matcher。

但 Runtime仍必须对手工修改/旧版本/绕过UI的 invalid config fail closed。

M3不实现 M6 i18n；沿用当前英文文案。

## 17. Activity / Audit

Policy evaluation evidence至少应能够关联：

```text
request ID
tool
effective decision
matched rule IDs
decision reason
execution outcome
```

M3不要求完成 M7 的最终完整 Evidence schema，但不能继续让 matched IDs/reason只存在于内部临时变量。

不得记录：

- raw file content。
- secrets。
-无必要绝对路径。

## 18. Expected Code Touchpoints

```text
protocol/src/v1/config.rs
protocol/src/v1/policies.rs
protocol/src/v1/control_read.rs / methods as needed
protocol generated TS/schema artifacts

ui/src-tauri/src/config/mod.rs
ui/src-tauri/src/runtime/mod.rs
ui/src-tauri/src/commands/* / harness.rs as needed

runtime/src/policy/engine.rs
runtime/src/policy/tests.rs
runtime/src/state.rs
runtime/src/execution/pipeline.rs
runtime/src/tools/effective.rs
runtime/src/control_read.rs
runtime/src/audit/* / activity/* as needed
runtime/src/mcp/* only for policy error/exposure consistency

ui/src/pages/policies/PoliciesPage.tsx
ui/src/app/providers/*
ui/src/services/harness/*
related tests
```

Workspace mutation internals、Tunnel、M1 atomic file primitive不应因 M3改动。

## 19. Compatibility Requirements

M3默认 policy config下必须保持 M2/M1行为：

```text
read_file ALLOW
glob ALLOW
write ALLOW
edit ALLOW
bash ASK but unimplemented/disabled
```

并保持：

- Tool Toggle semantics不变。
- Runtime generation immutability不变。
- M1 atomic write/edit不变。
- WorkspaceGuard不变。
- ExecutionPermit pipeline-only。
- MCP schemas/annotations不因 Policy Editor改变。
- Tool disabled优先于普通 user policy execution。
- no protocol major bump。
- old schema v1 config missing policies loads defaults。

## 20. Development Verification Requirements

每个 Task必须包含 unit/code-level tests。

开发完成至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests / typecheck / build commands defined by repository
git diff --check
```

必须有自动化覆盖：

```text
legacy config missing policies → defaults
policy config revision semantics
user rule CRUD persistence
runtime policy snapshot immutability
tool default fallback
explicit ALLOW overrides default ASK when no stronger rule
DENY > ASK > ALLOW among matched user rules
system DENY cannot be overridden
invalid enabled rule invalidates snapshot
invalid snapshot denies execution
matched rule IDs deterministic
effective reason output
ASK returns approval-required without execution
running desired/effective mismatch + restart required
restart convergence
M2 tool-toggle regression
M1 file-operation regression
```

## 21. Post-TASK Verification Documents

`SPEC / PLAN / TASKS` 冻结后独立生成：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；只有 Manual Acceptance需要人工介入。

M3 automated E2E应使用真实 Runtime process + Control RPC + loopback MCP + filesystem/activity，验证多个 policy generation与 DENY/ALLOW/ASK/invalid snapshot行为，不依赖浏览器或 External ChatGPT。

## 22. Final Acceptance Criteria

M3可 `ACCEPTED / FROZEN` 前必须证明：

- Tool Defaults可持久化。
- User Rules可创建/编辑/启用/禁用/删除并持久化。
- old config缺 policies安全 default。
- Runtime使用 immutable compiled Policy snapshot。
- System Guardrails不可被 User Policy覆盖。
- explicit matched User Rules按 `DENY > ASK > ALLOW`确定。
- 无 explicit match时才使用 Tool Default。
- 任一 enabled invalid rule使整个 snapshot fail closed。
- invalid snapshot下请求无 executor side effect。
- Runtime输出 effective decision + matched rule IDs + reason。
- ASK可作为真实 policy outcome观察且不执行；pending approval/resume仍留 M4。
- running policy change显示 desired/effective mismatch + Restart Required。
- restart后 effective与saved desired收敛。
- M0–M2 regression PASS。
- Integration / local black-box E2E / Manual Acceptance完成。

## 23. Open Questions

```text
0
```

局部类型名、control method名和组件拆分可在实现中选择，但不得改变以上 Policy precedence、fallback、fail-closed、immutable generation与 M4边界。
