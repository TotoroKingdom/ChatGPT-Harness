# M8 Visual Design System

> Status: DRAFT FOR HUMAN REVIEW  
> UX baseline: `UX_FREEZE.md` — FROZEN  
> Visual direction: Codex-inspired Minimal Developer Desktop UI  
> Implementation: not authorized

## 1. Purpose

本文件只定义 M8 的视觉语言与组件规则。

它不得重新设计已经冻结的：

- Information Architecture
- Page Contracts
- Interaction Grammar
- high-frequency user flows
- Runtime / Policy / Approval semantics

目标是让已经冻结的 UX 在视觉上具备以下特征：

- 简洁
- 克制
- 高信息清晰度
- 高密度但不拥挤
- Developer Tool / Desktop App 感
- 少装饰、少视觉噪音
- 状态优先于品牌色

---

## 2. Design Character

M8 视觉方向定义为：

> **Codex-inspired Minimal Developer Desktop UI**

不是复制某个 OpenAI 页面，而是采用以下设计语言：

- neutral grayscale foundation
- restrained semantic color
- low-decoration surfaces
- thin borders
- little or no gradient
- little or no persistent shadow
- compact controls
- consistent row rhythm
- typography + spacing 建立层级，而不是依赖巨大卡片
- content-first
- status-first
- desktop-native / IDE-like composition

### Explicitly not the target

M8 不采用：

- SaaS admin dashboard aesthetic
- large hero cards
- glassmorphism
- neon / cyberpunk
- large cyan brand glow
- heavy gradients
- shadow-heavy card wall
- oversized rounded controls
- colorful badge collection
- marketing landing-page typography

---

## 3. Theme Scope

M8 第一版保持当前产品的 **dark-first desktop UI**，不在本里程碑新增 Appearance / Theme 产品能力。

但 Design Tokens 必须使用 semantic naming，禁止把组件直接绑定到具体颜色名称，以便未来增加 light theme 时无需重写组件结构。

正确：

```text
--surface-1
--text-primary
--status-danger
```

避免：

```text
--dark-card-blue
--cyan-button
```

---

## 4. Visual Hierarchy

页面层级固定为：

```text
Page
  Page Header
  Primary content
    Section
      Row / Control
  Contextual Inspector
  Advanced / Diagnostics
```

视觉权重从高到低：

1. 当前页面和当前任务
2. 当前状态 / Primary Action
3. 对象 identity
4. 直接操作
5. explanation / helper
6. diagnostics / internal evidence

禁止让 diagnostics 的视觉权重高于当前任务。

---

## 5. Typography

### 5.1 Font stack

不引入自定义字体文件。

UI：

```css
font-family:
  ui-sans-serif,
  -apple-system,
  BlinkMacSystemFont,
  "Segoe UI",
  sans-serif;
```

Code / IDs / paths：

```css
font-family:
  ui-monospace,
  "SFMono-Regular",
  Consolas,
  "Liberation Mono",
  monospace;
```

### 5.2 Type scale

| Role | Size | Line height | Weight |
| --- | ---: | ---: | ---: |
| Page title | 24px | 32px | 600 |
| Page summary | 13px | 20px | 400 |
| Section title | 15px | 22px | 600 |
| Row title | 13px | 20px | 500 |
| Body | 13px | 20px | 400 |
| Secondary | 12px | 18px | 400 |
| Caption / metadata | 11px | 16px | 400–500 |
| Mono metadata | 11–12px | 18px | 400 |

### 5.3 Typography rules

- 不使用巨大标题制造层级。
- 不把 eyebrow / uppercase label 作为每个页面的固定组成。
- Uppercase 仅用于极少量 category / section label。
- Mono 只用于代码、路径、command、internal identifier。
- 普通产品文案禁止整段使用 mono。
- 重点优先用 weight，不优先用颜色。

---

## 6. Spacing System

基础单位：`4px`。

推荐 scale：

```text
4   micro
8   compact
12  control gap
16  standard
20  row / section internal
24  section gap
32  major section
40  page rhythm
48  large boundary
```

### Rules

- Row 内主要 horizontal gap：12–16px。
- Section 间距：24–32px。
- Page top/bottom padding：24–32px。
- Sidebar item vertical rhythm：4–6px gap。
- Inspector internal spacing：16–24px。
- 不使用大量 36–48px padding 的 Card 制造“高级感”。

---

## 7. Layout Metrics

这些值是 M8 visual baseline，可在实现前因真实窗口尺寸做小范围校正，但不得改变布局模型。

### Sidebar

```text
Preferred width: 220px
Acceptable: 208–232px
```

### Main content

```text
Page horizontal padding: 28–32px
Readable content target: 900–1180px
```

不强制所有页面居中成窄网页列；Tools / Policies / Activity 可以充分使用桌面宽度。

### Inspector

```text
Preferred width: 360px
Acceptable: 340–400px
```

宽窗口：固定右侧。

中等窗口：右侧 overlay。

Inspector 不应把 Main 挤压到无法阅读。

### Header

Page Header 不单独占用一个 100px+ 全局 topbar。

推荐：

```text
Page title
summary
24px gap
content
```

---

## 8. Radius

M8 使用 restrained radius。

```text
Control: 6px
Segmented container: 7px
Panel / Inspector: 8px
Sheet / Dialog: 10px
Pill: only when semantic shape is actually appropriate
```

避免：

- 12–20px 默认 Card radius
- 所有 Status 都做 pill
- 所有容器都圆角化

---

## 9. Surface Model

M8 不再把每个逻辑区块都变成 Card。

使用 4 层 surface：

```text
Surface 0 — app background
Surface 1 — sidebar / inspector / section background
Surface 2 — hover / selected / control container
Overlay   — sheet / dialog / menu
```

### Candidate dark palette

以下为视觉候选值，供后续实现使用；组件必须引用 semantic token。

```text
--bg                 #0f1012
--surface-1          #141518
--surface-2          #1b1d21
--surface-selected   #202329
--overlay            #181a1e

--border             #292c31
--border-strong      #3a3e45

--text-primary       #f0f1f2
--text-secondary     #b3b7bf
--text-muted         #7f858d
--text-disabled      #5f646b
```

### Surface rules

- 禁止 persistent radial gradient。
- 禁止默认 Card gradient。
- Persistent surface 不使用明显 box-shadow。
- Shadow 主要只允许：Sheet / Dialog / Popover / overlay Inspector。
- 主列表主要用 border / background delta 表达层级。

---

## 10. Semantic Colors

颜色必须表达语义，而不是装饰。

候选：

```text
--status-success     #49b675
--status-warning     #d6a84b
--status-danger      #e06c75
--status-info        #6fa8dc
```

### Rules

- Success green 不用于普通 primary button。
- Warning yellow 只用于 attention。
- Danger red 只用于 destructive / error / denied / critical。
- Info blue 只用于中性 information，需要时使用。
- `Allow / Ask / Deny` 不必始终整块染色；默认使用 neutral control，选择状态可辅以轻量 semantic indicator。
- 大面积彩色背景仅用于真正 Critical 的 blocking attention。

---

## 11. Primary Accent

M8 不采用长期高饱和 cyan 作为品牌主色。

Primary interaction 使用 **neutral high-contrast treatment**：

Dark theme 推荐：

```text
Primary button background: near-white
Primary button text: near-black
```

这样可以：

- 降低 Dashboard / Cyber 风格
- 让状态色专注表达状态
- 让操作优先级靠 contrast 而不是品牌色表达

产品 identity 可以在 logo / selected nav 的微弱 treatment 中保留极少量 accent，但不得扩散到所有控件。

---

## 12. Borders & Dividers

默认 divider：1px semantic border。

用途：

- Sidebar boundary
- Inspector boundary
- row separation
- grouped settings boundaries
- input boundary

规则：

- 优先 divider，不优先 Card box。
- 不为每行画完整矩形边框。
- Selected row 使用轻量 background + optional left/inner indicator，不叠加多个 selected affordance。

---

## 13. Buttons

统一四种按钮：

```text
Primary
Secondary
Quiet
Danger
```

### Primary

用途：页面或 focused workflow 唯一主要动作。

例：

- Start Harness
- Add rule
- Create rule
- Save rule

建议高度：32px。

### Secondary

用途：重要但非主操作。

例：

- Restart Harness
- Change path
- Retry

### Quiet

无背景或 hover 才出现 surface。

例：

- Close
- Advanced
- View details
- secondary text action

### Danger

仅 destructive。

例：

- Delete rule
- Reset configuration
- Deny（审批语义下可使用 danger treatment，但保持与 destructive delete 区别）

### Button rules

- 一个 Page Header 最多一个 Primary。
- 不允许一排 3–4 个同权重 buttons。
- Icon-only button 必须有 tooltip / aria-label。
- disabled 状态不能仅靠 opacity 低到不可读。

---

## 14. Switch

Switch 只表达 Boolean：

- Enabled
- Auto detect
- Rule enabled

推荐尺寸：

```text
track 28 × 16px 左右
```

规则：

- ON/OFF 必须可从 position + contrast 判断。
- Saving 时临时锁定该 Switch，而不是锁定整页。
- Save failure 回滚并在邻近位置解释。
- Switch 不用来表达 3-state 或 runtime status。

---

## 15. Segmented Control

用于 2–4 个互斥选项，M8 核心用途：

```text
Allow | Ask | Deny
```

推荐：

- 30–32px height
- 1px container border
- 2–4px internal padding
- selected 使用 surface + higher contrast text
- 未选项 neutral

语义：radiogroup 或等价可访问实现。

不要使用强烈的红/黄/绿三块按钮作为默认状态。

---

## 16. Select

Select 只用于：

- Language
- 多项 enum
- Tool selection in create flow
- 其他超过 segmented 合理数量的选择

不用于：

- Boolean
- Allow / Ask / Deny
- 只有两个显式选项且可以直接表达的选择

---

## 17. Inputs

推荐高度：32px。

视觉：

- Surface 1 / transparent-ish background
- 1px border
- 6px radius
- Focus 使用清晰 2px focus ring 或 border change

### Validation

错误必须靠：

- border / indicator
- inline message
- aria-invalid

不能只靠红色。

路径与 command 输入可使用 mono，但 label/helper 保持 UI font。

---

## 18. Rows

Row 是 M8 的核心构件。

### Standard interactive row

```text
Identity / summary            State / Controls            >
```

推荐：

```text
min-height: 44px
normal: 48–52px
```

Tool row 因同时包含 Switch + segmented，可使用约 52–56px。

### Row states

- default
- hover
- selected
- saving
- disabled
- attention
- error

### Rules

- hover 只改变轻微 surface。
- selected 比 hover 更明确，但不过度突出。
- error / restart required 优先使用 inline secondary line，而不是把整行染色。
- Row 内不能出现 3 个以上同权重 action buttons。

---

## 19. Status Indicator

优先格式：

```text
● Available
○ Disabled
! Unavailable
Restart required
```

### Status Dot

Status Dot 适用于：

- Runtime ready
- Available
- Degraded

不需要所有状态都变成 pill。

### Badge

Badge 只用于真正需要 compact label 的场景：

- pending count
- read-only
- risk level when concise label helps

避免当前 UI 中大量 uppercase capsule badges。

---

## 20. Notices / Attention

统一分为：

```text
Critical
Action required
Warning
Informational
Inline feedback
```

### Critical

允许明显 surface / border。

例：

- invalid policy snapshot / fail closed
- config migration failure
- fatal Runtime failure

### Action required

不需要满宽强色 banner。

例：

- Restart required
- Pending Approval

优先显示在相关对象附近或 Overview attention area。

### Informational

普通 `Saved` 不使用大型绿色 notice。

使用：

```text
Saved
Applied
Connected
```

短暂 inline feedback。

---

## 21. Inspector

Inspector 是统一的 context surface。

视觉：

- Surface 1
- 左侧 1px border
- Header sticky 可选
- 内部 section 主要使用 divider
- 不把 Inspector 里面每个 section 再做 Card

结构：

```text
Title                          [×]
Summary

Primary state / controls
────────────────────────────
Object-specific configuration
────────────────────────────
Advanced >
────────────────────────────
Danger / destructive if applicable
```

Inspector 关闭按钮是 quiet icon button。

---

## 22. Sheet / Dialog

### Sheet

主要用于 Create Rule / focused multi-field workflow。

推荐宽度：420–520px。

Sheet 比 Modal 更适合桌面开发工具，因为可以保留主页面上下文。

### Dialog

主要用于 confirmation。

例：

- Delete rule
- Reset configuration
- Discard dirty editor

### Rules

- 不使用 modal 查看普通详情。
- 一个 overlay 中只存在一个清晰 primary action。
- destructive confirmation 明确说明 consequence。

---

## 23. Settings Visual Pattern

Settings 不使用 Card wall。

推荐：

```text
Settings

General
────────────────────────────
Language                      English              >

Workspace
────────────────────────────
Workspace root                C:\AI\...       [Change]

Connection
────────────────────────────
Tunnel                        Connected            >
API key                       Configured           >

Security
────────────────────────────
Mode                           Normal               >

Advanced                                               >

Danger Zone
────────────────────────────
Reset configuration                              [Reset]
```

Section title + divider + rows 建立层级。

---

## 24. Activity Visual Pattern

Activity 是 feed，不是 database table。

推荐 row：

```text
14:32  bash      npm test
       Approved · Success · 2.4s                         >
```

规则：

- Tool / command 是视觉主信息。
- timestamp、duration、decision 是 secondary。
- Failure / Denied 可使用 semantic indicator。
- 不让 request ID、event ID 占用默认 feed。

---

## 25. Approvals Visual Pattern

Approvals 是 decision surface。

视觉顺序：

```text
Tool / command
Why approval is needed
Risk
Decision controls
Evidence disclosure
```

默认不能让 request ID、runtime generation、raw sanitized arguments 比 Allow/Deny 更醒目。

Decision buttons 保持清晰层级：

- Allow once → Primary
- Allow for session → Secondary
- Deny → Danger / high-attention secondary
- Create rule → Quiet / secondary workflow

---

## 26. Overview Visual Pattern

Overview 不使用大 Hero Card / glowing orb。

推荐：

```text
Overview
Harness is ready                         [ Stop Harness ]

Runtime        Ready
Tunnel         Ready
MCP            Ready

No action required
```

如果需要处理：

```text
Harness needs attention
Tunnel cannot connect to control plane.
                                            [ Open Settings ]
```

一屏最多一个最高优先级 attention block。

---

## 27. Iconography

使用统一 16px / 18px outline SVG icon system。

要求：

- 同一 stroke weight
- 同一 optical size
- 导航、菜单、状态、action 使用一致 icon grammar
- 不混用 Unicode 符号、emoji、不同风格图标

具体 icon library 在实现 SPEC 中确定；不得因此重新设计 IA。

---

## 28. Focus & Accessibility

必须有全局统一 focus-visible treatment。

建议：

```text
2px semantic focus ring
2px offset when required
```

要求：

- contrast 足够
- hover 不能替代 focus
- color 不能是状态唯一载体
- interactive row 与内部 controls focus 顺序合理
- Inspector / Sheet focus lifecycle 符合 `UX_FREEZE.md`

---

## 29. Motion

Motion 仅服务状态变化和空间关系。

推荐：

```text
hover/focus      100–140ms
control state    120–160ms
Inspector/Sheet  160–220ms
```

使用 ease-out 类曲线。

禁止：

- decorative background animation
- glowing pulse
- excessive spring
- 长时间状态动画

Loading 使用 restrained spinner / progress indicator。

---

## 30. Empty / Loading / Error

### Empty

不使用巨大插画。

格式：

```text
No custom rules yet.
Tool defaults and system guardrails still apply.
                                        [ Add rule ]
```

### Loading

已有 persisted content 时不整页 skeleton 覆盖。

例如：

```text
read_file   Checking Runtime…   [ON]   [Allow][Ask][Deny]
```

### Error

优先局部。

只有 global blocking failure 才使用页面级 Critical notice。

---

## 31. Content Style

用户可见文案遵循：

- short
- concrete
- action-oriented
- user terminology first
- implementation terminology second

推荐：

```text
Restart required
Restart Harness to apply this change.
```

避免：

```text
Desired state differs from current generation effective snapshot.
```

后者可以出现在 Advanced diagnostics。

---

## 32. Component Inventory

M8 实现阶段优先形成以下共享组件：

```text
AppShell
Sidebar
PageHeader
Section
Row
InteractiveRow
Inspector
Sheet
Dialog
Button
IconButton
Switch
SegmentedControl
Select
TextField
StatusIndicator
Badge
InlineFeedback
AttentionBlock
Disclosure / Advanced
EmptyState
LoadingState
ErrorState
```

页面不得自行重新实现等价视觉规则。

---

## 33. Rejected Visual Patterns

明确禁止：

1. radial-gradient app background。
2. gradient Card 作为默认 surface。
3. persistent large box-shadow。
4. 12px+ radius everywhere。
5. hero orb / glowing accent decoration。
6. cyan 作为所有 active / button / icon 的统一高饱和品牌色。
7. 每个 section 一张 Card。
8. 每个 status 一个彩色 pill。
9. oversized uppercase eyebrow system。
10. form controls 高度不一致。
11. 页面各自定义不同 Button / Input / Row 风格。
12. 为“高级感”增加无功能意义的装饰。

---

## 34. Design Review Checklist

进入 `DESIGN FROZEN` 前，需要人工确认：

1. 接受 dark-first，不在 M8 新增 Theme capability。
2. 接受 monochrome / grayscale 为主、semantic color 为辅。
3. 接受取消 gradient / glow / large shadow 的现有 Dashboard 表达。
4. 接受 neutral high-contrast Primary button，而不是 cyan Primary。
5. 接受 system font + mono，不引入自定义字体资产。
6. 接受 4px spacing base 与 compact desktop density。
7. 接受 restrained radius：6–10px。
8. 接受 divider / rows 替代 Card wall。
9. 接受 Badge 降级为少量语义组件，不作为默认状态容器。
10. 接受统一 Inspector / Sheet / Row visual grammar。
11. 接受状态色只表达状态，不承担品牌装饰。
12. 接受无 decorative animation / glow。

---

## 35. Current Gate

```text
UX              FROZEN
DESIGN_SYSTEM   DRAFT FOR HUMAN REVIEW
DESIGN          NOT FROZEN
SPEC            BLOCKED UNTIL DESIGN FROZEN
PLAN            BLOCKED UNTIL DESIGN FROZEN
TASKS           BLOCKED UNTIL DESIGN FROZEN
IMPLEMENTATION  NOT AUTHORIZED
```

本文件通过人工评审后，才允许创建实现级 `SPEC.md` / `PLAN.md` / `TASKS.md`。