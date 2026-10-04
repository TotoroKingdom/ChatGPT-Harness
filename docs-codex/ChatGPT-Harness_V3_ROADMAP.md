# ChatGPT-Harness V3 Roadmap

> Status: DRAFT / TO BE FROZEN  
> Baseline: V2 — Read-only Harness, ACCEPTED / FROZEN  
> Scope: V3 product delivery sequence only  
> Architecture: **Not defined in this document**

---

## 1. V3 Objective

V3 将 ChatGPT-Harness 从已经验收的只读 Harness，升级为一个**可控的 Workspace Agent Runtime**：

- 支持 `write` / `edit`
- 支持工具启用 / 禁用
- 支持可配置的 `ALLOW / ASK / DENY` Policy
- 支持 Human Approval
- 支持受控 `bash`
- 支持简体中文 / English UI
- 保持 Workspace、Policy、Audit、Runtime、Tunnel 与 UI 的安全边界和可观察性

V3 不以“功能全部实现”为第一目标。

**第一目标是先证明 External ChatGPT Write Path 真实可用。**

---

## 2. Global Delivery Rule

V3 使用阶段门禁开发。

```text
M0 PASS
  ↓
才允许继续开发 M1+
```

### Hard Gate

在以下链路完成真实人工验收前，不进入后续 V3 功能开发：

```text
ChatGPT
  ↓
Tunnel
  ↓
Runtime MCP
  ↓
write / edit
  ↓
Workspace filesystem
```

本地单元测试、MCP Inspector、loopback HTTP、mock、Runtime 进程存在、Tool Scan 成功，**均不能替代 External ChatGPT E2E**。

如果 External ChatGPT 当前不具备 write/modify MCP 能力：

```text
M0 = BLOCKED_BY_EXTERNAL_CAPABILITY
```

不得标记为 PASS。

---

# M0 — Write/Edit Capability Acceptance Gate

## Goal

用最小改动验证 ChatGPT 能通过当前 V2 链路真实调用：

- `write`
- `edit`

并在 Workspace 中产生预期文件变化。

## Scope

只实现 M0 验收必要能力：

- Runtime 注册 `write`
- Runtime 注册 `edit`
- MCP 正确暴露写工具
- WorkspaceGuard 继续生效
- 最小 Policy enforcement
- Activity 能记录调用结果
- Local MCP validation
- External ChatGPT manual acceptance

## Explicitly Out of Scope

M0 不开发：

- Policy Editor
- Approval Center
- `bash`
- Tool Toggle
- i18n
- Policy Simulator
- 新的通用权限架构
- 大规模 UI 重构

## Exit Criteria

必须同时完成：

1. MCP Inspector 能发现并成功调用 `write`
2. MCP Inspector 能发现并成功调用 `edit`
3. `write` 能创建 Workspace 内文件
4. `edit` 能精确修改已有文件
5. Workspace 外写入被拒绝
6. External ChatGPT 能发现并调用 `write`
7. External ChatGPT 能发现并调用 `edit`
8. External ChatGPT 调用后真实文件变化正确
9. Activity 中存在对应执行记录

只有以上全部通过，M0 才能标记为：

```text
ACCEPTED / FROZEN
```

---

# M1 — Write/Edit Production Hardening

## Goal

将 M0 的 write/edit 从“链路验证能力”升级为正式 V3 文件修改能力。

## Scope

### write

- Create
- Full overwrite
- Workspace only
- Atomic write / atomic replace
- UTF-8 text boundary
- 明确错误语义

### edit

- Exact patch
- `old_text` 必须唯一匹配
- 0 次匹配失败
- 多次匹配失败
- 不进行 fuzzy guessing
- Workspace only

## Exit Criteria

write/edit 的正常路径、错误路径和 Workspace 边界均有自动化测试，并完成 Runtime Integration 验证。

---

# M2 — Tool Enable / Disable

## Goal

让用户控制 Harness 当前向 MCP 暴露哪些工具。

## Default Tool State

| Tool | Default |
| --- | --- |
| `read_file` | ON |
| `glob` | ON |
| `write` | ON |
| `edit` | ON |
| `bash` | OFF |

## Required Behavior

工具关闭后：

- Runtime 的有效工具目录不再提供该工具
- MCP `tools/list` 不再暴露该工具
- Runtime 必须拒绝 stale / cached 的旧调用
- Tool 配置通过下一 Runtime generation 生效
- V3 不要求同一 Runtime generation 内实现 `tools/list_changed`
- UI 显示真实 Runtime effective state，并明确 Restart Required

不要求 ChatGPT 当前会话在没有 Refresh 的情况下瞬时更新其客户端缓存。

## Exit Criteria

1. 每个 configurable Tool 的 desired ON/OFF 状态可以持久化。
2. Harness 运行中修改 Tool 配置后，当前 Runtime effective state 不被伪装为已变更，并明确显示 Restart Required。
3. Restart 后 Runtime effective state 与 saved desired state 一致。
4. OFF Tool 不出现在新的 Runtime generation 的 MCP `tools/list`。
5. stale / cached `tools/call` 被 Runtime 拒绝。
6. enabled / disabled / stale-call 路径有自动化验证。

---

# M3 — Configurable Policy

## Goal

将现有只读 Policy 能力升级为真实可配置的安全策略系统。

## Policy Model

```text
Capability Gate
      ↓
System Guardrails
      ↓
User Policy
      ↓
ALLOW / ASK / DENY
```

优先级：

```text
DENY > ASK > ALLOW
```

## Required Behavior

- System Rules 不可被用户关闭或覆盖
- User Rules 可创建、编辑、启用、禁用、删除
- 无匹配显式 User Rule 时使用 Tool Default
- 非法 / 无法编译的 Policy 配置 fail closed
- 每次判定保留 matched rule evidence
- Policy UI 展示 effective decision，而非仅展示配置值

## Exit Criteria

1. System Guardrails 保持 Runtime-owned、只读且不可由 User Policy 覆盖。
2. User Rules 支持创建、编辑、启用、禁用、删除并可持久化。
3. 存在匹配 User Rules 时按 `DENY > ASK > ALLOW` 得到确定结果。
4. 无匹配显式 User Rule 时使用对应 Tool Default。
5. 任一 enabled invalid rule 使 Policy snapshot fail closed，不允许跳过坏规则寻找 ALLOW。
6. Runtime 输出 effective decision、matched rule IDs 和 decision reason，UI 不自行重算。

---

# M4 — Human Approval Workflow

## Goal

真正实现 `ASK` 的暂停、人工决策和继续执行。

## Required Actions

- Allow once
- Allow for session
- Deny
- Create rule

## Approval Evidence

Approval UI 至少展示：

- Tool
- Sanitized arguments
- Target path / command
- Working directory
- Matched policies
- Risk level
- Reason
- Created time / expiry

## Required Runtime Behavior

```text
ASK
 ↓
Pending
 ↓
User Decision
 ├─ Allow → resume exactly once / session rule
 └─ Deny  → terminate request
```

审批不能绕过 WorkspaceGuard 或 System Guardrails。

## Exit Criteria

1. `ASK` 请求保持原 request identity 并进入 Pending，而不是立即丢弃请求。
2. Allow once 只恢复当前精确请求；Deny 后请求不执行。
3. Allow for session 只创建当前 Runtime generation 内、范围不宽于审批展示范围的 grant。
4. Runtime stop / crash 会取消 pending approvals 和 session grants。
5. Approval 无法绕过 WorkspaceGuard、System Guardrails 或显式 DENY。
6. Approval request / resolution 与执行结果具有关联 evidence。

---

# M5 — Controlled Bash

## Goal

增加面向开发工作的高风险 Shell 工具。

## Availability

`bash`：

- 仅 Developer/Test Mode 有资格启用
- 默认 OFF
- 默认 Policy = ASK
- Normal Mode 不暴露

## Runtime Boundary

V3 不承诺完整 OS-level sandbox。

因此 Bash 必须被明确视为高风险能力，并至少具备：

- Command Policy
- Approval
- Workspace working directory
- Timeout
- Max output
- Max command length
- Max concurrency
- Process-tree termination
- Audit

## Bash Runtime

Windows 优先使用 Git Bash：

```text
bash.exe
```

支持：

- Auto detect
- Manual override
- Unavailable state + remediation

## Command Policy

不以简单字符串黑名单作为主机制。

Policy 应针对可识别的命令结构进行判定，例如：

```text
executable + argv prefix
```

组合命令按最高风险结果处理：

```text
ALLOW + ASK  → ASK
ALLOW + DENY → DENY
```

无法可靠分类的高风险 Shell 表达式默认进入 ASK。

## Exit Criteria

1. Normal Mode 不暴露 `bash`；Developer/Test Mode 也只有在 enabled、executable ready、Runtime ready 且 Policy 非 DENY 时才具备暴露资格。
2. Git Bash 支持 auto-detect、manual override 和明确的 unavailable/remediation 状态。
3. 可可靠分析的 command / compound command 产生 deterministic policy facts，并按最高 severity 决策。
4. 无法可靠分类的高风险表达式进入 ASK，而不是 ALLOW。
5. timeout、output、command length、concurrency 和 cancellation limits 被 Runtime enforcement。
6. timeout / cancellation / Runtime shutdown 能终止本次 invocation 拥有的 process tree，并留下 Activity/Audit evidence。

---

# M6 — Chinese / English UI

## Goal

提供完整的双语 UI。

## Scope

- 简体中文
- English
- 首次启动跟随系统语言
- Settings 中手动切换
- 即时生效
- 本地持久化

## Boundary

不翻译：

- MCP tool name
- Protocol field
- Runtime machine identifier
- Policy internal ID

只翻译用户可见文案。

## Exit Criteria

1. 首次启动在无 saved language 时跟随 OS language，不支持的语言 fallback English。
2. Settings 可在简体中文 / English 间即时切换，不要求 Runtime restart。
3. language choice 本地持久化并在应用重启后保持。
4. V3 主要用户可见页面、状态、提示和错误解释均使用 i18n resource。
5. MCP tool name、protocol field、Runtime machine identifier 和 Policy internal ID 保持未翻译。

---

# M7 — Migration, Evidence & UX Hardening

## Goal

让 V3 能安全升级、可诊断、可长期使用。

## Scope

### Config Migration

产品升级场景是 `ChatGPT-Harness V2 → V3`；持久化 AppConfig 的实际 schema migration 是 `schemaVersion v1 → v2`。两者不得混用。

V2 配置必须无损迁移至 V3：

- Workspace
- Tunnel ID
- API Key reference
- Tunnel settings
- Security mode

新增 V3 配置：

- Language
- Tool states
- User policies
- Bash configuration

### Policy / Execution Evidence

Activity / Audit 至少保留：

- Tool
- Sanitized arguments
- Policy decision
- Matched rule IDs
- Approval ID
- Execution result
- Duration

### UX Hardening

- 清晰的 Disabled / Denied / Ask / Unavailable 状态
- 可执行的 remediation
- Policy 错误不得静默降级为 Allow
- 工具状态、Policy 状态与 Runtime effective state 保持一致

## Exit Criteria

1. `schemaVersion v1 → v2` migration 自动完成并保留 Workspace、Tunnel、API Key reference、Proxy/Tunnel settings 和 Security Mode。
2. migration failure 不覆盖原配置、不明文化 SecretStore secret，并给出可诊断错误。
3. Activity/Audit 能关联 Tool、sanitized arguments、Policy decision、matched rule IDs、Approval ID、execution result 和 duration。
4. Disabled / Denied / Ask / Unavailable / execution failure 等状态在 UI 中可区分，并提供适用的 remediation。
5. desired config 与 Runtime effective state / configRevision 不一致时 UI 明确显示 Restart Required，不把 desired state 冒充为 effective state。
6. migration、evidence 和关键 UX 状态具备自动化回归验证。

---

# M8 — UI Visual Refresh

## Goal

对现有 UI 做视觉与交互表现升级。

## Boundary

- M0–M7 已实现的产品能力、安全语义和 Runtime 行为保持不变。
- M8 完成后，再进入 `M9 — Full V3 External Acceptance`。
- 具体视觉风格、页面方案、Design System、`SPEC.md` / `PLAN.md` / `TASKS.md` 后续单独讨论后再确定。

---

# M9 — Full V3 External Acceptance

## Goal

对完整 V3 能力执行最终 External ChatGPT 手工验收。

## Golden Paths

### File write

```text
ChatGPT → write → Workspace file created
```

### File edit

```text
ChatGPT → edit → exact file modification
```

### Tool disable

```text
Disable tool
→ Runtime no longer exposes it
→ stale call rejected
```

### Policy deny

```text
ChatGPT → tool call
→ Policy DENY
→ no execution
→ Activity evidence
```

### Approval

```text
ChatGPT → ASK operation
→ Approval Center
→ Allow once
→ exact request resumes
```

### Bash

```text
Developer/Test Mode
→ bash enabled
→ ASK
→ approve
→ command executes
→ output + audit recorded
```

### Bash deny

```text
Denied command
→ no shell execution
→ evidence recorded
```

## Exit Criteria

V3 只有在完整 External ChatGPT 链路完成手工验收后才能标记：

```text
V3 — ACCEPTED / FROZEN
```

自动化测试只能声明其真实覆盖范围，不能替代 External ChatGPT E2E。

---

## 3. Milestone Dependency

```text
V2 ACCEPTED
    ↓
M0 Write/Edit Acceptance Gate
    ↓
M1 Write/Edit Hardening
    ↓
M2 Tool Toggle
    ↓
M3 Configurable Policy
    ↓
M4 Approval
    ↓
M5 Controlled Bash
    ↓
M6 i18n
    ↓
M7 Migration / Evidence / Hardening
    ↓
M8 UI Visual Refresh
    ↓
M9 Full External Acceptance
    ↓
V3 ACCEPTED / FROZEN
```

---

## 4. Architecture Boundary

本 Roadmap 只冻结：

- 开发顺序
- Milestone 目标
- Gate
- Capability scope
- Acceptance boundary

以下内容必须在独立 `ARCHITECTURE.md` 中决定：

- V3 配置模型最终结构
- Policy storage / synchronization
- Approval state ownership
- Tool registry 动态更新机制
- Bash command parser
- Process execution architecture
- UI ↔ Tauri ↔ Runtime 写接口
- Protocol versioning / compatibility strategy
- Runtime restart / hot reload 边界

在 Architecture 冻结前，不应从本 Roadmap 推导具体实现方案。
