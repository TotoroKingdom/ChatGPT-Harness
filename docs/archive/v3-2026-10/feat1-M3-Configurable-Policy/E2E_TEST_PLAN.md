# M3 E2E Test Plan — Configurable Policy

> Status: ACCEPTED / FROZEN
> Milestone: M3 — Configurable Policy  
> Owner: Coding Agent / QA Agent  
> Scope: fully local black-box E2E against a real compiled Runtime process. No browser, no External ChatGPT, no Tunnel dependency, no in-process executor substitute.

## 1. Objective

验证最终/接近最终构建中的真实链路：

```text
Runtime process
→ stdin/stdout Control RPC
→ initialize / initialized
→ RuntimeConfigSnapshot.policies
→ real loopback MCP endpoint
→ tools/list / tools/call
→ ExecutionPipeline / PolicyEngine
→ filesystem + Activity evidence
→ stop Runtime
```

E2E重点证明：

- policy config随 Runtime generation生效。
- Tool Default fallback真实影响执行。
- explicit User Rules真实影响执行。
- DENY/ASK/ALLOW通过真实 MCP path表现正确。
- invalid policy snapshot fail closed。
- policy generation immutable，restart/new generation后才收敛。
- M2 Tool Toggle和M1 file semantics无回归。

## 2. Entry Criteria

执行前必须满足：

```text
Development = PASS
Unit Tests = PASS
Integration Tests = PASS
```

并使用实际 release或等价接近最终构建的 Runtime binary。

不得用：

- mock Runtime。
- direct PolicyEngine unit invocation。
- in-process MCP service替代真实 HTTP endpoint。
- browser/Tauri UI人工点击。
- External ChatGPT/Tunnel。

## 3. E2E Harness Requirements

推荐复用/扩展现有 local black-box E2E runner。

runner必须能够：

1. 创建临时 Workspace。
2. spawn真实 Runtime子进程。
3. 通过 stdin/stdout JSON-RPC执行 initialize / initialized。
4. 从 `mcp.endpoint.get`获得真实 loopback endpoint。
5. 通过真实 MCP HTTP发起 `tools/list` / `tools/call`。
6. 通过 Control read获取 effective tools/policies/activity。
7. 直接检查 filesystem bytes。
8. 停止 Runtime并清理临时资源。
9. 启动多个 generation，分别使用不同 policy snapshot/configRevision。

每个 generation使用明确 identity：

```text
A / B / C / ...
```

并记录其 configRevision与 policy config摘要。

## 4. Test Fixtures

Workspace至少准备：

```text
workspace/
├ read.txt          = hello from M3
├ edit.txt          = hello from M3 edit
├ deny-target.txt   = unchanged
└ write-target.txt  = old
```

专用 adjacent outside目录用于 Workspace boundary regression。

Policy fixture建议：

```text
DefaultPolicy:
  read_file ALLOW
  glob ALLOW
  write ALLOW
  edit ALLOW

RuleAllowWrite:
  id=user-write-allow
  matcher=tool(write)
  decision=ALLOW

RuleAskWrite:
  id=user-write-ask
  matcher=tool(write)
  decision=ASK

RuleDenyWrite:
  id=user-write-deny
  matcher=tool(write)
  decision=DENY
```

## 5. Required E2E Cases

### E2E-01 — Baseline generation uses default policy

启动 Generation A：

```text
revision = N
all current tools enabled
all file-tool defaults = ALLOW
userRules = []
```

通过真实 MCP：

- `tools/list`
- `read_file`
- `write`
- `edit`

期望：

- 默认工具按 M2/M1正常暴露/执行。
- write/edit真实文件变化正确。
- Activity包含 policy decision evidence。
- matched rule IDs为空或按 Tool Default evidence约定输出。

### E2E-02 — Tool Default DENY applies in new generation

启动 Generation B：

```text
revision = N+1
write default = DENY
no matching user rules
```

真实 MCP调用 `write`。

期望：

- no file mutation。
- 返回 policy-denied error。
- effective policy observation显示 `write = DENY`。
- reason表示 Tool Default。
- `matchedRuleIds=[]`。
- Activity无 ExecutionCompleted success。

### E2E-03 — Explicit User ALLOW overrides Tool Default DENY

启动 Generation C：

```text
write default = DENY
user-write-allow = ALLOW
```

调用真实 `write`。

期望：

- write成功。
- final bytes正确。
- policy evidence = ALLOW。
- matched IDs包含 `user-write-allow`。
- reason表示 matched user rule(s)。

### E2E-04 — Severity DENY > ASK > ALLOW

启动 Generation D，配置同一 `write` 的三条 matching rules：

```text
ALLOW
ASK
DENY
```

顺序故意不按 severity排列。

调用 write。

期望：

- effective = DENY。
- no mutation。
- matched IDs包含全部 matching enabled rules且顺序稳定。
- 不受 persisted vector顺序影响。

### E2E-05 — ASK is observable but non-executing before M4

启动 Generation E：

```text
write matching user rule = ASK
```

真实 MCP调用 write。

期望：

- 返回 `APPROVAL_REQUIRED`或冻结的等价 M3错误。
- filesystem unchanged。
- Activity显示 PolicyDecision=ASK / ApprovalRequired transition。
- 没有真实 pending Approval被人工解析的要求。
- runner无需调用 approvals.resolve。

### E2E-06 — Invalid enabled rule makes whole snapshot fail closed

启动 Generation F，包含：

```text
valid ALLOW rule
+
enabled invalid rule
```

invalid rule使用能够通过 transport但在 Runtime compile阶段被判 semantic invalid的 fixture。

期望：

- Runtime保持可诊断/可观察（按 SPEC冻结行为）。
- policy-controlled call被 DENY/fail closed。
- valid ALLOW不能被采用。
- filesystem unchanged。
- effective/control reason明确表示 invalid policy snapshot。

### E2E-07 — Disabled invalid semantic rule does not affect active snapshot

启动 Generation G：

- 同一 semantic-invalid rule设置 `enabled=false`。
- 有一个 valid ALLOW rule。

期望：

- Runtime正常使用 active compiled rules。
- ALLOW call成功。
- disabled invalid rule不出现在 matched evidence。

随后 Generation H 将该 rule改为 enabled=true。

期望 snapshot变 invalid、call fail closed。

### E2E-08 — Runtime generation immutability

流程：

```text
Generation I starts with write ALLOW, revision R
→ while I is running, persist desired write DENY, revision R+1
→ do not restart I
→ invoke write through I
```

期望：

- I仍按其 immutable ALLOW snapshot执行。
- active revision仍为 R。
- latest revision = R+1。
- restart-required可从 control/Tauri integration evidence确认；如果 pure Runtime E2E runner无 ConfigStore owner，则由配套 local orchestration test记录。

然后启动 Generation J with R+1：

- write必须 DENY。

### E2E-09 — System Guardrail cannot be overridden by User ALLOW

Generation K配置：

```text
user write ALLOW
```

发起 Workspace 外 mutation。

期望：

- WorkspaceGuard拒绝。
- outside target unchanged/not created。
- user ALLOW不能产生执行。
- Activity显示 boundary rejection，不伪装为 policy success。

### E2E-10 — Tool disabled still dominates ordinary User ALLOW

Generation L：

```text
write.enabled = false
write user rule = ALLOW
```

期望：

- `tools/list`不包含 write。
- cached direct write call → `TOOL_DISABLED`。
- 不进入普通 policy execution成功路径。
- filesystem unchanged。

### E2E-11 — Effective policy Control read is Runtime-owned

在至少三个 generation（ALLOW / DENY / ASK）中调用真实 Control read。

期望返回当前 generation的：

- policy snapshot status。
- per-tool effective decision。
- decision reason。
- matched rule IDs或对应 typed evidence。
- System Rules read-only projection（如最终 method包含）。

验证这些值与实际 MCP执行结果一致。

### E2E-12 — M1/M2 regression through real process

在 default policy generation执行：

- read_file normal。
- glob normal。
- write atomic overwrite。
- edit exact replacement。
- tool disabled stale call。
- outside Workspace mutation rejection。

期望全部保持既有语义。

### E2E-13 — Activity confidentiality

使用唯一 sentinel：

```text
M3_SECRET_CONTENT_SENTINEL
```

执行 ALLOW/DENY/ASK policy cases。

期望 Activity/Audit：

- 包含 tool、decision、matched IDs/reason等安全 evidence。
- 不包含 raw mutation content。
- 不包含 secret。
- 不泄露无必要绝对路径。

### E2E-14 — Clean shutdown across generations

所有 Runtime generation结束后：

- 子进程正常退出。
- MCP endpoint关闭。
- temporary Workspace/runner artifacts可清理。
- 无跨 generation policy state泄漏。

## 6. Black-box Evidence Requirements

每个 case记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED
Runtime binary/build identity
Runtime generation ID
configRevision
policy fixture summary
Control RPC evidence
MCP request/result
filesystem before/after
Activity/effective policy evidence
shutdown result
```

不得将内部 unit-test assertion截图替代黑盒 evidence。

## 7. E2E Exit Criteria

M3 Local Black-box E2E只有以下全部满足才为 PASS：

- E2E-01 ～ E2E-14 PASS。
- 使用真实 compiled Runtime subprocess。
- 使用真实 stdin/stdout Control RPC。
- 使用真实 loopback MCP HTTP。
- 至少覆盖多个 immutable policy generation。
- Tool Default fallback通过真实调用验证。
- User Rule ALLOW/DENY/ASK通过真实调用验证。
- severity与matched evidence真实可观察。
- invalid enabled rule whole-snapshot fail closed。
- System Guardrail不可由 User ALLOW覆盖。
- disabled tool仍保持 M2 capability enforcement。
- Activity confidentiality PASS。
- shutdown/cleanup PASS。

完成后已进入并通过 `MANUAL_ACCEPTANCE_CHECKLIST.md`。

## 8. Final Result / Evidence Retention

```text
Local Black-box E2E result: PASS
Cases: E2E-01–E2E-14 all PASS (`m3_local_black_box_e2e`; `LOCAL_M3_BLACK_BOX_E2E=PASS`)
Evidence: retained from the existing M3 Local Black-box E2E run; no E2E was re-executed during final closeout.
Implementation commit: b49c7a8031edcfe6fdf39acd8b7fd43be34bb08b
Test/evidence commit: 67b2fb81d5d1f3351d713512358493b0b2f62a85
Manual Acceptance: PASS (confirmed by Human Reviewer on 2026-10-02)
```

The existing Local Black-box E2E PASS evidence remains the source for this closeout. The observed `read_file=ASK` behavior difference from Manual Acceptance is recorded as a non-blocking observation; it does not change the M3 result and is not fixed in this closeout.

## 9. Non-Claims

E2E PASS不代表：

```text
M4 human approval works
ASK request can resume
Bash command policy works
External ChatGPT has been exercised
M4 human approval works or has started
```

The Local Black-box E2E PASS result did not by itself establish the human UI gate; that gate subsequently passed and the milestone is now recorded as `M3 — ACCEPTED / FROZEN`.
