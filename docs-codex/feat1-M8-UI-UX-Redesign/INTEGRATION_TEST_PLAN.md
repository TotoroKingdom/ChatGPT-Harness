# M8 Integration Test Plan — UI/UX Simplification & Visual Redesign

> Status: DRAFT / READY FOR IMPLEMENTATION QA DESIGN  
> Milestone: M8 — UI/UX Simplification & Visual Redesign  
> Inputs: `UX_FREEZE.md`, `DESIGN_SYSTEM.md`, `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: React presentation/state integration, scoped config mutation, routing/draft guards, page composition, localization, accessibility behavior, and M0–M7 UI regression.  
> Execution gate: run only after M8 G0 is approved and T1–T9 Development is complete.

## 1. Objective

M8 Integration 验证的是 UI 重构后的**组合行为**，不是像素截图，也不是单个 primitive 的 unit test。

核心链路：

```text
Authoritative persisted config / Runtime observation
        ↓
HarnessDataProvider / shared selectors
        ↓
scoped mutation coordinator
        ↓
router / context / draft guard
        ↓
shared design-system components
        ↓
Overview / Tools / Policies / Approvals / Activity / Settings
```

Integration 必须证明：

- UI 不把 desired/configured 冒充 Runtime effective。
- 多个 auto-save / explicit-save mutation 不互相覆盖。
- Runtime/Policy/Approval/Bash 的 authority 仍在现有后端。
- 六个页面共享相同状态、保存、错误、remediation 和导航语义。
- M7 evidence 被重新组织但没有被删除或伪造。
- English / zh-CN 均使用同一行为模型。

## 2. Entry Criteria

执行前必须满足：

- G0 / T0 已由人工完成。
- T1–T9 Development Complete。
- 页面迁移已经完成，不再有计划中的大范围旧 UI 未迁移区域。
- targeted unit/component tests PASS。
- `npm run typecheck` PASS。
- `npm test` PASS 或至少不存在未解释失败。
- `npm run build` PASS。
- 后端若有零修改则无需为 M8 增加后端功能；若出现必要兼容改动，必须先重新确认 M8 scope。

未达到则：

```text
M8 Integration = BLOCKED_BY_IMPLEMENTATION
```

## 3. Integration Boundary

Integration 可以：

- 使用真实 `HarnessDataProvider`。
- 使用真实 router / navigation context。
- 使用真实 config mutation coordinator。
- 使用真实 page components + shared design-system primitives。
- 使用 deterministic fake client / fixtures 注入 authoritative backend states。
- 使用真实 i18n resources。
- 使用 jsdom / React testing environment 验证 keyboard/focus/state behavior。

Integration 不得：

- 用前端重新实现 Policy decision 来生成 fixture。
- 用 UI 推断 approval eligibility / session grant scope。
- 用 snapshot/className 测试代替保存、导航、状态和 accessibility 行为测试。
- 声称已经验证真实 Portable UI 或真实 External ChatGPT。
- 声称肉眼视觉质量 PASS；这些属于 Manual Acceptance。

## 4. Required Integration Cases

### INT-01 — App shell navigation contract

验证一级导航固定为：

```text
Overview
Tools
Policies
Approvals
Activity
Settings
```

期望：

- 当前项只有一套 selected state。
- pending approvals badge 只在有 authoritative pending count 时显示。
- 未读取 approvals 时不编造 `0`。
- global Harness status 可进入 Overview。

### INT-02 — Overall state priority

构造同时存在 restart-required、pending approvals、component blocker 的 state。

期望 Overview attention priority：

```text
Critical / blocker
→ restart action
→ non-blocking notice
```

pending count 独立保留，不能被 restart message覆盖。

### INT-03 — Unknown / stale never becomes Ready

对 Runtime observation：

- loading。
- read failure。
- stale previous generation。
- stopped。

UI不得显示 current Ready/Applied。

### INT-04 — Configured vs effective Tool state

Fixture：

```text
Saved: write enabled = OFF
Running generation: write = ON
Restart required = true
```

期望 Tools row / Inspector：

- 明确显示 saved/configured OFF。
- 明确显示 current Runtime仍 ON。
- Restart required 可见。
- 不把两者压成单一 Disabled/Enabled badge。

### INT-05 — Overview lifecycle actions

验证：

- Ready → Secondary Stop 可达。
- Restart required → Restart 成为当前主要 action。
- starting/stopping 时 action被正确锁定。
- lifecycle failure 有 remediation，不误显示 Ready。

### INT-06 — Shared Tool mutation coordinator

快速连续修改两个不同 Tool：

```text
read_file.enabled = false
write.default = ASK
```

期望两项都持久化，不发生 stale full-config overwrite。

### INT-07 — Same-object fast mutation ordering

同一 Tool 快速：

```text
ON → OFF → ON
```

期望最终 persisted intent = 最后一次用户意图，UI saving状态不重复提交旧 snapshot。

### INT-08 — Mutation failure does not erase later success

第一笔 Tool mutation失败，第二笔独立 Language/Policy mutation成功。

期望：

- 第一笔恢复/标记失败。
- 第二笔成功值保留。
- Retry基于最新 persisted config，不重放旧完整 snapshot。

### INT-09 — Persistence success + refresh failure

config save成功但后续 refresh失败。

期望 UI：

```text
Saved / state not refreshed
```

不得：

- 回滚为未保存。
- 自动重复提交。
- 假装 effective 已收敛。

### INT-10 — Language mutation remains Runtime-neutral

切换 English / 简体中文 / System。

期望：

- UI即时切换。
- language failure回滚 UI preference。
- 不制造 Runtime Restart Required。
- 不覆盖并发成功的 Tool/Workspace mutation。

### INT-11 — Reset serialized with config mutations

Reset 与正在提交的普通 config mutation交错。

期望：

- coordinator不并发提交相互覆盖的 whole-config snapshots。
- Reset期间相关 mutations有明确阻塞/取消语义。
- 不出现“Reset成功后旧保存又把配置写回来”。

### INT-12 — Tools row event separation

Tools row包含：

- row detail trigger。
- Switch。
- segmented control。

期望：

- 点击 Switch不打开 Inspector。
- 点击 segmented不打开 Inspector。
- 点击 row 非控件区域打开 Inspector。
- keyboard focus顺序合理。

### INT-13 — Tool Inspector responsive mode

在 wide/base/minimum width fixtures下：

- wide有足够 Main width时 side-by-side Inspector。
- minimum width使用 overlay Inspector。
- overlay trap focus。
- non-modal Inspector不锁 Main。
- close后恢复合理焦点。

### INT-14 — Shared control interaction states

对 Button / Switch / Segmented / Select / TextField验证：

- disabled。
- saving。
- error。
- focus-visible。
- repeated click/mutation suppression。

### INT-15 — Segmented keyboard semantics

`Allow / Ask / Deny` segmented：

- 单选组语义正确。
- Arrow keys改变 selection/focus。
- Tab不提交 mutation。
- 仅实际值变化提交一次。

### INT-16 — Policy page structure

Policy 页面同时存在：

```text
Tool defaults
User rules
System guardrails
Effective diagnostics
```

期望 System Guardrails始终 read-only；diagnostics 不与 configurable rules混为一层。

### INT-17 — Policy default auto-save shares source with Tools

在 Policies 修改 Tool Default后切到 Tools。

期望 Tools读取同一 persisted value，不出现第二份页面 draft/source。

反向也成立。

### INT-18 — Rule create default safety

Create Rule 打开：

- default Decision = ASK。
- 未点击 Create前不写 config。
- internal ID由 UI生成但默认不作为主要 UI信息。

### INT-19 — Rule ID stability

编辑 existing rule 的 name/reason/decision 后保存。

期望：

- rule ID保持不变。
- matcher未编辑时保持 exact round-trip。
- Retry不重复创建新 rule。

### INT-20 — Bash natural matcher safe subset

验证自然输入：

```text
git status
git status --short
```

能确定性转换为 executable + argvPrefix。

以下不得 fuzzy parse：

- compound command。
- redirection。
- expansion。
- unsupported quoting/escaping。

应提供 Advanced literal-token path。

### INT-21 — Prefix scope explanation

`git status` Bash matcher 的普通层必须说明其是 token prefix、可能覆盖附加参数。

不得显示成 exact full-command match。

### INT-22 — Advanced matcher lossless round-trip

现有 matcher包含：

- empty token。
- whitespace-containing token。
- 非自然输入可表达值。

打开/不修改/保存后必须无损保留。

### INT-23 — Rule dirty leave guard

Rule edit dirty状态下：

- Close。
- Esc。
- 切 row。
- 切 page。

全部走一致 leave guard：Stay / Discard。

saving时不得离开。

### INT-24 — Rule conflict handling

dirty editor 已观察到同一 rule 外部/refresh变化。

期望：

- 保留 draft。
- 标记 conflict。
- 禁止 blind save。
- 提供 Reload / Cancel 或等价安全动作。

### INT-25 — Approval decision-first projection

Pending approval首层显示：

- Tool。
- target/command preview。
- why approval required。
- risk。
- expiry。
- Allow once/session/Deny。

raw IDs / structured evidence后置 Advanced。

### INT-26 — Approval incomplete-preview warning

当 Runtime facts指出：

- preview redacted。
- preview truncated。
- argv redacted。
- unclassified/risk flags。

UI必须在决策前明确提示，不得仅藏在 Advanced。

### INT-27 — Approval eligibility authority

Allow for session 是否可用完全使用 Runtime authoritative field。

UI不得通过 command preview / risk label自行推断 eligibility。

### INT-28 — Approval resolving dedup

对同 request快速双击 Allow once：

- 只提交一次 resolve。
- request进入 resolving。
- 终局由 authoritative refresh决定。

### INT-29 — Approval stale / disconnected behavior

存在旧 pending内容但当前 read失败或 generation改变。

期望：

- 保留旧内容 + stale/disconnected标识。
- 暂停无法确认有效性的决策。
- 不显示 `No pending approvals`。

### INT-30 — Approval → Create Rule handoff

只有 display-safe 且无歧义 facts才能 prefill。

对 truncated/redacted/multi-segment facts：

- 不静默取第一段。
- 不升级为 entire-tool ALLOW。
- 不自动 resolve approval。

Create/Cancel后可返回原 request context并 refresh。

### INT-31 — Activity feed outcome semantics

feed必须区分：

- Policy denied。
- Approval denied。
- Execution failed after allow。
- Tool unavailable。
- Pending / Cancelled。

不能全部显示为同一 Error。

### INT-32 — Activity filter scope

All / Success / Needs attention / Tool filter只作用于当前 loaded bounded snapshot，并有清楚范围说明。

不得暗示在搜索完整历史数据库。

### INT-33 — Activity refresh stale preservation

refresh失败：

- 旧 feed保留。
- 标记 stale/error。
- 不清空成 empty state。

### INT-34 — Activity evidence completeness

Inspector Advanced保留 M7已有 evidence：

- request/event IDs。
- sanitized arguments。
- matched rules。
- approval evidence。
- duration/result summary。

M8不得删除字段，仅允许渐进披露。

### INT-35 — Settings scoped mutations

Workspace、Language、Security、Connection分别执行保存。

期望每次只合并其负责字段，不覆盖其他 section 更新。

### INT-36 — Connection Apply / Cancel

Connection dirty draft：

- Cancel不保存非敏感 config。
- API Key replacement输入被清除。
- Apply只提交当前 Connection section + controlled secret action。
- 不宣称跨 ConfigStore/SecretStore原子事务。

### INT-37 — Credential removal isolation

Credential removal：

- 需要 clean editor。
- 独立 destructive confirmation。
- 完成后不被后续 Connection Cancel恢复。
- UI不从 Configured状态推断 Tunnel Connected。

### INT-38 — First Run readiness truthfulness

Readiness步骤：

- 配置可启动 ≠ Tunnel已连接。
- Start后只有 authoritative Harness Ready才能显示完成。
- failure提供关联配置 remediation，不要求重做整个流程。

### INT-39 — Recovery semantics

分别构造：

- invalid config。
- migration failure。
- unsupported schema。

期望：

- 正确原因/remediation。
- 不把未知错误泛称“数据完全安全”。
- Reset与普通 review actions分离。

### INT-40 — English / zh-CN behavior parity

关键页面/actions在 English / zh-CN：

- 行为完全一致。
- 状态语义不弱化。
- 不翻译 machine IDs/tool names/user-entered policy text。
- 中文长文案不使主要 action不可达。

### INT-41 — Long-content layout fixtures

覆盖：

- 长 Workspace path。
- 长 Bash command。
- 长 Policy reason。
- multiline error。
- raw evidence。

期望：

- 关键状态/controls仍可见。
- path/command有安全展开或换行。
- 不撑宽整个页面。

### INT-42 — Reduced-motion / focus lifecycle

验证：

- reduced-motion下信息不丢失。
- Dialog/Sheet overlay焦点正确。
- close后恢复到 trigger 或合理邻近元素。
- aria-live只播报必要局部反馈。

### INT-43 — No unauthorized backend authority

通过 mocks/spies/diff-oriented tests验证 UI 没有新增：

- local Policy evaluator。
- local approval grant calculator。
- Bash risk classifier。
- Runtime effective-state inference from desired config。

### INT-44 — M0–M7 behavioral regression surface

使用现有 UI/service tests确保：

- Tool toggle语义。
- Policy CRUD。
- Approval resolve actions。
- Bash settings/eligibility presentation。
- i18n persistence。
- migration recovery。
- Activity evidence。

在 M8 重构后仍保持原安全/配置语义。

## 5. Acceptance Mapping

| SPEC Acceptance | Integration Coverage |
| --- | --- |
| AC01 | INT-01–03, 13–15 |
| AC02 | INT-02–05 |
| AC03 | INT-06–11, 17, 35 |
| AC04 | INT-05 |
| AC05 | INT-20–22 |
| AC06 | INT-18–24 |
| AC07 | INT-23–24, 36 |
| AC08 | INT-25–30 |
| AC09 | INT-27–30 |
| AC10 | INT-31–34 |
| AC11 | INT-35–37 |
| AC12 | INT-38–40, 44 |
| AC13 | INT-13–15, 40–42 |
| AC14 | INT-43–44 |
| AC15 | Not Integration; Portable / Manual gate |

## 6. Evidence Format

每个 Integration case记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED
Test file / test name
Authoritative fixture state
User action
Persisted mutation / client call evidence
Rendered state / accessibility assertion
Regression scope
```

不要只记录 screenshot/className。

## 7. Exit Criteria

M8 Integration PASS要求：

- INT-01 ～ INT-44全部 PASS，或有明确非适用说明且由其它层覆盖。
- scoped mutation coordinator 无 stale-overwrite bug。
- configured/effective/restart/stale 状态 truthful。
- Rule / Approval / Credential 安全边界无退化。
- M7 evidence未丢失。
- English/zh-CN behavior parity PASS。
- keyboard/focus/reduced-motion基础行为 PASS。
- M0–M7 UI/service regression PASS。

随后进入 Local Black-box E2E。

## 8. Non-Claims

Integration PASS 不代表：

```text
Portable UI visually accepted
real desktop layout accepted
External ChatGPT accepted
M8 ACCEPTED / FROZEN
V3 ACCEPTED / FROZEN
```
