# M2 SPEC — Tool Enable / Disable

> Status: ACCEPTED / FROZEN
> Milestone: M2 — Tool Enable / Disable  
> Baseline: M1 implementation complete; M0 accepted/frozen  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, M0/M1 milestone docs

## 1. Objective

M2 让用户可以持久化控制 Harness 下一 Runtime generation 启用哪些工具，并保证：

```text
Desired Tool State (AppConfig)
        ↓ save / configRevision changes
Restart Required
        ↓ restart Harness
RuntimeConfigSnapshot
        ↓
ToolRegistry / CapabilityGate
        ↓
Effective Tool State
        ↓
MCP tools/list + stale tools/call enforcement
```

M2 不做 same-generation hot reload。

## 2. Baseline Facts

当前代码已经具备：

- `AppConfig.schemaVersion = 1`，已有 `configRevision`，但没有 `tools` 字段。
- `RuntimeConfigSnapshot` 已是 immutable-at-startup，但没有工具配置。
- `ToolRegistry` 是工具定义/capability 单一事实源，目前 `read_file/glob/write/edit` 都硬编码 `enabled=true`。
- `EffectiveToolCatalog` 已有 `UnavailableDisabled` 状态。
- MCP exposure 和 execution 都基于 Runtime 当前 tool/effective state。
- Tools UI 当前只读显示 Runtime effective tools，没有 toggle。
- Architecture 已冻结 desired state ≠ effective state，并以 `active configRevision != latest configRevision` 表示 restart required。

M2 应扩展现有模型，而不是新增第二套工具目录或 live mutable Runtime config。

## 3. Scope

M2 实现：

1. 持久化 Tool desired state。
2. RuntimeConfigSnapshot 携带 tool state。
3. ToolRegistry 从 immutable snapshot 派生 configured enabled state。
4. disabled tool 不进入 MCP `tools/list`。
5. cached/stale `tools/call` 被 Runtime 明确拒绝。
6. Activity/effective state 能区分 disabled 与 policy denied/runtime unavailable。
7. Tools UI 可编辑 desired ON/OFF。
8. Harness 运行中保存变更时显示 Restart Required，而不伪装当前 Runtime 已改变。
9. Restart 后 effective state 与 saved desired state 对齐。
10. M0/M1 tool behavior regression。

M2 不实现：

- same-generation `tools/list_changed` requirement。
- persistent config hot reload。
- User Policy / Tool Default editor。
- ASK / Approval。
- Bash executor。
- i18n。
- AppConfig formal `schemaVersion v1 → v2` migration pipeline（M7）。

## 4. Config Contract

### 4.1 Transitional M2 persisted model

M2 在现有 `schemaVersion = 1` 中增加 backward-compatible、defaultable tool configuration：

```text
AppConfig
├ schemaVersion = 1
├ configRevision
├ workspace
├ tunnel
├ security
├ ui
└ tools
   ├ readFile.enabled = true
   ├ glob.enabled = true
   ├ write.enabled = true
   ├ edit.enabled = true
   └ bash.enabled = false
```

具体 Rust/JSON naming 必须遵循现有 camelCase convention；建议协议类型：

```text
ToolConfig
ToolEnabledConfig { enabled: bool }
```

或等价的强类型结构。

### 4.2 Backward compatibility before M7

旧 schema v1 文件没有 `tools` 时必须通过 `serde(default)` 或等价 typed default 正常加载，并得到：

```text
read_file = ON
glob      = ON
write     = ON
edit      = ON
bash      = OFF
```

M2 不把 `CURRENT_SCHEMA_VERSION` 升为 2。

原因：M7 才负责正式 schemaVersion v1→v2 migration、migration failure safety 和完整 V3 config migration evidence。

M2 只允许 backward-compatible additive persistence；不能实现一半 v1→v2 migration pipeline。

### 4.3 Config revision

任何 semantic tool state change 必须增加 `configRevision`。

重复保存相同 tool state：

```text
configRevision unchanged
```

这继续复用现有 ConfigStore semantic revision 规则。

## 5. Built-in Tool State

M2 desired defaults：

| Tool | Desired Default | M2 Runtime Implementation |
| --- | --- | --- |
| `read_file` | ON | implemented |
| `glob` | ON | implemented |
| `write` | ON | implemented |
| `edit` | ON | implemented |
| `bash` | OFF | not implemented until M5 |

`bash.enabled` 可以被配置模型保留为 future field，但 M2 不得因此注册可执行 bash、暴露 MCP bash 或新增 DeveloperShell execution path。

Tools UI 的可操作 toggle 必须来自 Runtime/known built-in metadata，而不是另写一份硬编码 authoritative tool list。M2 可以只对当前 implemented configurable tools提供操作；bash 的执行能力留给 M5。

## 6. Runtime Snapshot Contract

`RuntimeConfigSnapshot` 增加：

```text
tools
```

仅包含 non-secret persistent tool desired state。

Runtime generation 初始化后该 snapshot immutable。

运行中 Tauri 保存：

```text
write.enabled true → false
```

当前 Runtime generation 仍保持：

```text
write.enabled = true (effective)
```

直到下一次 Runtime generation。

禁止 Runtime 监听 config 文件或通过 Tauri event live mutate registry。

## 7. ToolRegistry Contract

### 7.1 Built-in definition remains authoritative

ToolRegistry 继续拥有：

```text
name
description
implemented
exposed capability
risk
readiness
capability
```

configured `enabled` 来自 RuntimeConfigSnapshot。

推荐构造语义：

```text
ToolRegistry::from_config(&snapshot.tools, security/mode if needed)
```

或等价显式构造。

不得出现：

```text
UI hardcoded tools
+ Runtime hardcoded tools
+ Config hardcoded tools
```

三套互相漂移的事实源。

### 7.2 Disabled state

一个 implemented tool 若 configured OFF：

```text
implemented = true
enabled = false
```

其 capability declaration仍可识别，但 executable capability gate 必须失败。

EffectiveTool：

```text
availability = UnavailableDisabled
```

decision 不得被 UI 解释为 Policy DENY；disabled 是 capability/config state，不是 policy state。

## 8. MCP Exposure Contract

对当前 Runtime generation：

### Enabled + eligible

正常进入 effective evaluation并可出现在 `tools/list`。

### Disabled

必须满足：

```text
not present in MCP tools/list
```

即使 ChatGPT/client 仍缓存旧 catalog，也不代表 tool 可以执行。

M2 不要求 same-generation `tools/list_changed`。

新的 tool list 在新的 Runtime generation / 新连接上体现 saved desired state。

## 9. Stale / Cached Call Contract

Runtime 必须在 execution authority 内拒绝 disabled tool 的 stale call。

例如：

```text
Generation A: write enabled
client caches write
user saves write OFF
restart
Generation B: write disabled
client sends cached tools/call write
→ Runtime reject
```

必须是显式 disabled rejection，不允许：

- 落到 executor。
- 写文件后再报告失败。
- 只依赖 `tools/list` 隐藏。
- 由 UI/Tunnel 代替 Runtime enforcement。

外部错误建议冻结：

```text
TOOL_DISABLED
```

或现有协议中等价且明确的 capability-unavailable code；必须能与 `POLICY_DENIED`、`RUNTIME_NOT_READY`、unknown tool区分。

## 10. Desired vs Effective UI Contract

Tools 页面至少展示：

```text
Tool
Desired State
Effective State
Availability / Reason
Restart Required (when applicable)
```

### Harness stopped

用户改变 toggle：

- 保存 AppConfig。
- 下次 start 使用新状态。
- 不需要声称存在 current Runtime effective state。

### Harness running

用户改变 toggle：

- 保存 desired state。
- `configRevision` 变化。
- current effective state继续来自 Runtime。
- UI 显示 Restart Required。
- toggle 不得让 current Runtime row 假装已经 disabled/enabled。

例如：

```text
Desired: write OFF
Effective: write ON
Restart Required: true
```

Restart 后：

```text
Desired: write OFF
Effective: write OFF / UnavailableDisabled
Restart Required: false
```

## 11. Restart Required Contract

复用 Architecture 已冻结 identity：

```text
active Runtime configRevision
!=
latest persisted configRevision
```

则：

```text
restartRequired = true
```

不得新增第二个独立 dirty flag 作为事实源。

如果 Harness 没有 active Runtime，则不应错误显示“当前 Runtime 已使用旧配置”。

## 12. Control / Protocol Evolution

优先 additive evolution `protocol/src/v1`，不升级 protocol major。

需要新增/扩展：

```text
AppConfig.tools
RuntimeConfigSnapshot.tools
Tool state DTOs / generated TS types
```

现有 `runtime.getEffectiveTools` 保持 Runtime effective truth。

Desired state来自 ConfigStore/AppConfig control surface；UI 不自行从 effective state反推 desired state。

若现有 config read/save API 足够，复用它；不得为了 M2 新建独立 tool config persistence store。

## 13. Activity / Audit

当 stale disabled call 到达 Runtime：

- 必须有拒绝 evidence。
- category/reason 能识别 `disabled`。
- 不进入 executor success lifecycle。
- 不把 disabled rejection 记录成 policy denied。

单纯用户在 UI 保存 desired state不需要伪造一次 Runtime tool execution event。

## 14. Security / Failure Semantics

以下全部 fail closed：

- unknown tool config key（若使用 map）。
- malformed tool state。
- Runtime snapshot缺失必需字段且不能安全 default。
- stale disabled invocation。

旧 v1 config 缺 `tools` 是明确兼容路径，不属于 malformed config。

Tool OFF 不得影响 WorkspaceGuard / Policy / ExecutionPermit 对其它工具的安全边界。

## 15. Expected Code Touchpoints

```text
protocol/src/v1/config.rs
protocol/src/v1/tools.rs                 # if DTO/state enums belong here
protocol generated TS/schema artifacts

ui/src-tauri/src/config/mod.rs
ui/src-tauri/src/runtime/mod.rs          # build RuntimeConfigSnapshot
ui/src-tauri/src/harness.rs / commands   # only as required for save/read/restart state

runtime/src/state.rs
runtime/src/tools/registry.rs
runtime/src/tools/effective.rs
runtime/src/execution/*                  # capability rejection only if needed
runtime/src/mcp/service.rs
runtime/src/mcp/errors.rs

ui/src/pages/tools/ToolsPage.tsx
ui/src/app/providers/*
ui/src/services/harness/*
related tests
```

Tunnel、write/edit mutation internals、Policy rule model不应因 M2 改动。

## 16. Compatibility Requirements

M2 必须保持：

- 默认配置下 `read_file/glob/write/edit` 全部与 M1 行为一致。
- M1 atomic write/edit semantics不变。
- MCP schemas/annotations不变。
- WorkspaceGuard不变。
- PolicyEngine semantic baseline不变。
- Activity/Audit confidentiality不变。
- no protocol major bump。
- old schema v1 config without tools loads with defaults。

## 17. Development Verification Requirements

每个 Task 带 unit/code-level tests。

开发完成至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm / pnpm UI test command used by repository
git diff --check
```

必须有自动化覆盖：

```text
old config missing tools → defaults
save toggle → configRevision increments
overwrite identical state → revision unchanged
snapshot carries tools
registry reflects snapshot
disabled effective state
disabled absent from tools/list
stale call rejected before executor
restart applies desired state
running desired != effective shows restart required
M1 write/edit regression
read/glob regression
```

## 18. Post-TASK Verification Documents

`SPEC / PLAN / TASKS` 冻结后独立生成：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E 均应由 Coding Agent 自动完成；Manual Acceptance 才需要人工介入。

M2 automated E2E 应使用真实 Runtime process、Control RPC、MCP endpoint、filesystem/Activity 等本地黑盒链路，不要求人工 External ChatGPT。

最终人工验收可验证真实 UI desired/effective/restart UX；不需要重新执行 M0/M1 已完成的 External ChatGPT capability proof，除非 M2 production change导致相关链路回归风险。

## 19. Final Acceptance Criteria

M2 可 `ACCEPTED / FROZEN` 前必须证明：

- tool desired state可持久化。
- old config缺 tools安全 default。
- semantic changes正确更新 configRevision。
- Runtime generation使用 immutable tool snapshot。
- running config change不 live mutate current Runtime。
- UI明确区分 desired/effective。
- configRevision mismatch显示 Restart Required。
- restart 后 effective 与 desired一致。
- OFF tool不在新 generation `tools/list`。
- stale OFF tool call被 Runtime拒绝且无 executor side effect。
- disabled与policy denied/runtime unavailable可区分。
- M1 write/edit + read/glob regression PASS。
- Integration / local black-box E2E / Manual Acceptance完成。

## 20. Open Questions

```text
0
```

M2 实施只允许在上述边界内选择局部 API/组件组织方式，不应提前设计 M3 Policy、M4 Approval 或 M5 Bash execution。

> Final status: `M2 — ACCEPTED / FROZEN`. Integration, Local Black-box E2E, and Manual Acceptance evidence are complete; M3 remains planned and not implemented.
