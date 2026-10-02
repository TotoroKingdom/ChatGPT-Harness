# M3 PLAN — Configurable Policy

> Status: ACCEPTED / FROZEN
> Input: `SPEC.md`  
> Baseline: M2 development complete  
> Goal: 将当前 Runtime 内部固定/只读 Policy 基础设施升级为 persistent desired policy + immutable compiled Runtime snapshot + editable UI。  
> Rule: PLAN 只决定如何实现 SPEC；Integration/E2E/Manual Acceptance 另行生成独立文档。

## 1. Strategy

沿用 M2 已验证的 immutable-generation 模型：

```text
AppConfig.policies
→ ConfigStore / configRevision
→ RuntimeConfigSnapshot.policies
→ compile + validate immutable PolicyEngine
→ ExecutionPipeline / EffectiveTool / Control evidence
→ Policies UI desired/effective/restart UX
```

不引入 live mutable policy state。

核心原则：

- System Guardrails 仍由 Runtime owns。
- User Policy 只来自 persisted config。
- Tool Default 是 fallback，不是始终参与竞争的 rule。
- Runtime compile/evaluation 是 authorization authority；UI 不重算 policy。
- enabled invalid User Rule 使 whole snapshot fail closed。
- ASK 在 M3 是真实 policy outcome，但 pending approval/resume 留给 M4。

## 2. Phase A — Protocol / Persisted Policy Config

扩展 `protocol/src/v1/config.rs` 与 `policies.rs`。

新增建议类型：

```text
PolicyConfig
ToolPolicyDefaults
UserPolicyRuleConfig
PolicyMatcherConfig
ToolMatcherConfig
```

### Persisted defaults

```text
read_file = ALLOW
glob      = ALLOW
write     = ALLOW
edit      = ALLOW
bash      = ASK
userRules = []
```

### Compatibility

- `CURRENT_SCHEMA_VERSION` 继续为 1。
- `policies` 使用 backward-compatible default。
- old schema v1 missing policies 能正常 load。
- 不实现 M7 v1→v2 migration pipeline。

### User rule persisted shape

只持久化 user-owned fields：

```text
id
name
enabled
decision
matcher
reason
```

不要让 config携带 authoritative `source=system` / `editable=false`。

### Typed matcher

M3 实现：

```text
ToolMatcher { tool }
```

使用 tagged enum以便 M5扩展。

Exit：protocol/schema/TS + ConfigStore save/reload/revision tests PASS。

## 3. Phase B — Config Validation / CRUD Persistence

在 ConfigStore/Tauri control 层提供保存 User Policy 的 typed validation。

至少验证：

- required fields。
- non-empty ID/name where required。
- duplicate user rule IDs。
- matcher type supported by M3。
- tool identity known/configurable。
- decision合法。

UI保存失败不能损坏旧 config。

CRUD 本质上仍是更新完整 `AppConfig.policies`：

```text
create/edit/toggle/delete
→ update desired PolicyConfig
→ ConfigStore.save
→ configRevision semantics
```

不新建 policy-only数据库或单独 persistence store。

Exit：CRUD persistence和 revision semantics可稳定自动化。

## 4. Phase C — Runtime Snapshot Wiring

Tauri启动 Runtime时传：

```text
AppConfig.policies
→ RuntimeConfigSnapshot.policies
```

保证：

- snapshot non-secret。
- current generation immutable。
- saving policy while running不改变 current Runtime engine。
- revision mismatch复用现有 Restart Required identity。

Exit：policy generation A/B可以由不同 snapshots确定构造。

## 5. Phase D — Compile Policy Snapshot

重构 `PolicyEngine` construction，使其从：

```text
fixed system rules only
```

变成：

```text
fixed system guardrails
+ compiled user rules
+ tool defaults
+ snapshot validity
```

建议拆分：

```text
PolicyCompiler
CompiledPolicyRule
CompiledPolicySnapshot
PolicyEngine
```

也可在现有 engine内部实现，只要职责清晰。

### Compilation

启动时遍历全部 enabled User Rules：

- validate unique ID。
- validate matcher。
- validate tool against Registry built-in identity。
- compile to immutable matcher representation。

任一 enabled semantic invalid：

```text
CompiledPolicySnapshot::Invalid(...)
```

不要跳过坏 rule。

Disabled rule不参与 effective compile，但保留在 desired/effective projection中。

Exit：invalid enabled rule无论顺序如何都让 snapshot invalid。

## 6. Phase E — Evaluation Semantics

重构当前以 free-string scope为中心的 user-policy matching。

保留 System guardrail所需的 internal workspace/security facts，但 User Rules使用 typed tool matcher。

建议 evaluation顺序：

```text
validate snapshot
→ validate context/capability
→ evaluate hard/system guardrails
→ collect matching explicit user rules
   ├ exists → max severity DENY > ASK > ALLOW
   └ none   → tool default
→ PolicyEvaluation evidence
```

### Evidence

扩展 reason enum至少区分：

```text
SystemGuardrail
MatchedUserRules
ToolDefault
InvalidRuleSet
InvalidContext
UnsupportedCapability
```

`matched_rule_ids`：

- user match时包含全部 matched IDs，稳定排序。
- tool default fallback为空。
- invalid snapshot为空或包含专门 invalid evidence，但不得伪造 match。

Exit：Tool Default fallback与 explicit user rules语义完全符合 SPEC。

## 7. Phase F — ExecutionPipeline / ASK Boundary

ExecutionPipeline继续消费统一 `PolicyEvaluation`。

### ALLOW

正常执行。

### DENY

```text
PolicyDenied
no executor side effect
```

### ASK

M3保持：

```text
AwaitingApproval state/evidence
→ APPROVAL_REQUIRED
→ no executor execution
```

但不新增：

- pending ApprovalManager entry。
- async wait。
- resume。
- approval actions。

这些属于 M4。

确保 Activity/Audit 能看到 decision + matched IDs + reason。

Exit：ASK是真实可诊断 Policy outcome，但不会执行。

## 8. Phase G — Effective Tool / MCP Policy Projection

更新 EffectiveTool computation，使 reason/evidence反映新的 PolicyEngine：

- default ALLOW。
- explicit user ALLOW/ASK/DENY。
- invalid snapshot DENY。
- disabled/runtime unavailable优先状态仍保持 M2语义。

M3不强行完成 M4 最终 ASK exposure model。

允许过渡：

```text
ASK effective decision observable
ASK direct call → APPROVAL_REQUIRED
ASK tool may remain hidden from tools/list until M4
```

DENY tool不得因旧 MCP缓存调用而绕过 policy。

Exit：MCP direct calls始终重新执行 policy evaluation。

## 9. Phase H — Runtime Policy Control Read

新增/扩展 read-only Control projection。

建议 contract：

```text
PolicyEffectiveSnapshot
├ valid
├ status/reason
├ systemRules[]
├ userRules[]
├ toolDefaults
└ perTool[]
   ├ tool
   ├ decision
   ├ matchedRuleIds
   └ reason
```

UI只能消费 Runtime output，不重写 evaluation算法。

如果现有 `policies.list` surface足够演进，可复用；否则新增 additive read method，例如：

```text
policies.getEffective
```

Exit：当前 generation的 policy truth可稳定由 Control read观察。

## 10. Phase I — Policies UI CRUD / Desired vs Effective

将当前只读 PoliciesPage改为两类信息：

### Desired

来自 `AppConfig.policies`：

- Tool Defaults controls。
- User Rule list。
- Create/Edit/Delete。
- Enable/Disable。

### Effective

来自 Runtime control read：

- System Guardrails read-only。
- Runtime policy snapshot status。
- per-tool effective decision。
- matched IDs / reason。

### Running mismatch

保存 policy后：

```text
Desired changes
Effective remains generation A
Restart Required = true
```

restart生成 generation B后收敛。

### System rule UX

System rule无：

```text
edit/delete/toggle
```

### Validation UX

duplicate ID / unknown tool / malformed matcher等保存失败时显示明确错误，不假装保存成功。

Exit：用户能清楚区分“已保存下一代策略”与“当前 Runtime正在执行的策略”。

## 11. Phase J — Regression / Security Boundary

必须回归：

### M2

- tool toggle desired/effective/restart。
- disabled tool stale call优先 fail。
- disabled不被 User ALLOW重新开启。

### M1

- atomic write/edit。
- Workspace boundary。

### M0

- read/glob正常。
- MCP schemas/annotations。

### Security

- WorkspaceGuard仍在 Policy之前独立 enforcement。
- System Guardrail不可由 User ALLOW覆盖。
- ExecutionPermit仍只由 Pipeline构造。
- raw content/secrets不进入 policy evidence。

## 12. Change Discipline

- 不升级 schemaVersion=2。
- 不做 M7 migration pipeline。
- 不实现 M4 ApprovalManager / resume lifecycle。
- 不改成 live Policy hot reload。
- 不实现 M5 Bash command parser/matcher/executor。
- 不实现 arbitrary expression language。
- 不让 React/Tauri成为 policy execution authority。
- 不修改 M1 atomic mutation internals。

## 13. Development Completion Check

完成所有 Tasks 后运行 repository标准检查：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests / typecheck / build commands defined by repository
git diff --check
```

稳定自动化至少覆盖：

```text
policy config compatibility + revision
CRUD persistence
snapshot immutability
compile invalid fail-closed
Tool Default fallback
explicit ALLOW/ASK/DENY
severity aggregation
System deny cannot be overridden
matched IDs deterministic
reason evidence
ASK no-execution behavior
desired/effective mismatch
restart convergence
M0-M2 regression
```

M3 Integration、Local Black-box E2E 与 Manual Acceptance 已独立完成并 PASS；本 milestone 已收口为 `M3 — ACCEPTED / FROZEN`。M4 仍仅作为下一 milestone 规划，本次未开始实现。
