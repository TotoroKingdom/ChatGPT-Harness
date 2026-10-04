# M8 Interaction Wireframes — Part 2

> Status: DRAFT FOR HUMAN REVIEW  
> Scope: Overview, Approvals, Activity, Settings, First Run / Recovery, cross-page states  
> Depends on: `UX_AUDIT.md`, `INFORMATION_ARCHITECTURE.md`, `WIREFRAMES.md`  
> Visual styling: intentionally unresolved  
> Implementation: not authorized

## 1. Purpose

本文件补完 M8 的低保真交互设计。

Part 1 已覆盖：

- App Shell
- Tools
- Policies

Part 2 覆盖：

- Overview
- Approvals
- Activity
- Settings
- First Run / Recovery
- Cross-page states

本阶段仍然只冻结页面结构、信息层级、控件语法与操作路径；不冻结颜色、字体、精确 spacing、radius、shadow、icon 或动效。

---

# 2. Overview Wireframe

## 2.1 Primary task

> Harness 现在是否正常？如果不正常，我下一步该做什么？

Overview 不再承担“全部 Runtime metadata 总表”的职责。

## 2.2 Ready state

```text
Overview
Harness is ready
Everything required for normal operation is available.
────────────────────────────────────────────────────────────────────

Status
Runtime                                            Ready
Tunnel                                             Ready
MCP                                                Ready

No action required.

Recent activity
14:32  bash      npm test                    Success
14:31  write     src/config.ts               Success
                                                    [ View activity ]
```

### Decisions

- Ready 状态不展示 Hero Card。
- 不用大面积绿色装饰表达“正常”。
- 不重复展示 route / proxy / generation 等 diagnostics。
- 正常状态下可以没有 Primary Action。
- Recent activity 只保留极短摘要，不复制 Activity 页面。

## 2.3 Stopped state

```text
Overview
Harness is stopped
Start Harness to make local tools available to ChatGPT.
────────────────────────────────────────────────────────────────────

[ Start Harness ]

Readiness
Workspace                                          Ready
Connection                                         Ready
Runtime                                            Stopped
```

Primary Action：

```text
Start Harness
```

如果不能启动：

```text
Harness cannot start
Tunnel connection is not configured.

[ Fix connection settings ]
```

用户不需要先理解 eligibility enum 或内部 reason code。

## 2.4 Restart required

```text
Overview
Restart required
Configuration changed after the current Runtime started.
────────────────────────────────────────────────────────────────────

[ Restart Harness ]

Current Runtime is still using the previous configuration.
Your saved changes are not lost.
```

下面只列出需要用户理解的 affected areas：

```text
Changed
Tools                                            2 changes
Policies                                         1 change
```

不默认显示 configRevision / Runtime generation。

## 2.5 Degraded / error

```text
Overview
Harness needs attention
Tunnel cannot reach the control plane.
────────────────────────────────────────────────────────────────────

[ Open connection settings ]

Runtime                                            Ready
Tunnel                                             Error
MCP                                                Ready

What happened
Control-plane connection failed.

What to do
Check proxy or tunnel credentials.

Technical details >
```

原则：

1. 先说明用户可理解的问题。
2. 再给一个明确 remediation。
3. 最后才给 Technical details。

## 2.6 Component Inspector

点击 Runtime / Tunnel / MCP 行：

```text
Tunnel
Status                                             Error

Connection
Control plane                                      Unreachable

Recommended action
Check connection settings.
[ Open Settings ]

Advanced >
```

Advanced：

```text
Route mode
Proxy source
Proxy URL
Last error code
Runtime / tunnel identifiers
```

## 2.7 Overview Interaction Budget

| Task | Budget |
| --- | ---: |
| Determine whether Harness is usable | 0 extra interactions |
| Start / stop Harness | 1 |
| Restart after config change | 1 |
| Identify blocking component | 0 |
| Navigate to relevant remediation | <= 1 |
| Inspect low-level component diagnostics | <= 2 |

---

# 3. Approvals Wireframe

## 3.1 Primary task

> 这个请求是什么？风险是什么？我要允许还是拒绝？

Approvals 是 decision queue，不是 evidence catalog。

## 3.2 Default Pending view

```text
Approvals
2 requests need your decision.
────────────────────────────────────────────────────────────────────

Pending

bash                                                Medium risk
npm test
Policy requires approval before this command runs.

[ Allow once ]   [ Allow for session ]   [ Deny ]       More >

────────────────────────────────────────────────────────────────────

write                                               Low risk
src/config.ts
A user policy requires approval.

[ Allow once ]                         [ Deny ]          More >

────────────────────────────────────────────────────────────────────

History                                                        >
```

### Decisions

默认第一屏只展示 decision-critical information：

- Tool
- Target / command
- Risk
- Why approval is required
- Valid decisions

不默认展示：

- request ID
- runtime ID
- generation ID
- sanitized argument object
- normalized segments
- analyzer flags
- exact matched rule IDs

## 3.3 Action hierarchy

对于 pending request：

Primary positive action：

```text
Allow once
```

Secondary positive action：

```text
Allow for session
```

Destructive / restrictive action：

```text
Deny
```

`Create rule` 不与三种审批决定并列。

放到：

```text
More > Create rule
```

或者审批完成后的 contextual follow-up。

## 3.4 Bash approval detail

点击 request 非按钮区域：

```text
bash approval
npm test
Medium risk

Why approval is required
Default Bash policy is Ask.

Command
npm test

Working directory
C:\AI\ChatGPT-Harness

Policy
Matched rule: Bash default · Ask

Risk
No elevated analyzer flags detected.

[ Allow once ]
[ Allow for session ]
[ Deny ]

Create rule >
Advanced evidence >
```

对高风险 Bash：

```text
High risk
This command could not be safely classified for reusable approval.

[ Allow once ]   [ Deny ]

Allow for session is unavailable for this request.
Why? >
```

UI 必须使用 Runtime authoritative eligibility，不自行重新推断是否允许 session grant。

## 3.5 Decision confirmation

`Allow once`：不额外确认。

`Allow for session`：如果范围清晰且 Runtime 允许，不额外增加第二次 modal；按钮附近必须明确语义：

```text
Allow for session
Allows matching requests only for this Runtime session.
```

`Deny`：普通 request 不额外确认，因为审批本身就是显式 decision；防止为每次拒绝制造摩擦。

只有未来出现不可逆的额外 side effect 时才增加 confirmation。

## 3.6 Resolve feedback

点击 `Allow once`：

```text
Resolving…
```

成功：request 从 Pending 区移除，并出现轻量 inline acknowledgement：

```text
Allowed once · request resumed
```

失败：

```text
Could not resolve this request.
The request may have expired or Runtime may have changed.
[ Refresh ]
```

不得让用户误以为失败的 resolution 已执行。

## 3.7 Create Rule handoff

```text
Approval
→ More
→ Create rule
→ Policies Add Rule sheet
```

预填：

- tool / command matcher
- suggested name
- reason
- source approval context

创建 rule 后：

- 不自动 resolve 当前 approval
- 返回 approval context 时仍显示当前 request 最新状态

## 3.8 History

History 默认折叠或进入同页 secondary view：

```text
History

14:20   bash      npm test       Allowed once
14:03   write     src/a.ts       Denied
13:58   bash      pnpm lint      Expired
```

Pending request 永远比 History 优先。

## 3.9 Approvals Interaction Budget

| Task | Budget |
| --- | ---: |
| Understand what needs approval | 0 |
| Understand risk level | 0 |
| Allow once | 1 |
| Deny | 1 |
| Allow for session when eligible | 1 |
| See why session approval unavailable | 1 |
| Inspect policy/risk explanation | 1 |
| Create a reusable rule from approval | <= 3 before rule-specific decisions |

---

# 4. Activity Wireframe

## 4.1 Primary task

> 最近发生了什么？为什么成功、被拒绝或失败？

Activity 从 audit table 改成 activity feed。

## 4.2 Default feed

```text
Activity
Understand recent tool requests and outcomes.
────────────────────────────────────────────────────────────────────

[ All ] [ Success ] [ Needs attention ]              Tool: [ All v ]

14:32   bash      npm test
        Approved · Success · 2.4s                                  >

14:31   write     src/config.ts
        Allowed · Success · 12ms                                   >

14:29   glob      **/*.rs
        Denied by user rule                                        >

14:25   bash      cargo test
        Execution failed · Exit code 1                             >
```

### Decisions

默认不再展示六列 table header。

每个 entry 首层只回答：

- when
- tool
- target / command
- decision
- outcome
- duration when useful

## 4.3 Attention semantics

Success：

```text
Allowed · Success
Approved · Success
```

Policy denied：

```text
Denied by user rule
```

Approval denied：

```text
Denied by user
```

Execution failure：

```text
Allowed · Execution failed
```

Tool unavailable：

```text
Not executed · Tool unavailable
```

用户第一眼要能区分：

```text
Policy prevented execution
vs
Execution was allowed but failed
```

## 4.4 Activity Inspector

选择被拒绝的 glob：

```text
Activity
Glob request was denied

Tool
 glob

Target
 **/*.rs

Decision
 Deny

Why
 Matched user rule: Block broad glob

Outcome
 Not executed

[ View policy rule ]

Advanced evidence >
```

选择执行失败的 bash：

```text
Activity
Bash command failed

Command
cargo test

Decision
Allowed

Outcome
Exit code 1
Duration: 3.8s

What to do next
Review command output or retry after fixing the test failure.

Result summary
...

Advanced evidence >
```

## 4.5 Advanced evidence

```text
Request ID
Event ID
Sanitized arguments
Matched rule IDs
Approval ID / decision
Raw result evidence
Failure category
Structured Bash evidence
Runtime generation
```

Advanced 信息可复制，但默认不应成为阅读主路径。

## 4.6 Contextual navigation

如果对象仍存在：

```text
View policy rule
View approval
```

如果对象已经不可定位：

- 仍展示 snapshot evidence
- 不显示无效跳转

## 4.7 Refresh model

第一版保留明确 Refresh 能力，但视觉权重低：

```text
Activity                                             Refresh
```

未来是否 auto-refresh 不在 M8 交互冻结阶段扩展功能。

## 4.8 Activity Interaction Budget

| Task | Budget |
| --- | ---: |
| See recent outcome | 0 |
| Distinguish denied vs execution failure | 0 |
| Understand why one request failed/was denied | 1 |
| Open matched rule / approval | <= 2 |
| Inspect raw evidence | <= 2 |

---

# 5. Settings Wireframe

## 5.1 Primary task

> 配置 Harness 的基础环境、连接、安全模式和用户偏好。

Settings 不再把所有字段作为同层级 Card wall。

## 5.2 Default page structure

```text
Settings
────────────────────────────────────────────────────────────────────

General
Language                                   System / English / 简体中文

Workspace
C:\AI\ChatGPT-Harness                                      [ Change ]

Connection
Tunnel ID                                  tun_••••••••••
API key                                    Configured
Status                                     Connected                  >

Security
Mode                                       Normal                      >

Advanced                                                          >

Danger Zone                                                        >
```

### Decisions

- 默认页更接近 settings list，而不是多个大 Card。
- 每个 section 只展示最关键 current value。
- 复杂配置进入 section detail / expandable area。
- Reset 与普通设置物理分离。

## 5.3 General

```text
General

Language
[ System ][ English ][ 简体中文 ]
```

Language 属于独立安全 mutation：auto-save，立即应用。

失败：

- UI language 回滚
- preference 回滚
- inline error

保留现有 M6 对 language mutation serialization 的行为要求。

## 5.4 Workspace

```text
Workspace

Root
C:\AI\ChatGPT-Harness                              [ Change ]

Harness tools are restricted to this workspace.
```

选择新目录后：

- 保存 configured workspace
- 如 Runtime 已运行，inline 显示 `Restart required`
- 不把当前 Runtime workspace 立即伪装为新路径

需要时 Inspector / detail：

```text
Configured workspace
C:\AI\NewWorkspace

Running Runtime
C:\AI\OldWorkspace

Restart required
[ Restart Harness ]
```

## 5.5 Connection summary

默认：

```text
Connection

Tunnel ID                    tun_••••••••••
API key                      Configured
Status                       Connected
                                                        [ Configure ]
```

进入 Configure：

```text
Connection

Tunnel ID
[ tun_...                                  ]

API key
Configured                                      [ Replace ] [ Remove ]

Tunnel client
Bundled                                             [ Change ]

Advanced connection >

                                      [ Cancel ] [ Apply ]
```

为什么使用显式 Apply：

- Tunnel ID / credential / executable override 可形成一组连接配置
- 中途半保存可能造成不必要的断裂状态
- Connection 是少数适合 atomic edit 的 settings section

## 5.6 Advanced Connection

```text
Advanced connection

Control-plane HTTP proxy
[ http://127.0.0.1:7897                         ]

Tunnel client executable
Bundled
or
C:\...\tunnel-client.exe

Detection / route details >
```

Proxy validation 与现有安全约束保持一致；错误直接绑定字段。

## 5.7 Security

默认：

```text
Security

Mode
(●) Normal
    Standard safety boundary. Bash is unavailable.

( ) Developer/Test
    Enables high-risk developer capabilities such as Bash.
```

选择 Developer/Test：

```text
Developer/Test mode enables higher-risk capabilities.
Existing policy and approval protections still apply.

[ Cancel ] [ Enable Developer/Test ]
```

这里允许一次明确 confirmation，因为模式变化显著改变 capability eligibility。

成功后如需 Runtime restart：

```text
Developer/Test saved
Restart required                                       [ Restart ]
```

## 5.8 Advanced / Diagnostics

```text
Advanced

Configuration
Config directory                ...
Config file                     ...
Secret provider                 Windows Credential Manager
Schema version                  2
Config revision                 ...

Runtime / support diagnostics >
```

这类信息不进入默认 Settings 首屏。

## 5.9 Danger Zone

```text
Danger Zone

Reset configuration
Restore Harness configuration to defaults.
This does not silently preserve user rules or workspace settings.

[ Reset configuration ]
```

点击：

```text
Reset configuration?

This will reset local configuration to defaults.
Runtime may need to restart.

[ Cancel ] [ Reset configuration ]
```

危险操作不得和普通 Apply / Change 并列。

## 5.10 Settings saving matrix

| Setting | Interaction |
| --- | --- |
| Language | Auto-save |
| Workspace | Save after picker selection; restart may be required |
| Connection group | Explicit Apply |
| Security mode | Explicit confirm + save |
| Advanced diagnostics | Read-only unless specifically editable |
| Reset configuration | Confirmed destructive action |

## 5.11 Settings Interaction Budget

| Task | Budget |
| --- | ---: |
| See current language/workspace/security mode | 0 |
| Change language | 1 |
| Change workspace | <= 2 |
| Replace connection config | <= 4 meaningful decisions |
| Switch to Developer/Test | <= 2 including confirmation |
| Find config file / secret provider | 1 |
| Reset configuration | <= 2 including confirmation |

---

# 6. First Run Wireframe

## 6.1 Primary task

> 用最少步骤把 Harness 配置到可以第一次启动。

First Run 是 task flow，不是普通 Settings 的复制版。

## 6.2 Step model

```text
Set up ChatGPT Harness

1 Workspace   2 Connection   3 Ready
```

不引入复杂 wizard 动画；重点是减少一次性认知负担。

## 6.3 Step 1 — Workspace

```text
Set up ChatGPT Harness

Workspace
Choose the local folder Harness is allowed to work in.

C:\AI\MyProject                                      [ Choose ]

                                             [ Continue ]
```

只要求 workspace。

不展示：

- config directory
- schema
- revision
- policy defaults

## 6.4 Step 2 — Connection

```text
Connection
Connect Harness to your OpenAI tunnel.

Tunnel ID
[ tun_...                                      ]

API key
[ •••••••••••••••••••                         ]

Advanced connection >

                              [ Back ] [ Save & continue ]
```

Advanced 才包含 proxy / tunnel executable override。

## 6.5 Step 3 — Ready

```text
Ready to start

Workspace                                      Ready
Connection                                     Ready
Runtime                                        Stopped

[ Start Harness ]
```

启动失败：

```text
Could not start Harness
Tunnel connection failed.

[ Fix connection ]
Technical details >
```

而不是把用户扔回完整 Settings。

## 6.6 Existing configuration

如果已有可用 config：First Run 不应重复出现。

如果 config migration / validation 失败：进入 Recovery，而不是伪装成全新安装。

---

# 7. Recovery Wireframe

## 7.1 Primary task

> 告诉用户哪里坏了、数据是否安全、有哪些安全恢复动作。

## 7.2 Invalid config

```text
Configuration needs repair
Harness cannot safely use the current configuration.

Your existing configuration has not been overwritten.

Recommended action
Review the problem or reset to defaults.

[ Review problem ]   [ Reset configuration ]
```

`Reset configuration` 是 destructive path，仍需 confirmation。

## 7.3 Migration failure

```text
Configuration upgrade failed
Harness kept the previous configuration unchanged.

What happened
The stored configuration could not be migrated safely.

[ View technical details ]
[ Reset configuration ]
```

必须明确：

- fail closed
- 原配置未被覆盖
- secret 不明文化

但这些信息用人类语言表达，不要求用户理解 migration transaction 内部实现。

## 7.4 Technical detail

```text
Technical details
Error code
Schema version
Config path
Migration reason
Recommended remediation
```

默认折叠。

---

# 8. Cross-page State Rules

## 8.1 Loading

原则：已有 persisted data 时，Runtime observation loading 不应阻塞整页。

例如：

```text
write       Checking Runtime…       ON      Ask
```

而不是全页 spinner。

## 8.2 Empty

Empty state 必须回答两件事：

1. 为什么这里是空的？
2. 用户是否需要做什么？

例如 Approvals：

```text
No pending approvals
Requests that require your decision will appear here.
```

不要放无意义的大插画或 CTA。

## 8.3 Success feedback

成功优先通过对象状态变化表达：

```text
Saved
Applied
Allowed once
Restart required
```

成功 Toast 可以作为辅助，但不得成为唯一 evidence。

## 8.4 Failure feedback

失败位置遵循：

```text
local mutation failure  → local inline error
page data failure        → page-level state
system fatal failure     → global + Overview remediation
```

## 8.5 Restart Required

统一语义：

```text
Saved configuration != current Runtime generation
```

可出现位置：

- Sidebar global Runtime status
- 具体 changed row / setting
- Overview remediation
- Inspector exact comparison

不得出现互相矛盾的 restart wording。

## 8.6 Disabled vs unavailable vs denied

必须保持语义区分：

- Disabled = user configured tool off
- Unavailable = tool cannot currently run due mode/readiness/runtime condition
- Denied = policy decision prevents request

视觉系统后续必须为三者提供不同但克制的 state expression。

## 8.7 Read-only

System-owned / historical / resolved objects必须明确 read-only，但不把 disabled styling 用作唯一表达，因为 disabled 容易被理解为“暂时不能点击”。

---

# 9. Cross-page Navigation

## 9.1 Overview → Settings

系统 blocker：

```text
Overview
→ Fix connection settings
→ Settings / Connection
```

直接定位 relevant section。

## 9.2 Tools → Settings

Bash 因 Normal Mode unavailable：

```text
Tool Inspector
→ Open Security Settings
→ Settings / Security
```

## 9.3 Approval → Policy

```text
Approval
→ Create rule
→ Policies Add Rule sheet with context
```

## 9.4 Activity → Policy / Approval

```text
Activity Inspector
→ View matched rule
or
→ View approval
```

## 9.5 Navigation invariant

跨页跳转必须携带最少必要 context，但不得隐式执行 mutation。

Navigation ≠ Save
Navigation ≠ Approve
Navigation ≠ Create rule

---

# 10. Explicitly Rejected Patterns — Part 2

本轮明确拒绝：

1. Overview 使用巨大 Hero status Card 作为默认视觉中心。
2. Overview 默认展示 route / proxy / generation / revision 等低层 diagnostics。
3. Approvals 默认展示完整 evidence catalog 再让用户找按钮。
4. `Create rule` 与 `Allow once / Allow for session / Deny` 同等级排列。
5. 对每个 `Allow once` 或 `Deny` 再弹确认框。
6. Activity 使用六列审计表作为主要阅读体验。
7. Activity 把 policy deny 和 execution failure 混成一个泛化 Error。
8. Settings 使用多个大 Card 平铺所有字段。
9. Settings 使用一个全页 Save / Discard 控制所有异质配置。
10. Reset configuration 与普通 Save / Apply 同一操作区。
11. First Run 直接复制完整 Settings 页面。
12. Recovery 默认展示 migration / schema 内部细节。
13. 使用页面级 spinner 隐藏已经可用的 persisted configuration。
14. 为追求“简洁”而合并 Disabled / Unavailable / Denied 的不同语义。

---

# 11. Human Review Gate — Full Wireframes

进入 `UX FROZEN` 候选状态前，需要人工确认：

1. Overview 接受“状态 + 当前最重要操作 + concise components”，不再使用 Hero dashboard。
2. Overview 正常状态可以没有 Primary Action。
3. Approvals 接受 decision-first queue。
4. `Allow once / Allow for session / Deny` 直接决策，不额外增加无必要 confirmation。
5. `Create rule` 降为 secondary workflow。
6. Activity 接受 feed 替代 audit table。
7. Activity 默认明确区分 Policy Denied 与 Execution Failed。
8. Settings 接受 list/section 模型，不再 Card wall。
9. Settings 接受按字段语义混合使用 auto-save / Apply / confirmation，而不是一个全局 Save。
10. Connection 配置使用 explicit Apply。
11. Developer/Test mode 使用明确 confirmation。
12. First Run 接受 `Workspace → Connection → Ready → Start`。
13. Recovery 接受 task-focused design，技术细节默认折叠。
14. 全产品统一 Restart Required、Disabled、Unavailable、Denied 语义。
15. 跨页面导航只携带 context，不隐式 mutation。

如果 Part 1 + Part 2 均确认，下一步：

```text
UX_AUDIT
IA
WIREFRAMES Part 1
WIREFRAMES Part 2
        ↓
UX FROZEN
        ↓
DESIGN_SYSTEM.md
```

---

## 12. Current M8 State

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

本文件是设计提案，不授权 production UI implementation。
