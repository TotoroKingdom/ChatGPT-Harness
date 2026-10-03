# M5 Manual Acceptance Checklist — Controlled Bash

> Status: ACCEPTED / FROZEN
> Milestone: M5 — Controlled Bash  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Owner: Human Reviewer  
> Purpose: M5唯一需要人工介入的最终验收层。

## 1. Acceptance Responsibility

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

人工验收重点：

- review真实 Git Bash / process-tree自动化 evidence。
- Tools UI Bash配置/readiness。
- Policies UI BashCommand规则。
- Approval Center Bash风险信息。
- External ChatGPT → Tunnel → Runtime → Bash真实用户路径。
- 确认产品没有虚假 OS sandbox承诺。

人工不负责手工等待 timeout、制造 output flood或检查 race；这些由自动化完成。

## 2. Entry Criteria

只有以下全部 PASS：

```text
Development
Unit
Integration
Local Black-box E2E
```

才进入人工验收。

必须提供：

```text
Production commit
Automated evidence commit
Acceptance build / Portable identity
Git Bash executable identity
Integration summary
LOCAL_M5_BLACK_BOX_E2E=PASS
```

若真实 Git Bash E2E未执行，不能靠 UI smoke补成 M5 ACCEPTED。

## 3. Build / Source Identity

- [x] 记录 production implementation commit。
- [x] 记录 automated evidence commit。
- [x] 记录人工验收 Portable/build。
- [x] 记录 Runtime binary identity。
- [x] 记录 Runtime实际选择的 Git Bash executable。
- [x] 确认之后没有未经验证的 Bash production change。

记录：

```text
Production commit: df586c489d7d49776694555987a0f5285e139979
Evidence commit: bc3c4a20f9ab2a699ba4790287771145579dac15
Acceptance build: artifacts/M5-Human-Approval/ChatGPT-Harness_0.1.1_x64-portable
Runtime: runtime/chatgpt-harness-runtime.exe; SHA-256 31404FD9EF3F3B2966B004BB2008BE015A0B18F80C7B95EF2E53413FBAB3708F
Git Bash: C:\Program Files\Git\bin\bash.exe; SHA-256 04FB7475D5F43BFCCD31A64587BD123AEC81E3863B2AC6B8AD08F8EF38E847B3
Reviewer: Human Reviewer（用户确认 Manual Acceptance 已通过）
Date: 2026-10-03
```

## 4. Automated Evidence Review

### Development

- [x] T1 Bash Config / Discovery / Registry PASS。
- [x] T2 Command Analyzer / Bash Policy PASS。
- [x] T3 Bash Runtime / Process Tree / Limits PASS。
- [x] T4 Pipeline / MCP / Approval / Evidence PASS。
- [x] T5 UI PASS。
- [x] T6 Regression PASS。

### Integration

- [x] INT-01 ～ INT-44 PASS。
- [x] unclassified ASK floor PASS。
- [x] explicit DENY no-spawn PASS。
- [x] exact classified session grant PASS。
- [x] timeout/output/cancel process-tree cleanup PASS。
- [x] Runtime shutdown descendant cleanup PASS。

### Local Black-box E2E

- [x] real release Runtime。
- [x] real Git Bash。
- [x] real MCP/Control。
- [x] Normal Mode hard gate。
- [x] same-request Bash approval。
- [x] unclassified session-ineligible。
- [x] concurrency/output/timeout real enforcement。
- [x] descendant PID cleanup证据。
- [x] M0–M4 real MCP regression。
- [x] `LOCAL_M5_BLACK_BOX_E2E=PASS`。

## 5. Tools UI — Bash Availability

### MA-01 — Normal Mode

在 Normal Mode：

- [x] Bash row可见但明确显示 unavailable by mode，或按产品设计提供等价清晰状态。
- [x] 即使 Desired Enabled=ON，也不能显示 Ready/Available。
- [x] 文案不暗示 Policy/Approval可绕过 Mode。

### MA-02 — Developer/Test + default OFF

切换 Developer/Test并按现有 generation规则重启。

- [x] Bash默认 OFF。
- [x] effective为 Disabled。
- [x] 不出现在 External ChatGPT tool availability中。

### MA-03 — Enable Bash / restart semantics

将 Bash ON：

- [x] desired立即保存。
- [x] 运行中的旧 Runtime不被伪装成已开启。
- [x] Restart Required出现。
- [x] restart后 effective收敛。

## 6. Git Bash Discovery / Readiness UX

### MA-04 — Auto Detect

启用 Auto Detect，启动新 generation。

- [x] Runtime识别支持的 Git Bash。
- [x] UI展示 Runtime effective readiness。
- [x] 用户能看到足够的 executable/remediation信息。

### MA-05 — Manual override

选择明确的 Git Bash `bash.exe`：

- [x] config保存。
- [x] Restart Required。
- [x] restart后 Runtime验证该 exact path。

### MA-06 — Invalid manual path

配置无效 manual path。

- [x] Runtime显示 Unavailable。
- [x] UI提供 remediation。
- [x] 不静默 fallback成另一个 auto-detected Bash。
- [x] Bash不暴露。

恢复到有效配置后继续验收。

## 7. External ChatGPT — Default ASK Golden Path

确保：

```text
Developer/Test
bash enabled
Git Bash Ready
Bash default ASK
```

### MA-07 — Bash discoverability

External ChatGPT：

- [x] 可发现 `bash`。
- [x] metadata/用户体验没有把它描述为 read-only或 sandboxed。

### MA-08 — Safe classified command enters Approval

请求一个 harmless、可观察的命令，例如：

```text
printf 'm5-manual-ok\n'
```

- [x] ChatGPT实际调用 bash。
- [x] Approval Center出现 Pending。
- [x] command尚未执行完成。

### MA-09 — Approval detail quality

Bash Approval至少能看到：

- [x] sanitized command。
- [x] normalized command/segments。
- [x] Workspace working directory。
- [x] matched policies。
- [x] High risk。
- [x] reason。
- [x] created/expiry。
- [x] session eligibility。

不得显示 secret/full environment。

### MA-10 — Allow once

点击 Allow once：

- [x] External ChatGPT原 request继续。
- [x] 不需要重发 command。
- [x] stdout/result正确。
- [x] Activity链路完整。

再次发相同命令：

- [x] 仍应重新 ASK，证明 one-shot。

## 8. Deny Path

### MA-11 — Deny prevents side effect

请求一个只会创建 harmless Workspace sentinel的 command，例如：

```text
printf denied > m5-denied.txt
```

点击 Deny：

- [x] External ChatGPT原 request明确结束为 denied。
- [x] sentinel不存在。
- [x] Activity没有 execution success。

## 9. Allow For Session

### MA-12 — Classified session grant

发一个 classified deterministic command，点击 Allow for session。

- [x] 当前 request成功。
- [x] UI明确 session-only / current Runtime generation。

### MA-13 — Exact command re-use

再次发 exact same normalized command：

- [x] 不再出现 approval。
- [x] 正常执行。

### MA-14 — Changed command does not inherit

改变 argv/command shape：

- [x] 再次出现 approval。
- [x] 不被 previous grant静默放行。

## 10. Unclassified High-risk UX

### MA-15 — Dynamic expression remains ASK

在存在宽泛 Bash ALLOW rule的受控测试配置下，发一个无破坏性的 unclassified fixture，例如 command substitution输出固定文本。

- [x] 仍进入 ASK。
- [x] UI明确说明 analyzer不能可靠分类/高风险。

### MA-16 — No reusable session grant

对该 Pending：

- [x] Allow for session不可用。
- [x] UI给出原因。
- [x] Allow once仍可用。
- [x] Deny仍可用。

人工不需要使用危险 command。

## 11. BashCommand Policy Editor

### MA-17 — Create command-specific rule

创建类似：

```text
executable = git
argvPrefix = ["status"]
decision = ALLOW
```

- [x] matcher UI是 token/prefix语义，不是 regex。
- [x] save后 configRevision变化。
- [x] 当前 generation不伪装成已应用。
- [x] restart后规则生效。

### MA-18 — Specific rule behavior

External ChatGPT调用匹配的 harmless command：

- [x] 按 effective policy执行。

调用不匹配 command：

- [x] 不错误命中。

### MA-19 — DENY command rule

创建 harmless但可验证的 DENY command fixture。

- [x] 调用后无 Pending Approval。
- [x] command没有执行。
- [x] Activity显示 Policy DENY。

## 12. No-Sandbox Product Boundary

### MA-20 — Wording review

检查 Tools / Policies / Approval / Settings相关 Bash文案：

- [x] 明确 working directory是 Workspace。
- [x] 未声明“Bash只能访问 Workspace”。
- [x] 未声明完整 filesystem/network sandbox。
- [x] Developer/Test Mode没有被描述成安全隔离模式。

这是 M5 acceptance gate。

## 13. Runtime Limits Evidence Review

这些不要求人工重复执行危险/长时测试，只 review Local Black-box E2E evidence：

- [x] command length limit在 spawn前拒绝。
- [x] concurrency limit真实生效。
- [x] output limit终止 process tree。
- [x] timeout终止 parent + descendants。
- [x] Runtime shutdown终止 parent + descendants。
- [x] no orphan PID。
- [x] unrelated process不受影响。

## 14. Activity / Audit Review

选择至少：

- approved Bash。
- denied Bash。
- session grant hit。
- timeout/output evidence中的一条自动化记录。

确认：

- [x] request/tool。
- [x] classification/risk。
- [x] Policy/matched rules。
- [x] Approval信息。
- [x] execution result/exit code或limit category。
- [x] duration。
- [x] 不含 full environment。
- [x] 不含 unbounded stdout/stderr。
- [x] 不含 secret。

## 15. M0–M4 Regression Smoke

人工只做少量高价值 smoke：

- [x] read_file仍正常。
- [x] glob仍正常。
- [x] write/edit仍正常。
- [x] file ASK approval仍能 same-request resume。
- [x] Tool OFF仍阻止 stale call。

不要求重跑旧里程碑完整自动化矩阵。

## 16. Rejection Conditions

任一项出现即不能 ACCEPT：

- Normal Mode暴露/执行 Bash。
- invalid manual path静默 fallback。
- bash默认 ON。
- Bash默认 policy变成 ALLOW。
- unclassified high-risk在 broad ALLOW下自动执行。
- command DENY仍 spawn shell。
- AllowOnce需要客户端重发 command。
- unclassified command可以创建 reusable session grant。
- session grant宽到整个 Bash tool。
- timeout/output/shutdown只 kill parent、留下 descendant。
- cwd被客户端任意指定。
- UI宣称不存在的 OS sandbox。
- Runtime limits只存在文档、没有真实 enforcement。
- Local Black-box E2E使用 mock shell。
- Activity泄露 secret/full env/unbounded output。

## 17. Final Decision

全部通过后：

- [x] Development PASS
- [x] Unit PASS
- [x] Integration PASS
- [x] Local Black-box E2E PASS
- [x] Manual Acceptance PASS
- [x] M5 — ACCEPTED / FROZEN

记录：

```text
M5 Decision: ACCEPTED / FROZEN
Reviewer: Human Reviewer（用户确认 Manual Acceptance 已通过）
Date: 2026-10-03
Production commit: df586c489d7d49776694555987a0f5285e139979
Evidence commit: bc3c4a20f9ab2a699ba4790287771145579dac15
Acceptance build: artifacts/M5-Human-Approval/ChatGPT-Harness_0.1.1_x64-portable
Git Bash: C:\Program Files\Git\bin\bash.exe; SHA-256 04FB7475D5F43BFCCD31A64587BD123AEC81E3863B2AC6B8AD08F8EF38E847B3
Notes: Integration / Local Black-box E2E / Manual Acceptance 均 PASS；无未关闭 FAIL / BLOCKED。详见 FINAL_ACCEPTANCE.md。
```

M5 已冻结；本次收口不启动 M6 Chinese / English UI。不得在 M5 freeze后继续改变 Bash authorization、limits、session grant或process ownership语义而不重新验证。
