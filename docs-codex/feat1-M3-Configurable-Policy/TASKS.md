# M3 TASKS — Configurable Policy

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Baseline: M2 development complete  
> 每个 Task 必须包含实现 + Unit Test 或代码级自动化验证。  
> Integration Test、E2E Test、Manual Acceptance 不属于本文件。

## Dependency

```text
T1 Policy Config / Protocol / Persistence
        ↓
T2 Compile Immutable Policy Snapshot / Engine
        ↓
T3 Runtime Evidence / Control / MCP Policy Semantics
        ↓
T4 Policies UI CRUD / Desired / Effective / Restart Required
        ↓
T5 Regression / Completion
```

---

## T1 — Policy Config / Protocol / Persistence

**Primary files:** `protocol/src/v1/config.rs`, `protocol/src/v1/policies.rs`, generated TS/schema artifacts, `ui/src-tauri/src/config/mod.rs`, config/control tests.

### Implement

新增强类型 persistent policy config：

```text
PolicyConfig
ToolPolicyDefaults
UserPolicyRuleConfig
PolicyMatcherConfig
ToolMatcherConfig
```

至少表达：

```text
Tool Defaults:
read_file = ALLOW
glob      = ALLOW
write     = ALLOW
edit      = ALLOW
bash      = ASK

User Rules:
id
name
enabled
decision
matcher
reason
```

要求：

- `AppConfig` 增加 `policies`。
- `RuntimeConfigSnapshot` 增加 `policies`。
- 保持 `CURRENT_SCHEMA_VERSION = 1`。
- `policies` 使用 backward-compatible default。
- old schema v1 config无 policies时正常 load defaults。
- persisted User Rule不允许配置 authoritative `source=system` / `editable=false`。
- ToolMatcher使用 typed/tagged structure，不以自由字符串 `scope`作为 User Rule matcher。
- M3只实现 ToolMatcher semantics。
- ConfigStore save/reload持久化 policy config。
- semantic policy change增加 `configRevision`。
- 相同 semantic policy config不增加 revision。
- failed save不得损坏旧 config。
- 不实现 M7正式 v1→v2 migration pipeline。

### Validation

Tauri/Config层至少验证：

- required fields。
- empty rule ID invalid。
- duplicate user rule IDs invalid。
- unsupported matcher type invalid。
- unknown tool matcher invalid。
- invalid decision/config shape fail closed。

不得维护一份脱离 Registry概念的随意 tool name列表；如果 host侧需要 known config names，只能与协议内置 ToolConfig identity保持单一冻结映射并由 Runtime再次 authoritative validate。

### Unit Tests

至少覆盖：

- old v1 JSON missing policies → default tool policies + empty userRules。
- safe defaults policy值正确。
- save/reload Tool Defaults。
- create/edit/enable/disable/delete User Rule persistence。
- semantic policy change revision increments。
- identical save revision unchanged。
- duplicate ID rejected。
- malformed matcher rejected。
- generated TS/schema同步。

### Done

- persisted desired Policy可稳定 CRUD。
- schemaVersion仍为1。
- System Rules未进入 AppConfig。

---

## T2 — Compile Immutable Policy Snapshot / Engine

**Primary files:** `runtime/src/policy/engine.rs`, optional compiler/internal matcher module, `runtime/src/policy/tests.rs`, `runtime/src/state.rs`, Registry integration tests.

### Implement

将当前 fixed-rules PolicyEngine演进为 generation-specific immutable compiled snapshot：

```text
fixed System Guardrails
+ Tool Defaults
+ compiled enabled User Rules
+ snapshot validity
```

Runtime initialize时使用：

```text
RuntimeConfigSnapshot.policies
+ ToolRegistry
→ compile PolicyEngine
```

### Compile rules

对所有 enabled User Rules：

- validate non-empty/unique ID。
- validate matcher type。
- validate matcher tool identity against Registry built-in identity。
- compile typed matcher。

任一 enabled semantic invalid：

```text
whole Policy snapshot = Invalid
```

不得跳过坏 rule继续寻找另一个 ALLOW。

Disabled semantic-invalid rule可以不参与 effective compile，但 JSON/type层仍必须可解析。

### Evaluation

冻结顺序：

```text
snapshot valid?
→ context/capability valid?
→ System Guardrails
→ matching explicit User Rules?
   ├ yes → DENY > ASK > ALLOW
   └ no  → Tool Default
```

Tool Default绝不与 matched User Rules一起 severity competition。

### Evidence

扩展 `PolicyEvaluation`：

```text
decision
matched_rule_ids
reason
```

reason至少支持：

```text
SystemGuardrail
MatchedUserRules
ToolDefault
InvalidRuleSet
InvalidContext
UnsupportedCapability
```

matched IDs稳定排序。

### Unit Tests

至少覆盖：

- default no-user-rule → per-tool Tool Default。
- write default ALLOW。
- bash default ASK config存在但不产生 executor capability。
- explicit User ALLOW匹配后使用 ALLOW而不是 fallback。
- explicit User ASK匹配。
- explicit User DENY匹配。
- multiple matches：DENY > ASK > ALLOW。
- vector/order变化不影响结果。
- duplicate IDs → invalid snapshot。
- unknown tool matcher → invalid snapshot。
- invalid enabled rule即使不匹配当前 tool也使 snapshot invalid。
- disabled rule不参与 decision。
- System DENY不可被 User ALLOW覆盖。
- matched IDs deterministic。

### Done

- Runtime Policy truth来自 immutable compiled snapshot。
- Tool Default fallback语义正确。
- User Policy不能覆盖 System Guardrails。

---

## T3 — Runtime Evidence / Control / MCP Policy Semantics

**Primary files:** `runtime/src/execution/pipeline.rs`, `runtime/src/tools/effective.rs`, `runtime/src/control_read.rs`, protocol control-read DTO/methods, audit/activity modules, MCP error/exposure tests as needed.

### Implement

让 Runtime execution/effective/control surfaces全部消费同一个 compiled PolicyEngine。

### Execution behavior

#### ALLOW

正常进入 executor。

#### DENY

```text
PolicyDenied
no executor side effect
```

#### ASK

M3冻结为 M4前过渡：

```text
PolicyEvaluation = ASK
→ AwaitingApproval state/evidence
→ APPROVAL_REQUIRED
→ no executor execution
```

不得实现 pending ApprovalManager、async wait或resume。

### Invalid snapshot

任何 policy-controlled request：

```text
DENY / fail closed
no executor side effect
reason = invalid policy snapshot
```

Runtime本身仍应保持可观察，便于 Control/UI诊断。

### Effective Tool

更新 effective tool reason/decision projection：

- Tool Default ALLOW。
- matched User Rule decision。
- invalid snapshot DENY。
- disabled/runtime unavailable继续保持 M2 precedence。

M3不要求完成 M4 最终 `AvailableWithApproval` exposure model；ASK direct call必须可观察为 `APPROVAL_REQUIRED`且不执行。

### Control read

新增或扩展 Runtime policy read model，至少返回：

```text
snapshot valid/status
system rules
current generation user rule projection
tool defaults
per-tool effective decision
matched rule IDs
reason
```

UI不得自行重算。

### Activity / Audit

Policy evaluation evidence至少关联：

```text
request ID
tool
decision
matched rule IDs
reason
outcome
```

不得泄露 raw content/secret。

### Tests

至少覆盖：

- ALLOW executes。
- DENY no side effect。
- ASK returns APPROVAL_REQUIRED + no side effect。
- invalid snapshot rejects + no side effect。
- direct stale/cached call仍重新 policy evaluate。
- disabled tool仍优先得到 M2 disabled semantics，不被 User ALLOW开启。
- matched IDs/reason进入 audit/effective/control evidence。
- Runtime NotReady / Disabled / Policy Deny / Ask可区分。

### Done

- Policy不是 UI-only配置。
- Runtime enforcement/evidence/control truth一致。
- M4 Approval未提前实现。

---

## T4 — Policies UI CRUD / Desired / Effective / Restart Required

**Primary files:** `ui/src/pages/policies/PoliciesPage.tsx`, related components, `ui/src/app/providers/*`, `ui/src/services/harness/*`, Tauri config commands/read models as needed, UI tests.

### Implement

将当前 read-only Policies页升级为 Policy配置 + Runtime observation页面。

### Desired area

来自 `AppConfig.policies`：

#### Tool Defaults

对每个已知 built-in config tool显示：

```text
ALLOW / ASK / DENY
```

当前 executable tools至少：

```text
read_file
glob
write
edit
```

bash default可以显示为 future/default config，但不能暗示 bash已实现。

#### User Rules

支持：

```text
Create
Edit
Enable / Disable
Delete
```

编辑字段：

```text
name
decision
tool matcher
reason
enabled
```

rule ID必须稳定；编辑已有 rule不应无理由生成新 identity。

### Effective area

来自 Runtime Control：

- policy snapshot valid/status。
- System Guardrails（read-only）。
- current generation Tool Defaults。
- current generation User Rules projection。
- per-tool effective decision。
- matched IDs / reason。

### System rules

System Rules UI必须：

```text
no edit
no delete
no enable/disable toggle
```

### Desired vs Effective

Harness running时保存 Policy：

```text
Desired = saved new config
Effective = current generation old policy
Restart Required = true
```

不得 optimistic 地把 effective row改成新 decision。

Restart后：

```text
Desired == Effective generation config
Restart Required = false
```

### Validation / save failure

- duplicate ID / unknown tool / unsupported matcher等显示明确错误。
- save failure后 rollback或重新读取 persisted truth。
- 不永久显示未保存 rule。
- 不允许编辑 System Rule。

### UI Tests

至少覆盖：

- create User Rule persists desired state。
- edit preserves rule identity。
- enable/disable。
- delete。
- Tool Default edit。
- System Rule read-only controls absent/disabled。
- running desired/effective mismatch + Restart Required。
- restart convergence。
- save validation failure不假装成功。
- invalid Runtime policy snapshot显示 fail-closed状态。
- effective decision来自 Runtime response，不由 React重新算。

### Done

- User Rules完整 CRUD。
- UI清楚区分 desired与effective。
- System Rules不可修改。

---

## T5 — Cross-cutting Regression / Development Completion

**Primary scope:** protocol/config/runtime/Tauri/UI automated regression；不新增产品功能。

### Regression Requirements

#### M2 Tool Toggle

- old schema v1 without policies/tools compatibility组合正常。
- Tool desired/effective/restart行为不回归。
- disabled tool absent list + stale call rejection不回归。
- User ALLOW不能重新启用 disabled tool。

#### M1 File Operations

- write atomic create/overwrite。
- edit exact atomic mutation。
- target-change/failure safety重要 regression。
- Workspace boundary。

#### M0 / Base

- read_file / glob。
- MCP schemas/annotations。
- Runtime lifecycle/control reads。

#### Policy Security

- WorkspaceGuard先于 ordinary Policy permission，不被 User Rule替代。
- System DENY不可覆盖。
- invalid snapshot fail closed。
- ASK no execution。
- ExecutionPermit仍 pipeline-only。
- Policy evidence无 raw content/secret。

### Development Completion Commands

执行 repository标准命令：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend test command(s)
frontend typecheck/build command(s) where repository requires
git diff --check
```

全部 PASS。

### Done

T1–T5完成只代表 M3 development complete。

随后必须独立生成并执行：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；只有 Manual Acceptance需要人工介入。

---

## Development Completion Checklist

- [ ] T1 Policy Config / Protocol / Persistence PASS
- [ ] T2 Immutable Policy Snapshot / Engine PASS
- [ ] T3 Runtime Evidence / Control / MCP semantics PASS
- [ ] T4 Policies UI CRUD / desired/effective/restart PASS
- [ ] T5 Regression PASS
- [ ] legacy config missing policies defaults PASS
- [ ] configRevision semantics PASS
- [ ] User Rule CRUD persistence PASS
- [ ] Tool Default fallback PASS
- [ ] DENY > ASK > ALLOW explicit-rule aggregation PASS
- [ ] System Guardrail cannot be overridden PASS
- [ ] enabled invalid rule fail-closed PASS
- [ ] matched rule IDs deterministic PASS
- [ ] effective reason evidence PASS
- [ ] ASK = APPROVAL_REQUIRED / no execution PASS
- [ ] running desired/effective mismatch PASS
- [ ] restart convergence PASS
- [ ] M0–M2 regression PASS
- [ ] cargo fmt/check/test PASS
- [ ] frontend tests/typecheck/build PASS as applicable
- [ ] git diff --check PASS
- [ ] TASKS未声称 Integration/E2E/Manual Acceptance PASS
