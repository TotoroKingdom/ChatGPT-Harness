# M8 Manual Acceptance Checklist — UI/UX Simplification & Visual Redesign

> Status: DRAFT / READY FOR FINAL HUMAN UX REVIEW  
> Milestone: M8 — UI/UX Simplification & Visual Redesign  
> Inputs: `UX_FREEZE.md`, `DESIGN_SYSTEM.md`, `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Owner: Human Reviewer  
> Purpose: M8 唯一需要人工完成的桌面 UX / visual acceptance 层。

## 1. Acceptance Responsibility

```text
Unit / Component        → Coding Agent
Integration             → Coding Agent / Independent QA
Local Black-box E2E     → Coding Agent / Independent QA
Portable build/identity → Coding Agent
Manual UX Acceptance    → Human Reviewer
```

人工验收重点：

- 信息架构是否真的比旧 UI 更直观。
- 高频操作是否减少不必要步骤。
- configured / effective / restart / error 状态是否容易理解。
- Design System 是否统一、克制、无视觉噪音。
- Inspector / Sheet / Dialog / direct manipulation 是否自然。
- 双语言、长内容、minimum/base/wide 窗口是否可用。
- keyboard/focus/error feedback 是否达到桌面工具基本质量。
- M7 evidence、安全提醒与 remediation 是否没有被“简化掉”。

人工不负责：

- 重跑自动化 concurrency/mutation race。
- 重新验证 Runtime Policy 算法。
- M9 External ChatGPT最终 golden paths。

## 2. Entry Criteria

只有以下全部完成才进入人工验收：

```text
G0 Design approved
T1–T9 Development Complete
Integration PASS
Local Black-box E2E PASS
Independent Automated QA PASS
Fresh M8 Portable built from same verified source
```

必须提供：

```text
Production/source HEAD
QA/evidence commit or worktree identity
Portable absolute path
UI executable SHA-256
Runtime SHA-256
frontend/build identity
known non-blocking observations
```

若 QA 后 production UI 有变更，必须重新执行受影响验证并重新构建 Portable。

## 3. Package / Source Identity

- [ ] 当前验收包来自最新已验证 M8 source。
- [ ] 不是 M7 或旧 M8 staging 包。
- [ ] Portable目录独立，不覆盖旧 milestone acceptance package。
- [ ] UI executable / Runtime hash已记录。
- [ ] 启动的实际进程来自该 Portable目录。
- [ ] embedded frontend/custom protocol加载正常。

记录：

```text
Source HEAD:
Relevant diff identity:
Portable path:
UI SHA-256:
Runtime SHA-256:
Reviewer:
Date:
```

## 4. Automated Evidence Review

### Integration

- [ ] INT-01 ～ INT-44 PASS，或有明确其它层覆盖说明。
- [ ] scoped config mutation无 stale overwrite。
- [ ] configured/effective/restart truthfulness PASS。
- [ ] Policy/Approval/credential safety boundaries PASS。
- [ ] English/zh-CN behavior parity PASS。

### Local Black-box E2E

- [ ] real production build identity记录。
- [ ] real ConfigStore / Runtime / Control / MCP使用。
- [ ] real config mutation readback PASS。
- [ ] real Tool / Policy / Approval / Activity regression PASS。
- [ ] Recovery / migration regression PASS。
- [ ] M0–M7 real behavior regression PASS。
- [ ] clean shutdown PASS。

自动化 evidence 缺失时，人工 UI smoke不能替代。

## 5. Global App Shell — AC01

### MA-01 — Navigation clarity

检查 Sidebar：

```text
Workspace
  Overview
  Tools
  Policies
  Approvals
  Activity

System
  Settings
```

- [ ] 六个一级工作域顺序正确。
- [ ] selected item清楚但不过度突出。
- [ ] 没有旧大 Topbar / admin-dashboard navigation残留。
- [ ] pending approval count有数据才显示，不把未知显示为 0。
- [ ] Harness global status点击后能自然进入 Overview。

### MA-02 — Page task clarity

依次打开六页。

每页在不阅读 Advanced evidence 的情况下，能在数秒内回答：

- [ ] 这里主要做什么？
- [ ] 当前最重要状态是什么？
- [ ] 下一步可执行操作是什么？

如果需要先理解 Runtime revision、internal ID、raw JSON 才能操作，则 FAIL。

## 6. Visual System — AC01 / AC13 / AC14

### MA-03 — Overall visual direction

确认实际 UI 符合：

```text
Codex-inspired Minimal Developer Desktop UI
```

- [ ] grayscale / neutral foundation。
- [ ] semantic color克制。
- [ ] thin borders / low decoration。
- [ ] compact desktop density。
- [ ] hierarchy主要由 typography / spacing建立。

不得出现：

- [ ] large hero card。
- [ ] card wall主布局。
- [ ] gradient decoration。
- [ ] glow/neon。
- [ ] persistent heavy shadow。
- [ ] oversized pill controls。
- [ ] colorful badge collection。

### MA-04 — Shared visual grammar

跨页面对比：

- [ ] Button外观/状态一致。
- [ ] Switch一致。
- [ ] Segmented一致。
- [ ] Row一致。
- [ ] Inspector一致。
- [ ] Sheet/Dialog一致。
- [ ] Inline save/error feedback一致。

如果每个页面像不同产品，不能 ACCEPT。

## 7. Overview — AC02 / AC04

### MA-05 — Ready state

Harness Ready时：

- [ ] overall state容易理解。
- [ ] Stop存在但不是不必要的大红主按钮。
- [ ] Runtime/Tunnel/MCP concise status可看见。
- [ ] diagnostics/internal IDs不抢主视觉。

### MA-06 — Restart required

修改一个 Runtime-bound设置但不 restart。

- [ ] UI明确说明配置已保存但当前 Runtime仍使用旧值。
- [ ] Restart成为合理 primary action。
- [ ] 不能把 saved desired显示成 running effective。

### MA-07 — Error / blocker

制造或使用安全测试状态：Runtime stopped/error/unavailable。

- [ ] blocker优先于普通 restart提醒。
- [ ] remediation具体可执行。
- [ ] 不显示 stale Ready。

## 8. Tools — AC02 / AC03 / AC04

### MA-08 — Compact Tool rows

- [ ] 五个 built-in tools可快速扫描。
- [ ] Enabled是直接 Switch。
- [ ] Default Policy是直接 Allow/Ask/Deny segmented。
- [ ] row没有恢复成大型 Card/form。

### MA-09 — Direct manipulation

对一个安全 Tool：

- [ ] Switch一次直接交互即可保存。
- [ ] segmented一次交互即可保存。
- [ ] saving/failed/restart-required反馈就在操作附近。
- [ ] 不需要额外页面级 Save。

### MA-10 — Row vs controls

- [ ] 点击 row空白区域打开 Inspector。
- [ ] 点击 Switch不会意外打开 Inspector。
- [ ] 点击 Policy segmented不会意外打开 Inspector。

### MA-11 — Tool Inspector

- [ ] 首层解释当前状态和为什么。
- [ ] Bash配置/remediation在需要时可找到。
- [ ] Advanced diagnostics后置。
- [ ] 关闭后焦点/位置合理恢复。

## 9. Policies — AC03 / AC05 / AC06 / AC07

### MA-12 — Information structure

页面能明显区分：

```text
Tool defaults
User rules
System guardrails
Diagnostics / Effective policy
```

- [ ] System guardrails只读。
- [ ] invalid policy Critical状态不会藏在折叠区。

### MA-13 — Default policy direct edit

修改一个 Tool Default：

- [ ] segmented auto-save。
- [ ] Tools页显示同一 saved value。
- [ ] 无第二份不一致 draft。

### MA-14 — Create Rule

打开 Create Rule：

- [ ] focused Sheet而不是整页大型 form。
- [ ] default Decision = ASK。
- [ ] internal Rule ID不抢主界面。
- [ ] Cancel不创建 rule。

### MA-15 — Bash matcher scope

创建类似：

```text
git status
```

- [ ] UI明确说明它是 executable + argv prefix。
- [ ] 清楚表达附加参数也可能匹配。
- [ ] 不说成“只匹配完整命令 git status”。

### MA-16 — Unsupported natural command

输入 compound/redirection/expansion等超出安全自然子集的表达式。

- [ ] UI明确拒绝自动转换。
- [ ] 提供 Advanced literal token path。
- [ ] 不 fuzzy guessing。

### MA-17 — Edit Rule dirty guard

修改 existing rule但不 Save，然后分别尝试：

- Close。
- Esc。
- 选另一 rule。
- 切页面。

每条路径：

- [ ] 都出现一致的 Stay/Discard保护。
- [ ] saving过程中不能误关闭。

## 10. Approvals — AC08 / AC09

### MA-18 — Decision-first queue

使用真实或 QA准备的当前 Runtime pending approval。

默认层首先能看见：

- [ ] Tool。
- [ ] target/command。
- [ ] why approval required。
- [ ] risk。
- [ ] expiry。
- [ ] Allow once / Allow for session / Deny。

request ID/raw evidence不应比决策信息更醒目。

### MA-19 — Incomplete / high-risk preview

对 redacted/truncated/unclassified或 risk flag case：

- [ ] 决策前有明显提示。
- [ ] 不需要进入 Advanced才知道预览不完整。
- [ ] 不把未知风险写成“无风险”。

### MA-20 — Session scope wording

- [ ] 说明 grant只在当前 Runtime session内。
- [ ] 说明复用 Runtime认定的相同作用范围。
- [ ] 不把 command preview误写成完整内部 grant key。
- [ ] 不声称 write session grant绑定文件内容 fingerprint。

### MA-21 — Resolve feedback

分别体验 Allow once / Deny；若环境允许再体验 Allow session。

- [ ] resolving防止双击。
- [ ] 成功只表达“决定已提交”，不提前声称 execution success。
- [ ] terminal request不能继续操作。

### MA-22 — Stale/disconnected

当 generation变化或 read失败：

- [ ] 旧内容保留但明确 stale。
- [ ] 不显示虚假的 `No pending approvals`。
- [ ] 无法确认有效性的按钮被安全暂停。

### MA-23 — Create Rule handoff

- [ ] 不自动 approve当前 request。
- [ ] 不静默用截断/多段 preview生成过宽 rule。
- [ ] Cancel/Create后能返回原 request context并刷新状态。

## 11. Activity — AC10

### MA-24 — Feed readability

Activity默认首先回答：

- [ ] What happened。
- [ ] Why。
- [ ] Outcome。
- [ ] Duration when useful。

不应恢复成默认大型审计表。

### MA-25 — Outcome distinction

确认不同 outcome的视觉/文案明显区分：

- Policy denied。
- Approval denied。
- Execution failed。
- Tool unavailable。
- Success。
- Cancelled/Pending（如存在）。

不能全部叫 Error。

### MA-26 — Advanced evidence preservation

打开 Activity Inspector → Advanced。

- [ ] M7 request/event IDs仍存在。
- [ ] sanitized arguments仍存在。
- [ ] matched rules仍存在。
- [ ] approval evidence仍存在。
- [ ] duration/result summary仍存在。

M8只能后置，不能删除。

### MA-27 — Refresh failure

在 QA可控条件下观察 refresh失败：

- [ ] 旧内容保留。
- [ ] stale/read failure明确。
- [ ] 不把 failure显示为空列表。

## 12. Settings — AC03 / AC11 / AC12

### MA-28 — Section structure

应为：

```text
General
Workspace
Connection
Security
Advanced
Danger Zone
```

- [ ] 不再是大型 Card + 页面全局 Save。

### MA-29 — Language

切换：

```text
System
English
简体中文
```

- [ ] UI即时切换。
- [ ] 无 Runtime restart需求。
- [ ] 失败时 preference/UI回滚合理。

### MA-30 — Workspace

Change workspace：

- [ ] picker Cancel不保存。
- [ ] 保存后 configured root更新。
- [ ] running root若仍旧 generation则单独解释。

### MA-31 — Connection Apply / Cancel

- [ ] 多字段 Connection使用显式 Apply/Cancel。
- [ ] Cancel不保存 draft。
- [ ] API Key replacement输入在 Cancel/失败后清除。
- [ ] `Configured` 不等于 `Connected`。
- [ ] partial failure文案不虚构“全部回滚成功”。

### MA-32 — Credential removal

- [ ] 与普通 Connection draft物理分离。
- [ ] 有 destructive confirmation。
- [ ] 完成后 Cancel不能恢复 credential。

### MA-33 — Security

- [ ] Normal / Developer-Test清楚。
- [ ] 进入 Developer-Test有后果确认。
- [ ] saved mode和running effective mode区分。

### MA-34 — Danger Zone

Reset：

- [ ] 与普通 Settings操作分开。
- [ ] 清楚说明实际损失范围。
- [ ] 有确认。
- [ ] 不承诺不存在的 backup/rollback。

## 13. First Run / Recovery — AC12

### MA-35 — First Run simplicity

clean install流程：

```text
Workspace
→ Connection
→ Readiness
→ Start Harness
```

- [ ] 不复制完整 Settings。
- [ ] 不默认展示 schema/revision/policy diagnostics。
- [ ] Back不撤销已经成功保存的独立步骤。

### MA-36 — Readiness truthfulness

Start前：

- [ ] 只能表达“配置已准备，可启动”。
- [ ] 不宣称 Tunnel Connected/Harness Ready。

Start成功后才显示 Ready。

### MA-37 — Recovery clarity

对 invalid/migration/unsupported fixture：

- [ ] 先说明发生什么。
- [ ] 在 contract可证明时说明数据是否安全。
- [ ] 给 safest next action。
- [ ] Technical details可展开。
- [ ] Reset明显后置且 destructive。

## 14. Window / Long-content — AC13

### MA-38 — Minimum window 980×680

- [ ] 六页面主要操作均可达。
- [ ] Inspector使用 overlay而不是压缩 Main到不可用。
- [ ] rows必要时两行布局但不丢 controls。
- [ ] Sheet可滚动且 Save始终可达。

### MA-39 — Base 1280×820

- [ ] 页面密度自然。
- [ ] Inspector在有足够空间时可并排。
- [ ] 不出现大量空白或巨型 card。

### MA-40 — Wide window

- [ ] content不会无限拉伸成难读长行。
- [ ] Tools/Policies/Activity能合理利用桌面宽度。

### MA-41 — Long path

- [ ] 长 Workspace/path不挤掉状态/buttons。
- [ ] 可以安全查看完整值。

### MA-42 — Long Bash command / Approval preview

- [ ] 影响决定的内容可换行/展开。
- [ ] UI自身截断时明确标识。
- [ ] 不能只靠 tooltip隐藏重要风险。

### MA-43 — Multiline error / raw evidence

- [ ] 不覆盖相邻 controls。
- [ ] raw evidence局部滚动/换行，不撑宽整个 page。

## 15. Keyboard / Accessibility — AC13

### MA-44 — Sidebar / pages keyboard path

不用鼠标完成：

- [ ] 导航到多个页面。
- [ ] 进入主要控件。
- [ ] visible focus始终清楚。

### MA-45 — Segmented

- [ ] Arrow keys切换。
- [ ] Tab不会意外保存。
- [ ] focus target清楚。

### MA-46 — Inspector / Sheet / Dialog focus

- [ ] overlay打开时焦点进入正确区域。
- [ ] modal trap focus。
- [ ] Esc/Close遵守 dirty guard。
- [ ] close后焦点回到合理 trigger。

### MA-47 — Reduced motion

系统 Reduced Motion：

- [ ] transition减少/取消。
- [ ] 状态/层级/信息不丢失。

### MA-48 — Contrast / target usability

- [ ] 普通文本可读。
- [ ] disabled与enabled可区分。
- [ ] focus ring可见。
- [ ] Switch真实 click/focus target足够大。

## 16. English / 简体中文 — AC12 / AC13

### MA-49 — Page parity

六页面、First Run、Recovery分别检查 English / 简体中文：

- [ ] 主要操作都存在。
- [ ] 安全语义一致。
- [ ] 中文没有弱化 DENY/ASK/Bash风险。

### MA-50 — Layout under zh-CN

- [ ] button不裁切。
- [ ] status/help text不重叠。
- [ ] Inspector/Sheet仍可用。

### MA-51 — Machine-value preservation

确认以下不被错误翻译：

- [ ] tool names。
- [ ] rule IDs。
- [ ] request IDs。
- [ ] paths/commands。
- [ ] user-authored policy name/reason。

## 17. Security / Truthfulness Regression — AC14

### MA-52 — No fake Runtime truth

抽查 Tools/Overview/Settings：

- [ ] Saved ≠ Running时明确区分。
- [ ] Stale/Unknown不显示 Ready。

### MA-53 — No local Policy/Approval invention

- [ ] UI不显示“根据前端判断”产生的 policy decision。
- [ ] session eligibility来自 Runtime。
- [ ] risk flags来自现有 evidence。

### MA-54 — Bash wording

- [ ] 只说明 working directory为 Workspace。
- [ ] 不声称 Bash被 OS sandbox到 Workspace。

### MA-55 — Evidence privacy

UI不得新增展示：

- [ ] raw API Key。
- [ ] file write content。
- [ ] edit old/new raw content。
- [ ] full Bash environment。
- [ ] 未经现有 contract提供的敏感数据。

## 18. Interaction Budget Review

抽查高频任务：

| Task | Target |
| --- | --- |
| 查看主要系统状态 | 0 额外操作 |
| Tool Enabled toggle | 1 direct interaction |
| Tool Default policy | 1 direct interaction |
| Allow / Deny approval | 1 decision |
| 查看普通对象详情 | 1 interaction |
| Advanced evidence | ≤ 2 interactions |

- [ ] 没有为了“少点击”而隐藏风险/隐式保存危险操作。
- [ ] 完整 Rule / Connection仍保留必要 explicit Save/Apply。

## 19. Portable Acceptance — AC15

### MA-56 — Final package isolation

- [ ] M8 final package位于独立 `artifacts/M8-UI-UX-Redesign/...`。
- [ ] 没覆盖旧 acceptance package。
- [ ] staging/final file list/hash已记录。

### MA-57 — Final package startup

从最终复制后的 Portable启动：

- [ ] UI来自新设计。
- [ ] Runtime来自同包。
- [ ] embedded assets正常。
- [ ] 不依赖开发 server。

## 20. Rejection Conditions

出现任一项即不能 M8 ACCEPT：

- Design System在主要页面明显不一致。
- configured/saved被冒充 running/effective。
- auto-save存在可复现 stale snapshot覆盖。
- Language触发 Runtime restart。
- System Guardrail可编辑。
- Rule natural parser silent broaden/fuzzy guess。
- dirty Rule/Connection可无提示丢失。
- Approval truncated/high-risk信息藏到决策后。
- UI重新计算 session eligibility。
- Create Rule自动批准当前 request。
- Activity删掉 M7 evidence。
- Credential UI声称 ConfigStore + SecretStore完整原子回滚。
- API Key/secret进入 router/log/snapshot/普通展示。
- Recovery未知错误宣称“数据绝对安全”。
- 980×680关键操作不可达。
- keyboard focus不可见或 modal focus失控。
- 中文导致重要安全文案/按钮不可见。
- 大量恢复旧 Card wall / Hero / gradient / glow / form-first UI。
- 为 M8 修改 M0–M7 Runtime/Protocol安全语义且未单独批准。
- 用旧 Portable进行验收。

## 21. Final Decision

全部满足后：

- [ ] G0 Design APPROVED / FROZEN
- [ ] T1–T9 Development PASS
- [ ] Integration PASS
- [ ] Local Black-box E2E PASS
- [ ] Independent Automated QA PASS
- [ ] Fresh Portable verified
- [ ] Manual UX Acceptance PASS
- [ ] M8 — ACCEPTED / FROZEN

最终记录：

```text
M8 Decision:
Reviewer:
Date:
Source HEAD:
Evidence identity:
Portable path:
UI SHA-256:
Runtime SHA-256:
Residual observations:
```

## 22. Boundary After M8

M8 人工验收通过只代表：

```text
M8 — ACCEPTED / FROZEN
```

不代表：

```text
V3 — ACCEPTED / FROZEN
```

随后才进入独立：

```text
M9 — Full V3 External Acceptance
```

M9 必须用真实 External ChatGPT → Tunnel → Harness → Runtime golden paths完成最终 V3人工验收；不得把 M8 Local E2E或 Manual UX Acceptance替代 M9。
