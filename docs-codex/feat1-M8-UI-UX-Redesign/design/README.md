# M8 — UI/UX Simplification & Visual Redesign

> Status: UX FROZEN / DESIGN SYSTEM REVIEW / NO IMPLEMENTATION

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

```text
Phase A — UX Audit
        ↓
Phase B — Information Architecture
        ↓
Phase C — Interaction Wireframes
        ↓
      UX FROZEN                 ← COMPLETE
        ↓
Phase D — Visual Design System  ← CURRENT
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

`UX FROZEN` 后，视觉阶段不得重新发明页面信息架构或高频交互路径。

在 `DESIGN FROZEN` 前，不允许开始 production UI rewrite。

## Design Documents

- `UX_AUDIT.md` — 当前 UI 的任务、问题、复杂度与简化方向
- `INFORMATION_ARCHITECTURE.md` — 页面职责、导航与信息层级
- `WIREFRAMES.md` — App Shell、Tools、Policies
- `WIREFRAMES_PART2.md` — Overview、Approvals、Activity、Settings、First Run / Recovery、Cross-page states
- `UX_FREEZE.md` — 人工确认后的 UX 冻结记录；后续 UX 权威门禁
- `DESIGN_SYSTEM.md` — 当前视觉系统评审稿

设计冻结后再生成：

- `SPEC.md`
- `PLAN.md`
- `TASKS.md`
- `INTEGRATION_TEST_PLAN.md`
- `LOCAL_BLACK_BOX_E2E.md`
- `MANUAL_ACCEPTANCE_CHECKLIST.md`

## Frozen UX Summary

已经冻结：

- 稳定 Sidebar + page-local header + contextual Inspector。
- Tools 使用 compact rows；Enabled Switch 与 `Allow / Ask / Deny` 行内直接操作。
- 独立 Tool / Policy 配置采用 auto-save + inline feedback。
- configured / desired 与 Runtime effective state 必须准确区分；`Restart required` 不得伪装已生效。
- Policies Tool Defaults 使用 segmented control；User Rule enable/disable 为直接操作。
- Create/Edit Rule 作为完整对象使用 focused Sheet / Inspector edit + explicit Save。
- internal Rule ID、argvPrefix JSON、generation/revision/raw evidence 默认后置。
- Approvals 使用 decision-first queue。
- Activity 使用 activity feed，而不是默认 audit table。
- Settings 使用 `General / Workspace / Connection / Security / Advanced / Danger Zone` section/list 模型。
- First Run 简化为 `Workspace → Connection → Readiness → Start Harness`。
- Recovery 优先展示问题、数据安全和安全恢复动作。

详细冻结声明见 `UX_FREEZE.md`。

## Current Visual Direction

`DESIGN_SYSTEM.md` 当前提出：

- dark-first，不在 M8 新增 Theme capability
- grayscale / monochrome 为主，semantic color 为辅
- neutral high-contrast Primary action
- system UI font + mono，不引入自定义字体文件
- 4px spacing base，compact desktop density
- restrained 6–10px radius
- thin borders / divider / row hierarchy
- no radial gradient / glowing orb / default gradient Card
- little persistent shadow
- Badge 只用于少量 compact semantic label
- 统一 Button / Switch / Segmented / Row / Inspector / Sheet visual grammar
- no decorative motion / glow

这些视觉内容尚未冻结，需人工评审 `DESIGN_SYSTEM.md`。

## Current Design State

```text
UX_AUDIT        FROZEN
IA              FROZEN
WIREFRAMES      FROZEN
UX              FROZEN
DESIGN_SYSTEM   DRAFT FOR HUMAN REVIEW
DESIGN          NOT FROZEN
SPEC            BLOCKED UNTIL DESIGN FROZEN
PLAN            BLOCKED UNTIL DESIGN FROZEN
TASKS           BLOCKED UNTIL DESIGN FROZEN
IMPLEMENTATION  NOT AUTHORIZED
```

## Core UX Rule

> 默认界面只展示用户完成当前任务所需要的信息；低频配置、诊断信息和内部实现细节通过 Detail / Advanced 渐进披露。

现有 JSX / CSS 只作为“已有功能与必须保留状态”的证据，不作为 M8 新页面结构的设计模板。

## Next Gate

人工评审并确认 `DESIGN_SYSTEM.md` 后：

```text
DESIGN_SYSTEM
→ DESIGN FROZEN
```

只有 `DESIGN FROZEN` 后，才生成实现级 `SPEC.md` / `PLAN.md` / `TASKS.md`。

## Milestone Exit

M8 完成并人工验收后，再进入：

`M9 — Full V3 External Acceptance`
