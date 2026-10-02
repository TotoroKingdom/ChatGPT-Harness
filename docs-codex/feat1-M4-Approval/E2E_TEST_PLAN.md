# M4 E2E Test Plan — Local Black-box Approval Workflow

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M4 — Human Approval Workflow  
> Owner: Coding Agent / QA Agent  
> Requirement: fully automated local black-box test against real compiled Runtime process.  
> Forbidden substitutions: mock server, in-process ExecutionPipeline-only test, External ChatGPT/manual UI.

## 1. Objective

从 Runtime公开边界证明 M4 的真实 Approval lifecycle：

```text
real Runtime subprocess
    ↕ stdin/stdout JSONL Control
real loopback MCP HTTP
    ↓
tools/call stays pending
    ↕
approvals.list / approvals.resolve
    ↓
same original MCP call resumes or terminates
    ↓
filesystem + Activity evidence
```

最重要的 E2E invariant：

> **不能把“第一次 call 返回 APPROVAL_REQUIRED，批准后第二次重新 call”算作 M4 PASS。**

必须证明第一次真实 MCP request本身保持 Pending并在 resolve 后结束。

## 2. Entry Criteria

- T1–T5 complete。
- Unit PASS。
- Integration PASS。
- release/final-ish Runtime build PASS。
- disposable Workspace。
- E2E runner可并发管理 MCP request和 Control stdin/stdout。

推荐 runner：

```text
runtime/tests/m4_e2e.rs
```

或等价自动化 runner。

## 3. Runner Architecture

### Runtime

启动真实：

```text
chatgpt-harness-runtime --release
```

记录 PID/binary identity。

### Control

通过真实 stdin/stdout：

```text
initialize
initialized
mcp.endpoint.get
approvals.list
approvals.resolve
activity.list
runtime.stop
```

### MCP

连接真实 Runtime返回的 loopback endpoint：

```text
initialize MCP session
tools/list
tools/call
```

### Concurrency

runner必须能：

1. 启动一个 `tools/call` future/thread。
2. 确认它尚未完成。
3. 在同一 Runtime process的 Control channel调用 approvals.list。
4. resolve approval。
5. 等待原 `tools/call` 完成。

不得阻塞 Runtime Control处理。

## 4. Generation Fixtures

至少启动多个 Runtime generation：

### Generation A — ASK approval flow

```text
write default/user policy = ASK
write enabled = true
```

### Generation B — restart/reset proof

用于验证 pending/session grant不跨 generation。

如需 DENY/ALLOW regression可额外 generation。

## 5. Required E2E Cases

### E2E-01 — Boot / Control / MCP discovery

- release Runtime正常 initialize/ack。
- 获取真实 loopback MCP endpoint。
- `tools/list`成功。
- Activity/Control read正常。

### E2E-02 — ASK tool is exposed

Generation A：

- write effective decision ASK。
- `tools/list`包含 write。
- Runtime effective projection显示 approval-required availability。

### E2E-03 — Real MCP call becomes Pending

发起：

```text
write("approval-once.txt", "approved once")
```

但暂不 resolve。

验证：

- MCP call在短 deterministic window内仍未完成。
- filesystem目标不存在。
- Control `approvals.list(Pending)`出现一个 write request。
- request有 sanitized scope/evidence。
- Runtime pendingApprovals = 1。

### E2E-04 — AllowOnce resumes same MCP call

对 E2E-03 request调用真实：

```text
approvals.resolve(AllowOnce)
```

验证：

- 原 MCP call随后成功返回。
- 未发第二次 tools/call。
- 文件精确创建。
- pending count归零。
- approval terminal = Approved / AllowOnce。
- Activity同 request关联 ASK→Pending→Approved→ExecutionCompleted。

### E2E-05 — AllowOnce is not reusable

再次对同 approval ID resolve：

- ALREADY_RESOLVED。

再次发同 target write ASK：

- 产生新的 Pending，证明 AllowOnce不形成 session grant。

随后 Deny cleanup。

### E2E-06 — Deny terminates same call

发起新的 ASK write，确认 Pending后 Deny。

验证：

- 原 MCP call结束为 APPROVAL_DENIED。
- target不存在/unchanged。
- no ExecutionCompleted success。
- approval terminal = Denied。

### E2E-07 — AllowForSession current request

发起：

```text
write("session.txt", "first")
```

Pending后 AllowForSession。

验证原 call成功、文件内容正确、approval终局正确。

### E2E-08 — Session grant matches same scope

同 Generation A再次：

```text
write("session.txt", "second")
```

验证：

- call无需人为 resolve即可完成。
- 没有新增 Pending。
- 文件变为 second。
- Activity能观察 session grant match/equivalent evidence。

### E2E-09 — Session grant does not match different path

调用：

```text
write("other.txt", "other")
```

验证：

- 新 Pending出现。
- 不自动执行。
- resolve Deny后 other.txt不存在。

### E2E-10 — Tool identity isolates grant

如果当前 build能对另一个 ASK file tool构造同 logical target，则验证不同 tool不命中 write grant。

若工具输入语义不允许自然构造，可由 Integration承担，E2E记录 N/A_WITH_INTEGRATION_EVIDENCE，不得伪造。

### E2E-11 — Runtime stop cancels real pending call

发起 ASK write并确认 Pending。

通过真实 Control `runtime.stop`/process shutdown。

验证：

- pending MCP request不会无限挂起。
- target未创建。
- process clean shutdown。
- 无 orphan Runtime/MCP endpoint。

### E2E-12 — New generation has no old session grant

启动 Generation B，使用与 E2E-07相同 policy与 target。

调用：

```text
write("session.txt", ...)
```

期望重新 Pending，而不是继承 Generation A grant。

resolve Deny/AllowOnce完成清理。

### E2E-13 — Old approval ID not valid after restart

对 Generation A approval ID调用 Generation B `approvals.resolve`：

- NOT_FOUND。
- 不影响任何 B request。

### E2E-14 — Disabled capability still precedes approval

启动或配置 generation：

```text
write disabled
policy ASK
```

验证：

- `tools/list`不含 write。
- stale direct call被 TOOL_DISABLED。
- 不产生 Pending approval。
- no filesystem side effect。

### E2E-15 — Explicit DENY does not create approval

真实 generation配置 write DENY：

- tool隐藏或 direct call POLICY_DENIED。
- approvals.list无新增 Pending。
- no side effect。

### E2E-16 — Workspace/System boundary cannot be approved around

对 Workspace外 write发起 direct call。

期望：

- Workspace/System rejection。
- 不产生可供用户“批准绕过”的 Pending。
- outside target unchanged。

### E2E-17 — Activity confidentiality and lifecycle

检查真实 Activity：

- request ID可关联。
- policy ASK、approval、execution outcome完整。
- 不包含 sentinel raw write content。
- 不包含 secret。
- 不无必要泄露 absolute root。

### E2E-18 — Clean shutdown

最后：

- 所有测试-created pending已terminal/cancelled。
- Runtime退出。
- MCP endpoint释放。
- no hanging child/thread。
- disposable Workspace只包含期望文件。

## 6. Timing Rules

E2E可以使用短 timeout来证明“call仍 pending”，例如 runner内部 bounded wait。

禁止：

- 用短 sleep推断 expiry。
- 等 5 分钟测试 TTL。
- race-based flaky assertions。

Expiry / resolve race / test-clock属于 Integration确定性验证。

## 7. Evidence

最终输出至少包含：

```text
Runtime binary identity
Runtime PID per generation
MCP endpoint per generation
Control commands
Approval request IDs
Original MCP call identity/result
Filesystem assertions
Activity assertions
E2E-01...E2E-18 PASS/FAIL/BLOCKED
LOCAL_M4_BLACK_BOX_E2E=PASS
```

## 8. Exit Criteria

Local Black-box E2E PASS要求：

- E2E-01～E2E-18全部满足；仅明确允许的 tool-cross-scope case可使用 Integration evidence标记 N/A。
- 至少一次 AllowOnce same-call resume。
- 至少一次 Deny same-call termination。
- 至少一次 AllowForSession + subsequent grant hit。
- 至少一次不同 path grant miss。
- 至少一次 pending request因 Runtime stop结束。
- restart不继承 pending/grants。
- no mock/in-process substitution。
- no browser/Tunnel/External ChatGPT required。

PASS后才进入 Manual Acceptance。
