# M5 Manual Acceptance Checklist — Controlled Bash

> Status: DRAFT / READY FOR FINAL HUMAN REVIEW  
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

- [ ] 记录 production implementation commit。
- [ ] 记录 automated evidence commit。
- [ ] 记录人工验收 Portable/build。
- [ ] 记录 Runtime binary identity。
- [ ] 记录 Runtime实际选择的 Git Bash executable。
- [ ] 确认之后没有未经验证的 Bash production change。

记录：

```text
Production commit:
Evidence commit:
Acceptance build:
Runtime:
Git Bash:
Reviewer:
Date:
```

## 4. Automated Evidence Review

### Development

- [ ] T1 Bash Config / Discovery / Registry PASS。
- [ ] T2 Command Analyzer / Bash Policy PASS。
- [ ] T3 Bash Runtime / Process Tree / Limits PASS。
- [ ] T4 Pipeline / MCP / Approval / Evidence PASS。
- [ ] T5 UI PASS。
- [ ] T6 Regression PASS。

### Integration

- [ ] INT-01 ～ INT-44 PASS。
- [ ] unclassified ASK floor PASS。
- [ ] explicit DENY no-spawn PASS。
- [ ] exact classified session grant PASS。
- [ ] timeout/output/cancel process-tree cleanup PASS。
- [ ] Runtime shutdown descendant cleanup PASS。

### Local Black-box E2E

- [ ] real release Runtime。
- [ ] real Git Bash。
- [ ] real MCP/Control。
- [ ] Normal Mode hard gate。
- [ ] same-request Bash approval。
- [ ] unclassified session-ineligible。
- [ ] concurrency/output/timeout real enforcement。
- [ ] descendant PID cleanup证据。
- [ ] M0–M4 real MCP regression。
- [ ] `LOCAL_M5_BLACK_BOX_E2E=PASS`。

## 5. Tools UI — Bash Availability

### MA-01 — Normal Mode

在 Normal Mode：

- [ ] Bash row可见但明确显示 unavailable by mode，或按产品设计提供等价清晰状态。
- [ ] 即使 Desired Enabled=ON，也不能显示 Ready/Available。
- [ ] 文案不暗示 Policy/Approval可绕过 Mode。

### MA-02 — Developer/Test + default OFF

切换 Developer/Test并按现有 generation规则重启。

- [ ] Bash默认 OFF。
- [ ] effective为 Disabled。
- [ ] 不出现在 External ChatGPT tool availability中。

### MA-03 — Enable Bash / restart semantics

将 Bash ON：

- [ ] desired立即保存。
- [ ] 运行中的旧 Runtime不被伪装成已开启。
- [ ] Restart Required出现。
- [ ] restart后 effective收敛。

## 6. Git Bash Discovery / Readiness UX

### MA-04 — Auto Detect

启用 Auto Detect，启动新 generation。

- [ ] Runtime识别支持的 Git Bash。
- [ ] UI展示 Runtime effective readiness。
- [ ] 用户能看到足够的 executable/remediation信息。

### MA-05 — Manual override

选择明确的 Git Bash `bash.exe`：

- [ ] config保存。
- [ ] Restart Required。
- [ ] restart后 Runtime验证该 exact path。

### MA-06 — Invalid manual path

配置无效 manual path。

- [ ] Runtime显示 Unavailable。
- [ ] UI提供 remediation。
- [ ] 不静默 fallback成另一个 auto-detected Bash。
- [ ] Bash不暴露。

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

- [ ] 可发现 `bash`。
- [ ] metadata/用户体验没有把它描述为 read-only或 sandboxed。

### MA-08 — Safe classified command enters Approval

请求一个 harmless、可观察的命令，例如：

```text
printf 'm5-manual-ok\n'
```

- [ ] ChatGPT实际调用 bash。
- [ ] Approval Center出现 Pending。
- [ ] command尚未执行完成。

### MA-09 — Approval detail quality

Bash Approval至少能看到：

- [ ] sanitized command。
- [ ] normalized command/segments。
- [ ] Workspace working directory。
- [ ] matched policies。
- [ ] High risk。
- [ ] reason。
- [ ] created/expiry。
- [ ] session eligibility。

不得显示 secret/full environment。

### MA-10 — Allow once

点击 Allow once：

- [ ] External ChatGPT原 request继续。
- [ ] 不需要重发 command。
- [ ] stdout/result正确。
- [ ] Activity链路完整。

再次发相同命令：

- [ ] 仍应重新 ASK，证明 one-shot。

## 8. Deny Path

### MA-11 — Deny prevents side effect

请求一个只会创建 harmless Workspace sentinel的 command，例如：

```text
printf denied > m5-denied.txt
```

点击 Deny：

- [ ] External ChatGPT原 request明确结束为 denied。
- [ ] sentinel不存在。
- [ ] Activity没有 execution success。

## 9. Allow For Session

### MA-12 — Classified session grant

发一个 classified deterministic command，点击 Allow for session。

- [ ] 当前 request成功。
- [ ] UI明确 session-only / current Runtime generation。

### MA-13 — Exact command re-use

再次发 exact same normalized command：

- [ ] 不再出现 approval。
- [ ] 正常执行。

### MA-14 — Changed command does not inherit

改变 argv/command shape：

- [ ] 再次出现 approval。
- [ ] 不被 previous grant静默放行。

## 10. Unclassified High-risk UX

### MA-15 — Dynamic expression remains ASK

在存在宽泛 Bash ALLOW rule的受控测试配置下，发一个无破坏性的 unclassified fixture，例如 command substitution输出固定文本。

- [ ] 仍进入 ASK。
- [ ] UI明确说明 analyzer不能可靠分类/高风险。

### MA-16 — No reusable session grant

对该 Pending：

- [ ] Allow for session不可用。
- [ ] UI给出原因。
- [ ] Allow once仍可用。
- [ ] Deny仍可用。

人工不需要使用危险 command。

## 11. BashCommand Policy Editor

### MA-17 — Create command-specific rule

创建类似：

```text
executable = git
argvPrefix = ["status"]
decision = ALLOW
```

- [ ] matcher UI是 token/prefix语义，不是 regex。
- [ ] save后 configRevision变化。
- [ ] 当前 generation不伪装成已应用。
- [ ] restart后规则生效。

### MA-18 — Specific rule behavior

External ChatGPT调用匹配的 harmless command：

- [ ] 按 effective policy执行。

调用不匹配 command：

- [ ] 不错误命中。

### MA-19 — DENY command rule

创建 harmless但可验证的 DENY command fixture。

- [ ] 调用后无 Pending Approval。
- [ ] command没有执行。
- [ ] Activity显示 Policy DENY。

## 12. No-Sandbox Product Boundary

### MA-20 — Wording review

检查 Tools / Policies / Approval / Settings相关 Bash文案：

- [ ] 明确 working directory是 Workspace。
- [ ] 未声明“Bash只能访问 Workspace”。
- [ ] 未声明完整 filesystem/network sandbox。
- [ ] Developer/Test Mode没有被描述成安全隔离模式。

这是 M5 acceptance gate。

## 13. Runtime Limits Evidence Review

这些不要求人工重复执行危险/长时测试，只 review Local Black-box E2E evidence：

- [ ] command length limit在 spawn前拒绝。
- [ ] concurrency limit真实生效。
- [ ] output limit终止 process tree。
- [ ] timeout终止 parent + descendants。
- [ ] Runtime shutdown终止 parent + descendants。
- [ ] no orphan PID。
- [ ] unrelated process不受影响。

## 14. Activity / Audit Review

选择至少：

- approved Bash。
- denied Bash。
- session grant hit。
- timeout/output evidence中的一条自动化记录。

确认：

- [ ] request/tool。
- [ ] classification/risk。
- [ ] Policy/matched rules。
- [ ] Approval信息。
- [ ] execution result/exit code或limit category。
- [ ] duration。
- [ ] 不含 full environment。
- [ ] 不含 unbounded stdout/stderr。
- [ ] 不含 secret。

## 15. M0–M4 Regression Smoke

人工只做少量高价值 smoke：

- [ ] read_file仍正常。
- [ ] glob仍正常。
- [ ] write/edit仍正常。
- [ ] file ASK approval仍能 same-request resume。
- [ ] Tool OFF仍阻止 stale call。

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

- [ ] Development PASS
- [ ] Unit PASS
- [ ] Integration PASS
- [ ] Local Black-box E2E PASS
- [ ] Manual Acceptance PASS
- [ ] M5 — ACCEPTED / FROZEN

记录：

```text
M5 Decision:
Reviewer:
Date:
Production commit:
Evidence commit:
Acceptance build:
Git Bash:
Notes:
```

冻结后进入 M6 Chinese / English UI；不得在 M5 freeze后继续改变 Bash authorization、limits、session grant或process ownership语义而不重新验证。
