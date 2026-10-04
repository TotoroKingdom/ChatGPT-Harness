# M8 — UI/UX Simplification & Visual Redesign

> Status: DESIGN DISCOVERY / NO IMPLEMENTATION

## Goal

在不改变 M0–M7 已冻结产品能力、安全语义、Protocol 和 Runtime 行为的前提下，重新设计 ChatGPT-Harness 的信息架构、交互模型与视觉系统，使高频操作更直观、点击更少、认知负担更低，并形成一致的 Developer Workspace UI。

M8 的目标不是给现有页面“换皮”，也不是逐页一比一重绘，而是先重新确认用户任务与交互模型，再实现视觉系统。

## Product Direction

目标体验：

- 简洁、克制、开发者工具导向
- Codex-inspired Minimal Developer Desktop UI
- Workspace-first，而不是 Admin Dashboard-first
- Direct manipulation 优先于 form-first interaction
- Rows / lists 优先于 card wall
- Progressive disclosure：默认只显示完成当前任务所需的信息
- 系统内部复杂度保留，但后置到 Detail / Advanced / Diagnostics
- 配置状态与 Runtime effective state 必须继续清晰区分

## Hard Boundary

M8 不重新设计以下已冻结语义：

- Tool enable / disable 语义
- Policy precedence 与 `ALLOW / ASK / DENY`
- System Guardrails
- Human Approval 安全语义
- Bash enforcement 与风险边界
- WorkspaceGuard
- Config schema / migration 语义
- Runtime generation / configRevision 生效模型
- Activity / Audit evidence 语义
- Tunnel / MCP / Runtime 协议边界

只有在 UI 表达确有必要时，允许增加纯展示层 adapter / selector / view-model；不得借 UI 重构修改 Runtime authoritative behavior。

## Design Process

M8 使用设计门禁，而不是直接进入实现：

```text
Phase A — UX Audit
        ↓
Phase B — Information Architecture
        ↓
Phase C — Interaction Wireframes
        ↓
      UX FROZEN
        ↓
Phase D — Visual Design System
        ↓
    DESIGN FROZEN
        ↓
SPEC / PLAN / TASKS
        ↓
Implementation
        ↓
Automated Validation
        ↓
Manual UX Acceptance
```

在 `UX FROZEN` 前，不进行大规模 production UI 重写。

在 `DESIGN FROZEN` 前，不允许各页面自行发明视觉规则或组件风格。

## Design Documents

当前设计输入：

- `UX_AUDIT.md` — 当前 UI 的任务、问题、复杂度与简化方向
- `INFORMATION_ARCHITECTURE.md` — 页面职责、导航与信息层级
- `WIREFRAMES.md` — Part 1：App Shell、Tools、Policies
- `WIREFRAMES_PART2.md` — Part 2：Overview、Approvals、Activity、Settings、First Run / Recovery、Cross-page states
- `DESIGN_SYSTEM.md` — 尚未创建；UX 冻结后进入

设计冻结后再生成：

- `SPEC.md`
- `PLAN.md`
- `TASKS.md`
- `INTEGRATION_TEST_PLAN.md`
- `LOCAL_BLACK_BOX_E2E.md`
- `MANUAL_ACCEPTANCE_CHECKLIST.md`

## Current Design State

```text
UX_AUDIT        DRAFT COMPLETE
IA              DRAFT COMPLETE
WIREFRAMES      FULL DRAFT COMPLETE — HUMAN REVIEW REQUIRED
DESIGN_SYSTEM   NOT STARTED
SPEC            NOT STARTED
PLAN            NOT STARTED
TASKS           NOT STARTED
IMPLEMENTATION  NOT STARTED
```

### Wireframe Part 1

已明确候选交互：

- 移除大尺寸全局 Topbar，改为稳定 Sidebar + page-local header。
- Tools 使用 compact rows；Enabled Switch 与 `Allow / Ask / Deny` 直接行内操作。
- Tool Detail Inspector 主要承担状态解释、Bash 特有配置和 Advanced diagnostics。
- 独立 Tool / Policy 配置采用 auto-save，并在对象附近显示 Restart Required / save failure。
- Policies Tool Defaults 使用 segmented control，不再使用 Select + 全页 Save。
- User Rule enable/disable 为直接操作；Create/Edit Rule 作为完整对象仍显式 Save。
- Create Rule 使用 focused Sheet，并按用户任务组织输入，不直接暴露 internal Rule ID 或 argvPrefix JSON。
- System Guardrails 与 Effective Policy 默认降级到 secondary / collapsed；安全异常必须自动提升 attention。

### Wireframe Part 2

已明确候选交互：

- Overview 只回答 Harness 是否可用、当前最重要操作和核心组件状态，不再默认展示完整 Runtime metadata。
- Approvals 改为 decision-first queue；审批证据后置到详情与 Advanced。
- `Allow once / Allow for session / Deny` 直接完成决策，`Create rule` 降为 secondary workflow。
- Activity 从 audit table 改为 activity feed，并明确区分 policy deny 与 execution failure。
- Settings 改为 section/list 模型，不再使用多个大 Card + 一个全局 Save。
- Settings 按字段语义分别使用 auto-save、explicit Apply、confirmation。
- First Run 简化为 `Workspace → Connection → Ready → Start`。
- Recovery 优先解释问题、数据安全和安全恢复动作，技术细节默认折叠。
- Restart Required、Disabled、Unavailable、Denied 等状态在全产品使用统一语义。

以上内容仍属于人工评审中的设计提案，不是实现授权。

## Core UX Rule

> 默认界面只展示用户完成当前任务所需要的信息；低频配置、诊断信息和内部实现细节通过 Detail / Advanced 渐进披露。

现有 JSX / CSS 只作为“已有功能与必须保留状态”的证据，不作为 M8 新页面结构的设计模板。

## Next Gate

Part 1 + Part 2 Wireframe 人工确认后，将：

```text
UX_AUDIT
+ IA
+ WIREFRAMES
→ UX FROZEN
```

随后进入 `DESIGN_SYSTEM.md`，冻结视觉语言与组件系统。

## Milestone Exit

M8 完成并人工验收后，再进入：

`M9 — Full V3 External Acceptance`
