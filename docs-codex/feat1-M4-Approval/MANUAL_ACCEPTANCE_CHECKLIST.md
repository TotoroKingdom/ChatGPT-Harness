# M4 Manual Acceptance Checklist — Human Approval Workflow

> Status: DRAFT / READY FOR FINAL HUMAN REVIEW  
> Milestone: M4 — Human Approval Workflow  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Owner: Human Reviewer  
> Purpose: M4唯一需要人工介入的最终验收层。

## 1. Acceptance Responsibility

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

人工验收重点：

- review自动化 evidence。
- Approval Center真实 UX。
- External ChatGPT → Tunnel → Harness → Runtime 的 ASK approval用户路径。
- Allow once / session / deny是否符合用户预期。
- Create Rule是否明确与当前 approval decision分离。
- 最终 ACCEPTED / FROZEN 决策。

人工不负责手工制造 concurrency race / expiry race / TOCTOU，这些应由自动化完成。

## 2. Entry Criteria

只有以下全部 PASS 才进入人工验收：

```text
Development
Unit
Integration
Local Black-box E2E
```

必须提供：

- production implementation commit。
- automated test/evidence commit。
- final build identity。
- Integration summary。
- E2E summary，尤其是 same-call pending/resume evidence。

若 E2E 仍采用“批准后重新 tools/call”，Manual Acceptance不得补救，必须退回开发。

## 3. Build / Source Identity

- [ ] 记录 M4 production implementation commit。
- [ ] 记录 automated evidence commit。
- [ ] 记录实际人工验收 build / Portable identity。
- [ ] 确认之后没有未经验证的 Runtime approval production code变更。

Evidence：

```text
Production commit:
Evidence commit:
Acceptance build:
Reviewer:
Date:
```

## 4. Automated Evidence Review

### 4.1 Development / Unit

- [ ] T1 Protocol / ApprovalManager PASS。
- [ ] T2 Async Pipeline / same-request resume PASS。
- [ ] T3 Session Grants / Control / Shutdown PASS。
- [ ] T4 Approval Center / Create Rule PASS。
- [ ] T5 Regression PASS。

### 4.2 Integration

- [ ] INT-01 ～ INT-30 PASS。
- [ ] ASK = AvailableWithApproval。
- [ ] exact request identity isolation PASS。
- [ ] expiry/race/cancel PASS。
- [ ] narrow session grant PASS。
- [ ] grant cannot override DENY/System/disabled PASS。
- [ ] post-approval revalidation PASS。
- [ ] M0–M3 regression PASS。

### 4.3 Local Black-box E2E

- [ ] real release/final Runtime subprocess used。
- [ ] real stdin/stdout Control RPC used。
- [ ] real loopback MCP used。
- [ ] original MCP call held Pending while Control remained responsive。
- [ ] AllowOnce resumed original call。
- [ ] Deny terminated original call。
- [ ] AllowForSession + same-scope grant hit PASS。
- [ ] different-path grant miss PASS。
- [ ] Runtime stop cancelled pending call。
- [ ] restart cleared grant/pending state。
- [ ] Activity confidentiality PASS。
- [ ] `LOCAL_M4_BLACK_BOX_E2E=PASS`。

## 5. Approval Center UX

启动最终 Harness build，进入 Approval Center。

### MA-01 — Empty state

在无 Pending approval时：

- [ ] 页面明确显示无待审批请求。
- [ ] 不显示伪造/mock approval。
- [ ] Runtime stopped/not ready状态可理解。

### MA-02 — ASK tool visible to external ChatGPT

将一个安全、可逆的 file tool（建议 write 到临时 Workspace文件）设置为 ASK并重启 Harness。

在 External ChatGPT：

- [ ] tool仍可发现/调用。
- [ ] ChatGPT发起调用后不会立即得到普通 POLICY_DENIED。
- [ ] Harness Approval Center出现 Pending。

记录：

```text
Tool:
Target:
Approval request ID:
```

## 6. Allow Once User Path

### MA-03 — Inspect approval details

Pending card至少能看见：

- [ ] tool。
- [ ] normalized target/scope。
- [ ] sanitized arguments。
- [ ] matched policy IDs。
- [ ] reason。
- [ ] risk。
- [ ] created/expiry。
- [ ] 不显示 raw secret。
- [ ] write/edit raw content未被不必要展示。

### MA-04 — Allow once

点击 Allow once：

- [ ] Pending状态结束。
- [ ] External ChatGPT原请求继续并收到成功结果。
- [ ] 不需要用户在 ChatGPT重新发一遍同样请求。
- [ ] filesystem结果正确。
- [ ] Activity可看到同 request ID下 approval→execution链路。

### MA-05 — Allow once is one-shot

再次发同一 scope请求：

- [ ] 如果 Policy仍 ASK，应再次出现 Pending。
- [ ] 不因之前 Allow once自动执行。

## 7. Deny User Path

### MA-06 — Deny

发起另一个安全测试 target的 ASK request。

点击 Deny：

- [ ] External ChatGPT原请求以明确拒绝结束。
- [ ] target无 mutation。
- [ ] Approval Center显示 Denied/terminal状态。
- [ ] Activity无 ExecutionCompleted success。

## 8. Allow For Session User Path

### MA-07 — Allow for session

发起：

```text
write session-approval.txt
```

点击 Allow for session：

- [ ] 当前原请求成功。
- [ ] UI明确这是 current Runtime/session scope，不是永久 Policy rule。

### MA-08 — Same scope grant

External ChatGPT再次调用同一 tool、同一 target：

- [ ] 不再弹 approval。
- [ ] 请求正常执行。

### MA-09 — Different target does not inherit

调用同 tool不同 target：

- [ ] 再次产生 Pending。
- [ ] 不被上一条 session grant静默放行。

这是 M4 session grant安全边界的人工关键检查。

## 9. Runtime Restart Boundary

### MA-10 — Restart clears session grant

重启 Harness。

再次调用 MA-07同 tool + same target：

- [ ] 必须重新 ASK/Pending。
- [ ] 不继承上一个 Runtime generation的 session grant。

## 10. Create Rule UX

### MA-11 — Create Rule does not auto-approve

产生一个 Pending approval，点击 Create Rule：

- [ ] 打开/跳转 Policies editor draft。
- [ ] rule matcher/tool预填合理。
- [ ] 当前 approval仍 Pending。
- [ ] 没有自动 AllowOnce/AllowForSession。
- [ ] 没有自动保存。

### MA-12 — Scope widening warning

若当前 approval是：

```text
write one-file.txt
```

而 draft是 tool-level `write` Policy：

- [ ] UI明确提示 persistent rule比 exact approval scope更宽。
- [ ] 用户必须显式确认并保存。

### MA-13 — Saved rule follows M3 generation semantics

若人工选择保存 rule：

- [ ] configRevision变化。
- [ ] 当前 Runtime不会被假装 hot reload。
- [ ] Restart Required显示。
- [ ] 下个 Runtime generation才应用。

当前 Pending request仍需单独 resolve。

## 11. Expiry / Terminal UX Review

不要求人工等待 5 分钟。

review自动化 evidence并在 UI已有 terminal fixture/自然出现时确认：

- [ ] Expired状态不可再次 resolve。
- [ ] Cancelled状态不可再次 resolve。
- [ ] stale approval错误不会显示 false success。

若 UI无法在人工流程方便制造 Expired，可仅以 Integration自动化 evidence作为 acceptance依据。

## 12. Activity / Audit Review

选择至少一条：

- AllowOnce。
- Deny。
- AllowForSession。

确认：

- [ ] request ID贯穿。
- [ ] Policy ASK可见。
- [ ] approval decision可见。
- [ ] execution结果可见。
- [ ] session grant match有足够诊断信息。
- [ ] raw mutation content/secret未泄露。

## 13. Regression Smoke

人工只做高价值 smoke，不重复自动化矩阵：

- [ ] 一个默认 ALLOW tool仍无需 approval。
- [ ] 一个 explicit DENY tool不能通过 Approval Center绕过。
- [ ] disabled tool不会产生可批准的 Pending。
- [ ] M1 write/edit最终文件行为正常。

## 14. Rejection Conditions

出现任一项即不能 ACCEPT：

- ASK tool仍不暴露。
- ASK立即返回并要求重新 call。
- AllowOnce能复用到第二个 request。
- AllowForSession对整个 tool全局放行而非冻结的窄 scope。
- different target错误命中 session grant。
- explicit DENY/System/disabled能被 approval绕过。
- restart后 session grant仍存在。
- Runtime stop存在 hanging pending request。
- Create Rule自动批准或自动永久保存。
- raw secret/content泄露。
- Integration/E2E缺失。

## 15. Final Decision

全部满足后：

- [ ] Development PASS
- [ ] Unit PASS
- [ ] Integration PASS
- [ ] Local Black-box E2E PASS
- [ ] Manual Acceptance PASS
- [ ] M4 — ACCEPTED / FROZEN

最终记录：

```text
M4 Decision:
Reviewer:
Date:
Production commit:
Acceptance build:
Notes:
```

冻结后，M5才开始 Controlled Bash；不得在 M4 acceptance后继续偷偷扩展 approval scope/semantics而不重新验证。
