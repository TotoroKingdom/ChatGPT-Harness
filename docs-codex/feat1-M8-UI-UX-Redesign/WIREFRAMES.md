# M8 Interaction Wireframes

> Status: DRAFT FOR HUMAN REVIEW  
> Scope of this revision: App Shell, Tools, Policies  
> Depends on: `UX_AUDIT.md`, `INFORMATION_ARCHITECTURE.md`  
> Visual styling: intentionally unresolved  
> Implementation: not authorized

## 1. Purpose

本文件把已经提出的 UX / IA 原则转成可评审的低保真交互方案。

本阶段只冻结：

- 页面结构
- 信息优先级
- 控件类型
- 高频操作路径
- Detail / Advanced 边界
- 保存与反馈方式
- configured / effective / Restart Required 的表达方式

本阶段不冻结：

- 最终颜色
- 字体
- 精确字号
- 精确 spacing
- border radius
- shadow
- icon set
- 动效曲线
- 精确像素尺寸

ASCII wireframe 只表达关系和层级，不代表最终视觉密度。

---

## 2. Cross-page Interaction Decisions

### WF-D01 — Desktop workspace shell

M8 使用稳定的 Desktop Workspace shell：

```text
┌──────────────┬──────────────────────────────────────────────────────┐
│ Sidebar      │ Page                                                 │
│              │                                                      │
│              │                                                      │
│              │                                                      │
│              │                                                      │
└──────────────┴──────────────────────────────────────────────────────┘
```

当用户选择可检查对象时，出现统一 Inspector：

```text
┌──────────────┬───────────────────────────────┬──────────────────────┐
│ Sidebar      │ Page / List                   │ Inspector            │
│              │                               │                      │
│              │ selected row                  │ selected object      │
│              │                               │                      │
└──────────────┴───────────────────────────────┴──────────────────────┘
```

Inspector 是上下文层，不创建新的一级页面。

### WF-D02 — Direct manipulation first

高频、低歧义操作在列表中直接完成：

- Tool enabled → Switch
- Tool default policy → segmented `Allow / Ask / Deny`
- Policy Tool Default → segmented `Allow / Ask / Deny`
- User Rule enabled → Switch

不为这些操作增加“打开编辑页 → 修改 → Save”的额外步骤。

### WF-D03 — Saving model

上述独立配置采用 auto-save：

```text
User action
→ local control enters saving state
→ persisted desired config updates
→ inline feedback
→ if Runtime generation differs: show Restart required
```

不显示全页成功 Toast 作为唯一反馈。

失败时：

- 控件回滚到 persisted value
- 错误显示在该 row / Inspector 附近
- 保留可理解的 remediation

### WF-D04 — Runtime effective state is never faked

配置修改完成不等于 Runtime 已生效。

统一表达：

```text
Configured: Allow
Running now: Ask
Restart required
```

主列表只需要显示 `Restart required` attention；精确 desired/effective pair 进入 Inspector。

### WF-D05 — Row click vs direct control

一行既可以被选中，也可以包含直接控件：

- 点击 row 非控件区域 → 打开 Inspector
- 点击 Switch / Segmented Control → 只执行该动作，不打开 Inspector
- 键盘 focus 顺序必须清晰

### WF-D06 — Advanced is diagnostics, not a junk drawer

`Advanced` 只包含：

- internal IDs
- exact desired/effective pair
- Runtime generation
- configRevision
- raw / structured evidence
- exact readiness / reason code
- machine-oriented matcher representation

常用但页面空间不足的信息不能仅因为布局问题被随意塞进 Advanced。

---

# 3. App Shell

## 3.1 Primary shell

```text
┌──────────────────┬─────────────────────────────────────────────────┐
│ Harness           │ Tools                                           │
│                  │ Manage which tools are available and how they   │
│ WORKSPACE         │ are handled by default.                         │
│ Overview          │                                                 │
│ Tools          ●  │ ─────────────────────────────────────────────── │
│ Policies          │                                                 │
│ Approvals      2  │ Page content                                    │
│ Activity          │                                                 │
│                  │                                                 │
│ SYSTEM            │                                                 │
│ Settings          │                                                 │
│                  │                                                 │
│ ────────────────  │                                                 │
│ ● Runtime ready   │                                                 │
└──────────────────┴─────────────────────────────────────────────────┘
```

### Decisions

- 顶部不再保留当前大尺寸全局 Topbar。
- Page title 进入内容区域。
- Sidebar 保持稳定，不因页面切换改变结构。
- Approvals 有 pending request 时允许显示简洁 count badge。
- 当前导航选中状态用单一明确 affordance，不同时使用多种高亮。
- `Runtime ready` 是 concise global indicator，不展示 generation / route 等细节。

## 3.2 Global Runtime attention

Ready：

```text
● Runtime ready
```

Restart required：

```text
● Restart required
```

Stopped：

```text
○ Runtime stopped
```

Error：

```text
! Runtime error
```

点击该状态：

```text
→ Navigate to Overview
```

M8 第一版不增加 global status popover，避免同时维护两套系统状态详情 UI。

## 3.3 Page header

标准结构：

```text
Tools                                      [ optional primary action ]
Manage which tools are available and how they are handled by default.
─────────────────────────────────────────────────────────────────────
```

规则：

- 一个页面只允许一个显式 Primary Action。
- 无必要时不显示 Primary Action。
- 不使用 eyebrow 作为每页必备元素。
- 页面状态应尽量进入具体对象或 attention area，而不是堆到 header。

## 3.4 Inspector behavior

```text
┌───────────────────────────────┬────────────────────────────┐
│ Main                          │ Inspector              [×] │
│                               │                            │
│ > selected row                │ Object                     │
│                               │                            │
│                               │ fields / actions           │
│                               │                            │
└───────────────────────────────┴────────────────────────────┘
```

行为：

- 选中另一个 row → Inspector 原位切换对象。
- `Esc` → 关闭 Inspector。
- Inspector 关闭后不丢失列表滚动位置。
- 当前选中 row 有轻量 selected state。
- destructive action 不在 Inspector 中直接无确认执行。

## 3.5 Width adaptation

M8 是 Desktop-first。

宽窗口：

```text
Sidebar | Main | Inspector
```

中等窗口：

```text
Sidebar | Main
                Inspector overlays from right when opened
```

不通过挤压 Main 内容把列表变成不可读窄列。

精确 breakpoint 留给 Design System / implementation spec。

---

# 4. Tools Wireframe

## 4.1 Primary task

> 我现在有哪些工具可用？我如何快速打开/关闭它，或改变它的默认处理方式？

## 4.2 Default state

```text
Tools
Manage which tools are available and how they are handled by default.
────────────────────────────────────────────────────────────────────────────

              Status                Enabled        Default policy
read_file     ● Available             [●]        [Allow][Ask][Deny]      >
glob          ● Available             [●]        [Allow][Ask][Deny]      >
write         ● Available             [●]        [Allow][Ask][Deny]      >
edit          ● Available             [●]        [Allow][Ask][Deny]      >
bash          ○ Disabled              [ ]        [Allow][Ask][Deny]      >

────────────────────────────────────────────────────────────────────────────
```

### Decision: inline direct controls are accepted

Tools 数量目前很少且稳定，因此 M8 第一版直接在 row 中提供：

- Enabled Switch
- `Allow / Ask / Deny` segmented control

理由：

- Enable / Disable 达到 1 interaction。
- 修改 default policy 达到 1 interaction。
- 用户无需为简单配置频繁打开 Inspector。
- Inspector 可以专注解释状态与高级配置。

如果未来 Tool 数量大幅增加，再评估 compact mode；M8 不为尚不存在的规模提前牺牲易用性。

## 4.3 Tool row semantic priority

从左到右：

```text
Identity → Runtime status → Configuration → Drill-in
```

Tool description 不作为固定第二行。

仅当名称本身无法表达用途时，可通过：

- hover tooltip
- Inspector description

补充说明。

## 4.4 Restart required state

修改 read_file Enabled，但当前 Runtime 仍是旧 generation：

```text
read_file     ● Available             [ ]        [Allow][Ask][Deny]      >
              Restart required
```

这里的 `Available` 表示 running Runtime 当前 observation。
Switch 表示 configured desired state。

用户点击 row 后 Inspector 明确解释差异。

不把 `Available` 在保存后立即伪装成 `Disabled`。

## 4.5 Tool unavailable state

例如 Bash 在 Normal Mode：

```text
bash          ! Unavailable            [ ]        [Allow][Ask][Deny]      >
              Developer/Test mode required
```

此时：

- Switch disabled
- Policy control可以继续显示 configured value，但是否允许直接编辑取决于已有语义；若允许保存则可编辑，若没有意义则 disabled
- row 提供 concise remediation

点击：

```text
→ Inspector
```

Inspector 中：

```text
Bash
Unavailable
Developer/Test mode is required before Bash can run.

[ Open Security Settings ]
```

## 4.6 Saving interaction

用户点击 Tool switch：

```text
[●] → loading / disabled transient state
```

成功且立即 effective：

```text
Saved
```

成功但需要 restart：

```text
Restart required
```

失败：

```text
Could not save this change.                    [ Retry ]
```

并恢复 control 值。

成功提示应自动弱化，不持续占据页面 attention。

## 4.7 Tool Inspector — standard tool

选择 `write`：

```text
┌────────────────────────────── write ──────────────────────────────┐
│                                                                  │
│ Write files inside the active workspace.                         │
│                                                                  │
│ Enabled                                              [ ON ]       │
│                                                                  │
│ Default policy                                                   │
│                                        [ Allow ][ Ask ][ Deny ]   │
│                                                                  │
│ Current Runtime                                                  │
│ ● Available                                                      │
│                                                                  │
│ Configuration                                                    │
│ Applied                                                          │
│                                                                  │
│ ───────────────────────────────────────────────────────────────  │
│ Advanced                                                     >   │
└──────────────────────────────────────────────────────────────────┘
```

Inspector 中的 Enabled / Policy 与 row 是同一个 configuration source，不是第二份独立 draft。

## 4.8 Tool Inspector — restart mismatch

```text
write

Enabled                                              OFF

Default policy                                      Ask

Current Runtime
● Available

Configuration
Restart required
Running Runtime is still using:
Enabled: ON
Default policy: Allow

[ Restart Harness ]

Advanced >
```

`Restart Harness` 是 contextual remediation，不变成 Tools 页全局永久主按钮。

## 4.9 Bash Inspector

```text
Bash
Run controlled shell commands through Git Bash.

Enabled                                              [ ON ]
Default policy                         [ Allow ][ Ask ][ Deny ]

Current Runtime
● Available

Shell
Git Bash
C:\Program Files\Git\bin\bash.exe

Auto detect                                           [ ON ]
Executable override                              Not set   [ Choose ]

Advanced >
```

如果用户设置 manual override：

```text
Executable override
C:\...\bash.exe                              [ Change ] [ Clear ]
```

Auto-detect / executable override 属于 Bash 的常用 tool-specific configuration，因此不塞进 Advanced。

## 4.10 Tools Advanced

展开：

```text
Advanced

Configured enabled          OFF
Runtime effective enabled   ON
Availability                available
Readiness                   ready
Status reason               available
Runtime generation          ...
Config revision             ...
Technical detail            ...
```

这层面向诊断和支持场景，不要求普通用户理解。

## 4.11 Tools Interaction Budget

| Task | Budget |
| --- | ---: |
| Identify whether a tool is usable | 0 extra interactions |
| Enable / disable a normal tool | 1 |
| Change default policy | 1 |
| See why a tool is unavailable | 1 |
| Restart to apply a changed tool | <= 2 |
| Change Bash executable override | <= 3 |
| Inspect exact desired/effective mismatch | <= 2 |

---

# 5. Policies Wireframe

## 5.1 Primary task

> 什么规则控制工具执行？我如何快速改变默认策略或创建一个例外规则？

## 5.2 Default page

```text
Policies                                                     [ + Add rule ]
Control how Harness decides whether tool requests can run.
────────────────────────────────────────────────────────────────────────────

Tool defaults

read_file                             [ Allow ][ Ask ][ Deny ]
glob                                  [ Allow ][ Ask ][ Deny ]
write                                 [ Allow ][ Ask ][ Deny ]
edit                                  [ Allow ][ Ask ][ Deny ]
bash                                  [ Allow ][ Ask ][ Deny ]


User rules

Allow npm test
bash · npm test                       Allow                 [ ON ]          >

Ask before all Bash commands
bash                                  Ask                   [ ON ]          >


System guardrails                                              [ View 4 ]
Protected by Runtime · Cannot be overridden


Diagnostics / Effective policy                                      >
```

### Decisions

- Tool defaults 使用 segmented control，不用 Select。
- Tool defaults auto-save。
- 不再保留全页 Save Changes。
- Create Rule 是本页唯一 Primary Action。
- User Rules 不再是“大 Card + 三个同等级按钮”。
- System Guardrails 默认 collapsed。
- Effective Policy Snapshot 默认 collapsed / secondary。

## 5.3 Tool Default modification

用户将 `bash` 从 Ask 改为 Deny：

```text
bash                     [ Allow ][ Ask ][Deny]
                         Restart required
```

点击后立即保存 desired config。

如果 running Runtime 仍是 Ask：

- 主行只显示 `Restart required`
- 不增加 `Desired / Effective / Reason` 四列表格
- 点对应状态或打开 Diagnostics 才展示精确差异

## 5.4 User Rule row

```text
Allow npm test
bash · npm test                        Allow             [ ON ]            >
```

默认可识别：

- name
- matcher summary
- decision
- enabled state
- detail affordance

不默认展示：

- internal ID
- reason code
- source enum
- serialized argvPrefix
- matched IDs

## 5.5 User Rule enable / disable

Rule Switch 属于直接操作：

```text
[ ON ] → [ OFF ]
```

采用 auto-save。

如果需要 restart：

```text
Allow npm test
bash · npm test                        Allow             [ OFF ]           >
Restart required
```

失败则 switch 回滚并在该 rule 附近显示错误。

## 5.6 User Rule Inspector

点击 `Allow npm test`：

```text
┌────────────────────── Allow npm test ────────────────────────────┐
│                                                                  │
│ Enabled                                               [ ON ]      │
│                                                                  │
│ Applies to                                                       │
│ Bash command                                                     │
│ npm test                                                         │
│                                                                  │
│ Decision                                                         │
│                                         [ Allow ][ Ask ][ Deny ]  │
│                                                                  │
│ Reason                                                           │
│ Allow the project's normal test command.                         │
│                                                                  │
│ Configuration                                                    │
│ Applied                                                          │
│                                                                  │
│ [ Edit rule ]                                                    │
│                                                                  │
│ Advanced >                                                       │
│                                                                  │
│ ───────────────────────────────────────────────────────────────  │
│ Delete rule                                                      │
└──────────────────────────────────────────────────────────────────┘
```

`Delete rule` 视觉上与普通 mutation 分离，并要求确认。

## 5.7 Edit Rule

简单 Rule 字段可在 Inspector 内进入 edit mode：

```text
Allow npm test

Name
[ Allow npm test                         ]

Applies to
Bash command

Executable
[ npm                                    ]

Arguments prefix
[ test                                   ]

Decision
[ Allow ][ Ask ][ Deny ]

Reason
[ Allow the project's normal test ...   ]

                               [ Cancel ] [ Save ]
```

为什么 Edit 仍保留显式 Save：

- matcher / name / reason 是一个逻辑 rule object
- 多字段必须原子验证
- 中途修改不应产生半完成 rule

因此保存模型是：

```text
Tool Default / enabled toggle → auto-save
Create/Edit complete Rule     → explicit Save
```

## 5.8 Add Rule — focused Sheet

点击 `+ Add rule`：

```text
┌──────────────────────── Add rule ────────────────────────────────┐
│                                                                  │
│ What should this rule apply to?                                 │
│                                                                  │
│ (●) Entire tool                                                  │
│ ( ) Bash command                                                 │
│                                                                  │
│ Tool                                                             │
│ [ write                                     v ]                  │
│                                                                  │
│ What should Harness do?                                         │
│ [ Allow ][ Ask ][ Deny ]                                        │
│                                                                  │
│ Name                                                             │
│ [ Ask before write                       ]                       │
│                                                                  │
│ Reason (optional in UI unless Runtime contract requires value)   │
│ [                                              ]                 │
│                                                                  │
│                                       [ Cancel ] [ Create rule ]  │
└──────────────────────────────────────────────────────────────────┘
```

注意：现有 Runtime config 需要 `id / name / reason` 等字段时，UI 可以在 view-model / adapter 层：

- 自动生成稳定 internal ID
- 为允许自动生成的非用户关键字段提供安全值

如果某字段语义上确实要求用户明确输入，则必须在 SPEC 中说明，不能仅因为 protocol 有字段就默认暴露。

### Bash command branch

用户选择 `Bash command`：

```text
What should this rule apply to?
( ) Entire tool
(●) Bash command

Command
[ npm test                                      ]

What should Harness do?
[ Allow ][ Ask ][ Deny ]

Name
[ Allow npm test                                ]

Advanced matcher options >

                              [ Cancel ] [ Create rule ]
```

普通用户输入自然 command preview；UI 负责把它转换为已有 Runtime matcher 所需的 `executable + argvPrefix`，前提是转换可以确定完成。

如果输入不能可靠转换：

```text
This command cannot be converted into a safe exact matcher.
Adjust the command or use Advanced matcher options.
```

不得 fuzzy guessing。

## 5.9 Advanced matcher options

展开后才展示 machine-oriented matcher：

```text
Executable
[ npm ]

Argument prefix
[ test ]

Internal matcher preview
bash_command(executable="npm", argvPrefix=["test"])
```

不要求用户输入 JSON 数组文本。

现有 `argvPrefix` JSON textarea 必须从默认 Create Rule 流程移除。

## 5.10 Create Rule from Approval

上下文跳转：

```text
Approval
→ Create rule
```

打开同一个 Add Rule Sheet，并预填：

```text
Applies to
Command
Suggested decision
Name
Reason
```

顶部显示来源：

```text
Based on pending approval: bash · npm test
```

仍必须由用户点击 `Create rule`。

创建 rule：

- 不自动 approve 当前 request
- 不自动 resolve approval

## 5.11 System Guardrails

默认：

```text
System guardrails                                          [ View 4 ]
Protected by Runtime · Cannot be overridden
```

展开：

```text
System guardrails                                          [ Hide ]

Workspace boundary                                      Enforced      >
Explicit deny protection                               Enforced      >
...
```

点击单项可在 Inspector 解释：

- human-readable purpose
- why it cannot be edited
- relevant scope

internal ID 进入 Advanced。

## 5.12 Effective Policy Diagnostics

默认只有一行：

```text
Diagnostics / Effective policy                                      >
```

打开后进入 Inspector 或 inline expandable diagnostics view：

```text
Effective policy

Snapshot                Valid
Runtime generation      ...
Config revision         ...

Per-tool effective decision
read_file               Allow
...

Compiled user rules     2
System guardrails       4

Advanced evidence >
```

若 snapshot fail-closed invalid，则 Diagnostics 从 secondary 提升为页面最高级 Critical attention：

```text
Policy configuration is invalid
Harness is failing closed. Tool execution is restricted.
[ Review problem ]
```

安全异常不能因为“简洁”被隐藏。

## 5.13 Policies Interaction Budget

| Task | Budget |
| --- | ---: |
| Identify a tool's default policy | 0 extra interactions |
| Change tool default | 1 |
| Enable / disable user rule | 1 |
| Understand what a rule matches | 0–1 |
| Inspect rule details | 1 |
| Create simple whole-tool rule | <= 4 meaningful decisions |
| Create simple Bash exact-prefix rule | <= 5 meaningful decisions |
| Delete a rule | <= 3 including confirmation |
| Inspect effective diagnostics | 1 |

---

# 6. Keyboard & Accessibility Behavior

低保真阶段先冻结行为，不冻结最终样式。

### Rows

- row 本身可通过 keyboard focus 打开 Inspector
- 内部 Switch / Segmented Control 各自独立 focus
- 不使用嵌套 button inside button

### Segmented Control

语义上是单选：

```text
role="radiogroup"
```

或等价可访问实现。

支持方向键切换的具体实现由组件层决定，但必须满足标准 keyboard interaction。

### Inspector

- 打开后不能造成不可恢复的 focus 丢失
- Esc 可关闭
- 关闭后 focus 返回触发 row

### Create/Edit Rule Sheet

- 打开后 focus 进入 sheet
- Esc 关闭前，如果有未保存修改，应防止静默丢失
- Validation 与对应字段关联

---

# 7. Loading / Empty / Failure States

## Tools loading

不使用整页 spinner 覆盖已经存在的 persisted desired config。

若 configured state 可读、Runtime observation 正在加载：

```text
read_file     Checking Runtime…       [●]   [Allow][Ask][Deny]
```

## Tools Runtime stopped

```text
read_file     Runtime stopped         [●]   [Allow][Ask][Deny]
```

用户仍可编辑 configured state。

## Policies empty user rules

```text
User rules                                           [ + Add rule ]
No custom rules yet.
Tool defaults and system guardrails still apply.
```

## Save failure

局部失败优先局部展示：

```text
write     [Allow][Ask][Deny]
          Could not save. [ Retry ]
```

只有配置系统整体不可写等全局故障才提升为页面级 attention。

---

# 8. Explicitly Rejected Patterns

本轮明确不采用：

1. Tools 的 `Desired / Effective / Decision / Reason` 大表格作为默认界面。
2. Tools 每个工具一张大 Card。
3. 修改一个 Tool 时要求全页 Save。
4. Policies 使用 Select 选择 `Allow / Ask / Deny`。
5. Policies 在页面底部长期展示完整 Rule Editor form。
6. 创建 Rule 时默认要求输入 internal Rule ID。
7. 创建 Bash rule 时默认要求手写 argvPrefix JSON。
8. 每个 User Rule 默认显示 Edit / Enable / Delete 三个并列按钮。
9. 默认展示完整 compiled/effective policy snapshot。
10. 为查看 Tool / Rule 详情打开 modal。
11. 使用成功 Toast 替代局部状态反馈。
12. 为“看起来像 Codex”而删除 M7 必须保留的真实 effective / evidence / remediation 信息。

---

# 9. Human Review Gate — Part 1

App Shell + Tools + Policies 在继续设计剩余页面前，需要确认以下方向：

1. 接受取消大 Topbar，Page Header 进入 Main 内容区。
2. 接受 Sidebar 保留六个一级页面，并为 Approvals 显示 pending count。
3. 接受全局 Runtime 状态点击后直接进入 Overview，而不是增加 popover。
4. 接受 Tools row 同时提供 Enabled Switch + Policy Segmented Control。
5. 接受 Tool Inspector 主要用于状态解释、Bash 特有配置和 Advanced diagnostics。
6. 接受 Tool 配置采用 auto-save + inline Restart Required。
7. 接受 Policies Tool Defaults 直接 segmented auto-save，不再使用全页 Save。
8. 接受 User Rule enabled switch 为直接 auto-save 操作。
9. 接受 Create Rule 使用 focused Sheet，并按用户任务而不是 protocol 字段组织输入。
10. 接受简单 Bash rule 输入自然 command，由 UI 确定性转换为 `executable + argvPrefix`；无法安全转换则明确拒绝，不 fuzzy guessing。
11. 接受 internal Rule ID 自动管理并默认隐藏。
12. 接受 System Guardrails / Effective Policy 默认降级为 secondary / collapsed，但安全异常自动提升 attention。

如果以上方向通过，下一轮继续补：

```text
Overview
Approvals
Activity
Settings
First Run / Recovery
Cross-page states
```

之后才进入 `UX FROZEN` 候选状态。

---

## 10. Current M8 State

```text
UX_AUDIT        DRAFT COMPLETE
IA              DRAFT COMPLETE
WIREFRAMES      PART 1 DRAFT — APP SHELL / TOOLS / POLICIES
DESIGN_SYSTEM   NOT STARTED
SPEC            NOT STARTED
PLAN            NOT STARTED
TASKS           NOT STARTED
IMPLEMENTATION  NOT STARTED
```

本文件是设计提案，不授权 production UI implementation。
