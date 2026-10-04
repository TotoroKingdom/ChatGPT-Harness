# M8 Information Architecture

> Status: DRAFT FOR REVIEW  
> Depends on: `UX_AUDIT.md`  
> Implementation: Not started

## 1. Objective

本文件定义 M8 的信息架构（IA）与页面职责。

目标不是决定颜色、字号或最终组件样式，而是先冻结：

- 一级导航
- 每个页面唯一 Primary Task
- 默认信息层级
- Detail / Advanced 的职责
- Global App Shell 应承担的全局状态
- 页面之间的跳转与对象详情行为

在本文件冻结前，不进入高保真视觉设计，也不生成实现级 `SPEC.md` / `PLAN.md` / `TASKS.md`。

---

## 2. IA Decision Summary

M8 保留现有六个一级工作域，不新增一级页面：

```text
Overview
Tools
Policies
Approvals
Activity
Settings
```

理由：

- 六个页面分别对应独立且稳定的用户任务。
- 当前主要问题不是一级导航错误，而是页面内部把配置、effective state、证据、诊断混在同一层。
- M8 应优先降低页面内部复杂度，而不是通过增加导航项转移复杂度。

Sidebar 不按代码模块分类，而按用户工作域分类。

---

## 3. Global App Shell

App Shell 应承担跨页面的最小全局上下文，避免每个页面重复展示同一套状态。

### 3.1 Sidebar

建议结构：

```text
Harness

Workspace
  Overview
  Tools
  Policies
  Approvals
  Activity

System
  Settings

────────────────
Runtime ● Ready
```

### 3.2 Sidebar responsibilities

Sidebar 只展示：

- Product identity
- 一级导航
- 当前页面选中状态
- 一个 concise Runtime / Harness connectivity indicator

Sidebar 不展示：

- configRevision
- generationId
- tunnel route details
- policy snapshot validity details
- verbose status text

这些信息属于页面 Detail / Diagnostics。

### 3.3 Global status behavior

全局状态只回答：

> Harness 当前是否处于可工作的总体状态？

建议状态语义保持有限：

- Ready
- Starting
- Restart required
- Degraded
- Stopped
- Error

其中：

- `Restart required` 必须明显，但不需要成为大面积 warning banner。
- 点击全局状态可进入 Overview 或打开简短 status popover；具体交互在 Wireframe 阶段决定。

### 3.4 Page header

统一 Page Header：

```text
Page title
One-line purpose / status summary
Optional primary action
```

避免：

- 每页都出现大 Hero 区
- 重复 eyebrow + title + description + badge 堆叠
- 页面 Header 同时承担诊断详情

---

## 4. Global Information Layers

所有页面统一使用三层信息模型。

### Layer 1 — Primary status / decision

回答：

> 现在是什么状态？

必须无需打开详情即可判断。

### Layer 2 — Action / explanation

回答：

> 我能做什么？为什么是这个状态？

通过 row、inline message、detail panel 第一层展示。

### Layer 3 — Evidence / diagnostics

回答：

> 系统精确如何得到这个结果？

包含：

- internal IDs
- raw structured evidence
- generation / revision
- exact matched rule IDs
- raw result / arguments
- low-level route / executable diagnostics

Layer 3 默认不进入主工作流。

---

## 5. Overview Page Contract

### Primary Task

> 判断 Harness 是否正常，并执行当前最需要的系统级操作。

### Default view

建议首屏结构：

```text
Overview
Harness is ready

[ Stop Harness ]

Components
Runtime     Ready
Tunnel      Ready
MCP         Ready

Recent attention
No action required
```

异常时：

```text
Harness needs attention
Tunnel cannot reach control plane

[ Fix in Settings ]
```

或：

```text
Restart required
Configuration has changed

[ Restart Harness ]
```

### Default-visible

- Overall Harness state
- One context-aware primary action
- Runtime / Tunnel / MCP concise state
- Highest-priority blocker or remediation
- Restart Required if applicable

### Detail / Advanced

Component Inspector：

- route mode
- proxy source
- proxy URL
- detailed Runtime / Tunnel diagnostics
- identifiers / technical errors

### Must not become

Overview 不再承担“所有 Runtime metadata 的总览表”。

---

## 6. Tools Page Contract

### Primary Task

> 判断哪些工具可用，并直接控制 Tool enabled / default policy。

### Default view

采用 compact list：

```text
Tools

read_file      ● Available     Allow      >
glob           ● Available     Ask        >
write          ● Available     Ask        >
edit           ● Available     Ask        >
bash           ○ Disabled      Ask        >
```

### Row information

每个 Tool row 默认只展示：

- Tool name
- availability summary
- default policy
- attention indicator（Restart Required / unavailable）
- chevron / row affordance

Tool description 可作为 secondary text，但不得扩大每行高度到 Card 级别。

### Direct actions

如果空间和可访问性允许，row 可直接包含：

- Enabled Switch
- Policy Segmented Control

否则放入 Detail Panel。

最终布局在 Wireframe 阶段冻结。

### Tool Detail Panel

第一层：

```text
Tool name
Enabled          [switch]
Default policy   Allow | Ask | Deny
Current status   Available / Disabled / Unavailable
Restart state    Applied / Restart required
```

第二层（tool-specific）：

Bash：

- detected shell
- auto detect
- executable override

### Advanced

- desired vs effective exact values
- readiness
- technical reason
- Runtime capability
- revision / generation
- raw remediation metadata

### Invariant

简化后仍必须能准确区分：

- desired enabled
- Runtime effective availability
- policy outcome
- Restart Required

但这些概念不必同时作为主表格列出现。

---

## 7. Policies Page Contract

### Primary Task

> 查看和管理控制工具执行的规则。

### Page structure

固定分成三个清晰区域：

```text
Policies

Tool defaults
User rules
System guardrails
```

其中 System Guardrails 默认视觉权重最低，但必须容易找到。

### 7.1 Tool Defaults

默认列表：

```text
Tool defaults

read_file      Allow | Ask | Deny
glob           Allow | Ask | Deny
write          Allow | Ask | Deny
edit           Allow | Ask | Deny
bash           Allow | Ask | Deny
```

首层不展示：

- matched IDs
- Runtime reason code
- current generation details

如果 configured 与 effective 不一致，在对应 Tool 行显示：

```text
Restart required
```

而不是另起一张全局表解释。

### 7.2 User Rules

默认列表：

```text
User rules                         + Add rule

Allow npm test
bash · npm test                  Allow    >

Ask before bash commands
bash                             Ask      >
```

每条 rule 默认展示：

- human-readable name
- matcher summary
- decision
- enabled state
- optional attention state

### User Rule Detail

第一层：

- Enabled
- Applies to
- Decision
- Human-readable reason / description

第二层：

- Edit
- Duplicate（如后续需要）
- Delete

### Rule creation

Create Rule 使用 focused Sheet / Dialog，不嵌在整个 Policies 页面底部。

用户决策顺序：

```text
1. Applies to what?
2. Which tool / command?
3. Allow / Ask / Deny?
4. Optional name / reason
```

不要求用户先填写 internal Rule ID。

### Advanced Rule Fields

- internal ID
- exact matcher representation
- argvPrefix serialized representation
- source metadata
- compiled/effective evidence

### 7.3 System Guardrails

只读。

默认 collapsed 或 secondary section：

```text
System guardrails
Protected by Runtime · Cannot be overridden
```

展开后显示 guardrails 列表和解释。

### 7.4 Effective Diagnostics

现有 Current Generation / Per-tool Decision 等信息不删除，但整体移动到：

```text
Diagnostics / Effective policy
```

普通策略管理不需要默认看到完整 compiled snapshot。

---

## 8. Approvals Page Contract

### Primary Task

> 对 pending request 做安全且有依据的决定。

### Queue model

Approvals 应被视为一个 decision queue，而不是 evidence catalog。

建议：

```text
Approvals
2 pending

bash
npm test
Medium risk · Requires approval
[ Allow once ] [ Allow for session ] [ Deny ]

write
src/config.ts
Low risk · User rule requires approval
[ Allow once ] [ Deny ]
```

### Decision-critical information

默认必须看到：

- Tool
- Target / command
- Risk
- Why approval is required
- expiry when materially important
- allowed decisions

### Secondary action

`Create rule` 不与 Allow/Deny 处于同一主按钮层级。

可放：

- `...` menu
- secondary link
- post-decision follow-up

最终交互在 Wireframe 冻结。

### Approval Detail

第一层：

- scope / command
- cwd if relevant
- policy reason
- matched rule summary
- risk explanation

Advanced evidence：

- request ID
- runtime/generation ID
- sanitized arguments
- normalized segments
- analyzer flags
- exact matched rule IDs

### Terminal requests

已 resolved / expired 的请求默认不应和 Pending 请求竞争注意力。

建议：

```text
Pending
History
```

是否使用 tabs / filter 在 Wireframe 阶段决定。

---

## 9. Activity Page Contract

### Primary Task

> 快速理解最近发生了什么，以及失败/拒绝的原因。

### Default view

从传统 audit table 改为 compact activity feed：

```text
Activity

14:32   bash      npm test
        Approved · Success · 2.4s               >

14:31   write     src/config.ts
        Allowed · Success                       >

14:29   glob      **/*.rs
        Denied by user rule                     >
```

### Default-visible

- relative / concise time
- Tool
- target / command summary
- decision + outcome
- duration when meaningful

### Activity Detail

第一层必须先回答：

- What happened
- Why
- Which rule / approval affected it
- What to do next if failed

### Advanced evidence

- request ID
- event ID
- sanitized arguments
- exact matched IDs
- approval ID
- raw result evidence
- structured Bash evidence

### Filtering

支持后续增加简单 filter：

- All
- Success
- Needs attention / Failed
- Tool

不在 IA 阶段冻结具体控件。

---

## 10. Settings Page Contract

### Primary Task

> 配置 Harness 的基础环境、连接与偏好。

### Default sections

Settings 重组为：

```text
General
Workspace
Connection
Security
Advanced
Danger Zone
```

### 10.1 General

默认展示：

- Language

未来如增加 appearance 等用户偏好，也进入 General。

### 10.2 Workspace

默认展示：

- Workspace root
- Choose / Change action

必要的 security explanation 可为 secondary text。

### 10.3 Connection

默认展示：

- Tunnel ID
- API Key status
- connection readiness summary

Advanced：

- control plane proxy
- bundled tunnel client / manual override
- executable path / detection details

### 10.4 Security

默认展示：

- Normal
- Developer/Test
- concise consequence explanation

模式变化若需要 Runtime restart，必须 inline 显示。

### 10.5 Advanced / Diagnostics

包含：

- config directory
- config file
- secret provider
- schema / revision diagnostics（如需要）
- support-oriented metadata

### 10.6 Danger Zone

仅放：

- Reset configuration

危险操作与普通 Save / Apply 必须物理分离。

### Saving model

M8 不使用“一刀切”的保存模式。

字段按语义分为：

1. Safe independent mutation → auto-save
2. Atomic related fields → explicit Apply
3. Dangerous mutation → confirm
4. Runtime-bound mutation → save + inline Restart Required

具体字段分类在 Wireframe / SPEC 阶段冻结。

---

## 11. Detail Panel / Inspector Model

M8 推荐建立统一的 Right Detail Panel / Inspector pattern。

适用：

- Tool
- User Rule
- Activity Entry
- System component

不适用：

- Approval 的主决策（审批本身应保持主内容焦点）
- First Run Wizard
- 需要完整创建流程的 Rule creation

### Behavior principles

- 选择 row → 打开 detail
- detail 不改变一级路由
- Esc / Close → 返回列表上下文
- 主列表保持选中项
- Advanced 默认折叠
- Detail 中的 mutation 必须有局部反馈

是否将 panel state 写入 URL / router 在实现设计阶段决定。

---

## 12. Modal / Sheet Policy

M8 避免把简单设置放进 Modal。

Modal / Sheet 只用于：

- Create Rule
- destructive confirmation
- focused multi-step action

不用于：

- 查看普通 Tool 详情
- 查看 Activity 详情
- 修改一个 Boolean
- 修改 Allow / Ask / Deny

---

## 13. First Run / Recovery

First Run Wizard 保持独立流程，不塞回普通 Settings 页面。

Primary flow：

```text
Workspace
→ Connection
→ Readiness
→ Start Harness
```

必须优先帮助用户完成首次可用链路，不默认展示 advanced tunnel / diagnostics 字段。

Config Recovery 也保持 task-focused：

- what is wrong
- safe recovery action
- consequence

而不是暴露完整 migration / schema internals。

---

## 14. Navigation Between Workflows

允许上下文跳转，但必须保留用户意图。

### Approval → Policies

`Create rule`：

```text
Approval request
→ Policies Create Rule
→ prefilled matcher / scope / reason
```

不得自动创建 rule，也不得自动 resolve approval。

### Overview → Settings

如果 blocker 属于配置问题：

```text
Overview remediation
→ Settings relevant section
```

### Activity → Policies / Approvals

Activity Detail 可提供 contextual link：

- View matched rule
- View approval

前提是对象仍可定位。

这些 link 属于 secondary navigation。

---

## 15. Attention Model

全产品统一 attention priority：

### Critical

- fail-closed invalid policy
- configuration invalid / migration failure
- Runtime fatal error

### Action required

- Pending approval
- Restart required
- missing required connection setup

### Warning / degraded

- Tool unavailable
- optional component degraded
- manual override issue

### Informational

- saved
- connected
- effective
- read-only

同一页面优先只突出最高等级 attention，不堆叠多个同等醒目的 banners。

---

## 16. Default vs Advanced Matrix

| Information | Default | Detail | Advanced |
| --- | --- | --- | --- |
| Tool name | Yes | Yes | Yes |
| Tool availability | Yes | Yes | Yes |
| Tool desired/effective exact pair | No | Optional | Yes |
| Policy decision | Yes | Yes | Yes |
| Rule matcher summary | Yes | Yes | Yes |
| Rule internal ID | No | No | Yes |
| matched rule IDs | No | Summary | Yes |
| Restart Required | Yes | Yes | Yes |
| Runtime generation | No | No | Yes |
| configRevision | No | No | Yes |
| Approval risk | Yes | Yes | Yes |
| Approval request ID | No | No | Yes |
| Activity outcome | Yes | Yes | Yes |
| Raw sanitized arguments | No | No | Yes |
| Raw result evidence | No | No | Yes |
| Proxy / route detail | No | Component Detail | Yes |
| Config filesystem path | No | No | Settings Advanced |

---

## 17. IA Acceptance Questions

本 IA 在进入 Wireframe 前需要人工确认以下问题：

1. 六个一级页面是否继续保留？
2. Sidebar 是否接受 `Workspace / System` 两组结构？
3. Tools 是否接受 `List + Detail Panel`？
4. Policies 是否接受 `Tool Defaults + User Rules + collapsed System Guardrails`？
5. Create Rule 是否从页面内嵌 Form 改为 focused Sheet / Dialog？
6. Approvals 是否接受“decision first, evidence second”？
7. Activity 是否接受从 audit table 改为 activity feed？
8. Settings 是否接受 `General / Workspace / Connection / Security / Advanced / Danger Zone`？
9. 是否接受统一 Right Detail Panel / Inspector pattern？
10. 是否接受 internal IDs / raw evidence 默认进入 Advanced？

只有这些问题达成一致后，才创建 `WIREFRAMES.md` 并进入逐页低保真交互设计。

---

## 18. Current Decision State

当前建议状态：

```text
UX_AUDIT        DRAFT COMPLETE
IA              DRAFT COMPLETE
WIREFRAMES      NOT STARTED
DESIGN_SYSTEM   NOT STARTED
SPEC            NOT STARTED
PLAN            NOT STARTED
TASKS           NOT STARTED
IMPLEMENTATION  NOT STARTED
```

M8 当前仍处于设计阶段，不允许将本 IA 解释为已授权 production UI rewrite。
