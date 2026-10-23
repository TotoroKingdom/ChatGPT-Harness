# M5 E2E Test Plan — Local Black-box Controlled Bash

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M5 — Controlled Bash  
> Owner: Coding Agent / QA Agent  
> Requirement: fully automated local black-box test against real compiled Runtime + real Git Bash.  
> Forbidden substitutions: mock shell, in-process BashRuntime-only test, browser/manual UI, External ChatGPT.

## 1. Objective

从 Runtime公开边界证明：

```text
real release Runtime subprocess
→ real stdin/stdout Control
→ real loopback MCP
→ real bash.exe
→ real Policy / Approval
→ real Windows process tree
→ real Activity / filesystem/process evidence
```

Local Black-box E2E必须证明的不是“能调用一个 command parser”，而是：

> **真实 Git Bash process只有在 mode/tool/readiness/policy/approval允许后才执行，并且 timeout/output/cancellation/shutdown确实清理其完整 owned process tree。**

## 2. Entry Criteria

- T1–T6 development complete。
- Unit PASS。
- Integration PASS。
- release Runtime build PASS。
- Windows test host。
- supported Git for Windows Bash可用。

如果机器没有可验证的 Git Bash：

```text
M5 E2E = BLOCKED_BY_ENVIRONMENT
```

不得用 mock bash或 PowerShell冒充 PASS。

## 3. Runner Architecture

推荐：

```text
runtime/tests/m5_e2e.rs
```

或等价自动化 runner。

Runner启动真实：

```text
target/release/chatgpt-harness-runtime.exe
```

Control使用真实 stdin/stdout JSONL：

```text
initialize
initialized
runtime.status
mcp.endpoint.get
tools.list
policies.list/effective read
approvals.list
approvals.resolve
activity.list
runtime.stop
```

MCP使用真实 loopback HTTP：

```text
initialize
tools/list
tools/call bash
```

Runner可在 disposable Workspace预置 harmless test fixture scripts/files。

## 4. Required Runtime Generations

至少使用：

### Generation A — Normal Mode

```text
mode=Normal
bash.enabled=true
valid Git Bash
bash default=ASK
```

证明 mode hard gate。

### Generation B — Developer/Test ASK

```text
mode=DeveloperTest
bash.enabled=true
Git Bash Ready
bash default=ASK
```

用于 Approval / grant。

### Generation C — Developer/Test policy/limit fixtures

用于 ALLOW/DENY、timeout/output/concurrency，避免 session grant污染其它 case。

Restart必须创建真实新 Runtime process/generation。

## 5. Required E2E Cases

### E2E-01 — Real Runtime boot / Git Bash readiness

启动 release Runtime。

验证：

- Control initialize成功。
- Runtime reports Bash readiness。
- selected executable是 Runtime validated Git Bash。
- MCP endpoint真实可连接。

记录 Runtime PID / generation / bash executable identity。

### E2E-02 — Normal Mode never exposes Bash

Generation A：

- Runtime effective Bash = UnavailableByMode。
- MCP `tools/list`不含 bash。
- stale/direct `tools/call bash`被明确拒绝。
- 无 Bash process spawn。

### E2E-03 — Disabled Bash not exposed

Developer/Test generation with `bash.enabled=false`：

- tools/list不含 bash。
- stale call = TOOL_DISABLED。
- no approval。
- no process。

### E2E-04 — Unavailable executable not exposed

使用 invalid manual override创建 generation。

验证：

- Runtime Bash unavailable reason存在。
- tools/list不含 bash。
- direct call = BASH_UNAVAILABLE/equivalent。
- 不 auto fallback。
- no spawn。

### E2E-05 — Developer/Test + ready + default ASK exposes Bash

Generation B：

- effective = AvailableWithApproval。
- MCP tools/list包含 bash。
- MCP annotations符合 high-risk/destructive/open-world语义。

### E2E-06 — Classified ASK becomes real Pending

发起真实：

```text
bash: printf 'm5-allow-once\n'
```

暂不 resolve。

验证：

- 原 MCP tools/call仍 Pending。
- approvals.list出现 bash request。
- scope/command preview/matched policy/risk/cwd存在。
- no output/process completion yet。

### E2E-07 — AllowOnce resumes same Bash call

resolve E2E-06：

```text
AllowOnce
```

验证：

- 原 MCP call成功完成。
- 不发第二次 call。
- stdout包含预期文本。
- exitCode=0。
- approval→execution同 request ID串联。
- AllowOnce不留下 reusable grant。

### E2E-08 — Deny prevents shell execution

新 ASK command使用唯一 filesystem sentinel，例如：

```text
printf denied > denied-sentinel.txt
```

确认 Pending后 Deny。

验证：

- 原 call = APPROVAL_DENIED。
- sentinel不存在。
- no ExecutionCompleted success。

### E2E-09 — Classified AllowForSession

发起一个 deterministic classified command并选择 AllowForSession。

验证 current call成功，并建立 session grant。

### E2E-10 — Exact normalized command grant hit

再次发与 E2E-09 normalized shape完全相同的 command：

- 无新 Pending。
- call成功。

### E2E-11 — Different normalized command grant miss

只改变 argv/command shape。

期望：

- 新 Pending。
- 不被 prior grant放行。

随后 Deny清理。

### E2E-12 — Unclassified command requires ASK even under broad ALLOW

启动/configure generation：

```text
ToolMatcher bash = ALLOW
```

调用含 command substitution / nested shell / equivalent unclassified fixture。

期望：

- 仍 Pending ASK。
- Activity有 UnclassifiedHighRisk evidence。
- 不自动 spawn before approval。

### E2E-13 — Unclassified approval has no session option

对 E2E-12 Pending：

- approval projection显示 AllowForSession ineligible。
- 强制 Control resolve AllowForSession被拒绝且不生成 grant。
- AllowOnce可继续原 request。

### E2E-14 — Explicit command DENY prevents spawn

配置 BashCommand rule DENY一个 harmless sentinel command。

direct call：

- POLICY_DENIED。
- no Pending。
- no sentinel。
- no Bash child execution。

### E2E-15 — Compound severity

配置：

```text
safe segment → ALLOW
second segment → ASK
```

调用 compound command。

期望 whole call Pending ASK。

另一个含 DENY segment的 compound：

- whole command DENY。
- first segment也不得被部分执行。

### E2E-16 — cwd is real Workspace

执行 approved/allowed：

```text
pwd
```

输出对应 Git Bash representation of active Workspace。

验证 execution cwd源自 Runtime，不是 MCP参数。

### E2E-17 — Nonzero exit is structured result

执行允许的：

```text
exit 7
```

或等价 fixture。

期望 MCP正常返回 BashOutput：

```text
exitCode=7
```

不是 transport/internal failure。

### E2E-18 — Concurrency limit

Generation C使两个 classified harmless long-running commands无需人工 Approval或先建立适当 grant。

启动第一条保持 active，再发第二条。

期望：

```text
second → BASH_CONCURRENCY_LIMIT
```

第一条随后被正常取消/结束。

### E2E-19 — Output limit terminates real process

执行 deterministic high-output fixture超过 256 KiB。

验证：

- BASH_OUTPUT_LIMIT。
- process tree终止。
- captured evidence bounded。
- Runtime仍可继续处理 Control/MCP。

### E2E-20 — Timeout terminates real process tree

使用 fixture产生：

- parent bash invocation。
- descendant/child长期运行。
- child PID写入 Workspace test pid file。

让真实 30s production timeout触发。

验证：

- BASH_TIMEOUT。
- parent结束。
- recorded descendant PID不再存活。
- Runtime仍 Ready/可诊断。
- unrelated runner process存活。

不得只验证 MCP call超时。

### E2E-21 — Runtime stop kills active Bash tree

启动真实长运行 Bash，并记录 descendant PID。

在 execution active时通过真实 Control停止 Runtime。

验证：

- Runtime process退出。
- Bash parent/descendant全部退出。
- MCP request不永久 hang。
- endpoint释放。

### E2E-22 — Restart clears Bash session grants

Generation B建立 Bash session grant并成功命中。

stop B，启动新的同配置 generation。

同 exact command：

- 再次 Pending。
- 不继承 old grant。

### E2E-23 — Activity evidence / confidentiality

从真实 Activity读取至少：

- approved Bash。
- denied Bash。
- output-limit/timeout之一。
- session grant hit。

确认：

- request ID关联。
- normalized classification/risk。
- policy/matched IDs。
- approval。
- execution/limit result。
- duration。

同时确认没有：

- full environment。
- secret。
- unbounded stdout/stderr。

### E2E-24 — File-tool regressions through real MCP

同一 final Runtime build通过真实 MCP执行至少：

- read_file。
- glob。
- write。
- edit。

验证 M5没有破坏 M0–M4既有链路。

### E2E-25 — Clean final shutdown

最终：

- no Pending approvals。
- no active Bash process tree。
- Runtime clean exit。
- MCP endpoint释放。
- no orphan child PID。
- disposable Workspace只保留预期 evidence files。

## 6. Black-box Process Evidence Rules

涉及 tree termination必须记录：

```text
Runtime PID
Bash parent PID when observable
Descendant PID from fixture
termination trigger
post-trigger process existence check
MCP result/error
Activity result
```

只有 parent退出不足以判定 PASS。

## 7. Timing Rules

- Approval pending可使用 bounded short observation window证明 call未完成。
- timeout case应验证 production hard timeout path。
- 不用随机 sleep制造 race。
- concurrency用明确同步 fixture/barrier。
- process cleanup使用 bounded polling确认 PID消失。

## 8. E2E Evidence Summary

最终 runner输出至少：

```text
Runtime binary identity
Git Bash executable identity
Generation/PID mapping
MCP endpoint per generation
E2E-01 ... E2E-25 PASS/FAIL/BLOCKED
Process-tree evidence
Approval request IDs
Activity evidence
LOCAL_M5_BLACK_BOX_E2E=PASS
```

## 9. Exit Criteria

Local Black-box E2E PASS要求：

- 使用真实 release Runtime。
- 使用真实 Git Bash。
- Normal Mode gate PASS。
- ASK same-request Approval PASS。
- classified session grant exact-match PASS。
- unclassified ASK floor + no reusable grant PASS。
- explicit DENY no-spawn PASS。
- real concurrency/output/timeout enforcement PASS。
- timeout和Runtime stop都证明 descendant cleanup。
- Activity confidentiality PASS。
- M0–M4 real MCP regression PASS。
- no mock shell/in-process substitution。

PASS后才进入 Manual Acceptance。
