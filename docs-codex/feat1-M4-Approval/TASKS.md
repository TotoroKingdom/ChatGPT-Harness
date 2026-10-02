# M4 TASKS — Human Approval Workflow

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Baseline: M3 ACCEPTED / FROZEN  
> 每个 Task 必须包含实现 + Unit Test 或代码级自动化验证。  
> Integration / E2E / Manual Acceptance 不属于本文件。

## Dependency

```text
T1 Approval Protocol / Manager Core
        ↓
T2 Async Pipeline / ASK MCP Resume
        ↓
T3 Session Grants / Control / Shutdown
        ↓
T4 Approval Center / Create Rule UX
        ↓
T5 Regression / Completion
```

---

## T1 — Approval Protocol / Manager Core

**Primary files:** `protocol/src/v1/approvals.rs`, generated TS/schema, new Runtime approval module, related tests.

### Implement

#### Protocol

扩展：

```text
ApprovalDecision
├ AllowOnce
├ AllowForSession
└ Deny
```

`ApprovalRequest` additive包含足够 evidence：

- requestId。
- runtime/generation identity。
- tool。
- sanitizedArguments。
- normalized scope summary。
- matchedRuleIds。
- policyReason。
- riskLevel。
- createdAt / expiresAt。
- status。
- resolvedAt?。
- decision?。

`Create Rule` 不加入 `ApprovalDecision`。

#### ApprovalManager

新增 Runtime-owned thread-safe Manager：

```text
register
list/filter
resolve
expire
cancel_all
pending_count
```

每个 internal PendingApproval必须绑定：

```text
runtime_id
request_id
tool
request fingerprint
validated continuation/waiter
normalized grant scope
policy evidence
timestamps/status
```

默认 approval TTL = 5 minutes。

使用 deterministic test clock/seam。

### Required Semantics

- request ID唯一。
- duplicate registration fail closed。
- unknown resolve → `APPROVAL_NOT_FOUND`。
- terminal resolve → `APPROVAL_ALREADY_RESOLVED`。
- concurrent resolves只一个 winner。
- terminal history仅当前 generation memory内存在。
- 不写 AppConfig。

### Unit Tests

至少覆盖：

- register pending。
- list all / filter pending/approved/denied/expired/cancelled。
- allow-once resolve。
- allow-for-session resolve。
- deny resolve。
- duplicate resolve。
- unknown resolve。
- deterministic expiry。
- cancel_all。
- concurrent resolve exactly one winner。
- DTO不包含 raw mutation content/secret。

### Done

- ApprovalManager可被 MCP/Control安全共享。
- protocol artifacts同步。
- 无 execution resume逻辑偷进 T1。

---

## T2 — Async Pipeline / ASK MCP Resume

**Primary files:** `runtime/src/execution/*`, `runtime/src/mcp/context.rs`, `runtime/src/mcp/service.rs`, tool executor approval-facts seam, tests.

### Implement

把 M3：

```text
ASK → ApprovalRequired → return
```

升级为：

```text
ASK
→ check SessionGrant
→ register pending
→ AwaitingApproval
→ await async resolution
→ resume exact invocation or terminate
```

#### Async requirements

- MCP ServerHandler `call_tool`必须返回真正 async future。
- waiting approval不能阻塞 Tokio current-thread runtime。
- 使用 `tokio::sync` async primitive或等价。
- 禁止 blocking Condvar wait/polling sleep。
- ALLOW/DENY现有 path尽量保持语义不变。

#### Tool approval facts

为 read/glob/write/edit提供 Runtime-generated：

- sanitized display facts。
- request fingerprint。
- normalized approval scope。

冻结 file grant scopes：

```text
read_file → exact normalized logical path
write     → exact normalized logical target
edit      → exact normalized logical target
glob      → exact normalized pattern/query scope
```

write/edit raw text不得进入 UI evidence/session scope。

#### AllowOnce resume

AllowOnce唤醒后：

1. same request identity/fingerprint。
2. Runtime still Ready。
3. same generation。
4. tool still eligible/enabled。
5. WorkspaceGuard revalidate。
6. System/explicit DENY不可绕过。
7. only then create/use ExecutionPermit。
8. executor最多一次。

#### Deny / expiry / cancel

分别结束 exact invocation并返回 typed sanitized error：

```text
APPROVAL_DENIED
APPROVAL_EXPIRED
APPROVAL_CANCELLED
```

无 side effect。

### MCP Exposure

M4新增/启用：

```text
ToolAvailability::AvailableWithApproval
```

mapping：

- ALLOW → Available。
- ASK → AvailableWithApproval。
- DENY → UnavailableByPolicy。

`tools/list`必须包含 Available + AvailableWithApproval。

### Unit / Code Tests

至少覆盖：

- ASK tool出现在 MCP catalog。
- ASK invocation不会立即返回 ApprovalRequired。
- pending期间另一个 async/control task可以运行。
- AllowOnce恢复 same future。
- Deny/expiry/cancel结束 same future。
- request A approval不能唤醒 request B。
- post-approval target/workspace变化拒绝。
- tool disabled/runtime stopping during wait拒绝。
- executor exactly once。
- default ALLOW path无回归。
- DENY path无回归。

### Done

- M4核心 same-request async resume成立。
- 不要求 client重新 call tool。
- 不阻塞 MCP event loop。

---

## T3 — Session Grants / Control / Shutdown

**Primary files:** new session-grant module, `runtime/src/state.rs`, `runtime/src/server.rs`, `runtime/src/mcp/*`, Tauri control bridge as needed, Activity/Audit tests.

### Implement

#### SessionGrantStore

memory-only current-generation store：

```text
runtime_id
source_request_id
tool
normalized grant scope
created_at
```

AllowForSession：

1. resolve current pending request。
2. insert grant from Runtime-generated approval scope。
3. wake exact current invocation。
4. current invocation执行。
5. later matching ASK request先命中 grant，不再生成 Approval。

Matcher：

```text
same generation
+ same tool
+ same normalized scope
```

#### Precedence

Grant只能作用于 ASK。

以下永远不能被 grant覆盖：

- explicit User DENY。
- System Guardrail DENY。
- invalid policy snapshot。
- Tool disabled。
- Runtime unavailable。
- Workspace violation。

#### Control wiring

`approvals.list`：

- 返回 Manager真实 snapshot。
- status filter生效。
- expiry状态在返回前收敛。

`approvals.resolve`：

- 调用 Manager真实 resolve。
- AllowForSession创建 narrow grant。
- 返回 updated ApprovalRequest。
- error映射稳定 machine codes。

`RuntimeState.pendingApprovals` 使用真实 pending count。

#### Lifecycle

Runtime stop/rollback/crash cleanup：

```text
cancel_all pending
clear session grants
wake waiters
```

new generation manager/store为空。

#### Evidence

同 request ID记录：

- policy ASK。
- approval pending。
- resolved decision。
- grant creation/match when applicable。
- final execution result。

### Unit / Integration-level Code Tests

至少覆盖：

- AllowForSession current request执行。
- subsequent same tool+same path request免审批。
- different path miss grant。
- different tool miss grant。
- glob different pattern miss grant。
- explicit DENY ignores existing grant。
- disabled tool ignores grant。
- System Workspace boundary ignores grant。
- Runtime stop wakes all pending。
- Runtime stop clears all grants。
- restart/new generation no old grant。
- approvals.list真实 filter。
- pendingApprovals count。
- resolve/expiry/stop races one terminal winner。
- raw content不进入 evidence。

### Done

- Session grant scope不宽于 displayed scope。
- Control methods从 skeleton变为 production behavior。
- stop/restart无 approval/grant leakage。

---

## T4 — Approval Center / Create Rule UX

**Primary files:** new `ui/src/pages/approvals/*`, app navigation/routing, providers/client wiring, `ui/src/pages/policies/*` draft handoff, Tauri commands if needed, UI tests.

### Implement

新增 Approval Center。

Pending approval至少展示：

```text
Tool
Status
Normalized scope
Sanitized arguments
Matched rule IDs
Policy reason
Risk level
Created time
Expires time
```

Actions：

```text
Allow once
Allow for session
Deny
Create rule
```

#### Resolve UX

- 调用现有 `approvalsResolve`。
- 成功后重新读取 Runtime truth。
- pending card转 terminal/read-only或从 Pending filter消失。
- stale/expired/duplicate error给出可理解提示。
- Runtime stopped时不允许继续假装 approval成功。
- UI不本地伪造 execution completed。

#### Create Rule

Create Rule：

```text
ApprovalRequest
→ Policies editor draft
```

预填：

- matcher.type = tool。
- matcher.tool = approval.tool。
- readable name/reason seed。
- decision由用户明确选择/确认。

必须明确显示：

```text
Current approval scope may be one exact path
Persistent M3 rule is tool-level and therefore broader
```

要求：

- 不自动保存。
- 不自动 resolve当前 pending request。
- 保存继续使用 M3 configSave/revision/restart flow。
- 当前 pending request仍需 AllowOnce / AllowForSession / Deny。

### UI Tests

至少覆盖：

- pending empty/loading/error states。
- pending fields全部显示。
- AllowOnce action。
- AllowForSession action。
- Deny action。
- terminal item buttons disabled/hidden。
- expired item不可 resolve。
- resolve error不显示 false success。
- Create Rule opens/prefills Policy draft。
- Create Rule不调用 approvalsResolve。
- persistent rule widening warning可见。
- Runtime stop state。
- accessibility basics for action controls。

### Done

- UI是 Runtime approval的控制面，不是 authority。
- Create Rule与当前 execution decision分离。

---

## T5 — Cross-cutting Regression / Development Completion

**Primary scope:** Runtime/MCP/Control/UI automated regression，不新增产品功能。

### Regression Requirements

#### M3 Policy

- Tool Default fallback不变。
- DENY > ASK > ALLOW不变。
- invalid policy fail closed。
- System Rules不可覆盖。
- matched IDs/reason evidence不变。

#### M2 Tool Toggle

- disabled优先于 Approval。
- stale disabled call仍 `TOOL_DISABLED`。
- restart semantics不变。

#### M1 File mutation

- write/edit atomic semantics不变。
- approved write/edit仍经过 M1 commit-time revalidation。
- denied/expired/cancelled ASK无 filesystem mutation。

#### M0/read/glob

- normal behavior。
- Workspace boundaries。
- MCP schemas/annotations保持正确。

#### Concurrency / lifecycle

- multiple simultaneous pending requests可独立resolve。
- request A decision不能影响 B。
- one session grant只匹配自己的 scope。
- Runtime shutdown无 hanging MCP requests。
- MCP endpoint可以clean shutdown。

#### Evidence

- policy→approval→execution可关联。
- ApprovalRequest/Activity不泄露 raw content/secrets。

### Development Completion Commands

执行 repository标准命令：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests/typecheck/build per repository
git diff --check
```

全部 PASS。

### Done

T1–T5完成只代表 M4 development complete。

随后必须独立生成并执行：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；只有 Manual Acceptance需要人工介入。

---

## Development Completion Checklist

- [ ] T1 Approval Protocol / Manager PASS
- [ ] T2 Async Pipeline / ASK MCP Resume PASS
- [ ] T3 Session Grants / Control / Shutdown PASS
- [ ] T4 Approval Center / Create Rule UX PASS
- [ ] T5 Regression PASS
- [ ] ASK exposed as AvailableWithApproval
- [ ] ASK same-call pending/resume
- [ ] AllowOnce exact identity
- [ ] AllowForSession narrow grant
- [ ] Deny no side effect
- [ ] expiry/cancel no side effect
- [ ] duplicate resolve race-safe
- [ ] post-approval revalidation
- [ ] Runtime stop wakes pending
- [ ] restart clears grants
- [ ] Create Rule does not auto-resolve
- [ ] M0–M3 regression PASS
- [ ] cargo fmt/check/test PASS
- [ ] frontend tests/typecheck/build PASS as applicable
- [ ] git diff --check PASS
- [ ] TASKS未声称 Integration/E2E/Manual Acceptance PASS
