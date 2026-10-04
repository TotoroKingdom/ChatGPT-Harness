# M8 UX Freeze Record

> Status: UX FROZEN  
> Human approval: confirmed in design review  
> Implementation: not yet authorized

## 1. Purpose

本文件是 M8 交互层的正式冻结记录。

以下设计输入已经完成审计、信息架构和低保真交互评审，并作为后续视觉设计、SPEC、PLAN、TASKS 与实现的 UX 基线：

- `UX_AUDIT.md`
- `INFORMATION_ARCHITECTURE.md`
- `WIREFRAMES.md`
- `WIREFRAMES_PART2.md`

这些文件中仍出现的 `DRAFT` / `FOR REVIEW` header 属于设计阶段原始状态说明；自本文件生效后，其经人工确认的交互决策由本 Freeze Record 提升为 `UX FROZEN`。

---

## 2. Frozen UX Direction

M8 的交互模型正式冻结为：

- Desktop Workspace，而不是 Admin Dashboard。
- 稳定 Sidebar + page-local header + contextual Inspector。
- Rows / lists 优先于 card wall。
- Direct manipulation 优先于 form-first interaction。
- 默认只展示完成当前任务所必要的信息。
- Detail / Advanced / Diagnostics 承担低频技术信息和 evidence。
- 简单独立 mutation 使用 auto-save + inline feedback。
- 完整对象 Create/Edit 使用显式 Save / Apply。
- Dangerous mutation 使用 confirmation，并与普通操作物理分离。
- Runtime-bound mutation 必须准确区分 configured state 与 running effective state。
- `Restart required` 是明确的跨页面语义，不得把 saved desired state 冒充为已生效。

---

## 3. Frozen Navigation

一级工作域保持：

```text
Overview
Tools
Policies
Approvals
Activity
Settings
```

Sidebar 分组：

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

不新增一级 Diagnostics、Runtime、Tunnel 或 Audit 页面。

---

## 4. Frozen Page Contracts

### Overview

只回答：

> Harness 当前是否可工作？如果不能，我下一步做什么？

默认展示 overall state、一个 context-aware primary action、Runtime/Tunnel/MCP concise state、最高优先级 blocker/remediation。

### Tools

默认使用 compact rows。

冻结直接操作：

- Enabled → Switch
- Default Policy → `Allow / Ask / Deny` segmented control
- row 非控件区域 → Tool Inspector

Tool Inspector 负责状态解释、Bash 特有配置和 Advanced diagnostics。

### Policies

默认结构：

```text
Tool defaults
User rules
System guardrails
Diagnostics / Effective policy
```

冻结交互：

- Tool Default → segmented auto-save
- User Rule enable/disable → Switch auto-save
- Create Rule → focused Sheet
- Edit complete Rule → explicit Save
- internal Rule ID 默认隐藏并由 UI 管理
- Bash matcher 默认使用自然 command 输入，确定性转换为 `executable + argvPrefix`
- 无法安全转换时明确拒绝，不 fuzzy guessing

### Approvals

冻结为 decision-first queue。

默认优先展示：

- Tool
- target / command
- risk
- why approval is required
- `Allow once / Allow for session / Deny`

Evidence 后置到 Detail / Advanced。

`Create rule` 是 secondary workflow，不与主审批按钮竞争。

### Activity

冻结为 activity feed，而不是默认 audit table。

默认首先回答：

- What happened
- Why
- Outcome
- Duration when useful

Raw evidence、IDs、structured arguments 后置到 Advanced。

### Settings

冻结为 section/list 模型：

```text
General
Workspace
Connection
Security
Advanced
Danger Zone
```

不再使用多个大型 Cards + 一个页面级全局 Save 作为主要交互模式。

### First Run / Recovery

First Run 主路径冻结为：

```text
Workspace
→ Connection
→ Readiness
→ Start Harness
```

Recovery 必须先解释：

- what is wrong
- whether data is safe
- safest next action

schema / migration internals 默认折叠。

---

## 5. Frozen Interaction Grammar

| Intent | Pattern |
| --- | --- |
| Boolean | Switch |
| `Allow / Ask / Deny` | Segmented control |
| Many enum choices | Select |
| Inspect existing object | Right Inspector |
| Create focused multi-field object | Sheet / focused dialog |
| Safe independent mutation | Auto-save |
| Atomic multi-field mutation | Explicit Save / Apply |
| Dangerous mutation | Confirmation |
| Secondary actions | `...` / quiet action |
| Advanced diagnostics | Collapsible Advanced |
| Local save/error feedback | Inline near mutation |
| Runtime mismatch | Inline `Restart required` |

---

## 6. Frozen Safety / Truthfulness Invariants

视觉简化不得改变以下表达真实性：

1. configured / desired 与 Runtime effective state 必须可区分。
2. Policy precedence、System Guardrails 与 fail-closed 状态不得被 UI 弱化或重算。
3. Pending Approval、Denied、Disabled、Unavailable、Execution Failure 必须语义区分。
4. M7 evidence 不删除，只允许渐进披露。
5. remediation 必须保留并优先使用用户可理解语言。
6. UI 不得为了“看起来更简单”隐藏 Critical / Action Required 状态。

---

## 7. Change Control

从本文件生效起：

- 后续 `DESIGN_SYSTEM.md` 只能决定视觉语言、组件外观、spacing、typography、surface、states 与 motion。
- 视觉阶段不得重新发明页面信息架构或高频操作路径。
- 如果发现视觉实现必须改变已冻结 UX，必须显式记录为 `UX CHANGE REQUEST`，说明原因、影响和替代方案，并重新人工确认。
- Codex 不得在实现阶段以“更易实现”为理由恢复旧 table/card/form-first 交互。

---

## 8. Gate Result

```text
UX_AUDIT        FROZEN
IA              FROZEN
WIREFRAMES      FROZEN
UX              FROZEN
DESIGN_SYSTEM   NEXT
SPEC            BLOCKED UNTIL DESIGN FROZEN
PLAN            BLOCKED UNTIL DESIGN FROZEN
TASKS           BLOCKED UNTIL DESIGN FROZEN
IMPLEMENTATION  NOT AUTHORIZED
```

M8 现在正式进入 Phase D — Visual Design System。