# IMPLEMENTATION MATRIX

> 判定基准：当前 `HEAD` 的生产调用路径。状态使用 `Implemented`、`Implemented / Internal`、`Partially Integrated`、`Stub / Placeholder`、`Not Implemented`、`Not Applicable`、`Unclear`。  
> `Not Applicable` 表示该能力本来就不应由该层负责，不代表缺失实现。  
> `E2E` 表示目标用户或外部 caller 是否能从公开生产入口走完整链路。

## Status Matrix

| Capability | Core | Protocol | Tauri | UI | E2E | Evidence / Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Runtime lifecycle | Implemented | Implemented | Implemented | Implemented | Implemented | 完整 start/initialize/ack/status/stop；stop 超时会 kill；crash 后可手工再启动，但没有 auto-restart。`runtime/src/server.rs`; `ui/src-tauri/src/runtime/mod.rs`; `HarnessDataProvider.tsx` |
| Workspace initialization | Implemented | Implemented | Implemented | Implemented | Implemented | UI 选目录并保存；initialize snapshot 构造 canonical Workspace/Guard。`protocol/src/v1/config.rs`; `commands/mod.rs`; `runtime/src/state.rs` |
| Path security | Implemented | Not Applicable | Partially Integrated | Not Applicable | Partially Integrated | 真正 target enforcement 在 Runtime WorkspaceGuard；Tauri 只做本地 root 合法性检查。MCP read/glob 会经过 guard，但 external MCP 产品链尚未闭合。仍存在 TOCTOU 不确定性。`runtime/src/workspace/**`; `execution/pipeline.rs`; `mcp/read_file.rs`; `mcp/glob.rs` |
| Tool registry | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | `tools.list` 完整贯通并返回 declared tools；Provider 会加载，但当前 ToolsPage 主要展示 effective-tools control read。`tools/registry.rs`; `server.rs`; `HarnessDataProvider.tsx`; `ToolsPage.tsx` |
| Effective tools | Implemented | Implemented | Implemented | Implemented | Implemented | Runtime 根据 registry + policy + readiness 计算；`control.effective_tools.get` 到 ToolsPage 完整。`tools/effective.rs`; `control_read.rs`; `commands/control_read.rs`; `ToolsPage.tsx` |
| `read_file` | Implemented | Not Applicable | Not Applicable | Not Applicable | Partially Integrated | 该工具面向 MCP caller，不需要 control/Tauri/UI direct tool-call API。Runtime 内 MCP HTTP 可以列出并调用；external endpoint discovery/reachability 未闭合。`tools/read_file.rs`; `mcp/read_file.rs`; `mcp/service.rs`; `mcp/http.rs` |
| `glob` | Implemented | Not Applicable | Not Applicable | Not Applicable | Partially Integrated | 与 `read_file` 相同：Runtime 内 MCP 路径可执行，产品 external connection 未闭合。`tools/glob.rs`; `mcp/glob.rs`; `mcp/service.rs` |
| Policy evaluation | Implemented | Not Applicable | Not Applicable | Not Applicable | Partially Integrated | ExecutionPipeline 真实调用 fail-closed PolicyEngine；evaluation 本身是 Runtime 内部能力。外部 MCP 产品链仍未闭合。`policy/engine.rs`; `execution/pipeline.rs` |
| Policy listing / UI visibility | Stub / Placeholder | Implemented | Implemented | Implemented | Stub / Placeholder | protocol/Tauri/UI 链存在，但 Runtime `policies.list` 固定返回空数组，所以页面不能反映真实 system rules。`runtime/src/server.rs`; `PoliciesPage.tsx` |
| Execution pipeline | Implemented | Not Applicable | Not Applicable | Not Applicable | Partially Integrated | readiness→validate→workspace→policy→execute→audit 已接到 MCP adapters；direct control/Tauri/UI execution API 不属于当前设计。`max_concurrent_requests` 未见实际 enforcement，Runtime counters 仍为固定值。`execution/pipeline.rs`; `state.rs`; `mcp/context.rs` |
| Approval flow | Stub / Placeholder | Implemented | Partially Integrated | Not Implemented | Not Implemented | ASK 会进入 AwaitingApproval 并返回 `ApprovalRequired`，但没有 pending store/resume；list 固定空，resolve 固定 ApprovalNotFound。`execution/pipeline.rs`; `server.rs`; `runtime/mod.rs` |
| Lifecycle activity list/subscription | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | EventBus 与 notification bridge 可用，但生产 writer 主要是 lifecycle event；production component tree 未使用 `useActivitySubscription`。`events.rs`; `state.rs`; `server.rs`; `useActivitySubscription.ts` |
| Execution activity snapshot | Implemented | Implemented | Implemented | Implemented | Implemented | Pipeline audit → ActivityStore → `control.activity.snapshot` → ActivityPage；有界内存、手工刷新、非 live。`activity/mod.rs`; `control_read.rs`; `ActivityPage.tsx` |
| Audit emission | Implemented / Internal | Not Applicable | Partially Integrated | Partially Integrated | Partially Integrated | 结构化、redacted per-invocation audit 已接 execution；Tauri/UI 看到的是 activity projection，不是独立 durable audit API/store。`audit/**`; `execution/pipeline.rs`; `activity/mod.rs` |
| Runtime diagnostics snapshot | Implemented | Implemented | Implemented | Implemented | Implemented | `control.diagnostics.get` 提供真实 safe snapshot，Overview 实际使用。部分字段明确是 NotInstrumented / NotObserved。`diagnostics/mod.rs`; `control_read.rs`; `RuntimeOverview.tsx` |
| `diagnostics.run` | Stub / Placeholder | Implemented | Implemented | Not Implemented | Stub / Placeholder | Runtime 固定返回单个 `runtime_skeleton` PASS；Tauri command 注册但当前 UI 不调用。`server.rs`; `commands/mod.rs` |
| Local setup diagnostics | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | Tauri 检查 workspace/tunnel fields/secret/security；Provider 有 action，但无生产页面 caller。Protocol 在这里主要复用 `DiagnosticRun` DTO。`commands/mod.rs`; `HarnessDataProvider.tsx` |
| Control-read APIs | Implemented | Implemented | Implemented | Implemented | Implemented | diagnostics/activity/effective-tools 三条 typed read 均经过 stdio dispatch、Tauri command 和 UI parser/view。`protocol/src/v1/control_read.rs`; `runtime/src/control_read.rs`; `commands/control_read.rs`; `services/harness/control-read.ts` |
| Configuration load/save/reset | Implemented | Implemented | Implemented | Implemented | Implemented | `<app_data_dir>/config.json`、schema v1、semantic revision、atomic replace；运行中的 Runtime 只在 restart 后接收新 snapshot。`config/mod.rs`; `commands/mod.rs`; `SettingsPage.tsx` |
| Secret storage | Implemented | Implemented | Implemented | Implemented | Implemented | Windows production 使用 keyring Windows backend；JSON 只存 reference。这里的 E2E 仅指 UI↔secret store，不代表 Tunnel 已消费 secret。`secrets/mod.rs`; `commands/mod.rs` |
| Runtime discovery | Implemented | Not Applicable | Implemented | Partially Integrated | Partially Integrated | debug/installed discovery 与 manifest 校验存在；Eligibility 可向 UI 报错。未验证真实安装后启动 E2E。`runtime/discovery.rs`; `runtime/manifest.rs`; `commands/mod.rs` |
| Distribution / installer | Not Applicable | Not Applicable | Implemented | Implemented | Partially Integrated | staging + Tauri NSIS resources + verifier 存在；未发现自动安装并启动的 E2E；Tunnel payload 未纳入当前 staging。`ui/package.json`; `ui/scripts/**`; `tauri.conf.json` |
| MCP adapter/service | Implemented | Not Applicable | Not Applicable | Not Applicable | Partially Integrated | rmcp ServerHandler 真实 list/call `read_file`/`glob`；这是 Runtime 内 MCP 能力。UI 的 MCP 状态只是 Runtime readiness projection。`mcp/service.rs`; `mcp/read_file.rs`; `mcp/glob.rs`; `local-client.ts` |
| MCP Streamable HTTP transport | Implemented | Not Applicable | Not Applicable | Not Applicable | Partially Integrated | Runtime 初始化会绑定 `127.0.0.1:0/mcp`，stop 会 join；HTTP tests 实际调用工具。随机 URL 未交付给产品其他层。`mcp/http.rs`; `mcp/runtime.rs`; `server.rs` |
| External MCP / ChatGPT reachability | Not Applicable | Not Implemented | Not Implemented | Not Implemented | Not Implemented | 缺少 endpoint discovery/registration/public route；loopback随机 URL 仅存于 Runtime 私有 service。`mcp/runtime.rs`; `initialize.rs`; `runtime/mod.rs` |
| Tunnel process manager | Not Applicable | Not Applicable | Implemented / Internal | Not Applicable | Not Implemented | manager 可 start/stop/restart/watch，并被 Tauri `.manage` 构造；没有生产 lifecycle command/caller。`tunnel/mod.rs`; `lib.rs` |
| Tunnel orchestration | Not Applicable | Not Applicable | Not Implemented | Not Implemented | Not Implemented | `runtime_start` 不碰 Tunnel；无 start/stop/status command，无 UI action；real local client 永远报告 Tunnel stopped。`commands/mod.rs`; `lib.rs`; `local-client.ts` |
| Tunnel arguments / credentials delivery | Not Applicable | Not Applicable | Not Implemented | Partially Integrated | Not Implemented | Settings 可存 tunnel id/path/API key，但 manager production spawn 不传 tunnel id、endpoint、credential argv/env/stdin，也不读取 SecretStore。`commands/mod.rs`; `tunnel/mod.rs` |

## MCP Integration Trace

### What exists and is called

1. `RuntimeCore.initialize` 构造 shared ExecutionPipeline。
2. `server::handle_request("initialize")` 从 core 取得 `McpExecutionContext`，调用 `McpRuntimeService.start`。
3. service thread 启动 Tokio current-thread runtime，绑定 `127.0.0.1:0`，挂载 `/mcp` 的 rmcp Streamable HTTP service。
4. MCP `tools/list` 只返回 effective catalog 中 `Available` 的 `read_file`、`glob`。
5. MCP `tools/call` 进入 adapter，再进入 readiness/workspace/policy/execution/audit pipeline。
6. `runtime.stop`、stdin EOF 或 control I/O 退出都会关闭并 join endpoint。

### Where product integration stops

- 随机 endpoint URL 仅保存在 `McpRuntimeService.endpoint_url`。
- `InitializeResult`、`RuntimeState`、Tauri command/state、React client 均不携带该 URL。
- 没有仓库内生产代码把 ChatGPT 或 tunnel-client 配置到该 URL。
- UI 把 MCP 状态投影自 Runtime readiness；这不等于 external connection 成功。

结论：

- MCP adapter/service：`Implemented`
- Loopback HTTP transport：`Implemented / Internal`
- External endpoint discovery / reachability：`Not Implemented`
- 总体 integration level：`Partially Integrated`

## Tunnel Integration Trace

### Lifecycle implementation

`TunnelProcessManager` 实现 owner state、instance id、防 stale stop、start conflict、exit watcher、Windows graceful stop fallback、timeout kill 与 restart。Tauri app 创建 manager singleton。

### Missing product wiring

- 没有 Tauri tunnel lifecycle command。
- `runtime_start` / `runtime_stop` 只操作 `RuntimeSupervisor`。
- `HarnessControlClient` 没有 tunnel lifecycle API；UI 没有 start/stop action。
- `TunnelLaunch` 没有 API key 字段。
- production `spawn_process` 不传 tunnel id、runtime endpoint、credential argv/env/stdin。
- Runtime MCP endpoint URL 没有交给 Tauri/Tunnel。
- 当前 distribution staging 未纳入 tunnel-client payload。

结论：

- manager：`Implemented / Internal`
- orchestration：`Not Implemented`
- 用户可用 Tunnel：`Not Implemented`

## Code ↔ UI Gaps

| Gap | Evidence | Consequence |
| --- | --- | --- |
| Policy engine 有真实 system rules，但 `policies.list` 固定空 | `policy/engine.rs`; `server.rs` | PoliciesPage 空不能解释为“没有生效规则” |
| Approval DTO/Tauri methods 存在，但无 pending store/resume/UI | `server.rs`; `runtime/mod.rs`; `client.ts` | ASK 只能返回 ApprovalRequired，无法批准后继续执行 |
| Tunnel manager 被构造但无人走生产 lifecycle path | `lib.rs`; `tunnel/mod.rs` | 配置 Tunnel 不会启动 tunnel-client |
| UI 可保存 API key，但 manager 不读取 | `commands/mod.rs`; `tunnel/mod.rs` | secret storage 成功不等于 credential 已交给 Tunnel |
| MCP HTTP endpoint 启动，但 URL 未暴露 | `mcp/runtime.rs`; `server.rs`; `runtime/mod.rs` | 外部 MCP client 无仓库内 production discovery path |
| UI MCP 状态等于 Runtime 状态投影 | `local-client.ts` | 页面显示 MCP ready 不能证明 external reachability |
| Lifecycle activity 与 execution activity 分离 | `events.rs`; `activity/mod.rs`; `control_read.rs` | `activity.event` 不会实时推送 execution audit；ActivityPage 是 snapshot |
| `useActivitySubscription` 未挂入生产组件树 | `ui/src/**` | live subscription 能力存在，但当前生产 UI 不消费 |
| `diagnostics.run` 已注册但返回固定 skeleton | `server.rs`; `lib.rs` | command 存在不等于真实 diagnostic run |
| Tauri local diagnostics 有 provider action但无页面 caller | `commands/mod.rs`; `HarnessDataProvider.tsx` | 本机检查当前没有用户入口 |
| Provider 仍加载 legacy `tools.list`，ToolsPage 使用 effective control-read | `HarnessDataProvider.tsx`; `ToolsPage.tsx` | declared catalog 与 visible effective catalog 同时存在且语义不同 |
| MockClient 可模拟 Tunnel/MCP ready | `mock-client.ts` vs `local-client.ts` | Mock UI 行为不能证明 real adapter 的 Tunnel 能力 |

## Protocol / Fixture Drift

当前源码中存在至少两项 contract fixture 与 Runtime 实际输出的明确漂移：

- `rpc.error.not_initialized.json` 使用 numeric code `-32001`；Runtime 当前 `NOT_INITIALIZED_ERROR` 使用 `error_codes::HARNESS_MIN`，而 `-32001` 被用于 `ALREADY_INITIALIZED_ERROR`。
- `initialize.response.json` 的 server name/capability 与 Runtime 当前返回的 `chatgpt-harness-runtime` / default empty capabilities 不同。

`protocol/tests/contracts.rs` 验证 fixture 对 schema/model 可 round-trip，并验证 generated artifacts current；它不会启动 Runtime 并比较真实 wire output。

因此“contract tests 通过”不能消除这类 fixture/runtime drift。

## Test Coverage Matrix

| Layer | Present | What it proves | What it does not prove |
| --- | --- | --- | --- |
| Unit | Yes | workspace/tool/policy/execution/activity/diagnostics/config/secret/tunnel manager logic | product wiring |
| Protocol contract | Yes | fixtures validate schema/model；generated artifacts current；method registry exact | Runtime output equals fixtures |
| Runtime in-process integration | Yes | real `run_io` dispatch、initialize gates、MCP lifecycle、control reads | actual executable + Tauri + UI |
| MCP HTTP integration | Yes | loopback HTTP request、tools list/call、path security、HTTP guards | external ChatGPT/tunnel reachability |
| Tauri supervisor process fixture | Yes | OS child、stdin/stdout correlation、stop/crash/race behavior | bundled release Runtime / installed layout execution |
| UI component/client | Yes | React/provider behavior 与 invoke mapping | Tauri command/runtime correctness |
| Real Runtime integration from UI | Not Implemented | — | UI→Tauri→release Runtime→workspace |
| Installer E2E | Not Implemented | — | install→launch→discover bundled Runtime |
| Tunnel E2E | Not Implemented | — | UI→secret/config→tunnel process→MCP→external client |

## Verified This Archaeology Run

上一轮生成该 archaeology 基线时记录：

- `cargo test --workspace`: 205 passed（Runtime 154、Tauri 42、Protocol contract 9）。
- `npm test`: 77 passed（jsdom / mocked Tauri API）。
- `npm run build`: TypeScript checks 与 Vite production build passed。

这些成功结果只能按上面的测试层级解释，不能升级成未被测试覆盖的产品 E2E 结论。

## Uncertain Findings

- 未执行真实 NSIS 安装，因此无法仅凭源码确认安装器在目标机器上的最终文件布局、权限与启动行为。
- 未对真实 Windows Credential Manager 做集成操作；可以确认 production implementation 使用 Windows keyring backend，不能确认每个目标环境的运行结果。
- WorkspaceGuard 做 canonicalization/containment 后 executor 才继续 filesystem operation；未见 handle-based binding 或竞态替换测试，因此不能确认抵抗检查后替换（TOCTOU）攻击。
- 仓库中存在 tunnel-client 二进制包，但当前 `HEAD` 没有 production orchestration/packaging chain；无法从仓库源码确认该二进制期望的完整 CLI/env credential contract。
- 未发现仓库内外部 ChatGPT 配置或部署侧代码，因此仓库只能证明产品路径没有完成 external reachability；不能对仓库之外的人工配置作推断。
