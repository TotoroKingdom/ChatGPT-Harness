# M2 Integration Test Plan — Tool Enable / Disable

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M2 — Tool Enable / Disable  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: code/component integration across Protocol, ConfigStore, Runtime snapshot/registry/MCP, Control read models and UI data flow. This is not Local Black-box E2E and does not require External ChatGPT.

## 1. Objective

验证 M2 各开发 Task 组合后形成一致的 Tool Toggle 系统，而不是若干孤立 unit behavior。

Integration 重点验证：

```text
Persisted Desired Tool State
→ ConfigStore / configRevision
→ RuntimeConfigSnapshot
→ generation-specific ToolRegistry
→ EffectiveToolCatalog
→ MCP exposure + stale-call enforcement
→ Control observations
→ Tools UI desired/effective/restart model
```

核心不变量：

- ConfigStore 是 persistent desired state owner。
- Runtime generation 使用 immutable snapshot。
- ToolRegistry 仍是 built-in tool definition / capability single source of truth。
- MCP hiding 不是 authorization；stale call 仍由 Runtime 拒绝。
- UI 不把 desired state 冒充 effective state。
- Restart Required 只由 active revision 与 latest persisted revision mismatch 派生。

## 2. Entry Criteria

执行本计划前必须满足：

- T1–T5 development complete。
- repository Unit/code-level tests PASS。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- frontend tests/typecheck/build PASS（按仓库实际命令）。
- `git diff --check` PASS。

若实现尚未完成：

```text
Integration = BLOCKED_BY_IMPLEMENTATION
```

不得通过降低 case 断言来获得 PASS。

## 3. Test Ownership Boundary

Integration 可以：

- 直接实例化 ConfigStore / RuntimeCore / configured ToolRegistry 等组件。
- 使用真实 serialization/deserialization。
- 使用真实临时 config.json / Workspace。
- 调用真实 MCP service adapter / Control read handler 的进程内组合测试。
- 使用 React component/provider test 验证 Tools 页面数据组合。

Integration 不应：

- 用 mock 替代需要验证的 ConfigStore semantic revision logic。
- 把单个函数 unit test 重复记为 integration evidence。
- 声称已验证真实 Runtime 可执行子进程或真实 loopback HTTP；这些属于 E2E。
- 依赖 External ChatGPT、Tunnel 或人工浏览器操作。

## 4. Required Integration Cases

### INT-01 — Legacy schema v1 without `tools` loads safely

Fixture：真实旧式 schema v1 JSON，不包含 `tools`。

期望 load 后：

```text
read_file = ON
glob      = ON
write     = ON
edit      = ON
bash      = OFF
schemaVersion = 1
```

并验证旧 Workspace/Tunnel/Security/UI 字段保持原值。

### INT-02 — Tool desired state persists with semantic revision

使用真实 ConfigStore：

1. 保存 defaults。
2. 将 `write` 从 ON 改 OFF。
3. reload。
4. 再保存相同 semantic config。
5. 再将 `edit` 改 OFF。

期望：

- 第一次 semantic tool change → `configRevision + 1`。
- reload 后 toggle 保持。
- 保存相同 state → revision 不变。
- 第二次 semantic change → 再加 1。
- `schemaVersion` 始终仍为 1。

### INT-03 — Malformed tool config fails closed

至少覆盖：

- `enabled` 非 boolean。
- required typed object malformed。
- unsupported malformed shape。

期望：

- ConfigStore load/save失败。
- 不静默回退为 ON。
- 原 config 不因 failed save 被破坏。

缺失整个 `tools` 字段属于 INT-01 的兼容路径，不应判为 malformed。

### INT-04 — AppConfig tools become RuntimeConfigSnapshot tools

使用 Tauri snapshot construction path，输入：

```text
read_file ON
glob ON
write OFF
edit ON
```

期望 RuntimeConfigSnapshot：

- 精确携带同一 tool state。
- 携带保存后的 `configRevision`。
- 不把 Tunnel secret/API key material放入 snapshot。

### INT-05 — Runtime generation registry reflects snapshot

分别初始化多个 RuntimeCore：

```text
Generation A: all current tools ON
Generation B: write OFF
Generation C: read_file OFF + edit OFF
```

期望：

- stable tool order仍为 `read_file, glob, write, edit`。
- built-in capability/risk/description不随 toggle漂移。
- disabled registration仍保留 declared capability。
- executable capability lookup对 disabled tool失败。

### INT-06 — Effective catalog distinguishes disabled state

在 Ready Runtime：

- enabled tool → normal effective behavior。
- disabled tool → `UnavailableDisabled`。

同时构造/验证：

- Runtime NotReady → `UnavailableByRuntime`。
- existing policy-denied fixture → `UnavailableByPolicy`。

期望三种状态可明确区分；disabled 不伪装成 Policy DENY。

### INT-07 — Current generation remains immutable after desired save

流程：

1. Runtime A 从 revision N、`write=ON` snapshot启动。
2. ConfigStore 保存 revision N+1、`write=OFF`。
3. 不重建 Runtime A。
4. 读取 Runtime A effective tools。

期望：

```text
Desired write = OFF
Runtime A effective write = ON
active revision = N
latest revision = N+1
restartRequired = true
```

Runtime A 不发生 live mutation。

### INT-08 — New generation converges to saved desired state

延续 INT-07：

1. 停止/丢弃 Runtime A。
2. 使用 latest config启动 Runtime B。
3. Runtime B Ready。

期望：

```text
Desired write = OFF
Runtime B effective write = UnavailableDisabled
active revision = latest revision
restartRequired = false
```

### INT-09 — MCP `tools/list` filters disabled tools

在同一 generation 分别测试：

- `read_file OFF`
- `glob OFF`
- `write OFF`
- `edit OFF`

期望对应 tool 不出现在 `tools/list`；其它 enabled tools保持正确顺序、schema、annotations。

### INT-10 — Cached/stale disabled write/edit call is rejected

在 `write OFF` / `edit OFF` generation 中直接发起旧客户端可能缓存的 `tools/call`。

期望：

- 返回 `TOOL_DISABLED` 或 SPEC 冻结的等价明确 code。
- 不映射成 unknown tool。
- 不映射成 `POLICY_DENIED`。
- filesystem 无 mutation。
- Activity/Audit 能识别 disabled rejection。
- 不产生 execution success evidence。

### INT-11 — Cached/stale disabled read/glob call is rejected

在 `read_file OFF` / `glob OFF` generation 中直接调用。

期望：

- 明确 disabled rejection。
- 没有读取/枚举成功结果。
- 其它 enabled sibling tools仍可执行。

### INT-12 — Unknown / disabled / policy / readiness semantics remain distinct

构造四类请求：

```text
unknown tool
disabled known tool
policy-denied known tool
runtime-not-ready known tool
```

期望外部 error/effective category可区分，不能把所有 unavailable 压成同一个模糊错误。

### INT-13 — Activity evidence for disabled rejection

对 disabled stale call验证：

- request/tool identity可追踪。
- rejection reason/category = disabled/capability unavailable。
- 不出现 executor success。
- 不记录 raw write content、secret 或无必要绝对路径。

### INT-14 — UI stopped-state desired configuration

使用真实 provider/client test wiring或等价 component integration：

- 无 active Runtime。
- 用户将 `write` OFF并成功保存。

期望：

- desired toggle显示 OFF。
- persisted config为 OFF。
- effective区域明确显示无 active Runtime / not running，而不是推导成 disabled Runtime。
- 不显示虚假的 active-generation convergence。

### INT-15 — UI running mismatch shows desired/effective separately

Fixture：

```text
Persisted desired: write OFF, revision N+1
Current Runtime effective: write ON, active revision N
```

期望 UI 同时展示：

```text
Desired: OFF
Effective: ON / Available
Restart Required: visible
```

refresh Runtime effective tools不得覆盖 desired OFF。

### INT-16 — UI restart convergence

将 active Runtime observation替换为 revision N+1 / write disabled。

期望：

- desired仍 OFF。
- effective显示 disabled。
- Restart Required消失。

### INT-17 — UI save failure does not fake desired state

模拟真实 control-client save failure（可以 mock transport failure，但不得 mock掉 state reconciliation logic）。

期望：

- 用户看到明确失败。
- UI rollback或重新读取 persisted config。
- 不永久显示未保存 desired state。
- 不产生错误的 revision mismatch结论。

### INT-18 — M1 regression under default tool config

使用默认 tool state执行：

- `read_file` normal/boundary。
- `glob` normal/boundary。
- `write` M1 atomic create/overwrite。
- `edit` M1 exact/atomic path。
- write/edit Workspace boundary。

期望默认行为与 M1 无回归。

## 5. Integration Evidence Format

每个 case 记录：

```text
Case ID
Result: PASS / FAIL / BLOCKED
Test name / command
Input config revisions + relevant toggle state
Observed desired state
Observed effective state
Observed MCP/error result where applicable
Filesystem/Activity assertion where applicable
```

不得记录 raw secret。

## 6. Integration Exit Criteria

M2 Integration 只有在以下全部满足时为 `PASS`：

- INT-01 ～ INT-18 全部 PASS。
- legacy schema v1 compatibility PASS。
- semantic configRevision behavior PASS。
- Runtime snapshot/tool registry wiring PASS。
- current generation immutability PASS。
- restart convergence PASS。
- disabled absent from `tools/list` PASS。
- stale disabled calls在 executor 前被拒绝且无 side effect。
- disabled / policy denied / not-ready / unknown semantics可区分。
- UI desired/effective/restart-required组合行为 PASS。
- M1 read/glob/write/edit regression PASS。

Integration PASS 后进入 `E2E_TEST_PLAN.md` 的真实本地黑盒验证。

## 7. Non-Claims

Integration PASS 不代表：

```text
real Runtime subprocess E2E PASS
External ChatGPT cache instantly refreshes
same-generation tools/list_changed exists
M3 Policy complete
M5 Bash complete
M7 schemaVersion v1→v2 migration complete
M2 ACCEPTED / FROZEN
```
