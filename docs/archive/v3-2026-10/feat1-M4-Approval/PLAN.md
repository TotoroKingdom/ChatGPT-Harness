# M4 PLAN — Human Approval Workflow

> Status: ACCEPTED / FROZEN
> Input: `SPEC.md`  
> Baseline: M3 ACCEPTED / FROZEN  
> Goal: 将 M3 的 ASK no-execution transition升级为 Runtime-owned pending approval + same-request async resume。  
> Rule: PLAN 只决定如何实现 SPEC；Integration/E2E/Manual Acceptance另行生成。

## 1. Strategy

沿现有 Runtime / Control / MCP 架构扩展：

```text
MCP async call
→ ExecutionPipeline
→ Policy ASK
→ ApprovalManager.register
→ async waiter
↕
Control approvals.list / approvals.resolve
↕
Approval Center
→ wake exact request
→ revalidate
→ execute / deny / expire / cancel
```

并在 ASK 上增加 current-generation `SessionGrantStore`。

核心原则：

- Runtime owns pending continuation。
- Tauri只传输 approval list/resolve。
- request identity与grant scope分离。
- wait必须async/non-blocking。
- approve后必须重新验证。
- session grant memory-only且窄于 displayed scope。
- M3 policy仍 authoritative；Approval不能覆盖 DENY。

## 2. Phase A — Approval Protocol / Evidence Contract

扩展 `protocol/src/v1/approvals.rs`：

```text
ApprovalDecision += AllowForSession
ApprovalRequest += runtime/generation context
                 + matchedRuleIds
                 + policyReason
                 + riskLevel
                 + normalized scope summary
```

保留：

```text
requestId
sanitizedArguments
createdAt
expiresAt
status
resolvedAt
decision
```

更新：

- generated TS。
- JSON schema。
- protocol tests。
- Local client/mock DTO。

不把 Create Rule加进 ApprovalDecision。

Exit：wire contract可表达完整 pending/resolution evidence。

## 3. Phase B — ApprovalManager Core

新增 Runtime-owned `ApprovalManager`。

建议结构：

```text
ApprovalManager
└ Mutex/lock protected entries
   └ request_id → PendingApproval
```

PendingApproval内部保存：

```text
runtime_id
request_id
tool
request fingerprint
validated continuation handle
normalized approval scope
sanitized display DTO
policy evidence
created/expiry
async resolver/waker
terminal status
```

需要 thread-safe shared ownership：

```text
Arc<ApprovalManager>
```

因为 Control thread与 MCP Tokio thread并发访问。

实现：

- register。
- list(status filter)。
- resolve exactly once。
- expiry transition。
- cancel_all。
- terminal history for current generation。
- deterministic test clock/seam。

Exit：Manager本身能并发安全维护 Pending→terminal state。

## 4. Phase C — SessionGrantStore

新增 memory-only `SessionGrantStore`。

Grant内部至少：

```text
runtime_id
tool
normalized grant scope
created_at
source request_id
```

File-tool matcher：

- read_file = exact logical path。
- write = exact logical target path。
- edit = exact logical target path。
- glob = exact normalized pattern。

不包含 raw content。

API：

```text
matches(...)
insert_from_approved_scope(...)
clear()
```

Grant不得成为普通 persistent PolicyRule。

Exit：同 scope match、different scope miss、generation reset deterministic。

## 5. Phase D — Async-capable ExecutionPipeline

当前 pipeline同步执行，ASK立即返回。

M4把 public execution path升级为 async-capable，建议：

```text
execute_async<E>()
run_async<E>()
```

或等价实现。

ALLOW/DENY逻辑尽量复用现有代码。

ASK flow：

1. 完成 typed validation。
2. Workspace validation。
3. Policy evaluation。
4. 如果 ASK，先查 SessionGrantStore。
5. matching grant →继续 ALLOW path。
6. no grant →构建 Runtime-generated approval scope/fingerprint。
7. register pending。
8. transition AwaitingApproval。
9. emit evidence。
10. await async resolution。
11. 根据 resolution进入 resumed/denied/expired/cancelled path。

等待必须使用 async primitive，不用 blocking condvar。

Exit：MCP调用可以长期等待，同时 Control RPC仍可处理 list/resolve。

## 6. Phase E — Request Fingerprint / Scope Production

不要让 ApprovalManager自己猜 tool arguments。

由 validated executor / pipeline提供：

```text
approval display facts
request authorization fingerprint facts
session grant scope facts
```

可以扩展 `ToolExecutor` contract，例如概念方法：

```text
approval_facts()
approval_scope()
request_fingerprint()
```

或等价 typed helper。

要求：

- Runtime生成。
- deterministic。
- sanitized UI payload与internal exact identity分离。
- write/edit content不能进入 visible evidence。
- grant不因 content变化扩大/缩小为不可理解语义。

Exit：每个 file tool有稳定 approval scope。

## 7. Phase F — Post-Approval Revalidation

AllowOnce / AllowForSession唤醒后：

```text
require Runtime ready
→ require same generation
→ require tool enabled/capable
→ WorkspaceGuard revalidate
→ preserve explicit/System DENY authority
→ execute with existing ExecutionPermit
```

对 mutation：

- M1 executor自己的 commit-time revalidation仍保留。
- approval不是 path authorization缓存。

如果 revalidation失败：

- terminal approval仍可记录 Approved。
- execution result失败。
- 不产生 unsafe side effect。

Exit：等待窗口中的 filesystem/lifecycle变化不能绕过安全边界。

## 8. Phase G — MCP Exposure / Async Handler

EffectiveTool新增/启用：

```text
AvailableWithApproval
```

M4 mapping：

```text
ALLOW → Available
ASK   → AvailableWithApproval
DENY  → UnavailableByPolicy
```

`McpService::tools()` inclusion：

```text
Available OR AvailableWithApproval
```

`ServerHandler::call_tool` 改为真正 async future，await Runtime execution。

不要在 async handler里调用阻塞等待。

Exit：ASK tool可以被 client发现；同一次 call可以等待 UI resolve。

## 9. Phase H — Control Wiring

将 `ApprovalManager` shared handle注入：

- RuntimeCore。
- McpExecutionContext / Pipeline。
- control server。

`approvals.list`：

- 返回真实 items。
- 支持 status filter。
- 处理 expiry再返回一致状态。

`approvals.resolve`：

- parse AllowOnce/AllowForSession/Deny。
- unknown → ApprovalNotFound。
- terminal → ApprovalAlreadyResolved。
- success返回 updated ApprovalRequest。

Runtime status：

```text
pendingApprovals
```

应反映真实 pending count。

Exit：Control Plane可以在 MCP call仍 pending时并发 resolve。

## 10. Phase I — Shutdown / Cancellation

Runtime stop sequence必须先让所有 waiters有终局：

```text
ApprovalManager.cancel_all()
SessionGrantStore.clear()
MCP cancellation/shutdown
Runtime stop
```

或等价安全顺序。

要求：

- pending futures不会hang。
- terminal evidence可在shutdown前drain/emit。
- new generation使用全新 empty Manager/GrantStore。
- rollback initialize也不能保留旧 approval/grant state。

Exit：stop/restart无跨 generation leakage。

## 11. Phase J — Approval Center UI

当前 repo没有独立 Approval page；M4新增：

```text
ui/src/pages/approvals/
```

并接入导航/route。

数据源：

```text
approvalsList()
approvalsResolve()
```

Pending item展示：

- tool。
- scope。
- sanitized args。
- matched rule IDs。
- reason。
- risk。
- created / expires。
- current status。

Actions：

```text
Allow once
Allow for session
Deny
Create rule
```

resolve action成功后 re-fetch Runtime truth。

不在 React里模拟 pending state machine authority。

Exit：真实 Runtime approval可人工查看/resolve。

## 12. Phase K — Create Rule Handoff

Create Rule：

1. 从 ApprovalRequest构造 UI draft input。
2. 打开/导航 M3 Policies editor。
3. 预填 tool matcher。
4. 明确显示 current approval exact scope vs persistent tool-level rule scope。
5. 用户显式编辑/保存。
6. 使用现有 configSave/configRevision flow。
7. pending approval仍保持 Pending。

不要：

- auto save。
- auto AllowOnce。
- auto AllowForSession。
- 把 ApprovalManager写入 AppConfig。

Exit：Create Rule与 execution decision彻底解耦。

## 13. Phase L — Activity / Audit

扩展当前 request evidence：

```text
PolicyEvaluated ASK
ApprovalPending
ApprovalResolved
ApprovalDenied / Expired / Cancelled
ExecutionStarted
ExecutionCompleted / Failed
```

要求同 request ID可串联。

AllowForSession后续 hit grant时，应有可诊断 evidence说明：

```text
policy ASK
session grant matched
effective execution allowed
```

不泄露 raw content。

## 14. Failure / Race Handling

必须显式处理：

- resolve与expiry同时发生。
- 两个 resolve并发。
- resolve与Runtime stop并发。
- approval后target变化。
- approval后tool/runtime unavailable。
- HTTP client disconnect while pending。
- MCP server shutdown while waiter pending。

规则：

> 每个 pending request只能有一个 terminal transition；每个 executor最多启动一次。

测试不得依赖概率 race；使用 barriers/channels/test clock。

## 15. Change Discipline

- 不实现 Bash。
- 不实现 command-specific grants。
- 不改变 M3 persistent policy precedence。
- 不把 session grant写进 Policy config。
- 不建立 approval persistence。
- 不新增 approval hot reload配置。
- 不让 Tauri持有 continuation。
- 不绕过 WorkspaceGuard。
- 不放宽 M2 disabled semantics。
- 不升级 schemaVersion。

## 16. Development Completion Check

执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests/typecheck/build per repository
git diff --check
```

核心自动化覆盖：

```text
manager lifecycle
async pending/resume
allow-once exact identity
deny/expire/cancel
session grant narrow matching
grant cannot override DENY/system/disabled
post-approval revalidation
ASK MCP exposure
concurrent control resolve
shutdown cleanup
Approval Center state/actions
Create Rule handoff
M0–M3 regression
```

完成后独立执行 M4 Integration / Local Black-box E2E / Manual Acceptance。

## Final Status

M4 development、Integration、Local Black-box E2E 与 Manual Acceptance 均已完成并通过；本 milestone 已收口为 `M4 — ACCEPTED / FROZEN`。本次收口不修改 M4 产品逻辑，也不开始 M5。
