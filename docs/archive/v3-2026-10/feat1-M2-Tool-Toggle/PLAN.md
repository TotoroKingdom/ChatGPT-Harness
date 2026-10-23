# M2 PLAN — Tool Enable / Disable

> Status: ACCEPTED / FROZEN
> Input: `SPEC.md`  
> Baseline: M1 development complete  
> Goal: 将 Tool enabled state 从 Runtime 硬编码迁移为持久化 desired config + immutable Runtime snapshot，并实现 UI desired/effective/restart-required 语义。  
> Rule: PLAN 只决定如何实现 SPEC；Integration/E2E/Manual Acceptance 另行生成独立文档。

## 1. Strategy

M2 沿现有配置与 Runtime generation 模型扩展：

```text
AppConfig desired tools
→ ConfigStore save / configRevision
→ RuntimeConfigSnapshot.tools
→ ToolRegistry configured state
→ EffectiveToolCatalog
→ MCP tools/list + tools/call enforcement
→ Tools UI desired/effective presentation
```

不增加 live mutable Runtime state。

核心原则：

- ConfigStore 是 persistent desired state owner。
- Runtime snapshot 是 current generation effective config input。
- ToolRegistry 仍是 built-in tool definition/capability single source of truth。
- EffectiveToolCatalog 仍是 Runtime effective state projection。
- UI 只组合 desired config 与 Runtime observation，不自行授权/推导 execution capability。

## 2. Phase A — Protocol / Persisted Tool Config

扩展 `protocol/src/v1/config.rs`：

```text
AppConfig.tools
RuntimeConfigSnapshot.tools
```

定义强类型 Tool config，至少表达：

```text
read_file enabled
 glob enabled
 write enabled
 edit enabled
 bash enabled
```

推荐使用结构体字段而非自由字符串 map，以减少 unknown-tool persistence ambiguity。

### Compatibility

- 保持 `CURRENT_SCHEMA_VERSION = 1`。
- `tools` 使用 backward-compatible default。
- old schema v1 missing `tools` 正常 load。
- defaults：read/glob/write/edit ON，bash OFF。
- ConfigStore semantic equality自然包含 tools，所以 tool change 增加 configRevision；重复保存相同状态 revision不变。

更新：

- Rust serde/json-schema。
- generated TypeScript types。
- config tests。

Exit：旧 config、默认 config、save/reload、revision tests PASS。

## 3. Phase B — Snapshot Wiring

Tauri 构造 Runtime start config时把：

```text
AppConfig.tools
→ RuntimeConfigSnapshot.tools
```

必须确认：

- snapshot captured at Runtime generation start。
- running Runtime 不监听后续 ConfigStore save。
- active configRevision来自启动时 snapshot。
- saved latest revision变化后 current active revision保持不变。

如果现有 HarnessController 已有 restart-required calculation，直接复用；若目前只有基础 revision identity，则在现有 read model 上补充，不新增独立 dirty boolean truth source。

Exit：不同 configRevision/tool desired state能够生成确定 snapshot；current generation immutable。

## 4. Phase C — Registry Configured State

重构当前：

```text
ToolRegistry::normal_mode()
→ all implemented tools enabled=true
```

为 explicit configured construction，例如：

```text
ToolRegistry::normal_mode(&ToolConfig)
```

或：

```text
ToolRegistry::from_snapshot(...)
```

### Responsibilities

Built-in entry继续冻结：

```text
name
description
capability
implemented
exposed metadata
risk
readiness
```

`enabled` 由 snapshot tools state 注入。

保留 stable order：

```text
read_file, glob, write, edit
```

M2 不注册 bash executor；bash config仅 future persisted field。

### Tests

- each current tool independently ON/OFF。
- stable built-in order。
- capability declaration still present for disabled registration。
- executable `capability()` rejects disabled registration。
- effective catalog maps disabled → `UnavailableDisabled`。

Exit：没有第二份 authoritative Runtime tool catalog。

## 5. Phase D — Runtime Initialization / Capability Enforcement

`RuntimeCore::new()` 不应提前创建一个与 snapshot 无关、固定 all-enabled 的 authoritative registry。

调整 lifecycle，使 initialized Runtime 的 Registry 与当前 immutable snapshot tool config绑定。

可选实现：

```text
WaitingInitialize: built-in definitions only / safe placeholder
initialize(snapshot): construct configured ToolRegistry + Pipeline + Activity view
```

或其他不破坏 ownership 的实现。

必须确保 ActivityStore / Pipeline / MCP 都共享同一 generation Registry identity。

### Disabled execution

stale call flow：

```text
MCP tools/call
→ Runtime capability gate / registry
→ disabled
→ reject before executor
```

新增稳定错误：

```text
TOOL_DISABLED
```

如现有 ExecutionError 能表达 unavailable reason，则做最小 additive extension，不把 disabled伪装成 unknown tool或 policy denied。

Exit：disabled tool无法执行，且无 filesystem/read side effect。

## 6. Phase E — MCP Exposure

`McpService` tool list必须从 current Runtime generation effective/configured state生成。

验证：

```text
write OFF
→ tools/list excludes write
→ cached tools/call write → TOOL_DISABLED
```

其它 enabled工具正常列出/调用。

M2 不实现同 generation list-changed notification。

如果 MCP service当前在连接建立时缓存 tool list，必须确认它读取的是 generation-configured registry，而不是 compile-time list。

Exit：list exposure 与 execution enforcement一致，但 execution enforcement不依赖 list hiding。

## 7. Phase F — Desired / Effective Control Surface

复用现有 ConfigStore read/save command surface。

若当前前端 client不能读取/保存完整 AppConfig，则做最小 additive API wiring；不建立 tool-only persistence backend。

UI data source：

```text
Desired = AppConfig.tools
Effective = Runtime runtime.getEffectiveTools / RuntimeState
Restart Required = active configRevision != latest persisted configRevision
```

需要明确三种状态：

### Stopped

- desired toggle可保存。
- effective state显示 Not Running / no active generation。
- next start使用 desired。

### Running + revisions equal

- desired/effective应一致（除 policy/runtime readiness等其它 availability因素）。
- restartRequired=false。

### Running + revisions differ

- desired toggle显示用户最新保存值。
- effective仍显示 current Runtime state。
- restartRequired=true。

Exit：UI不把 desired state冒充 current Runtime state。

## 8. Phase G — Tools Page UX

扩展当前只读 `ToolsPage`，建议每一 row包含：

```text
Tool name
Desired toggle
Effective availability
Effective policy decision
Reason
Restart Required marker
```

UX rules：

- 保存成功后立即更新 desired state。
- current Runtime effective state只有 Control Plane refresh/change后更新。
- running mismatch显式显示 Restart Required。
- UI不声称 ChatGPT当前会话缓存会瞬时消失。
- disabled ≠ denied，文案/状态视觉必须能区分。

M2 文案可保持英文；完整 i18n留 M6。

Exit：用户可以理解“我关掉了工具，但当前 Runtime仍在旧 generation运行”。

## 9. Phase H — Regression / Failure Handling

覆盖：

### Config

- old schema v1 missing tools。
- malformed config fails closed。
- semantic revision behavior。

### Runtime

- all default ON behavior与 M1一致。
- each tool OFF effective unavailable disabled。
- stale call rejected。
- not-ready vs disabled vs policy denied区分。

### MCP

- OFF absent tools/list。
- other tools unaffected。

### UI

- stopped desired change。
- running desired/effective mismatch。
- restart required。
- restart后 convergence。
- save failure rollback/visible error，不假装成功。

### M0/M1

- read/glob normal and boundary。
- write/edit atomic production paths。
- Activity/Audit confidentiality。

## 10. Change Discipline

- 不升级 schemaVersion 到 2。
- 不实现 M7 migration pipeline。
- 不实现 M3 Policy editor/default config。
- 不实现 M4 Approval。
- 不实现 M5 bash executor。
- 不实现 same-generation tools/list_changed。
- 不增加 mutable Runtime config channel。
- 不在 UI 复制 Runtime authorization逻辑。
- 不修改 M1 atomic write/edit internals。

## 11. Development Completion Check

完成开发 Tasks 后执行 repository 标准检查：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests / typecheck / build commands defined by repository
git diff --check
```

必须有稳定自动化覆盖：

```text
config default compatibility
revision semantics
snapshot tool state
registry enabled state
effective disabled state
MCP list filtering
stale disabled rejection
restart-required identity
UI desired/effective mismatch
restart convergence
M1 regression
```

M2 Integration、Local Black-box E2E 与 Manual Acceptance 已独立完成并 PASS；本 milestone 已收口为 `M2 — ACCEPTED / FROZEN`。
