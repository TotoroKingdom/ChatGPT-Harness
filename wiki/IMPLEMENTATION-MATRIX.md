# IMPLEMENTATION MATRIX

> 判定基准：当前 `HEAD` 的生产调用路径。状态只使用 `Implemented`、`Implemented / Internal`、`Partially Integrated`、`Stub / Placeholder`、`Not Implemented`、`Unclear`。`Core` 表示能力实现本身；`E2E` 表示用户或外部 caller 是否能从公开生产入口走完整链路。

## Status Matrix

| Capability | Core | Protocol | Tauri | UI | E2E | Evidence / Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Runtime lifecycle | Implemented | Implemented | Implemented | Implemented | Implemented | 完整 start/initialize/ack/status/stop；stop 超时会 kill，crash 可手工再启动，但没有 auto-restart。`runtime/src/server.rs:139-282`; `ui/src-tauri/src/runtime/mod.rs:239-465`; `HarnessDataProvider.tsx:344-391` |
| Workspace initialization | Implemented | Implemented | Implemented | Implemented | Implemented | UI 选目录并保存；initialize snapshot 构造 canonical Workspace/Guard。`protocol/src/v1/config.rs`; `commands/mod.rs:171-180,215-225`; `runtime/src/state.rs:62-85` |
| Path security | Implemented | Not Implemented | Partially Integrated | Not Implemented | Implemented | Tauri 只做本地 root `is_dir` 校验；真正的 file target enforcement 在 Runtime WorkspaceGuard，MCP read/glob 都经过它。实现是 canonicalize 后再 metadata/open/traverse，仍有 filesystem TOCTOU 竞态窗口。`workspace/**`; `execution/pipeline.rs:166-188`; `mcp/read_file.rs`; `mcp/glob.rs` |
| Tool registry | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | `tools.list` 完整贯通并返回 `read_file`/`glob`，Provider 会加载；当前 ToolsPage 改用 effective-tools control read，不消费 legacy registry resource。`tools/registry.rs:45-113`; `server.rs:284-296`; `HarnessDataProvider.tsx:263-277`; `ToolsPage.tsx` |
| Effective tools | Implemented | Implemented | Implemented | Implemented | Implemented | Runtime 根据 registry+policy+readiness 计算；`control.effective_tools.get` 到 ToolsPage 完整。`tools/effective.rs`; `control_read.rs:48-53`; `commands/control_read.rs:35-40`; `ToolsPage.tsx:7-24` |
| `read_file` | Implemented | Not Implemented | Not Implemented | Not Implemented | Partially Integrated | MCP local HTTP 可列出并调用，执行经过统一 pipeline；control protocol/Tauri/UI 无 tool-call API，外部 endpoint URL 也未交付。`tools/read_file.rs`; `mcp/read_file.rs`; `mcp/service.rs:44-54`; `mcp/http.rs` |
| `glob` | Implemented | Not Implemented | Not Implemented | Not Implemented | Partially Integrated | 与 `read_file` 相同：Runtime 内 HTTP MCP E2E，有真实 workspace security；产品 external connection 未闭合。`tools/glob.rs`; `mcp/glob.rs`; `mcp/service.rs:44-54` |
| Policy evaluation | Implemented | Implemented | Stub / Placeholder | Stub / Placeholder | Partially Integrated | ExecutionPipeline 真实调用 fail-closed PolicyEngine；但 `policies.list` 固定空数组，所以 PoliciesPage 永远看不到 system rules。`policy/engine.rs:75-167`; `execution/pipeline.rs:189-223`; `server.rs:298-304`; `PoliciesPage.tsx` |
| Execution pipeline | Implemented | Not Implemented | Not Implemented | Not Implemented | Partially Integrated | readiness→validate→workspace→policy→execute→audit 已接到 MCP adapters；没有 control/Tauri/UI 直接执行入口。config 的 `max_concurrent_requests` 未见 semaphore/计数 enforcement，RuntimeState 的 active/pending/running counters 固定为 0。`execution/pipeline.rs:133-238`; `state.rs:242-255`; `mcp/context.rs` |
| Approval flow | Stub / Placeholder | Implemented | Partially Integrated | Not Implemented | Not Implemented | ASK 会进入 AwaitingApproval 并返回 `ApprovalRequired`，但没有 pending store/resume；list 固定空，resolve 固定 ApprovalNotFound。Tauri 有 adapter，UI 无页面/动作。`execution/pipeline.rs:194-204`; `server.rs:337-358`; `runtime/mod.rs:451-462` |
| Lifecycle activity list/subscription | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | EventBus 与 notification bridge 可用，但生产 writer 只见 RuntimeStarted/Stopped；production component tree 未使用 `useActivitySubscription`。`events.rs`; `state.rs:172-198`; `server.rs:306-335`; `useActivitySubscription.ts` |
| Execution activity snapshot | Implemented | Implemented | Implemented | Implemented | Implemented | Pipeline audit → ActivityStore → `control.activity.snapshot` → ActivityPage；有界内存、手工刷新、非 live。ActivityStore 固定 256，未使用 config 的 `max_activity_events`。`activity/mod.rs:12-14,82-120`; `control_read.rs:43-47`; `ActivityPage.tsx` |
| Audit emission | Implemented / Internal | Partially Integrated | Partially Integrated | Partially Integrated | Partially Integrated | 结构化、redacted per-invocation audit 已接 execution；只保存在 bounded ActivityStore，没有 durable audit store/export。control-read 暴露的是 display projection。`audit/**`; `execution/pipeline.rs:226-238`; `activity/mod.rs` |
| Runtime diagnostics snapshot | Implemented | Implemented | Implemented | Implemented | Implemented | `control.diagnostics.get` 提供真实 safe snapshot，Overview 实际使用。request count 明确 NotInstrumented，Tunnel 明确 NotObserved/NotEstablished。`diagnostics/mod.rs`; `control_read.rs:43-45`; `RuntimeOverview.tsx` |
| `diagnostics.run` | Stub / Placeholder | Implemented | Implemented | Not Implemented | Stub / Placeholder | Runtime 固定返回单个 `runtime_skeleton` PASS；Tauri command 注册但当前 UI 不调用。`server.rs:360-382`; `commands/mod.rs:357-362`; `local-client.ts` 无对应方法 |
| Local setup diagnostics | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | Tauri 检查 workspace/tunnel fields/secret/security；Provider 有 `runDiagnostics`，但无生产页面调用者。`commands/mod.rs:364-495`; `HarnessDataProvider.tsx:393-397` |
| Control-read APIs | Implemented | Implemented | Implemented | Implemented | Implemented | diagnostics/activity/effective-tools 三条 typed read 均经过 stdio dispatch、Tauri command 和 UI parser/view。`protocol/src/v1/control_read.rs`; `runtime/src/control_read.rs`; `commands/control_read.rs`; `services/harness/control-read.ts` |
| Configuration load/save/reset | Implemented | Implemented | Implemented | Implemented | Implemented | `<app_data_dir>/config.json`、schema v1、semantic revision、atomic replace；running Runtime 只在 restart 后接收新 snapshot。`config/mod.rs:40-186`; `commands/mod.rs:45-130`; `SettingsPage.tsx` |
| Secret storage | Implemented | Implemented | Implemented | Implemented | Implemented | Windows production 使用 keyring `windows-native`/Credential Manager；JSON 只存 reference。这里的 E2E 只指 UI↔store，不代表 Tunnel 消费 secret。`secrets/mod.rs:36-77`; `lib.rs:2-20`; `commands/mod.rs:78-168` |
| Runtime discovery | Implemented | Implemented | Implemented | Partially Integrated | Partially Integrated | debug 固定 target path；installed 固定 manifest+runtime layout并校验。Eligibility 向 UI 报错；缺少真实安装后启动 E2E。`runtime/discovery.rs`; `runtime/manifest.rs`; `commands/mod.rs:228-284` |
| Distribution / installer | Implemented | Implemented | Implemented | Implemented | Partially Integrated | `HEAD` staging + Tauri NSIS resources + verifier 存在；未发现自动安装并启动的 E2E。`ui/package.json`; `ui/scripts/stage-distribution.ps1` at `HEAD`; `verify-nsis-bundle.ps1`; `tauri.conf.json` |
| MCP adapter/service | Implemented | Implemented / Internal | Not Implemented | Partially Integrated | Partially Integrated | rmcp ServerHandler 真实列出/call `read_file`/`glob`；UI 仅把 MCP component 状态投影为 Runtime 状态，不是连通性证明。`mcp/service.rs`; `mcp/read_file.rs`; `mcp/glob.rs`; `local-client.ts:46-74` |
| MCP Streamable HTTP transport | Implemented | Implemented / Internal | Not Implemented | Not Implemented | Partially Integrated | Runtime 初始化会绑定 `127.0.0.1:0/mcp`，stop 会 join；HTTP tests 实际调用工具。URL 没进入 control response/Tauri/UI/Tunnel。`mcp/http.rs:105-141`; `mcp/runtime.rs:75-190`; `server.rs:226-282` |
| External MCP / ChatGPT reachability | Not Implemented | Not Implemented | Not Implemented | Not Implemented | Not Implemented | 没有 endpoint discovery/registration/public route；loopback随机 URL 仅存于 Runtime 私有 service。`mcp/runtime.rs:12-16,155-156`; `initialize.rs`; `runtime/mod.rs:508-552` |
| Tunnel process manager | Implemented / Internal | Partially Integrated | Implemented / Internal | Not Implemented | Not Implemented | manager 可 start/stop/restart/watch，且被 Tauri `.manage` 构造；只有自身 tests 调用，没有 lifecycle command。`tunnel/mod.rs:114-368`; `lib.rs:20-49` |
| Tunnel orchestration | Not Implemented | Partially Integrated | Not Implemented | Not Implemented | Not Implemented | `runtime_start` 不碰 Tunnel；无 start/stop/status command，无 UI action。Local client 永远报告 Tunnel stopped。`commands/mod.rs:215-225`; `lib.rs:22-49`; `local-client.ts:46-74,150-174` |
| Tunnel arguments / credentials delivery | Not Implemented | Partially Integrated | Not Implemented | Partially Integrated | Not Implemented | Settings 可存 tunnel id/path/API key；manager spawn 不传 argv/env/stdin，也不读 SecretStore，`runtime_endpoint_ref` 只进 status metadata。`config.rs`; `commands/mod.rs:78-108`; `tunnel/mod.rs:60-79,178-185,345-367` |

## MCP Integration Trace

### What exists and is called

1. `RuntimeCore.initialize` 构造 shared ExecutionPipeline。
2. `server::handle_request("initialize")` 从 core 取得 `McpExecutionContext`，调用 `McpRuntimeService.start`。
3. service thread 启动 Tokio current-thread runtime，绑定 `127.0.0.1:0`，挂载 `/mcp` 的 rmcp Streamable HTTP service。
4. MCP `tools/list` 只返回 effective catalog 中 `Available` 的 `read_file`、`glob`。
5. MCP `tools/call` 进入 adapter，再进入同一个 readiness/workspace/policy/execution/audit pipeline。
6. `runtime.stop`、stdin EOF 或 control I/O 退出都会关闭并 join endpoint。

### Where product integration stops

- 随机 endpoint URL 仅在 `McpRuntimeService.endpoint_url` 内保存。
- `InitializeResult`、`RuntimeState`、Tauri state/command、React client 均不携带该 URL。
- 没有代码把 ChatGPT 或 tunnel-client 配置到该 URL。
- UI 把 MCP 状态等同于 Runtime readiness；这表示“Runtime 内 endpoint 生命周期随 Runtime”，不表示外部连接成功。

结论：

- MCP adapter：`Implemented`
- MCP service：`Implemented`
- Runtime construction/call path：`Implemented`
- Loopback HTTP transport：`Implemented / Internal`
- ChatGPT/Tunnel E2E：`Not Implemented`
- 总体 integration level：`Partially Integrated`

## Tunnel Integration Trace

### Lifecycle implementation

`TunnelProcessManager` 实现了 owner state、instance id、防 stale stop、start conflict、exit watcher、Windows CTRL_BREAK graceful stop、timeout kill 与 restart。Tauri app 创建了 manager singleton。

### Missing product wiring

- 没有 Tauri tunnel lifecycle command。
- `runtime_start`/`runtime_stop` 只操作 RuntimeSupervisor。
- HarnessControlClient 没有 tunnel lifecycle API；UI 没有 start/stop action。
- `TunnelLaunch` 没有 API key 字段；production spawn 不传任何 tunnel id、endpoint、credential argv/env/stdin。
- Runtime MCP endpoint URL 没有交给 Tauri，因此即使调用 manager 也没有真实 endpoint 可用。
- `HEAD` 的 distribution staging/NSIS resources 不包含仓库里的 tunnel-client package。

结论：manager 是 `Implemented / Internal`，orchestration 与用户可用 Tunnel 都是 `Not Implemented`。

## Code ↔ UI Gaps

| Gap | Evidence | Consequence |
| --- | --- | --- |
| Policy engine 有真实 system rules，但 `policies.list` 固定空 | `policy/engine.rs:90-114`; `server.rs:298-304` | PoliciesPage 永远空，不能代表没有生效规则 |
| Approval DTO/Tauri methods 存在，但无 pending store/resume/UI | `server.rs:337-358`; `runtime/mod.rs:451-462`; `client.ts:76-77` | ASK 只能终止为 ApprovalRequired，无法批准继续 |
| Tunnel manager 被构造但无人调用 | `lib.rs:20-49`; `rg TunnelProcessManager` 仅 manager tests/start 本体 | 配置 tunnel 不会启动 tunnel-client |
| UI 可保存 API key，但 manager 不读取 | `commands/mod.rs:78-108`; `tunnel/mod.rs:345-367` | secret storage 成功不等于 credential 已交给 Tunnel |
| MCP HTTP endpoint 启动，但 URL 未暴露 | `mcp/runtime.rs:139-156`; `server.rs:240-250`; `runtime/mod.rs:508-552` | 外部 MCP client 无生产 discovery path |
| UI MCP 状态等于 Runtime 状态 | `local-client.ts:46-74`; `HarnessDataProvider.tsx:90-112` | 页面显示 MCP ready 只证明 Runtime readiness，不证明外部 reachability |
| Legacy activity stream 与 execution activity 分离 | `events.rs`; `activity/mod.rs`; `control_read.rs` | `activity.event` 不会实时推送 MCP execution audit；ActivityPage 是手工 snapshot |
| `useActivitySubscription` 只在 tests 使用 | `rg useActivitySubscription ui/src` | Provider 提供 live subscription 能力，但生产 UI 未订阅 |
| `diagnostics.run` command 已注册但返回固定 check | `server.rs:360-382`; `lib.rs:48` | command 存在不等于真实 diagnostic run |
| Tauri local diagnostics 有 provider action但无页面 caller | `commands/mod.rs:364-495`; `HarnessDataProvider.tsx:393-397` | 真实本机检查当前不可由用户触发 |
| Provider 仍加载 legacy `tools.list`，ToolsPage 使用 control-read | `HarnessDataProvider.tsx:263-277`; `ToolsPage.tsx:7-24` | 同时存在 declared catalog resource 与 visible effective catalog，含义不同 |
| MockClient 模拟 Tunnel/MCP 全部 ready | `mock-client.ts:71-74` vs `local-client.ts:46-74` | Mock UI 行为不能推断 real adapter 的 Tunnel 能力 |

## Protocol / Fixture Drift

源码中存在两项 contract fixture 与 Runtime 实际输出的明确漂移：

- `rpc.error.not_initialized.json` 使用 numeric code `-32001`，Runtime 当前使用 `HARNESS_MIN = -32099`。
- `initialize.response.json` 的 server name/capability 与 Runtime 当前返回的 `chatgpt-harness-runtime`/default empty capabilities 不同。

Protocol contract tests 验证 fixture 对 schema/model 可 round-trip，但不把 Runtime 真实输出与 fixture 做一致性断言。因此“contract tests 通过”不能消除此漂移。Evidence: `protocol/fixtures/v1/rpc.error.not_initialized.json`; `protocol/fixtures/v1/initialize.response.json`; `protocol/tests/contracts.rs`; `runtime/src/server.rs:17-21,240-250`。

## Test Coverage Matrix

| Layer | Present | What it proves | What it does not prove |
| --- | --- | --- | --- |
| Unit | Yes | workspace/tool/policy/execution/activity/diagnostics/config/secret/tunnel manager logic | product wiring |
| Protocol contract | Yes | fixtures validate schema/model，generated artifacts current，method registry exact | Runtime output equals fixtures |
| Runtime in-process integration | Yes | real `run_io` dispatch、initialize gates、MCP lifecycle、control reads | actual executable + Tauri + UI |
| MCP HTTP integration | Yes | loopback HTTP request、tools list/call、path security、Host/Origin/size handling | external ChatGPT/tunnel reachability |
| Tauri supervisor process fixture | Yes | actual OS child、stdin/stdout correlation、stop/crash/race behavior | bundled release Runtime or installed layout execution |
| UI component/client | Yes | React/provider behavior与 invoke mapping；77 tests | Tauri command/runtime correctness |
| Real Runtime integration from UI | Not Implemented | — | UI→Tauri→release Runtime→workspace |
| Installer E2E | Not Implemented | — | install→launch→discover bundled Runtime |
| Tunnel E2E | Not Implemented | — | UI→secret/config→tunnel process→MCP→external client |

## Verified This Archaeology Run

- `cargo test --workspace`: 205 passed（Runtime 154、Tauri 42、Protocol contract 9）。
- `npm test`: 77 passed（jsdom/Mocked Tauri API）。
- `npm run build`: TypeScript checks 与 Vite production build passed。
- 这些成功结果按上表限定解释，不升级为未被测试覆盖的 E2E 结论。

## Uncertain Findings

- 未执行真实 NSIS 安装，因此无法仅凭源码确认安装器在目标机器上的最终文件布局、权限与启动行为。
- 未对真实 Windows Credential Manager 做集成操作；可以确认 production implementation 使用 `keyring` Windows backend，不能确认每个目标 Windows 环境的运行结果。
- WorkspaceGuard 对目标做 canonicalization/containment 后，executor 才执行 metadata/open/read_dir；未见 handle-based 绑定或竞态替换测试，因此不能确认它抵抗检查后替换（TOCTOU）攻击。
- 仓库中存在 tunnel-client 二进制包，但当前 `HEAD` 没有 production orchestration/packaging chain；无法从源码确认该二进制期望的 CLI/env credential contract，也不应猜测。
- 未发现外部 ChatGPT 配置或部署侧代码，因此无法确认仓库之外是否有人手工发现并连接 Runtime 的 ephemeral MCP URL；仓库内产品路径不能做到这一点。
