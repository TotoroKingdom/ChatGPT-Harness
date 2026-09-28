# ChatGPT-Harness V3 Product Requirements Document

> Status: DRAFT / PRODUCT REQUIREMENTS  
> Baseline: V2 — Read-only Harness, ACCEPTED / FROZEN  
> Architecture: **TBD — will be defined separately**  
> Roadmap: See `ChatGPT-Harness_V3_ROADMAP.md`

---

# 1. Product Summary

ChatGPT-Harness V2 已完成只读链路：

```text
ChatGPT
→ Tunnel
→ Runtime MCP
→ read_file / glob
→ WorkspaceGuard / Policy / Execution
→ Activity
```

V3 的目标是把 Harness 从“只读本地能力”升级为：

> **一个可控、可配置、可审批、可审计的 Workspace Agent Runtime。**

V3 引入：

- `write`
- `edit`
- 动态工具开关
- 可配置 Policy
- Human Approval
- Developer/Test Mode `bash`
- 简体中文 / English
- V2 → V3 配置迁移
- 更完整的执行证据和安全边界

---

# 2. Product Principles

## P1 — External capability first

在继续开发 V3 大功能前，必须先证明 External ChatGPT Write Path 可用。

不得先完成大量 UI / Policy / Bash 功能，再到最后验证 ChatGPT 是否能够实际调用 write/modify MCP 工具。

---

## P2 — Server-side enforcement is authoritative

ChatGPT 客户端自身的确认、Tool UI 或安全提示都不能替代 Harness Runtime 的：

- WorkspaceGuard
- Tool state
- Policy
- Approval
- Execution validation

Runtime 是本地安全边界的最终执行者。

---

## P3 — Developer/Test Mode is not bypass mode

Developer/Test Mode 只能决定高风险能力是否具备暴露资格。

它不能绕过：

- Workspace constraints
- System Policy
- Approval
- Execution limits
- Audit

---

## P4 — Fail closed

以下情况不得默认 Allow：

- Unknown tool
- Invalid Policy
- Unsupported capability
- Invalid scope
- Stale disabled tool call
- Unclassifiable high-risk command
- Invalid approval state

---

## P5 — Observable, not inferred

UI 不自行推导工具或 Policy 的真实状态。

用户看到的状态必须来自 Runtime 的 effective result。

---

# 3. V3 Success Definition

V3 成功不等于“所有页面开发完成”。

最终必须证明：

```text
ChatGPT
→ Tunnel
→ Runtime
→ Tool Gate
→ Workspace/System Guardrails
→ Policy
→ Optional Approval
→ Execution
→ Activity/Audit
```

对于：

- `read_file`
- `glob`
- `write`
- `edit`
- `bash`

均符合其 V3 产品语义。

---

# 4. M0 Product Gate — Write/Edit External Acceptance

M0 是 V3 第一项需求，也是后续开发的 blocking gate。

## 4.1 Objective

验证 External ChatGPT 可以通过现有 Harness 链路：

1. 发现 `write`
2. 调用 `write`
3. 在 Workspace 中创建真实文件
4. 发现 `edit`
5. 调用 `edit`
6. 对该文件执行真实、精确修改

---

## 4.2 M0 Minimal Scope

只允许实现完成验收所需能力：

- `write`
- `edit`
- 对应 MCP Tool Schema
- 对应 Tool metadata / annotations
- WorkspaceGuard
- 最小 Policy enforcement
- Activity evidence
- Local MCP validation
- External ChatGPT validation

M0 不开发：

- Policy Editor
- Approval Center
- `bash`
- Tool Toggle
- i18n
- 新 Config UI
- 通用权限重构

---

## 4.3 Two-Level Acceptance

### Local Acceptance

使用 MCP Inspector 或等价 MCP 测试方式验证：

- tools/list
- write success
- edit success
- invalid input
- outside-workspace rejection

Local Acceptance 通过不能代替 External Acceptance。

### External ChatGPT Acceptance

必须由真实 ChatGPT 通过 Tunnel 调用 Harness。

Golden Path 1：

```text
Create:
v3-write-test.txt

Content:
hello from ChatGPT
```

期望：

- ChatGPT 调用 `write`
- Workspace 中生成文件
- 内容完全正确
- Activity 有记录

Golden Path 2：

```text
Replace exact text:
hello from ChatGPT
→
hello from V3
```

期望：

- ChatGPT 调用 `edit`
- 仅目标片段被修改
- Activity 有记录

Boundary Case：

```text
尝试写入 Workspace 外路径
```

期望：

- Runtime 拒绝
- 文件没有变化
- Activity/Audit 有拒绝证据

---

## 4.4 External Dependency

External ChatGPT write/modify MCP 能力由 ChatGPT 产品侧决定。

如果验收账号或 ChatGPT 当前产品能力不能调用 write/modify action：

```text
M0 = BLOCKED_BY_EXTERNAL_CAPABILITY
```

不得：

- 用 MCP Inspector PASS 冒充 External PASS
- 用 loopback PASS 冒充 External PASS
- 用 mock PASS 冒充 External PASS
- 将 Harness 标记为 M0 ACCEPTED

---

# 5. Tool Model

V3 内置工具：

| Tool | Normal Mode | Developer/Test Mode | Default |
| --- | --- | --- | --- |
| `read_file` | Available | Available | ON |
| `glob` | Available | Available | ON |
| `write` | Available | Available | ON |
| `edit` | Available | Available | ON |
| `bash` | Not eligible | Eligible | OFF |

---

# 6. Tool Enable / Disable

## 6.1 User Requirement

用户可在 Tools UI 独立控制每个可配置工具是否启用。

## 6.2 OFF Semantics

当一个工具为 OFF：

- Runtime 将其视为 disabled
- Runtime effective catalog 标记为不可用
- MCP `tools/list` 不再暴露该工具
- Runtime 拒绝来自 stale ChatGPT cache 的旧 tools/call
- Activity 应能区分“disabled”与“policy denied”

## 6.3 Client Cache Boundary

Harness 可以：

- 更新 Runtime 工具目录
- 发送 MCP 支持的 tool list change notification

但不能保证 ChatGPT 当前会话立即刷新客户端工具缓存。

产品不得将“ChatGPT UI 是否瞬时消失”作为 Runtime 开关是否正确的唯一判定。

---

# 7. `write` Product Requirement

## 7.1 Purpose

创建文本文件或完整覆盖目标文本文件。

## 7.2 Input Semantics

至少包含：

- `path`
- `content`

最终 Schema 在 Architecture / SPEC 阶段冻结。

## 7.3 Required Behavior

### File does not exist

```text
create
```

### File exists

```text
full overwrite
```

UI / MCP metadata 必须真实反映 overwrite 可能具有 destructive semantics。

## 7.4 Safety Boundary

`write` 必须：

- 经过 WorkspaceGuard
- 不允许 Workspace 外写入
- 不接受绕过路径边界的 path form
- 使用受控文本编码
- 对失败进行明确错误返回

## 7.5 Atomicity

禁止将正式 V3 `write` 实现为：

```text
truncate
→ partial write
```

必须具备原子性目标：

```text
validate
→ temporary write
→ flush
→ atomic replace / rename
```

具体 OS 实现由 Architecture 决定。

## 7.6 Evidence

记录：

- tool
- sanitized path
- policy result
- execution result
- duration

---

# 8. `edit` Product Requirement

## 8.1 Purpose

对已有文本文件执行精确局部修改。

## 8.2 Product Semantics

V3 第一版采用 exact replacement：

- `path`
- `old_text`
- `new_text`

最终 Schema 在 Architecture / SPEC 阶段冻结。

## 8.3 Matching Rule

```text
old_text match count = 0
→ FAIL

old_text match count = 1
→ EDIT

old_text match count > 1
→ FAIL
```

## 8.4 No Guessing

V3 `edit` 不进行：

- fuzzy match
- semantic guess
- approximate patch
- silent first-match selection

原则：

> Edit 宁可失败，也不能修改不确定位置。

## 8.5 Safety

与 `write` 一样必须经过 WorkspaceGuard。

---

# 9. Policy Product Model

## 9.1 Objective

用户能够配置工具执行策略，而不是只能查看 Runtime 当前 Policy。

## 9.2 Decision Types

```text
ALLOW
ASK
DENY
```

## 9.3 Evaluation Order

产品语义：

```text
Capability Gate
      ↓
System Guardrails
      ↓
User Policies
      ↓
Effective Decision
```

Decision severity：

```text
DENY > ASK > ALLOW
```

---

## 9.4 Capability Gate

负责判断：

- Tool 是否 implemented
- Tool 是否 enabled
- Tool 是否 exposed
- Mode 是否允许该 capability
- Runtime 是否 ready

Capability Gate 失败时不得进入 Allow。

---

## 9.5 System Guardrails

系统规则：

- 不可编辑
- 不可禁用
- 不可由 User Policy 覆盖

至少包括：

- Workspace boundary
- `bash` only in Developer/Test Mode
- Invalid configuration fail closed
- Unsupported capability fail closed

---

## 9.6 User Policy

用户可以：

- 创建
- 编辑
- 启用
- 禁用
- 删除

User Policy 至少能配置 Tool-level 默认决策：

```text
ALLOW
ASK
DENY
```

Bash 还需要 command-specific rules。

---

# 10. Policy Evidence

每次有效 Policy evaluation 必须能够提供：

- effective decision
- matched rule IDs
- decision reason

这些信息供：

- Activity
- Audit
- Approval
- Policy debugging
- UI effective state

使用。

Policy 判断证据不能只存在于日志字符串中。

---

# 11. Approval Product Requirement

## 11.1 Objective

真正实现 `ASK`，而不是把 ASK 当成不可用或 DENY。

## 11.2 State

Approval request 至少包含：

- Pending
- Approved
- Denied
- Expired
- Cancelled

## 11.3 User Actions

Approval Center 支持：

### Allow once

仅允许当前 request。

### Allow for session

允许本 Harness session 内符合相同授权范围的请求。

具体 session rule identity 在 Architecture 阶段决定。

### Deny

拒绝当前 request。

### Create rule

从当前请求生成 Policy 草稿。

必须进入明确的 Policy 编辑/保存流程。

不得把 Create Rule 设计成无确认的“永久允许”。

---

## 11.4 Approval Details

用户审批前至少能看到：

- Tool
- Sanitized arguments
- Target path / command
- Working directory
- Effective Policy
- Matched rule IDs / readable rules
- Risk level
- Reason
- Request time
- Expiry

---

## 11.5 Resume Semantics

ASK 请求：

```text
Request
 ↓
Policy = ASK
 ↓
Execution pauses
 ↓
Approval pending
 ↓
Decision
```

Allow 后只能继续与原 request identity 对应的执行。

不能将批准 A 请求错误复用于 B 请求。

---

# 12. Bash Product Requirement

## 12.1 Availability

`bash`：

- 仅 Developer/Test Mode
- 默认 OFF
- 默认 Policy = ASK

Normal Mode 必须不可暴露。

---

## 12.2 Runtime

Windows V3 优先支持 Git Bash：

```text
bash.exe
```

需要：

- Auto detect
- Manual override
- Clear unavailable state
- Remediation guidance

---

## 12.3 Security Position

V3 不声称 Bash 已受到完整 OS-level sandbox 隔离。

因此：

> Workspace cwd 不等于 filesystem sandbox。

产品文案和验收不得声称 `bash` 只能访问 Workspace，除非未来确实实现并验证 OS-level sandbox。

---

# 13. Bash Command Policy

## 13.1 Requirement

不能只依赖简单：

```text
contains
regex blacklist
```

作为主要安全策略。

Policy 应尽可能基于命令结构：

```text
executable
+
argv prefix / command pattern
```

示例：

```text
git status       → ALLOW
git diff         → ALLOW
git push         → ASK
git reset --hard → ASK
shutdown         → DENY
```

---

## 13.2 Compound Commands

对于可识别的组合命令：

```text
ALLOW + ALLOW → ALLOW
ALLOW + ASK   → ASK
ASK + DENY    → DENY
ALLOW + DENY  → DENY
```

始终采用最高风险 decision。

---

## 13.3 Unclassifiable Commands

对不能可靠静态判断的高风险 Shell 表达式，不得自动 ALLOW。

至少包括需要特别处理的形态：

- shell `-c`
- `eval`
- command substitution
- interpreter inline code
- compound operators
- nested shell execution

默认策略：

```text
ASK
```

System Guardrail 明确拒绝的情况仍为 DENY。

---

# 14. Bash Execution Limits

V3 Bash 必须有受控执行限制。

至少包括：

- execution timeout
- max output bytes
- max command length
- max concurrent bash executions
- child process ownership
- process-tree termination
- cancellation cleanup

如果触发 limit：

- 请求明确失败
- 子进程必须被清理
- Activity/Audit 记录原因

---

# 15. Activity & Audit

V3 Activity/Audit 至少能够关联：

- request ID
- tool
- sanitized arguments
- tool enabled state
- policy decision
- matched rule IDs
- approval ID
- execution result
- error category
- duration

对于 Bash 还应记录：

- sanitized command
- working directory
- timeout / limit outcome

Secret 不得进入 Activity。

---

# 16. Internationalization

## 16.1 Languages

V3：

- 简体中文
- English

## 16.2 Default

首次启动：

```text
follow OS language
```

不支持语言时 fallback：

```text
English
```

## 16.3 Settings

用户可在 Settings 手动修改。

要求：

- 即时切换
- 本地持久化
- 重启后保持

## 16.4 Translation Boundary

翻译：

- 页面标题
- 按钮
- 状态
- 提示
- 错误解释
- Policy friendly text
- Approval UI

不翻译：

- `read_file`
- `glob`
- `write`
- `edit`
- `bash`
- protocol field name
- policy ID
- machine-readable enum

---

# 17. Config Migration

当前 V2 配置不得因 V3 升级失效或丢失。

## 17.1 Must Preserve

- Workspace root
- Tunnel ID
- Tunnel client configuration
- Proxy configuration
- API Key secret reference
- Security mode

## 17.2 V3 Adds

预计新增：

- language
- tool enable states
- policy configuration
- bash configuration

最终 Config schema 由 Architecture 冻结。

## 17.3 Migration Requirement

V2 → V3：

- 自动迁移
- 可检测 migration failure
- migration failure 不得覆盖旧配置
- Secret 不重新写入普通配置文件
- 用户无需重新填写有效 Tunnel / API Key 信息

---

# 18. UI Product Requirements

V3 继续使用当前主要导航语义，但页面从 read-only observation 升级为管理界面。

## Tools

需要展示：

- Tool name
- Enabled
- Availability
- Effective Policy
- Mode restriction
- Reason

用户可以：

- Enable
- Disable

---

## Policies

需要展示：

- System Rules
- User Rules
- ALLOW / ASK / DENY
- Enabled state
- Scope / target
- Effective result

用户可以管理 User Rules。

System Rules 只读。

---

## Approval Center

需要：

- Pending count
- Approval details
- Allow once
- Allow for session
- Deny
- Create rule
- Expired / resolved history

---

## Activity

需要能区分：

- Success
- Denied
- Approval required
- Approval allowed
- Approval denied
- Disabled tool
- Execution failure
- Timeout / limit failure

---

# 19. External ChatGPT / MCP Requirements

MCP tool metadata 必须真实反映工具行为。

至少保证：

### read tools

`read_file` / `glob`：

```text
read-only semantics
```

### write tools

`write` / `edit`：

```text
not read-only
```

如果某个操作可能覆盖或破坏既有数据，对应 metadata 必须按实际行为声明。

不能通过虚假 annotation 来规避 ChatGPT 的确认体验。

---

# 20. Non-Goals for V3

除非后续 Architecture 明确扩大范围，V3 不包含：

- Full Windows OS sandbox
- Linux / macOS official release support
- Generic terminal abstraction
- PowerShell / CMD multi-shell platform
- Network sandbox
- Database tools
- Git-specific dedicated tool
- Plugin marketplace
- Multi-workspace
- Remote multi-user authorization
- Fuzzy AI edit engine

这些能力可以进入 V4+。

---

# 21. Quality Requirements

## Security

- All filesystem operations remain Workspace-aware
- Policies fail closed
- Approval cannot bypass System Guardrails
- Disabled tools cannot execute through stale calls
- Secrets never enter normal config or audit payloads

## Reliability

- write atomicity
- deterministic edit
- controlled bash process lifecycle
- deterministic Policy evaluation

## Observability

任何拒绝或失败都应能回答：

```text
What was requested?
Why was it allowed / asked / denied?
Was it executed?
What was the result?
```

---

# 22. V3 Acceptance

V3 最终验收必须覆盖真实 External ChatGPT：

1. `read_file`
2. `glob`
3. `write`
4. `edit`
5. Tool OFF
6. Policy DENY
7. Policy ASK
8. Approval Allow
9. Approval Deny
10. Developer/Test `bash`
11. Bash command DENY
12. Bash timeout / termination
13. Chinese / English persistence
14. V2 config migration

最终状态：

```text
V3 — ACCEPTED / FROZEN
```

只有 External ChatGPT E2E 完成后才能使用这一状态。

---

# 23. Architecture Questions — Explicitly Deferred

以下问题属于后续 `ARCHITECTURE.md`，本 PRD 不冻结答案：

- 是否升级 control protocol major/minor version
- Tool Toggle 热更新还是 Runtime restart 生效
- User Policy 存储位置
- Policy snapshot 如何进入 Runtime
- Approval owner 在 Runtime / Tauri 哪一层
- Approval pending request 如何持久化
- Session Allow 的 identity 模型
- Bash parser / command normalization 技术方案
- Bash process wrapper
- Tool list changed notification 实现
- Config schema version strategy
- UI state synchronization protocol
- Write atomic replace 的 Windows 具体实现

Architecture 必须满足本 PRD，但不得反向修改已冻结的产品语义；如确需修改，应先回到 PRD 重新评审。
