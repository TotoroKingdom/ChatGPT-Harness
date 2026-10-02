# M4 Integration Test Plan — Human Approval Workflow

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M4 — Human Approval Workflow  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: Runtime ApprovalManager / async ExecutionPipeline / SessionGrantStore / Control / MCP exposure / Approval Center integration. This is not Local Black-box E2E and does not require External ChatGPT.

## 1. Objective

验证 M4 各 Task 组合后形成真正的 Approval lifecycle：

```text
Policy ASK
→ Pending Approval
→ Control list/resolve
→ same request resumes / terminates
→ post-approval revalidation
→ execution evidence
```

Integration 的核心不是“能返回一个 approval DTO”，而是证明：

- Runtime owns pending continuation。
- same request identity可安全恢复。
- AllowOnce / AllowForSession / Deny语义不同且正确。
- Session Grant作用域不会扩大。
- Approval不能绕过 M0–M3 hard boundaries。
- Runtime shutdown不会遗留 pending waiter或grant。
- UI只是 control surface，不是 authorization authority。

## 2. Entry Criteria

执行前必须满足：

- T1–T5 development complete。
- Unit/code-level tests PASS。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- frontend tests/typecheck/build PASS。
- `git diff --check` PASS。

未完成则：

```text
Integration = BLOCKED_BY_IMPLEMENTATION
```

不得用 mock-only happy path代替真实 Runtime integration。

## 3. Integration Boundary

Integration 可以：

- 实例化真实 ApprovalManager / SessionGrantStore / RuntimeCore / ExecutionPipeline。
- 使用真实 async Tokio tests。
- 使用真实 temporary Workspace/filesystem。
- 调用真实 Control dispatch logic。
- 调用真实 MCP service adapter进程内接口。
- 使用真实 React provider/client/component integration。

Integration 不应：

- 只测 DTO serialization就声称 approval flow完成。
- 用重新发起第二个 tool request冒充 resume。
- 用 sleep race测试 expiry/concurrency。
- 依赖 External ChatGPT/Tunnel。
- 声称已验证真实 Runtime subprocess；这属于 E2E。

## 4. Required Integration Cases

### INT-01 — ASK effective tool becomes AvailableWithApproval

配置：

```text
write policy = ASK
write enabled = true
Runtime Ready
```

期望：

- effective decision = ASK。
- availability = AvailableWithApproval。
- MCP tools catalog包含 write。
- DENY仍隐藏。
- disabled仍隐藏。

### INT-02 — ASK registers one pending approval

发起一个真实 pipeline/MCP-level `write` ASK request。

在不 resolve 的情况下：

- request future仍未完成。
- ApprovalManager有且只有一个 Pending。
- pending request ID与 Execution request ID一致。
- `pendingApprovals = 1`。
- filesystem未变。

### INT-03 — approvals.list reflects Runtime truth

Pending期间验证：

- list all包含该 request。
- filter Pending包含。
- filter Approved/Denied不包含。
- displayed tool/scope/policy evidence正确。
- raw write content不出现在 DTO。

### INT-04 — AllowOnce resumes exact request once

对 INT-02 pending调用 AllowOnce。

期望：

- same original future完成。
- 不需要第二次 tool call。
- approval状态 Approved。
- decision = AllowOnce。
- write只执行一次。
- Activity/Audit request ID从 Policy ASK贯穿到 ExecutionCompleted。
- second resolve被拒绝。

### INT-05 — Request identity isolation

并发 pending A/B，使用不同 target。

批准 A：

- 只有 A waiter被唤醒。
- B仍 Pending。
- B filesystem无变化。

随后 Deny B。

### INT-06 — Deny terminates exact request

ASK write pending后 Deny：

- original future返回 APPROVAL_DENIED。
- approval状态 Denied。
- no ExecutionStarted。
- filesystem unchanged。

### INT-07 — Deterministic expiry

使用 injectable/test clock：

- Pending未到 TTL前仍 pending。
- advancing clock past TTL → Expired。
- waiter收到 APPROVAL_EXPIRED。
- no side effect。
- later resolve = already resolved。

不得真实等待 5 分钟。

### INT-08 — Runtime cancel wakes pending

创建多个 pending，再触发 Runtime stop/cancel_all。

期望：

- 全部 Pending → Cancelled。
- 全部 waiter被唤醒。
- 返回 APPROVAL_CANCELLED或等价 Runtime stopping error。
- no side effects。
- pending count归零。

### INT-09 — AllowForSession resumes current request

ASK write pending → AllowForSession。

期望：

- current exact request执行一次。
- approval Approved / AllowForSession。
- SessionGrantStore新增 one grant。
- grant source request identity可追踪。

### INT-10 — Matching session grant skips new approval

INT-09 后，在同一 generation再次请求：

```text
same tool = write
same normalized target
different content allowed
```

期望：

- Policy仍为 ASK。
- matching session grant → execute。
- 不产生新 Pending approval。
- evidence说明 session grant matched。

### INT-11 — Grant scope is narrow

已有：

```text
write foo.txt grant
```

测试：

- write bar.txt → new Pending。
- edit foo.txt → new Pending。
- glob不同 pattern → 不命中其它 grant。
- read/write不同 tool不互通。

### INT-12 — Grant cannot override explicit DENY

先创建 session grant，然后使当前 request对应 Policy outcome为 explicit DENY fixture。

期望 DENY直接终止，不检查/使用 grant放行。

### INT-13 — Grant cannot override System Guardrail

对 Workspace外路径等 hard violation：

- 即使存在同 tool grant，也必须 Workspace/System rejection。
- 不生成 ExecutionPermit。

### INT-14 — Grant cannot override disabled capability

tool disabled + stale direct call + existing grant：

- `TOOL_DISABLED`。
- 不进入 approval。
- no execution。

### INT-15 — Duplicate resolve race has one winner

使用 barrier同时发起两个 resolve，例如 AllowOnce vs Deny。

期望：

- exactly one success。
- loser = ALREADY_RESOLVED。
- executor最多一次。
- terminal state唯一。

### INT-16 — Resolve vs expiry race is single-terminal

使用 deterministic clock/barrier制造 race。

期望 terminal只能是 Approved/Denied/Expired其中之一，不出现双重 wake/double execution。

### INT-17 — Resolve vs Runtime stop race is safe

并发 approve和 stop：

- terminal transition唯一。
- 若 approval先胜出，也必须通过 post-approval readiness revalidation。
- 若 stop先胜出，不执行。
- 无 hanging waiter。

### INT-18 — Post-approval Workspace revalidation

ASK write获得 Pending后，在 approve前改变 symlink/junction/target使其不再安全。

AllowOnce后：

- approval可显示 Approved。
- execution必须失败于 Workspace revalidation。
- outside target无 mutation。

### INT-19 — Post-approval Runtime/capability revalidation

Pending期间使 Runtime进入 stopping或 capability不可用 fixture。

approve后：

- 不执行。
- 返回 typed Runtime/capability failure。
- Approval本身不等于永久 ExecutionPermit。

### INT-20 — Request fingerprint isolation

两个 request可具有同 tool+scope但 authorization-relevant request不同。

AllowOnce只能恢复其绑定的 exact invocation，不能作为 token用于另一请求。

### INT-21 — Runtime generation reset clears grants

Generation A创建 AllowForSession grant。

停止 A，创建 Generation B。

同 tool+same target ASK request：

- B不得继承 A grant。
- 必须产生新的 Pending。

### INT-22 — Runtime generation reset does not restore pending

A有 Pending，停止/重启。

B中：

- approvals.list不包含 A。
- resolve A request ID → NOT_FOUND。
- 不跨进程/restart replay。

### INT-23 — Control errors are stable

验证：

- unknown request → APPROVAL_NOT_FOUND。
- terminal request second resolve → APPROVAL_ALREADY_RESOLVED。
- malformed decision/params → INVALID_ARGUMENT/INVALID_PARAMS。
- 与 POLICY_DENIED / TOOL_DISABLED / RUNTIME_NOT_READY区分。

### INT-24 — Activity/Audit lifecycle correlation

对 AllowOnce、AllowForSession、Deny、Expired、Cancelled分别验证 evidence。

至少能回答：

```text
what request?
why ASK?
what approval decision?
did execution start?
what final outcome?
```

同 request ID串联，且不泄露 raw content/secrets。

### INT-25 — Approval Center pending integration

通过真实 Harness client/provider或等价 UI integration fixture：

- pending list展示。
- scope / matched IDs / reason / risk / expiry展示。
- Runtime返回状态是 UI truth source。

### INT-26 — Approval Center resolve actions

分别触发：

- Allow once。
- Allow for session。
- Deny。

期望调用正确 control decision并 re-fetch Runtime truth，不本地伪造成功。

### INT-27 — Terminal/stale UI handling

对 Approved/Denied/Expired/Cancelled：

- resolve controls disabled/隐藏。
- stale resolve error可理解。
- Runtime stopped时不可继续 approve stale request。

### INT-28 — Create Rule does not resolve approval

点击 Create Rule：

- 导航/打开 M3 Policy draft。
- prefill tool matcher。
- 不调用 approvals.resolve。
- pending request仍 Pending。

### INT-29 — Create Rule widening warning

对于 exact path approval：

```text
write foo.txt
```

Policy draft为 M3 tool-level `write` rule时，UI明确提示 scope更宽，且必须用户显式保存。

### INT-30 — M0–M3 regression

验证：

- M3 Tool Default / User Rule precedence / invalid snapshot。
- M2 Tool Toggle/stale disabled。
- M1 atomic write/edit。
- M0 Workspace/read/glob。
- default ALLOW tools不触发 approval。
- explicit DENY不触发 approval。

## 5. Evidence Format

每个 case记录：

```text
Case ID
PASS / FAIL / BLOCKED
Test name / command
Runtime generation / request ID
Policy decision
Approval status/decision
Grant scope when applicable
Execution outcome
Filesystem assertion
Activity/Audit assertion
```

不得记录 raw secret/content。

## 6. Exit Criteria

只有 INT-01 ～ INT-30 全部 PASS，M4 Integration才为 PASS。

特别要求：

- ASK真实 Pending。
- same request async resume。
- AllowOnce exact identity。
- AllowForSession narrow scope。
- grant不能绕过 DENY/System/disabled。
- expiry/stop/races单终局。
- post-approval revalidation。
- UI只是 control surface。
- M0–M3 regression PASS。

Integration PASS后进入真实 Local Black-box E2E。

## 7. Non-Claims

Integration PASS不代表：

```text
real Runtime subprocess flow PASS
External ChatGPT approval UX PASS
Bash approvals implemented
approval persistence implemented
M4 ACCEPTED / FROZEN
```
