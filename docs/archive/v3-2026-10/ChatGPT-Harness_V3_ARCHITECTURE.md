# ChatGPT-Harness V3 Architecture

> Status: DRAFT / TO BE FROZEN  
> Baseline: V2 Architecture — ACCEPTED / FROZEN  
> Product constraints: `ChatGPT-Harness_V3_PRD.md`  
> Delivery constraints: `ChatGPT-Harness_V3_ROADMAP.md`  
> Platform: Windows first  
> Scope: Technical architecture and component boundaries only

---

# 1. Architecture Goal

V3 在不破坏 V2 已验收主链路的前提下，将 ChatGPT-Harness 从只读 Runtime 扩展为支持：

- `write`
- `edit`
- Tool enable / disable
- Configurable Policy
- Human Approval
- Controlled `bash`
- zh-CN / English UI
- V2 → V3 config migration
- richer execution evidence

V3 的核心架构原则不是重构现有系统，而是：

> **保留 V2 已验证的进程边界、Control Plane、MCP Data Plane、生命周期 ownership 和单一 Runtime 执行入口，只扩展 Runtime 内的 capability、authorization 和 execution components。**

---

# 2. Architecture Baseline

当前 V2 已形成并验收以下架构：

```text
Control Plane

React UI
   ↓ Tauri IPC
Tauri Host
   ↓ stdin/stdout JSONL
Runtime Control Server


MCP Data Plane

ChatGPT
   ↓
Tunnel Client
   ↓
Runtime MCP
   ↓
ExecutionPipeline
   ↓
Workspace / Policy / Tools
```

进程关系：

```text
┌──────────────────────────────────────┐
│            Tauri Desktop App         │
│                                      │
│ React UI                             │
│   ↕                                  │
│ HarnessController                    │
│ ConfigStore / SecretStore            │
│ RuntimeSupervisor                    │
│ TunnelProcessManager                 │
└───────────────┬──────────────────────┘
                │
                ├──── spawn ─── Runtime
                │                ├ Control Server
                │                └ MCP HTTP Server
                │
                └──── spawn ─── Tunnel Client

ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
```

V3 **保持这一结构不变**。

---

# 3. Architecture Decisions Summary

| ID | Decision |
| --- | --- |
| ADR-01 | 保留 React → Tauri → Runtime Control Plane |
| ADR-02 | 保留 ChatGPT → Tunnel → Runtime MCP Data Plane |
| ADR-03 | Tauri 继续作为 Harness lifecycle / config / secret owner |
| ADR-04 | Runtime 继续作为 tool authorization / execution authority |
| ADR-05 | 所有工具继续通过单一 `ExecutionPipeline` |
| ADR-06 | `ToolRegistry` 继续作为 Runtime 工具定义与 capability 的单一事实源 |
| ADR-07 | Runtime 继续使用 per-generation immutable config snapshot |
| ADR-08 | 持久化 Tool / Policy / Security / Bash 配置在 V3 不做 live hot reload；通过下一 Runtime generation 生效 |
| ADR-09 | Approval 与 session grants 属于 Runtime generation 内的实时状态 |
| ADR-10 | `WorkspaceGuard` 保留现有 existing-path 行为，并新增独立 write-target validation |
| ADR-11 | `ALLOW` 与 `ASK` 工具可以进入 MCP exposure；`DENY` / disabled / unavailable 不暴露 |
| ADR-12 | Policy 使用 System Guardrails + User Policy + Tool Default 三层语义 |
| ADR-13 | `bash` 在 Runtime 内执行，Runtime 自己拥有其 process tree |
| ADR-14 | V3 config schema 升级并显式执行 v1 → v2 migration |
| ADR-15 | Control protocol 继续沿用现有 `protocol/src/v1` 架构，优先 additive evolution |
| ADR-16 | i18n 只属于 UI presentation layer，不进入 Runtime protocol |
| ADR-17 | V3 不引入新的数据库、daemon 或独立 policy service |
| ADR-18 | V3 不实现完整 OS-level Bash sandbox |

---

# 4. Target Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                    │
│                                                         │
│  React UI                                               │
│  ├ Tools                                                │
│  ├ Policies                                             │
│  ├ Approval Center                                      │
│  ├ Activity                                             │
│  ├ Diagnostics                                          │
│  └ Settings / i18n                                      │
│          │                                              │
│          ▼                                              │
│  HarnessControlClient                                   │
│          │                                              │
│          ▼                                              │
│  Tauri Commands                                         │
│  ├ ConfigStore ────── AppConfig v2                      │
│  ├ SecretStore                                          │
│  ├ HarnessController                                    │
│  ├ RuntimeSupervisor                                    │
│  └ TunnelProcessManager                                 │
└──────────┬───────────────────────────────┬───────────────┘
           │ JSONL Control RPC             │ process
           ▼                               ▼
┌────────────────────────────────┐      Tunnel Client
│            Runtime             │           ▲
│                                │           │
│ RuntimeCore                    │           │
│ ├ RuntimeConfigSnapshot        │       ChatGPT
│ ├ ToolRegistry                 │
│ ├ CapabilityGate               │
│ ├ WorkspaceGuard               │
│ ├ PolicyEngine                 │
│ ├ ApprovalManager              │
│ ├ SessionGrantStore            │
│ ├ ExecutionPipeline            │
│ ├ Activity / Audit             │
│ ├ BashRuntime                  │
│ └ MCP Service                  │
│       ▲                        │
│       │ loopback /mcp          │
└───────┼────────────────────────┘
        │
        └──────── Tunnel MCP target
```

---

# 5. Ownership Boundaries

## 5.1 React UI

React UI 负责：

- 用户输入
- configuration intent
- Tools / Policies / Approval / Activity 展示
- i18n
- restart-required 提示
- 调用 `HarnessControlClient`

React UI 不负责：

- path authorization
- Policy enforcement
- approval authorization
- bash execution
- tool-call routing
- secret storage
- Runtime process ownership

---

## 5.2 Tauri Host

Tauri 继续负责：

```text
ConfigStore
SecretStore
HarnessController
RuntimeSupervisor
TunnelProcessManager
Tauri Commands / Event Bridge
```

新增职责仅限：

- V3 AppConfig migration
- Tool / Policy / Bash 持久配置管理
- bash executable manual-picker UX
- configuration revision / restart-required coordination

Tauri **不成为 MCP tool-call proxy**。

V3 的 tool call 仍然：

```text
ChatGPT
→ Tunnel
→ Runtime MCP
→ Runtime
```

不会变为：

```text
ChatGPT
→ Tunnel
→ Tauri
→ Runtime
```

---

## 5.3 Runtime

Runtime 是本地 capability/security/execution authority。

Runtime 负责：

- Runtime config snapshot
- ToolRegistry
- CapabilityGate
- WorkspaceGuard
- PolicyEngine
- ApprovalManager
- SessionGrantStore
- ExecutionPipeline
- tool executors
- Bash child processes
- execution evidence
- MCP Server

任何 Tool Adapter 都不能绕过 Runtime enforcement。

---

## 5.4 Tunnel

Tunnel 继续只负责：

```text
ChatGPT
↔ remote tunnel/control plane
↔ Runtime MCP endpoint
```

V3 不修改 Tunnel 的产品职责。

Tunnel 不理解：

- Policy
- Approval
- Workspace
- Tool Toggle
- Bash rules
- i18n

---

# 6. Runtime Configuration Model

## 6.1 Keep Immutable Runtime Generations

V2 Runtime 在初始化时接收不可变 `RuntimeConfigSnapshot`。

V3 保留该模型。

每一个 Runtime generation 拥有：

```text
RuntimeConfigSnapshot
├ workspace
├ security
├ tools
├ policies
├ bash
└ runtime_limits
```

Runtime generation 启动后：

- persistent tool configuration 不热更新
- persistent policy configuration 不热更新
- Security Mode 不热更新
- Bash executable configuration 不热更新

这些变化在下一次 Runtime generation 生效。

---

## 6.2 Why No Persistent Hot Reload in V3

当前 Runtime 的稳定性建立在：

```text
immutable initialized snapshot
+
generation identity
+
deterministic failure convergence
```

如果 V3 为 Tool / Policy 引入 live mutable distributed state，需要同步：

```text
ConfigStore
Tauri
Runtime
ExecutionPipeline
MCP Tool List
ChatGPT cache
```

并处理：

- apply transaction
- rollback
- concurrent request consistency
- config revision races
- partial apply
- stale UI state

这会显著改变 V2 已验证的架构。

因此 V3 采用：

```text
Save persistent config
        ↓
configRevision changes
        ↓
Harness reports restart required
        ↓
Restart Harness
        ↓
new Runtime generation
        ↓
new immutable RuntimeConfigSnapshot
```

这是 V3 的正式配置应用模型。

---

## 6.3 Desired State vs Effective State

UI 必须区分：

### Desired State

来自 Tauri `AppConfig`。

表示下一 Runtime generation 应使用的配置。

### Effective State

来自当前 Runtime。

表示当前 generation 正在执行的真实状态。

例如：

```text
Desired:
write.enabled = false

Current Runtime:
write.enabled = true

Result:
Restart required
```

UI 不得把 Desired State 冒充为当前 Runtime Effective State。

---

## 6.4 Restart Detection

现有 Harness 已保存：

```text
active config_revision
```

Tauri ConfigStore 保存：

```text
latest config_revision
```

如果：

```text
active_revision != latest_revision
```

则：

```text
restart_required = true
```

不引入另一套 dirty-state identity。

---

# 7. V3 AppConfig

V3 将 `schemaVersion` 从当前版本升级到下一 schema。

概念结构：

```text
AppConfig
├ schemaVersion
├ configRevision
├ workspace
├ tunnel
├ security
├ ui
│  ├ theme
│  └ language
├ tools
│  ├ read_file.enabled
│  ├ glob.enabled
│  ├ write.enabled
│  ├ edit.enabled
│  └ bash.enabled
├ policies
│  ├ tool defaults
│  └ user rules
└ bash
   ├ autoDetect
   ├ executablePath?
   └ limits
```

具体 serialized field naming 在 SPEC 中冻结，但以上 ownership 不再变化。

---

## 7.1 Runtime Snapshot Excludes Host-only Data

传给 Runtime：

```text
workspace
security
tools
policies
bash
runtime_limits
config_revision
```

不传给 Runtime：

```text
Tunnel ID
Tunnel API Key reference
Tunnel proxy credentials
UI language
UI theme
```

API Key 仍然只能通过 SecretStore → Tunnel launch 使用。

---

## 7.2 SecurityMode

V3 继续以：

```text
SecurityMode::Normal
SecurityMode::DeveloperTest
```

作为 authoritative mode。

现有 `developer_test_mode` boolean 如继续保留，只作为兼容字段：

```text
developer_test_mode
==
(mode == DeveloperTest)
```

不得成为第二个独立安全状态源。

---

# 8. Config Migration Architecture

当前 ConfigStore 对未知 schema 直接 fail closed。

V3 新增显式 migration pipeline：

```text
read config.json as raw JSON
        ↓
inspect schemaVersion
        ↓
v1 ?
 ├ yes → migrate v1 → v2 in memory
 └ no  → validate supported schema
        ↓
deserialize typed V3 AppConfig
        ↓
normalize
        ↓
validate
        ↓
atomic persist
```

---

## 8.1 Migration Safety

Migration 必须：

- 先在内存完成
- 全部验证通过后才替换原文件
- 继续使用 atomic replace
- migration failure 不覆盖旧配置
- 不移动或明文化 SecretStore secret
- 保留 Tunnel ID
- 保留 API Key reference
- 保留 Workspace
- 保留 Proxy / Tunnel client config
- 保留 Security Mode

---

## 8.2 V1 → V2 Defaults

迁移时新增：

```text
read_file = enabled
glob      = enabled
write     = enabled
edit      = enabled
bash      = disabled
```

默认 Policy：

```text
read_file = ALLOW
glob      = ALLOW
write     = ALLOW
edit      = ALLOW
bash      = ASK
```

Language：

```text
auto / system language
```

Bash：

```text
autoDetect = true
```

---

# 9. Tool Architecture

## 9.1 Keep ToolRegistry as the Single Tool Definition Source

当前 `ToolRegistry` 已经负责：

- tool descriptor
- implemented
- enabled
- exposed
- readiness
- capability

V3 保留这一模型。

禁止出现：

```text
MCP has one tool list
UI has another tool list
Policy has another hardcoded name list
```

所有工具 identity 都从 ToolRegistry 派生。

---

## 9.2 V3 Tool Capabilities

`ToolCapability` 扩展为至少：

```text
WorkspaceRead
WorkspaceWrite
DeveloperShell
```

映射：

```text
read_file → WorkspaceRead
glob      → WorkspaceRead
write     → WorkspaceWrite
edit      → WorkspaceWrite
bash      → DeveloperShell
```

---

## 9.3 Built-in Definition vs Configured State

ToolRegistry 中区分：

### Built-in Definition

编译进 Runtime：

- name
- description
- capability
- MCP metadata
- risk level
- implementation presence

### Configured State

来自 RuntimeConfigSnapshot：

- enabled
- applicable security mode
- configured policy default

### Runtime Readiness

运行时计算：

- Runtime ready
- Bash executable available
- executor capability available

最终得到：

```text
EffectiveTool
```

---

# 10. CapabilityGate

V3 从现有 EffectiveTool computation 中提取明确的 Capability Gate 语义。

调用顺序首先判断：

```text
implemented?
enabled?
exposed?
mode eligible?
runtime ready?
capability ready?
```

失败则不进入普通 Policy Allow。

---

## 10.1 Stale MCP Calls

即使 ChatGPT 缓存了旧工具：

```text
tool disabled
        ↓
stale tools/call
        ↓
Runtime CapabilityGate
        ↓
reject
```

不能依赖 `tools/list` 隐藏来提供安全保证。

---

# 11. MCP Exposure Model

V2：

```text
ALLOW → exposed
ASK   → hidden
DENY  → hidden
```

V3 必须调整为：

```text
ALLOW → exposed
ASK   → exposed, execution requires Approval
DENY  → hidden
```

以及：

```text
disabled          → hidden
mode-ineligible   → hidden
runtime-unavailable → hidden
```

---

## 11.1 EffectiveTool Availability

建议扩展状态：

```text
Available
AvailableWithApproval
UnavailableByPolicy
UnavailableByMode
UnavailableByRuntime
UnavailableDisabled
```

MCP Exposure：

```text
Available
AvailableWithApproval
```

都进入 `tools/list`。

---

## 11.2 Direct Call Is Always Re-evaluated

MCP `call_tool` 不信任之前的 `tools/list`。

每次请求重新经过：

```text
CapabilityGate
WorkspaceGuard
PolicyEngine
Approval
```

---

## 11.3 MCP List Change Strategy

由于 V3 persistent Tool state 在新 Runtime generation 生效：

- V3 不要求同一 Runtime generation 内进行 Tool hot reload
- V3 不依赖 MCP `tools/list_changed` 完成配置应用
- Harness Restart 会创建新的 MCP generation / endpoint exposure state

未来如果引入 live Tool reload，再增加 list-changed notification。

---

# 12. MCP Tool Adapters

保持当前模式：

```text
MCP JSON
   ↓
tool-specific arguments
   ↓
typed Request
   ↓
ExecutionPipeline
   ↓
typed Output
   ↓
structured MCP result
```

MCP Adapter 只负责：

- schema
- deserialize
- result mapping
- sanitized error mapping
- tool annotations

MCP Adapter 不负责：

- Workspace authorization
- Policy authorization
- Approval decisions
- file mutation
- shell execution

---

## 12.1 Tool Annotations

### read_file / glob

```text
readOnly = true
destructive = false
```

### write

```text
readOnly = false
destructive = true
openWorld = false
```

因为 write 可以覆盖已有文件。

### edit

```text
readOnly = false
destructive = true
openWorld = false
```

### bash

```text
readOnly = false
destructive = true
openWorld = true
```

因为 V3 Bash 没有 OS network/filesystem sandbox。

Annotations 只描述行为。

它们不是 Harness authorization mechanism。

---

# 13. Unified ExecutionPipeline

V3 保留单一 ExecutionPipeline。

目标流程：

```text
RequestReceived
      ↓
ArgumentValidation
      ↓
CapabilityGate
      ↓
WorkspaceValidation
      ↓
PolicyEvaluation
      ↓
 ┌────┼──────────────┐
 │    │              │
DENY ASK           ALLOW
 │    │              │
 ▼    ▼              ▼
end Approval      Execute
     Manager          │
       │              │
       ▼              │
    decision          │
    ├ deny → end      │
    └ allow ──────────┘
                      ↓
               ExecutionCompleted
                      ↓
                  Evidence
```

---

## 13.1 ExecutionPermit

保留现有私有 `ExecutionPermit` 设计：

> Tool executor 只有在 Pipeline 授权后才能得到执行 permit。

MCP handler、Runtime helper、UI 都不能直接创建 permit。

---

## 13.2 ToolExecutor Evolution

当前 ToolExecutor 的原则保留：

> executor 做 typed validation / execution，不做 authorization。

为支持 V3，接口需要能够提供：

```text
tool name
workspace target semantics
policy facts
sanitized arguments
execution
```

Workspace target 不再只有单一 existing path。

建议概念：

```text
WorkspaceTarget
├ ExistingPath
├ WriteTarget
└ WorkspaceRoot
```

映射：

```text
read_file → ExistingPath
glob      → ExistingPath / directory
edit      → ExistingPath
write     → WriteTarget
bash      → WorkspaceRoot
```

---

## 13.3 Async-capable Pipeline

V2 的 ExecutionPipeline 是同步执行。

V3 的 Approval 和 Bash timeout 需要非阻塞等待。

因此 V3 ExecutionPipeline 应升级为 async-capable request lifecycle：

- file operations 可以继续使用同步执行内部实现
- Approval wait 必须异步
- Bash process wait / timeout 必须异步或放入受控 blocking execution
- 不能阻塞 Runtime MCP 的 Tokio event loop 等待用户批准

这属于 Runtime 内部实现升级，不改变进程架构。

---

# 14. Workspace Architecture

## 14.1 Preserve Existing Read Boundary

当前 WorkspaceGuard 对 existing targets：

- canonicalize
- reject escape
- reject Windows special path forms
- reject sibling-prefix attack
- reject outside symlink / junction targets
- missing target fail closed

V3 保留现有 existing-target 行为。

不能为了实现 `write`：

> 把现有 `approve()` 改成“missing path 也通过”。

否则会改变 read/edit/glob 的安全语义。

---

## 14.2 Add Dedicated Write Target Validation

新增独立的 write-target path：

```text
approve_write_target(...)
```

概念流程：

```text
validate path form
      ↓
normalize logical path
      ↓
identify parent
      ↓
parent must exist
      ↓
canonicalize parent
      ↓
verify parent remains in Workspace
      ↓
if target exists:
    canonicalize target
    verify target in Workspace
      ↓
return approved write target
```

V3 `write` 默认只创建：

> 已存在且已批准 parent directory 下的新文件。

不把 recursive directory creation 隐式塞进 `write`。

---

## 14.3 TOCTOU Rule

文件系统可以在：

```text
validation
→ approval wait
→ execution
```

之间发生变化。

因此：

> mutation executor 必须在真正提交变更前重新执行关键 path validation。

一次早期 Workspace approval 不能永久授权一个之后可能已经改变的 symlink / junction target。

---

# 15. Write Architecture

`write` executor 语义：

```text
path
content
```

执行路径：

```text
validate request
      ↓
approve write target
      ↓
Policy
      ↓
optional Approval
      ↓
revalidate target
      ↓
create temp file in same parent
      ↓
write full content
      ↓
flush
      ↓
sync
      ↓
atomic replace / rename
      ↓
return result
```

---

## 15.1 Atomic Replacement

Runtime 不依赖 `ui/src-tauri` crate。

V3 在 Runtime 内实现文件 atomic replace primitive。

Windows 可采用与当前 ConfigStore 已验证思路一致的：

```text
MoveFileExW
REPLACE_EXISTING
WRITE_THROUGH
```

但实现归 Runtime 自己所有。

不为了一个 helper 引入新的 shared platform crate。

---

## 15.2 Write Result

结构化结果应至少能够表达：

- normalized logical path
- bytes written
- created / overwritten

绝对 Workspace root 不应无必要暴露给 ChatGPT。

---

# 16. Edit Architecture

`edit` 使用：

```text
path
old_text
new_text
```

执行：

```text
approve existing file
      ↓
read exact text
      ↓
count old_text matches
      ↓
0   → fail
1   → replace
>1  → fail
      ↓
atomic file replacement
```

`edit` 和 `write` 共享 atomic replacement primitive。

不实现：

- fuzzy matching
- first-match guessing
- semantic patching

---

# 17. Policy Architecture

## 17.1 Separate Hard Guardrails from User Policy

最终决策链：

```text
Capability Gate
      ↓
System Guardrails
      ↓
User Policy
      ↓
Session Grant
      ↓
Effective Decision
```

---

## 17.2 System Guardrails

System Guardrails 由 Runtime 代码和 system rules 共同实现。

不可：

- edit
- disable
- delete
- override

至少包括：

- Workspace path enforcement
- invalid configuration fail closed
- unsupported capability fail closed
- Bash only in Developer/Test Mode
- Runtime readiness
- stale disabled Tool rejection

WorkspaceGuard 是硬 enforcement。

System Policy 不能代替 WorkspaceGuard。

---

## 17.3 Tool Default Policy

每个工具有 default decision：

```text
read_file → ALLOW
glob      → ALLOW
write     → ALLOW
edit      → ALLOW
bash      → ASK
```

Tool Default 是 fallback。

它不是一个永远参与 severity competition 的固定 ASK rule。

否则：

```text
bash default ASK
+
user explicit ALLOW
```

永远无法变成 ALLOW。

---

## 17.4 User Rules

如果有匹配的显式 User Rules：

```text
DENY > ASK > ALLOW
```

多个同级匹配 rule 采用最高 severity。

如果没有显式规则：

```text
use tool default
```

---

## 17.5 Policy Rule Shape

V3 不再只靠一个自由字符串 `scope` 表达所有逻辑。

Policy 需要 typed matcher。

概念：

```text
PolicyRule
├ id
├ name
├ enabled
├ source
├ decision
├ matcher
└ reason
```

Matcher：

```text
ToolMatcher
└ tool

BashCommandMatcher
├ executable
└ argvPrefix
```

V3 不需要一开始实现任意 Boolean expression policy language。

---

## 17.6 Compiled Policy Snapshot

Runtime 启动时：

```text
RuntimeConfigSnapshot
      ↓
validate all User Rules
      ↓
compile matchers
      ↓
PolicyEngine immutable snapshot
```

任何 enabled invalid rule：

```text
policy snapshot invalid
      ↓
fail closed
```

不能跳过坏 rule 继续寻找 Allow。

---

# 18. Policy Evaluation Facts

当前 PolicyContext 主要包含：

```text
tool_name
security_mode
scope
```

V3 扩展为 typed non-secret facts：

```text
PolicyFacts
├ tool
├ capability
├ security_mode
├ workspace_scope
└ operation facts
```

对于 Bash：

```text
BashPolicyFacts
├ normalized segments
├ executable
├ argv
├ operators
└ risk flags
```

Policy facts 不保存：

- raw secret
- API Key
- arbitrary full file content

---

# 19. Approval Architecture

## 19.1 Runtime Owns Approval

Approval owner：

```text
Runtime
```

不是：

```text
React
Tauri
ChatGPT
```

原因：

只有 Runtime 持有：

- exact request identity
- validated request
- Policy evidence
- execution continuation
- Runtime generation identity

---

## 19.2 ApprovalManager

Runtime 新增：

```text
ApprovalManager
```

职责：

- register pending request
- expose sanitized ApprovalRequest
- resolve request
- enforce expiry
- cancel on Runtime shutdown
- wake exact waiting execution

---

## 19.3 Control Path

现有 control methods：

```text
approvals.list
approvals.resolve
```

继续使用。

路径：

```text
React Approval Center
        ↓
HarnessControlClient
        ↓
Tauri RuntimeSupervisor
        ↓ JSONL
Runtime ApprovalManager
```

Tauri 只是 transport。

---

## 19.4 Execution Wait

当 Policy = ASK：

```text
ExecutionPipeline
      ↓
create pending approval
      ↓
async wait
      ↓
Approval Center resolves
      ↓
wake exact request
      ↓
revalidate readiness / target
      ↓
execute
```

不能：

```text
ASK
→ immediately return generic error
→ lose original request
```

---

## 19.5 Approval Identity

Approval 必须绑定：

```text
runtime generation
request_id
tool
request fingerprint
```

批准 Request A 不能授权 Request B。

---

## 19.6 Approval Persistence

Pending approvals：

- 仅存在于当前 Runtime generation
- 不写入 AppConfig
- 不跨 Runtime restart 恢复

Runtime stop / crash：

```text
all pending approvals → Cancelled
```

不实现跨进程 replay。

---

# 20. Approval Decisions

V3：

```text
AllowOnce
AllowForSession
Deny
```

`Create Rule` 不是 Runtime execution decision。

它是 UI 从 ApprovalRequest 创建 Policy draft 的操作。

流程：

```text
Approval Request
      ↓
Create Rule
      ↓
Policy Editor
      ↓
save AppConfig
      ↓
rule applies next Runtime generation
```

当前 pending request 仍需：

```text
AllowOnce
or
AllowForSession
or
Deny
```

这样避免把 persistent config editing 混入 ApprovalManager。

---

# 21. SessionGrantStore

`AllowForSession` 创建：

```text
SessionGrantStore
```

特点：

- memory-only
- current Runtime generation only
- cannot bypass System Guardrails
- cannot override explicit DENY
- expires with Runtime generation

Policy flow：

```text
System violation
→ DENY

Explicit User DENY
→ DENY

Policy = ASK
   ↓
matching Session Grant?
   ├ yes → ALLOW
   └ no  → Approval
```

---

## 21.1 Grant Matcher

Grant 使用 Runtime 生成的 normalized matcher。

不使用 UI 自己拼接字符串。

例如 Bash：

```text
normalized executable + argv pattern
```

File tools 的具体 session-grant scope 在 SPEC 阶段定义，但必须：

- deterministic
- sanitized
- no broader than displayed approval scope

---

# 22. Bash Architecture

## 22.1 Location

Bash execution 位于 Runtime：

```text
Runtime
└ BashRuntime
   ├ BashDiscovery / validation
   ├ CommandAnalyzer
   ├ BashPolicyFacts
   ├ ProcessRunner
   ├ OutputLimiter
   └ ProcessTreeOwner
```

不能让 Tauri 代执行 Bash。

---

## 22.2 Bash Availability

Bash 只有同时满足：

```text
SecurityMode = DeveloperTest
AND
bash.enabled = true
AND
bash executable ready
AND
Runtime ready
AND
Policy != DENY
```

才进入 MCP exposure。

默认：

```text
enabled = false
default policy = ASK
```

---

## 22.3 Git Bash

Windows V3 使用 Git Bash：

```text
bash.exe
```

Config：

```text
autoDetect
executablePath?
```

Tauri 可以提供 file picker。

Runtime 对最终 executable 做 authoritative validation。

UI 不能因为 picker 找到一个路径就宣称 Runtime Bash Ready。

---

# 23. Bash CommandAnalyzer

V3 不建立字符串 blacklist 作为主安全模型。

CommandAnalyzer 负责：

- tokenize supported shell syntax
- identify executable
- identify argv
- identify top-level operators
- split safely analyzable compound command
- detect dynamic / nested shell constructs
- produce normalized Policy facts

---

## 23.1 Compound Command

例如：

```text
git status && git push origin main
```

Analyzer：

```text
segment 1:
git status

operator:
&&

segment 2:
git push origin main
```

每个 segment 评估 Policy。

最终：

```text
max severity
```

即：

```text
ALLOW + ASK  → ASK
ALLOW + DENY → DENY
```

---

## 23.2 Conservative Parsing

特别处理：

- `&&`
- `||`
- `;`
- `|`
- redirection
- `$()`
- backticks
- `eval`
- nested shell
- `bash -c`
- `sh -c`
- interpreter inline code such as `python -c`, `node -e`

如果 analyzer 无法可靠分类：

```text
UnclassifiedHighRisk
→ ASK
```

不是：

```text
parse failed
→ ALLOW
```

---

## 23.3 Analyzer Does Not Rewrite Execution

Policy analyzer 只生成 matcher facts。

真正执行：

> 原始、已批准的 command。

不得执行一个被 analyzer 重新拼接的不同 command。

这避免：

```text
analyzed command != executed command
```

---

# 24. Bash Process Execution

## 24.1 Process Ownership

Bash child 是 Runtime 的 child。

因此：

```text
Runtime owns Bash process tree
```

Tauri 的 Tunnel Job Object 不能代替 Bash ownership。

---

## 24.2 Windows Process Tree

每次 Bash invocation 使用受控 process-tree ownership。

目标：

- timeout → terminate full owned tree
- cancellation → terminate full owned tree
- Runtime shutdown → terminate full owned tree
- unrelated process 不受影响

Windows 优先使用 Job Object 风格 ownership。

---

## 24.3 Limits

BashRuntime enforcement：

```text
max command length
max concurrent bash
execution timeout
max captured output bytes
```

触发 hard limit：

```text
terminate owned process tree
      ↓
return typed execution error
      ↓
record evidence
```

---

## 24.4 Bash Is Not Sandboxed

V3 明确：

```text
cwd = Workspace
```

不等于：

```text
filesystem sandbox
```

Bash child 可能访问：

- Workspace 外文件
- network
- user environment

因此产品和 Runtime metadata 不得声明其拥有不存在的 sandbox guarantee。

完整 Windows sandbox 属于未来版本。

---

# 25. Activity and Audit Architecture

保持 Runtime 为 execution evidence owner。

每次 request 至少生成：

```text
request_id
tool
sanitized_arguments
capability result
workspace validation result
policy decision
matched_rule_ids
approval_id?
execution result
error category?
duration
```

Bash 增加：

```text
normalized command evidence
working directory
limit outcome
```

---

## 25.1 Sanitization

Activity/Audit 禁止写入：

- Tunnel API Key
- SecretStore raw values
- full secret-bearing environment
- unbounded file content

---

## 25.2 Evidence Source

Policy matched rule IDs 来自 PolicyEngine。

Approval ID 来自 ApprovalManager。

UI 不重新计算这些 evidence。

---

## 25.3 Storage Scope

V3 不引入数据库。

Approval history / execution evidence 默认属于当前 Runtime/session 的现有 Activity/Audit storage model。

如未来需要 durable audit，应单独设计，不在 V3 隐式增加数据库。

---

# 26. Runtime State

现有 RuntimeState 中：

```text
active_requests
pending_approvals
running_executions
```

目前主要是 skeleton values。

V3 将它们连接到真实 Runtime counters：

```text
active_requests
pending_approvals
running_executions
```

Tool catalog 状态也必须来自当前 Runtime snapshot，而不是 UI guess。

---

# 27. Control Protocol

## 27.1 Keep Existing Protocol Architecture

继续使用：

```text
protocol/src/v1/**
```

作为 Tauri ↔ Runtime shared contract。

V3 不因为产品版本叫 V3 就自动创建：

```text
protocol/v2
```

产品版本和 wire protocol major version 是不同概念。

---

## 27.2 Additive Evolution First

优先：

- add fields with safe defaults
- add enum variants
- add typed DTOs
- complete existing methods

已有：

```text
tools.list
policies.list
approvals.list
approvals.resolve
activity.*
```

应优先补全现有 contract，而不是新增第二套同义 API。

---

## 27.3 Breaking Wire Changes

如果实施中确实需要 wire-incompatible change：

- bump `PROTOCOL_VERSION`
- Tauri / Runtime handshake fail fast
- 不做 silent downgrade

但这需要显式 Architecture amendment。

V3 当前架构不要求新的 protocol major。

---

# 28. Protocol Additions

至少需要扩展：

## Config

```text
ToolConfig
PolicyConfig
BashConfig
UiConfig.language
RuntimeConfigSnapshot additions
```

## Tools

effective state 支持：

```text
AvailableWithApproval
UnavailableByMode
```

## Policy

typed matcher / rule DTO。

## Approval

`ApprovalDecision` 增加：

```text
AllowForSession
```

ApprovalRequest 增加：

- policy evidence
- reason code
- risk
- working directory / sanitized target
- expiry evidence

具体 payload 在 SPEC 冻结。

---

# 29. Tauri / UI API Boundary

React 继续只通过：

```text
HarnessControlClient
```

访问后端。

不允许页面直接散落 `invoke(...)`。

V3 在 client interface 增加 domain methods，例如概念上的：

```text
toolSetEnabled(...)
policyCreate(...)
policyUpdate(...)
policyDelete(...)
approvalList(...)
approvalResolve(...)
```

这些 methods：

- persistent config mutation → Tauri ConfigStore
- runtime observation / approval → Runtime control RPC

UI 不需要知道底层是哪个 transport。

---

# 30. Tools UI Architecture

Tools 页面同时读取：

```text
Desired Tool Config
+
Runtime EffectiveTool
+
active configRevision
+
saved configRevision
```

显示：

- desired enabled
- current effective availability
- Policy decision
- mode restriction
- readiness
- reason
- restart required

---

## 30.1 Toggle While Harness Running

用户修改 tool：

```text
toggle
  ↓
save AppConfig
  ↓
configRevision increments
  ↓
show Restart Required
```

推荐操作：

```text
Save & Restart Harness
```

重启成功后：

```text
Desired == Effective
```

---

# 31. Policies UI Architecture

Policies 页面区分：

```text
System Guardrails
User Policies
```

System：

- Runtime-owned
- read-only

User：

- ConfigStore-owned persistent configuration
- editable

运行中编辑：

```text
save config
→ restart required
```

UI 不能显示“当前已生效”，直到新 Runtime generation 报告对应 configRevision。

---

# 32. Approval Center Architecture

Approval Center 是 live Runtime surface。

数据来自：

```text
approvals.list
```

操作：

```text
AllowOnce
AllowForSession
Deny
```

`Create Rule`：

```text
ApprovalRequest
→ local UI draft
→ Policy Editor
→ ConfigStore save
```

Approval Center 不保存 raw execution payload。

---

# 33. i18n Architecture

i18n 只存在于 React presentation layer。

建议：

```text
i18next
+
react-i18next
```

资源：

```text
ui/src/i18n/
├ en.json
└ zh-CN.json
```

---

## 33.1 Language Selection

Startup：

```text
UiConfig.language?
  ├ present → use saved language
  └ absent  → detect OS language
```

unsupported：

```text
fallback → English
```

---

## 33.2 Backend Error Translation

Runtime / Tauri 提供：

```text
machine code
+
fallback message
```

UI：

```text
machine code
→ localized string
```

不要求 Rust Runtime 输出中文。

---

# 34. M0 Architecture Compatibility

M0 的目标是最小验证 External ChatGPT `write` / `edit`。

M0 不需要先实现完整 V3 Policy / Approval / Tool Toggle。

但 M0 的实现必须走最终架构入口：

```text
ChatGPT
→ Tunnel
→ Runtime MCP
→ write/edit MCP Adapter
→ ExecutionPipeline
→ WorkspaceGuard
→ PolicyEngine
→ executor
→ Activity/Audit
```

禁止 M0 为了“先跑通”直接：

```text
MCP handler
→ fs::write
```

---

## 34.1 M0 Minimal Changes

M0 只需要：

1. `ToolCapability::WorkspaceWrite`
2. Registry 增加 `write` / `edit`
3. write/edit typed request + executor
4. dedicated write-target WorkspaceGuard support
5. minimal default ALLOW policy for workspace write
6. MCP adapter + correct annotations
7. structured errors/results
8. Activity/Audit evidence

M2 后再让 Registry enable state 来自 AppConfig。

这样 M0 验证不会形成一次性架构分支。

---

# 35. Lifecycle Architecture

Harness lifecycle 保持：

```text
Start
 ↓
Runtime
 ↓
Runtime MCP
 ↓
Tunnel
 ↓
Harness Ready
```

Stop：

```text
Tunnel stop
 ↓
Runtime stop
 ↓
clear session
```

---

## 35.1 Runtime Stop with V3 State

Runtime stop 新增清理：

```text
mark not ready
→ cancel pending approvals
→ cancel/terminate active Bash process trees
→ stop MCP
→ exit Runtime
```

HarnessController 的总体 ownership 不变。

---

## 35.2 Crash Convergence

Runtime crash：

- MCP unavailable
- pending approvals lost/cancelled
- Bash descendants must be OS-cleaned
- Tunnel stopped by existing Harness convergence
- config remains in ConfigStore

Tunnel crash：

- Runtime may remain for diagnostics
- Approval / Bash 不因此绕过 lifecycle state

---

# 36. Security Invariants

以下必须在整个 V3 保持：

## S1

Runtime MCP only on loopback.

## S2

Tauri does not proxy normal MCP tool calls.

## S3

All known tool calls pass through ExecutionPipeline.

## S4

No Tool Adapter may construct ExecutionPermit.

## S5

Filesystem tools cannot bypass WorkspaceGuard.

## S6

`write` missing-target support cannot weaken existing read-path validation.

## S7

System Guardrails cannot be disabled by User Policy.

## S8

DENY always beats ASK / ALLOW at equivalent policy level.

## S9

Session grants never override DENY or System Guardrails.

## S10

Developer/Test Mode does not bypass Policy or Workspace enforcement.

## S11

Disabled / stale tool calls are rejected by Runtime, not only hidden from MCP list.

## S12

Bash is not represented as sandboxed in V3.

## S13

Bash timeout/cancel kills owned process trees only.

## S14

Secrets never enter Runtime config, MCP payload, Activity, Audit or literal command argv.

## S15

Approval resolution is bound to exact Runtime request identity.

## S16

Persistent config only becomes effective when Runtime reports the matching configRevision.

---

# 37. Failure Semantics

V3 继续 fail closed。

Examples:

```text
invalid config
→ no Runtime Ready
```

```text
invalid Policy snapshot
→ DENY / startup invalid according to validation stage
```

```text
unknown tool
→ MCP method/tool error
```

```text
disabled known tool
→ typed unavailable/disabled error
```

```text
workspace escape
→ reject before execution
```

```text
approval expires
→ no execution
```

```text
bash analyzer uncertain
→ ASK
```

```text
bash executable unavailable
→ tool unavailable
```

---

# 38. Expected Code Ownership

V3 should extend existing code locations rather than create parallel subsystems.

## Protocol

```text
protocol/src/v1/config.rs
protocol/src/v1/tools.rs
protocol/src/v1/policies.rs
protocol/src/v1/approvals.rs
protocol/src/v1/methods.rs
```

## Runtime

```text
runtime/src/state.rs
runtime/src/server.rs

runtime/src/execution/**
runtime/src/tools/**
runtime/src/policy/**
runtime/src/workspace/**
runtime/src/mcp/**
runtime/src/activity/**
runtime/src/audit/**
```

Expected new Runtime modules:

```text
runtime/src/approval/**
runtime/src/bash/**
```

Exact module names can change in SPEC.

## Tauri

```text
ui/src-tauri/src/config/**
ui/src-tauri/src/runtime/**
ui/src-tauri/src/harness.rs
ui/src-tauri/src/commands/**
```

## React

```text
ui/src/services/harness/**
ui/src/app/providers/**
ui/src/pages/tools/**
ui/src/pages/policies/**
ui/src/pages/settings/**
ui/src/pages/activity/**
```

Expected new UI surface:

```text
ui/src/pages/approvals/**
ui/src/i18n/**
```

---

# 39. Architecture Non-Goals

V3 Architecture explicitly不增加：

- new daemon
- external database
- policy microservice
- Tauri tool-call proxy
- multiple Runtime processes
- multiple workspaces
- generic PowerShell/CMD abstraction
- remote multi-user authorization
- durable cross-restart approvals
- full Windows filesystem sandbox
- network sandbox
- fuzzy edit engine
- hot-reload distributed configuration system

---

# 40. Deliberate Tradeoffs

## 40.1 Restart-bound Persistent Configuration

Tradeoff：

```text
less instant UX
```

换来：

```text
deterministic Runtime snapshot
simpler rollback
no partial config apply
clear configRevision semantics
automatic MCP reconnect
preserved V2 lifecycle model
```

这是 V3 的有意选择。

---

## 40.2 Session-only Approval

Tradeoff：

```text
approval state disappears on restart
```

换来：

```text
no replay risk
no durable pending executor serialization
exact generation ownership
simpler failure convergence
```

---

## 40.3 Bash without OS Sandbox

Tradeoff：

```text
Developer/Test Bash remains high risk
```

因此补偿措施：

```text
default OFF
Developer/Test only
default ASK
command classification
approval
limits
process-tree cleanup
audit
```

完整 OS sandbox 留给未来版本。

---

# 41. PRD Compatibility Check

本 Architecture 满足已确认 PRD：

| PRD Requirement | Architecture |
| --- | --- |
| M0 first verify write/edit | Same Runtime MCP → Pipeline path |
| read/glob/write/edit Normal | ToolCapability + Registry |
| bash Developer/Test only | CapabilityGate |
| default tools ON except bash | AppConfig migration defaults |
| tool toggle | desired config + next Runtime generation |
| disabled tool disappears | new generation MCP exposure excludes it |
| stale call rejected | CapabilityGate |
| ALLOW / ASK / DENY | PolicyEngine |
| System Guardrails | Runtime hard boundary |
| Policy evidence | PolicyEvaluation → Activity/Audit |
| Approval resume | Runtime ApprovalManager |
| Allow once | ApprovalManager |
| Allow for session | SessionGrantStore |
| Create rule | UI → ConfigStore |
| command interception | CommandAnalyzer + typed Bash matcher |
| atomic write | Runtime atomic replace |
| exact edit | exact matcher + atomic replace |
| Bash limits | BashRuntime |
| process-tree cleanup | Runtime-owned Job Object model |
| config migration | ConfigStore v1 → v2 migration |
| zh-CN / English | React i18n |
| Windows first | preserved |
| no fake Bash sandbox | explicitly documented |

没有发现需要推翻 V2 顶层架构的 PRD 冲突。

---

# 42. Final Architecture

V3 最终架构可以概括为：

```text
                     CONTROL PLANE

React UI
   ↓
HarnessControlClient
   ↓
Tauri Host
├ ConfigStore v2
├ SecretStore
├ HarnessController
├ RuntimeSupervisor
└ TunnelProcessManager
   ↓ JSONL
Runtime Control Server
   ├ Policy / Tool observation
   ├ Approval control
   └ Activity / Diagnostics


                       DATA PLANE

ChatGPT
   ↓
Tunnel Client
   ↓
Runtime MCP
   ↓
MCP Tool Adapter
   ↓
ExecutionPipeline
   ↓
CapabilityGate
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
SessionGrant / ApprovalManager
   ↓
ExecutionPermit
   ↓
┌───────────┬──────────┬───────────┐
│ read/glob │write/edit│   bash    │
└───────────┴──────────┴───────────┘
                ↓
          Activity / Audit
```

核心不变：

> **Tauri 管生命周期与持久配置，Runtime 管安全与执行，Tunnel 只连接 MCP，UI 只表达用户意图。**

V3 的新增能力全部沿着这条边界扩展，而不是重新定义边界。

---

# 43. Freeze Criteria

Architecture 可以进入 `ACCEPTED / FROZEN` 前，需要确认：

1. PRD 与 ROADMAP 不再有会改变进程 ownership 的新需求
2. M0 write-target WorkspaceGuard 设计被认可
3. Persistent config 采用 restart-bound model 被认可
4. ASK tool 在 MCP exposure 中保持可调用被认可
5. Runtime-owned ApprovalManager 被认可
6. Session grants 不跨 Runtime generation 被认可
7. Bash process tree 由 Runtime ownership 被认可
8. V3 不实现完整 OS sandbox 被认可
9. Config v1 → v2 migration ownership 在 Tauri 被认可

冻结后，下一步才能据此拆：

```text
SPEC
→ PLAN
→ TASKS
```
