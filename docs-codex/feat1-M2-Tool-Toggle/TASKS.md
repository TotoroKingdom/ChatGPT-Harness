# M2 TASKS — Tool Enable / Disable

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Baseline: M1 development complete  
> 每个 Task 必须包含实现 + Unit Test 或代码级自动化验证。  
> Integration Test、E2E Test、Manual Acceptance 不属于本文件。

## Dependency

```text
T1 Tool Config / Protocol / ConfigStore
        ↓
T2 Runtime Snapshot / Registry / Capability Enforcement
        ↓
T3 MCP Exposure / Stale-call Rejection
        ↓
T4 Tools UI Desired / Effective / Restart Required
        ↓
T5 Regression / Completion
```

---

## T1 — Persisted Tool Config / Protocol

**Primary files:** `protocol/src/v1/config.rs`, generated TS/schema artifacts, `ui/src-tauri/src/config/mod.rs`, related tests.

### Implement

新增强类型 Tool config，至少表达：

```text
read_file.enabled = true
glob.enabled      = true
write.enabled     = true
edit.enabled      = true
bash.enabled      = false
```

要求：

- `AppConfig` 增加 `tools`。
- `RuntimeConfigSnapshot` 增加 `tools`。
- 保持 `CURRENT_SCHEMA_VERSION = 1`。
- `tools` 必须有 backward-compatible default，使旧 schema v1 config 无 `tools` 也能正常 load。
- 不实现 M7 正式 v1→v2 migration pipeline。
- ConfigStore save/reload持久化 tools。
- tool semantic change导致 `configRevision + 1`。
- 保存相同 tools state不增加 revision。
- malformed tools config fail closed。
- defaults不猜测新的外部环境状态。

### Unit Tests

至少覆盖：

- old v1 JSON missing `tools` → read/glob/write/edit ON，bash OFF。
- safe defaults含正确 tools state。
- save/reload preserves each toggle。
- one toggle changed → revision increments once。
- same semantic config → revision unchanged。
- malformed/non-boolean tool enabled value fails closed。
- generated protocol/TS schema保持同步。

### Done

- config persistence PASS。
- schemaVersion仍为1。
- 没有引入第二个 tool persistence store。

---

## T2 — Runtime Snapshot / Registry / Capability Enforcement

**Primary files:** Tauri Runtime snapshot construction, `runtime/src/state.rs`, `runtime/src/tools/registry.rs`, `runtime/src/tools/effective.rs`, execution tests.

### Implement

把 tool enabled state从当前 Registry硬编码迁移到 Runtime generation snapshot。

要求：

- Tauri start Runtime时把 `AppConfig.tools` 放入 `RuntimeConfigSnapshot.tools`。
- Runtime initialize后使用 snapshot构造 generation-specific configured ToolRegistry。
- current Runtime generation工具状态 immutable。
- Runtime运行期间 ConfigStore save不能 live mutate current Registry。
- Registry built-in definition仍为 single source of truth。
- stable order保持：

```text
read_file, glob, write, edit
```

- `bash` config不等于 bash implementation；M2不得注册可执行 bash。
- disabled registration仍保留 declared capability metadata，但 executable capability lookup必须失败。
- `EffectiveToolCatalog` 对 OFF tool返回 `UnavailableDisabled`。
- disabled不映射成 PolicyDenied。
- Pipeline、ActivityStore、MCP必须共享同一 generation Registry identity。

### Unit Tests

至少覆盖：

- all-default ON → M1 effective behavior unchanged。
- independently disable read_file / glob / write / edit。
- disabled effective availability = `UnavailableDisabled`。
- disabled declared capability仍可观察，但 executable capability unavailable。
- snapshot A enabled、保存 config B disabled后，A generation仍保持 enabled。
- restart/new Runtime with snapshot B后 disabled生效。
- runtime not ready / disabled / policy denied可区分。

### Done

- Runtime tool state来自 immutable snapshot。
- 没有 hot reload channel。
- M1 security/execution ownership无变化。

---

## T3 — MCP Exposure / Stale-call Rejection

**Primary files:** `runtime/src/mcp/service.rs`, `runtime/src/mcp/errors.rs`, execution/capability path tests.

### Implement

确保 MCP exposure和 execution enforcement都使用 current generation configured state。

#### tools/list

OFF tool：

```text
not present
```

其它 enabled tool继续正常暴露。

#### stale tools/call

即使 client缓存旧 catalog：

```text
disabled tool call
→ reject before executor
```

新增稳定 external error：

```text
TOOL_DISABLED
```

或等价明确 capability-unavailable code，但必须与：

```text
unknown tool
POLICY_DENIED
RUNTIME_NOT_READY
```

可区分。

要求：

- disabled write/edit无 filesystem side effect。
- disabled read/glob不读取/枚举 Workspace。
- Activity/Audit记录 disabled rejection category。
- 不产生 ExecutionCompleted success。
- error sanitized。
- 不实现 same-generation `tools/list_changed`。

### Unit / Code-level Tests

至少覆盖：

- write OFF → absent tools/list。
- edit OFF → absent tools/list。
- read_file/glob OFF同理。
- cached write call → TOOL_DISABLED，file unchanged。
- cached read call → TOOL_DISABLED，无 read success evidence。
- unknown tool仍保持原有 unknown semantics。
- policy deny仍保持 policy semantics。
- enabled sibling tools不受影响。

### Done

- hiding不是唯一 enforcement。
- stale disabled call不能进入 executor。

---

## T4 — Tools UI Desired / Effective / Restart Required

**Primary files:** `ui/src/pages/tools/ToolsPage.tsx`, `ui/src/app/providers/*`, `ui/src/services/harness/*`, Tauri commands/read models as needed, UI tests.

### Implement

把当前只读 Tools 页面扩展为配置 + observation 页面。

每个当前 implemented configurable tool至少展示：

```text
Tool name
Desired ON/OFF toggle
Effective availability/state
Effective policy decision
Reason
Restart Required marker when desired != current generation config
```

数据来源：

```text
Desired = AppConfig.tools
Effective = Runtime control read
RestartRequired = active configRevision != latest persisted configRevision
```

#### Harness stopped

- toggle可保存 desired state。
- effective显示没有 active Runtime generation。
- next start应用 desired。

#### Harness running

保存 toggle后：

- desired UI立即显示 saved value。
- current Runtime effective仍显示旧 generation值。
- 显示 Restart Required。
- 不调用 Runtime live toggle API。

restart后：

- effective收敛到 desired。
- revision mismatch消失。
- Restart Required消失。

### UX Requirements

- disabled与policy denied明显区分。
- save failure必须显示失败并恢复/重新读取真实 persisted state，不能 optimistic 永久假装成功。
- 不承诺 ChatGPT client cache会瞬时刷新。
- 不引入 M6 i18n framework；沿用当前英文 UI。
- 不在 React硬编码另一份 authoritative tool registry。

### UI Tests

至少覆盖：

- stopped toggle save。
- running toggle save → desired changes, effective unchanged, Restart Required visible。
- restart/new effective snapshot → desired/effective convergence。
- save error does not display false saved state。
- disabled vs denied rendering不同。
- refresh effective tools不覆盖 desired state。

### Done

- UI不会把 desired冒充 effective。
- revision identity是 Restart Required唯一事实源。

---

## T5 — Cross-cutting Regression / Development Completion

**Primary scope:** protocol/runtime/Tauri/UI automated regression;不新增产品功能。

### Regression Requirements

必须证明：

#### Config / lifecycle

- pre-M2 schema v1 config without tools仍可启动。
- tool config持久化稳定。
- default M1 startup行为不变。

#### Runtime / MCP

- default tools/list仍为 `read_file, glob, write, edit`。
- read/glob正常及 Workspace boundary PASS。
- write/edit M1 atomic正常及 boundary PASS。
- MCP schemas/annotations不变。
- WorkspaceGuard语义不变。
- Policy baseline不变。
- disabled stale call无 side effect。

#### Activity / Audit

- disabled rejection可诊断。
- 不泄露 raw mutation content/secret。

#### UI

- current read-only effective observation capability不丢失。
- desired/effective/restart state combinations有组件/集成测试。

### Development Completion Commands

执行 repository标准命令：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend test command(s)
frontend typecheck/build command(s) where repository requires
git diff --check
```

全部 PASS。

### Done

T1–T5 完成仅代表 M2 development complete。

随后必须独立生成并执行：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；只有 Manual Acceptance需要人工介入。

---

## Development Completion Checklist

- [ ] T1 Tool Config / Protocol PASS
- [ ] T2 Runtime Snapshot / Registry PASS
- [ ] T3 MCP Exposure / Stale-call Rejection PASS
- [ ] T4 Tools UI Desired / Effective / Restart Required PASS
- [ ] T5 Regression PASS
- [ ] old schema v1 compatibility PASS
- [ ] configRevision semantics PASS
- [ ] disabled absent from tools/list PASS
- [ ] stale disabled call rejection PASS
- [ ] running desired/effective mismatch PASS
- [ ] restart convergence PASS
- [ ] M1 regression PASS
- [ ] cargo fmt/check/test PASS
- [ ] frontend tests/typecheck/build PASS as applicable
- [ ] git diff --check PASS
- [ ] TASKS未声称 Integration/E2E/Manual Acceptance PASS
