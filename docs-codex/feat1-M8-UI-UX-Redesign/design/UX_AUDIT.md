# M8 UX Audit

> Status: DRAFT FOR DESIGN REVIEW  
> Scope: M7 current UI → M8 interaction redesign input  
> Implementation: Not started

## 1. Audit Objective

本审计不评价 M7 功能正确性。M0–M7 的能力、安全语义和 Runtime 行为继续作为冻结基线。

本审计只回答：

1. 用户进入每个页面真正想完成什么任务？
2. 当前 UI 在哪里增加了不必要的认知负担、点击和错误机会？
3. 哪些信息应该默认展示？
4. 哪些信息应该后置到 Detail / Advanced / Diagnostics？
5. M8 应如何简化交互，而不丢失 M7 的真实状态与安全证据？

## 2. Audit Principles

M8 后续设计必须遵守以下原则。

### UX-01 — Task first

页面按用户任务组织，不按后端 DTO、数据库字段或 Runtime 内部模块组织。

### UX-02 — Progressive disclosure

默认界面只展示完成当前任务所需的信息。低频字段、诊断数据、内部 ID 和 Runtime 证据后置。

### UX-03 — Direct manipulation

能够直接切换或选择的配置，不优先使用“编辑表单 → Save”模式。

### UX-04 — Clear state hierarchy

优先回答：

1. 现在是什么状态？
2. 我能做什么？
3. 为什么？
4. 技术细节是什么？

### UX-05 — Preserve authoritative state

不得为了简化界面而隐藏关键安全差异：

- configured / desired
- Runtime effective
- Restart Required
- Disabled / Denied / Ask / Unavailable

这些差异必须保留，但不要求同时占据首屏同等视觉权重。

### UX-06 — One interaction grammar

同类操作在所有页面使用一致控件：

- Boolean → Switch
- 2–4 个互斥选项 → Segmented Control
- 大量枚举 → Select
- 单项详情 → Detail Panel / Inspector
- 高级信息 → Advanced
- 危险操作 → 独立 Danger action + confirmation
- 更多低频操作 → overflow menu

### UX-07 — Local feedback

操作后的状态变化优先在操作发生位置反馈；Toast / global notice 不能成为唯一反馈。

### UX-08 — Internal terminology is optional knowledge

普通操作不得要求用户先理解 `configRevision`、`generationId`、`matchedRuleIds`、policy snapshot 等内部概念。

### UX-09 — Rows over card wall

需要扫描和比较多个同类对象时优先使用紧凑 row/list，而不是每项一个大 Card。

### UX-10 — Visual restraint

颜色用于语义状态，而不是装饰。层级优先通过 typography、spacing、surface 与 border 表达。

---

## 3. Current Global Findings

### 3.1 Current IA is mostly correct, but page responsibilities are too broad

当前一级页面：

- Overview
- Tools
- Policies
- Activity
- Approvals
- Settings

一级导航本身基本合理，不需要为了“重设计”强行新增页面。

问题主要在页面内部：一个页面同时承担配置、运行状态、诊断、证据和内部实现解释，导致信息权重过于接近。

### 3.2 Current UI is system-model-first

多个页面直接映射 Runtime / Config 模型，例如：

- Desired vs Effective
- Decision / Reason / Readiness
- Rule ID / Matcher Type / argvPrefix
- Runtime generation
- matched rule IDs
- raw sanitized arguments / result evidence

这些信息对诊断和安全审计很重要，但并非所有用户任务都需要首屏看到。

### 3.3 Form-first interaction is overused

当前 Policies / Settings 存在明显的“修改多个字段 → dirty state → Save / Discard”模式。

这会增加：

- 用户记忆负担
- 保存前不确定性
- 页面离开时的状态风险
- 对简单三选一操作的额外交互步骤

### 3.4 Card hierarchy is overused

当前视觉系统以大 Card、渐变、阴影、Status Badge 和 Hero 区为主要层级方式。

对 developer tool 而言，这会：

- 降低扫描密度
- 放大低优先级信息
- 让多个同等级 Card 竞争注意力
- 产生 dashboard 感，而不是 workspace 感

### 3.5 Important feedback exists, but is often global rather than contextual

例如 Save Success / Save Failed / Restart Required 等已经存在，但常以页面级 Notice 出现。

M8 应尽量把反馈放回对应对象：

- Tool row
- Policy row
- Approval request
- Setting field / section

### 3.6 Technical completeness is stronger than decision clarity

M7 UI 已经能展示非常完整的 evidence，但用户经常需要先阅读很多字段，才能回答最简单的问题：

- bash 现在能不能用？
- 这条规则到底允许还是拒绝？
- 为什么刚才失败？
- 这个审批我应该点哪个？

M8 应把“decision clarity”放到首层，把“evidence completeness”放到下一层。

---

## 4. Page Audit — Overview

### Primary user question

> Harness 现在正常吗？如果不正常，我下一步应该做什么？

### Current responsibilities

当前 Overview 同时包含：

- Harness overall status
- Start / Stop / Restart
- Restart Required
- Start eligibility
- Control Plane warning
- Runtime / Tunnel / MCP 状态
- Proxy route / source / URL
- Runtime overview / technical facts

### Problems

1. Hero Card 占据较大视觉空间，但核心任务其实只是状态 + 主操作。
2. Start / Stop / Restart 与多个 Notice 分散在页面不同区域。
3. Runtime / Tunnel / MCP 是重要状态，但 proxy route/source/URL 属于诊断层。
4. 用户需要在多个 Card 之间拼接判断“现在到底能不能用”。

### Keep default-visible

- Harness overall state
- Primary action：Start / Stop / Restart
- Runtime / Tunnel / MCP concise status
- 一个最高优先级 blocker / remediation
- Restart Required（如果存在）

### Move to Detail / Advanced

- Proxy source
- Proxy URL
- route mode
- low-level Runtime identifiers
- verbose component diagnostics

### Target interaction

Overview 应成为“system health + next action”页面，而不是完整 observability dashboard。

### Interaction budget

- 判断 Harness 是否正常：0 click
- Start / Stop：1 click
- 处理 Restart Required：1 click
- 查看某组件为什么异常：≤ 2 clicks

---

## 5. Page Audit — Tools

### Primary user question

> 哪些工具能用？它们当前被允许做什么？

### Current responsibilities

Tools 当前同时展示：

- Tool name / description
- Desired enabled state
- Effective availability
- Policy decision
- Reason
- Readiness
- Technical detail / remediation
- Bash auto-detect
- Bash executable override

### Problems

1. `Desired / Effective / Decision / Reason` 同时作为表格主列，需要用户理解多个内部概念。
2. Checkbox + ON/OFF 文案视觉上不够直接。
3. Bash 的 Tool 状态与 Bash executable 配置分离，用户需要跨区域理解。
4. Runtime 停止与运行时使用两套不同表格结构，增加学习成本。
5. technical detail 默认暴露过多。

### Keep default-visible

每个 Tool row 只保留：

- Tool name
- Enabled
- 简化后的 availability
- Default Policy
- Restart Required / attention indicator（如适用）

推荐首层概念：

```text
read_file   Available   Allow
bash        Disabled    Ask
```

### Move to Detail / Advanced

- desired / effective 的完整对比
- readiness code
- Runtime reason
- technical detail
- capability / internal identifiers
- executable detection details

### Bash detail

Bash 详情中展示：

- Enabled switch
- Default policy segmented control
- Current status
- Detected shell
- executable override
- Advanced diagnostics

### Target interaction

采用 Master / Detail：

```text
Tool list → click row → Detail Panel
```

### Interaction budget

- 判断 Tool 是否可用：0 click
- Enable / disable Tool：1 direct interaction
- 查看 / 修改 default policy：1 direct interaction
- 查看为什么 unavailable：≤ 2 clicks
- 查看 Bash executable details：≤ 2 clicks

---

## 6. Page Audit — Policies

### Primary user question

> 什么规则控制工具执行？我要怎样安全地修改它？

### Current responsibilities

当前页面同时承担：

- Tool Defaults desired configuration
- Runtime effective per-tool decision
- Save dirty state
- Desired User Rules list
- Rule create/edit form
- System Guardrails
- Current-generation User Rules
- Per-tool matched rule IDs / reason / availability

### Problems

这是当前认知负担最高的页面。

1. 配置层、Runtime observation 层、System Rules 层同时首屏出现。
2. Tool Default 的三态选择使用 Select，修改后还需要 Save。
3. Rule Editor 暴露大量实现字段：ID、Matcher Type、argvPrefix JSON、Reason 等。
4. `Desired Rules` 与 `Current Generation Rules` 都在页面展示，用户容易不清楚哪个才是“我要改的”。
5. System Guardrails 很重要，但不应该和日常 User Rules 争夺编辑区注意力。
6. Rule ID 属于稳定内部标识，不应是创建普通规则时的第一决策。

### Keep default-visible

首屏建议只显示：

- Tool defaults
- User rules
- 简明 effective / restart warning
- `+ Add rule`

### Tool defaults interaction

`ALLOW / ASK / DENY` 改为 segmented control：

```text
Allow | Ask | Deny
```

安全且可回滚的单项修改优先 auto-save。

### User rule list

每行只显示：

- Human-readable name
- scope / matcher summary
- decision
- enabled state
- chevron / overflow

### Rule creation model

不要从 schema 开始，而从用户意图开始：

1. 规则作用于什么？
   - Entire tool
   - Bash command
2. 选择 Tool 或 Command
3. Harness 应如何处理？
   - Allow / Ask / Deny
4. 可选名称 / reason
5. Advanced 才显示内部 ID、argv prefix representation 等

### Move to Detail / Advanced

- Rule ID
- Runtime generation
- matched rule IDs
- raw matcher representation
- policy source internals
- snapshot status details
- per-tool diagnostics

### System Guardrails

保留为只读独立区域，但默认 collapsed / secondary。必须清楚告诉用户：

> System Guardrails cannot be overridden.

### Target interaction

Policies 应从“Policy Debugger + Config Editor 混合页”变成“Rule Manager”。

### Interaction budget

- 看出某 Tool 默认策略：0 click
- 修改 Tool Default：1 interaction
- 启用 / 禁用 User Rule：1 interaction
- 编辑现有 Rule：≤ 2 clicks 进入编辑
- 创建普通 Tool Rule：≤ 4 meaningful decisions
- 查看 Runtime exact evidence：≤ 2 clicks from rule/tool detail

---

## 7. Page Audit — Approvals

### Primary user question

> 这个请求是什么、风险在哪里，我应该 Allow 还是 Deny？

### Current responsibilities

Approval Card 当前默认展示：

- request ID
- tool
- status
- risk
- scope / command
- runtime generation
- created / expires
- normalized segments
- risk flags
- sanitized arguments
- matched rules
- policy reason
- Allow once / Allow for session / Deny / Create rule

### Problems

1. evidence 很完整，但决策前需要扫描的信息太多。
2. request/runtime IDs 与 decision-critical facts 视觉权重接近。
3. 四个操作同时出现，主次关系还可以更清晰。
4. Bash evidence 默认展开过深。

### Keep default-visible

- Tool
- Human-readable action / target / command
- Risk level
- Why approval is required
- Allow once
- Allow for session（仅 eligible 时）
- Deny

### Secondary action

- Create rule

应从主要审批按钮组弱化，可放 overflow / secondary link。

### Move to Detail / Advanced

- request ID
- Runtime / generation ID
- normalized segments
- raw sanitized arguments
- exact matched rule IDs
- analyzer flags

### Target interaction

先支持“安全决策”，再支持“证据审计”。

### Interaction budget

- 理解请求是什么：0 click
- Allow once / Deny：1 click
- 查看为什么要求审批：0–1 click
- 查看完整安全证据：1 click
- 从审批创建 Rule：≤ 2 clicks 进入预填 Rule Editor

---

## 8. Page Audit — Activity

### Primary user question

> 刚才发生了什么？为什么成功或失败？

### Current responsibilities

当前 Activity table 展示：

- Event
- Timestamp
- Tool
- Decision
- Outcome
- Detail button

展开后展示：

- request / event ID
- sanitized arguments
- enabled / capability
- policy reason
- matched rules
- approval ID / decision
- result evidence
- duration
- failure category
- Bash evidence

### Problems

1. 当前形式接近审计数据库表。
2. Timestamp 原始值、Event Type 等对普通定位问题帮助有限。
3. Detail 里 raw evidence 的可读性弱。
4. “失败原因 + remediation”没有成为详情的第一层。

### Keep default-visible

采用 activity feed / compact rows：

- Time
- Tool
- Target / command summary
- Decision / Outcome
- Duration（适用时）

例如：

```text
14:32  bash   npm test        Approved · Success · 2.4s
14:31  write  src/config.ts   Allowed · Success
14:29  glob   **/*.rs         Denied by user rule
```

### Detail first layer

- What happened
- Why
- Matched rule summary
- Approval summary
- Remediation if failed

### Advanced evidence

- request ID
- event ID
- raw sanitized arguments
- raw result evidence
- exact matched IDs
- Bash structured evidence

### Interaction budget

- 判断最近一次调用成功/失败：0 click
- 找到失败原因：≤ 1 click
- 找到 remediation：≤ 1 click
- 查看 raw audit evidence：≤ 2 clicks

---

## 9. Page Audit — Settings

### Primary user question

> Harness 的基础环境和偏好怎么配置？

### Current responsibilities

当前 Settings 包含：

- Language
- Config recovery / restart state
- Workspace
- Tunnel ID
- Proxy
- Tunnel client path / detection
- API Key
- Security mode
- Local config path / secret provider
- Save / Discard / Reset

### Problems

1. 高频设置与低频诊断设置处于同一层。
2. Tunnel connection 一组字段较复杂，但没有明显区分“required / advanced”。
3. 全局 dirty + Save / Discard 让简单设置也继承表单复杂度。
4. Reset 与普通 Save 操作处于同一底部动作区。
5. Local data paths 属于诊断 / support 信息，不是普通配置任务。

### Keep default-visible

建议默认一级 Sections：

- General
  - Language
- Workspace
  - Root path
- Connection
  - Tunnel ID
  - API key status
- Security
  - Normal / Developer Test

### Advanced

Connection Advanced：

- Proxy
- Tunnel client override
- bundled/manual executable details

Diagnostics / About：

- Config directory
- Config file
- secret provider
- schema / revision（如需要）

### Save behavior

需要在后续 IA / Wireframe 阶段逐项分类：

- 可立即安全保存 → auto-save
- 多字段有原子一致性要求 → explicit Apply
- 需要 Runtime restart → 保存后 inline `Restart required`

不能简单把所有字段强制改成 auto-save。

### Dangerous actions

Reset configuration 必须离开普通保存区域，进入独立 Danger Zone，并保留 confirmation。

### Interaction budget

- 切换语言：1 interaction
- 切换 Security mode：1 interaction + clear consequence feedback
- 选择 Workspace：≤ 2 interactions
- 判断连接凭据是否已配置：0 click
- 修改 Proxy：Advanced 内 ≤ 2 clicks 进入
- Reset：低频、显式、多步骤确认

---

## 10. Cross-page Interaction Grammar Proposal

以下内容进入下一阶段进一步验证，尚未 `UX FROZEN`。

| Need | Preferred Pattern |
| --- | --- |
| Boolean | Switch |
| ALLOW / ASK / DENY | Segmented Control |
| Small mutually exclusive mode set | Segmented / option group |
| Large enum | Select |
| Scan many same-type objects | Compact list / row |
| Inspect one object | Right Detail Panel / Inspector |
| Create structured object | Sheet / focused dialog |
| Advanced technical info | Collapsible Advanced |
| Low-frequency actions | Overflow menu |
| Dangerous action | Isolated danger action + confirm |
| Safe single-field config | Prefer auto-save |
| Atomic multi-field config | Explicit Apply |
| Runtime not yet converged | Inline Restart Required |
| Success / error | Inline local feedback; optional toast |

---

## 11. Proposed Information Priority

M8 所有页面尽量遵循三层结构：

```text
Layer 1 — Status / decision
What is happening?

Layer 2 — Action / explanation
What can I do? Why?

Layer 3 — Evidence / diagnostics
How exactly did the system reach this state?
```

这三层不能反转。

---

## 12. Proposed Visual Direction

当前只冻结方向，不冻结 token。

目标：`Codex-inspired Minimal Developer Desktop UI`

原则：

- Neutral grayscale foundation
- Low decoration
- Thin borders
- Restrained radius
- Minimal shadow
- Sparse semantic color
- Rows over large cards
- Compact but readable density
- Typography + spacing create hierarchy
- Developer workspace rather than admin dashboard
- Avoid decorative gradients / glowing hero elements as primary identity

具体颜色、字号、spacing、radius、surface、hover、focus、dark/light token 在 `DESIGN_SYSTEM.md` 阶段决定。

---

## 13. What M8 Must Not Do

- 不为了 UI 简单而合并不同的安全状态。
- 不把 desired state 冒充为 Runtime effective state。
- 不隐藏 Restart Required。
- 不在前端重新计算 Runtime authoritative policy decision。
- 不删除 M7 Activity / Approval evidence，只改变默认信息层级。
- 不修改 Policy precedence。
- 不修改 Approval semantics。
- 不修改 Bash enforcement。
- 不修改 config migration / schema semantics。
- 不把现有页面 JSX 逐一“换皮”当作完成重构。

---

## 14. Audit Conclusion

M7 UI 的主要问题不是缺少功能，而是：

> technical completeness > task clarity

M8 的目标应反转为：

> task clarity first, technical evidence on demand

下一阶段应生成 `INFORMATION_ARCHITECTURE.md`，冻结：

1. 一级导航是否保持六页；
2. 每页唯一 Primary Task；
3. 默认信息 / Detail / Advanced 的分层；
4. Global App Shell 的状态与主操作位置；
5. Master / Detail、Sheet、Advanced 的统一导航行为。

在 IA 完成前，不进入 Wireframe，不生成 SPEC / PLAN / TASKS。
