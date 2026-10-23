# M00 As-Is Baseline — Current Main Ownership and Verification

> Baseline date: 2026-10-23 (Asia/Shanghai)
> Observed commit: `7f2a482b1babfdb08b5e80cabfa430d3d0227dac`
> Branch: `main`
> Scope: T1 source/test inspection only; this is not V4 feature acceptance.

本文只记录当前源码、manifest 和本轮可重复的本地测试所能证明的事实。历史里程碑文档和历史 E2E 文件只作为背景，不会把旧版本结论升级为当前候选版本的 `EXTERNALLY_ACCEPTED`。

## 1. Status vocabulary

本表只使用 M00 SPEC 允许的状态：

- `IMPLEMENTED`：当前源码中存在对应行为或所有权边界；不等于本轮完整验收。
- `LOCAL_TESTED`：本轮执行了对应的本地自动化测试。
- `EXTERNALLY_ACCEPTED`：当前候选版本有可核验的外部真实链路证据。
- `NOT_VERIFIED`：源码或本轮证据不足以确认。
- `KNOWN_ISSUE`：已知缺口或风险已明确记录。
- `NOT_IMPLEMENTED`：当前产品没有该 V4 能力。

## 2. Current fact table

| Fact ID | Current claim | Source evidence | Test evidence | Status | V4 dependency | Gap / note |
| --- | --- | --- | --- | --- | --- | --- |
| ASIS-01 | 生产 Cargo workspace 由 `protocol`、`runtime`、`ui/src-tauri` 三个 crate 组成，版本均为 `0.1.3`。 | `Cargo.toml`; `protocol/Cargo.toml`; `runtime/Cargo.toml`; `ui/src-tauri/Cargo.toml` | `cargo check --workspace` | `LOCAL_TESTED` | A01 | `ui/` React 前端不属于根 Cargo workspace。 |
| ASIS-02 | Tauri Host 的 Harness 生命周期由 `HarnessController` 负责，Start/Stop/Restart 共用 `operation: Mutex<()>` 串行化。 | `ui/src-tauri/src/harness.rs`: `HarnessController`, `start`, `stop`, `restart`, `start_locked`, `stop_locked` | `cargo test -p chatgpt-harness-ui harness`：10 passed | `LOCAL_TESTED` | A01 | Tray/Notification 尚未实现。 |
| ASIS-03 | `HarnessController` 组合 Runtime 和 Tunnel；Harness Ready 由 Runtime/MCP/Tunnel 状态与 Tunnel readiness 收敛决定，而不是由窗口或 PID 单独推断。 | `ui/src-tauri/src/harness.rs`: `HarnessStatus`, `on_runtime_event`, `on_tunnel_event`, `apply_tunnel_start_snapshot_if_pending` | 同 ASIS-02 | `LOCAL_TESTED` | A01 | 本轮未执行真实桌面外部 E2E。 |
| ASIS-04 | Runtime 子进程、generation、Control RPC、MCP endpoint 和进程 identity 由 `RuntimeSupervisor` 管理。 | `ui/src-tauri/src/runtime/mod.rs`: `RuntimeSupervisor`, `RuntimeProcessPhase`, `RuntimeProcessIdentity`, generation/control methods | `cargo check --workspace`; `cargo test -p chatgpt-harness-ui harness` | `LOCAL_TESTED` | A01 | 当前基线没有 V4 startup phase recorder。 |
| ASIS-05 | Tunnel 由 `TunnelProcessManager` 管理；Tunnel 只把已解析的 Runtime MCP endpoint、Tunnel ID、SecretStore 得到的 key 和 proxy 传给受管 Tunnel client，并上报 health/readiness/recovery。 | `ui/src-tauri/src/tunnel/mod.rs`; `ui/src-tauri/src/tunnel/discovery.rs`; `ui/src-tauri/src/harness.rs` | `cargo check --workspace`; `cargo test -p chatgpt-harness-ui harness` | `LOCAL_TESTED` | A01 | 当前 Tunnel 是传输桥，不是 Policy 或 Approval authority。 |
| ASIS-06 | Runtime MCP 使用 loopback Streamable HTTP `/mcp`，endpoint 由 Runtime 绑定并由 Host/Supervisor 取得。 | `runtime/src/mcp/http.rs`; `runtime/src/mcp/service.rs`; `ui/src-tauri/src/runtime/mod.rs` | Runtime HTTP tests are present; 本轮 T1 未执行外部 ChatGPT/Tunnel E2E | `IMPLEMENTED` | A01, M00-AC11 | M00 外部基线 E2E 仍待单独授权/执行。 |
| ASIS-07 | MCP 请求进入 `ExecutionPipeline`，依次经过 readiness/tool availability、WorkspaceGuard、PolicyEngine、Approval/SessionGrant、ExecutionPermit 和 executor；只有 pipeline 内部构造 permit。 | `runtime/src/execution/pipeline.rs`; `runtime/src/execution/executor.rs`; `runtime/src/workspace/guard.rs`; `runtime/src/policy/engine.rs` | `cargo test -p chatgpt-harness-runtime audit`：14 passed；现有 execution/mcp tests included in the filtered run | `LOCAL_TESTED` | A01, A17 | M00 不改变执行语义。 |
| ASIS-08 | `WorkspaceGuard` 是 Workspace 边界 authority，Runtime tool executor 不直接接受外部物理路径作为授权依据。 | `runtime/src/workspace/guard.rs`; `runtime/src/workspace/path_resolver.rs`; `runtime/src/execution/pipeline.rs` | `cargo test -p chatgpt-harness-runtime activity`; existing MCP boundary tests | `LOCAL_TESTED` | SEC-AC02 | symlink/junction/TOCTOU 的 V4 grep 覆盖属于 M05。 |
| ASIS-09 | PolicyEngine 编译并保存 generation-specific immutable policy snapshot；Policy decision 结果通过 Runtime pipeline 产生，UI 不重新计算。 | `runtime/src/policy/engine.rs`; `runtime/src/state.rs`; `ui/src/pages/policies/*` | `cargo test -p chatgpt-harness-runtime audit`; `cargo test -p chatgpt-harness-ui config` | `LOCAL_TESTED` | A01, SEC-AC02 | 当前没有 Policy Simulator。 |
| ASIS-10 | Pending Approval 由 Runtime `ApprovalManager` 持有；Session Grant 由 Runtime `SessionGrantStore` 持有，Runtime generation 重置时清理实时状态。 | `runtime/src/approval.rs`; `runtime/src/session_grant.rs`; `runtime/src/state.rs`; `runtime/src/execution/pipeline.rs` | `cargo test -p chatgpt-harness-runtime audit`; existing approval/session tests | `LOCAL_TESTED` | SEC-AC03, A17 | 历史记录尚不存在，不能恢复这些 authority 对象。 |
| ASIS-11 | `AuditEvent` 是当前 Runtime 的 typed、安全观察事件；包含 `event_id`、`request_id`、tool/policy/approval/execution 等投影字段，不包含 Runtime identity。 | `runtime/src/audit/event.rs`; `runtime/src/audit/mod.rs` | `cargo test -p chatgpt-harness-runtime audit`：AuditEvent/redaction tests pass | `LOCAL_TESTED` | A04, A08 | `runtime_id` 目前在 `RuntimeCore` 存在，但未随 AuditEvent 明确携带；M03 必须建立 confirmed mapping，不能按时间猜。 |
| ASIS-12 | `AuditSink` 只接收已创建的 `AuditEvent`，trait 没有返回 authorization/execution result 的接口。 | `runtime/src/audit/mod.rs`; `runtime/src/state.rs` | `cargo check --workspace`; audit tests | `LOCAL_TESTED` | A04, A17 | M00 T2 只加 test-only projection proof，不给 sink 增加持久化副作用。 |
| ASIS-13 | 当前 Activity 有两种观察投影：Runtime 内存 `ActivityStore`（capacity 256）和 control `EventBus`/`ActivityEventInput`；两者都消费 audit evidence，不是 durable History。 | `runtime/src/activity/mod.rs`; `runtime/src/events.rs`; `runtime/src/state.rs`; `protocol/src/v1/activity.rs` | `cargo test -p chatgpt-harness-runtime activity`：9 passed；`cargo test -p harness-control-protocol` | `LOCAL_TESTED` | A04, A06 | SQLite History/Trace/Stats 尚未实现。 |
| ASIS-14 | 持久化 AppConfig 当前 schema 是 2；生产 `ConfigStore` 只加载/保存 schema 1→2 或 schema 2，不接受未知 schema；SecretReference 只保存 provider/key opaque reference。 | `protocol/src/v1/config.rs`; `ui/src-tauri/src/config/mod.rs`; `ui/src-tauri/src/config/migration.rs` | `cargo test -p chatgpt-harness-ui config`：53 passed；`cargo test -p harness-control-protocol`：18 passed | `LOCAL_TESTED` | A15 | M00 T4 只验证未来 v2→v3 contract，不修改 `CURRENT_SCHEMA_VERSION`。 |
| ASIS-15 | `RuntimeConfigSnapshot` 只包含 Runtime 生效所需字段，不包含 `UiConfig`；UI-only changes 不推进 Runtime-facing revision。 | `protocol/src/v1/config.rs`; `ui/src-tauri/src/config/mod.rs` (`same_restart_semantics`) | `cargo test -p chatgpt-harness-ui config` | `LOCAL_TESTED` | A15 | future desktop/history host-only fields must remain outside this snapshot. |
| ASIS-16 | SecretStore 在 Host 边界解析 SecretReference；raw API key 不进入普通 AppConfig、Runtime snapshot、Activity 或 AuditEvent。 | `ui/src-tauri/src/secrets/mod.rs`; `ui/src-tauri/src/config/mod.rs`; `runtime/src/audit/event.rs` | `cargo test -p chatgpt-harness-ui config`; runtime audit redaction tests | `LOCAL_TESTED` | SEC-AC02 | M00 fixtures only use synthetic opaque references/sentinels. |
| ASIS-17 | ToolRegistry/ToolConfig/Policy defaults 当前显式包含 `read_file`、`glob`、`write`、`edit`、`bash`；正常模式不暴露 bash；没有 grep。 | `runtime/src/tools/registry.rs`; `protocol/src/v1/config.rs`; `runtime/src/state.rs`; `runtime/src/mcp/service.rs` | `cargo test -p chatgpt-harness-runtime audit`; `cargo test -p chatgpt-harness-ui config` | `LOCAL_TESTED` | A12, M05 prerequisite | `grep` 是 M05 新功能，M00 不注册新 Tool。 |
| ASIS-18 | desired config 与 Runtime generation-effective snapshot 分离；Runtime 重新初始化时采用 immutable snapshot。 | `ui/src-tauri/src/config/mod.rs`; `runtime/src/state.rs`; `runtime/src/tools/effective.rs` | `cargo test -p chatgpt-harness-ui config`; existing Runtime state tests | `LOCAL_TESTED` | A01, A15 | M00 不引入 v3 production migration。 |
| ASIS-19 | 当前不存在生产 SQLite History、V4 Evidence Store、Tray、Windows Notification、grep、MCP Request Trace 或完整 Controlled Evaluation runner。 | absence confirmed against `runtime/`, `ui/src-tauri/src`, `protocol/`, root workspace and manifests; V4 docs are planning inputs only | `cargo check --workspace`; source/manifest inspection | `NOT_IMPLEMENTED` | M00 scope boundary | M00 POCs must stay isolated and removable. |
| ASIS-20 | 当前候选版本的真实 ChatGPT → Tunnel → Runtime MCP 外部基线调用尚未在本轮执行。历史 `docs/archive/` 记录不绑定本候选 commit。 | `docs/archive/` historical artifacts; `docs-codex/v4/milestones/M00-baseline-contract-gate/E2E_TEST_PLAN.md` | no external E2E run in T1 | `NOT_VERIFIED` | M00-AC11 | Must remain a separate M00 E2E case; do not claim PASS from local MCP tests. |

## 3. Ownership summary

```text
React UI            → 展示状态、表达操作意图
Tauri Host          → ConfigStore/SecretStore、HarnessController、RuntimeSupervisor、TunnelProcessManager
Runtime             → MCP service、ToolRegistry、WorkspaceGuard、PolicyEngine、ApprovalManager、SessionGrantStore、ExecutionPipeline/Permit、AuditEvent
Tunnel              → ChatGPT ↔ Runtime MCP transport bridge
Activity/EventBus   → observation-only bounded projections
```

M00 T2 的 projection 只能消费 Runtime 已产生的 `AuditEvent`。它不能从 UI、历史数据或时间相近的事件重新推导 Policy/Approval/Permit。

## 4. Local verification record

Environment: Windows host, Rust `1.98.0`, Cargo `1.98.0`.

| Command | Result |
| --- | --- |
| `cargo check --workspace` | PASS; production workspace checked; existing compiler warnings only. |
| `cargo test -p harness-control-protocol` | PASS; 18 tests passed across unit/contract/config/policy suites. |
| `cargo test -p chatgpt-harness-runtime audit` | PASS; 14 filtered tests passed. |
| `cargo test -p chatgpt-harness-runtime activity` | PASS; 9 filtered tests passed. |
| `cargo test -p chatgpt-harness-ui config` | PASS; 53 filtered tests passed. |
| `cargo test -p chatgpt-harness-ui harness` | PASS; 10 filtered tests passed. |

These are local development checks only. They do not execute M00 Integration, real ChatGPT E2E, a paid/networked Agent/Model, or Manual Acceptance.

## 5. T1 conclusion

- M00-AC01/AC02 的当前源码 ownership baseline 已形成；T2 可使用明确的 identity/evidence 缺口继续实施。
- `runtime_id` 尚未在 `AuditEvent` 中出现，不能由时间或 Host generation 猜测；这是 M03 需要补足的 confirmed mapping contract，而不是 T1 伪造的值。
- M00 没有修改生产行为、CURRENT_SCHEMA_VERSION、工具目录或授权边界。
- M00-E2E-01 仍为 `NOT_VERIFIED`，留给冻结的 E2E 计划和明确授权后的执行。
