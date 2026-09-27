# CODEBASE MAP

> 基准：当前源码、manifest、测试与构建脚本。V2 已于 2026-09-27 完成最终外部验收。

## 1. Repository Structure

| Area | Current responsibility | Boundary |
| --- | --- | --- |
| `protocol/` | Tauri/Runtime 共用 control protocol、JSON Schema、TS generated types | Contract only |
| `runtime/` | WorkspaceGuard、Tools、Policy、ExecutionPipeline、Audit/Activity、Diagnostics、loopback MCP | 不拥有 Tunnel / Secret |
| `ui/src-tauri/src/runtime/` | Runtime distribution、child lifecycle、MCP endpoint handoff | endpoint 只供 Tauri 内部编排 |
| `ui/src-tauri/src/tunnel/` | Tunnel distribution、launch、readiness、process-tree cleanup | 不拥有 Workspace/Policy |
| `ui/src-tauri/src/harness.rs` | 产品级 eligibility/start/stop/restart/status、session/generation convergence | lifecycle 唯一 owner |
| `ui/src-tauri/src/secrets/` | Windows Credential Manager + test memory SecretStore | raw secret 不暴露给 frontend/Runtime |
| `ui/src/` | React 页面、Provider、real/mock client、状态/事件适配 | UI 不执行 tools |
| `ui/scripts/` | distribution staging、NSIS/portable build 与验证 | release packaging |
| `tunnel-client-v0.0.13-windows-amd64/` | Tunnel client + cloudflared + metadata/licenses/SBOM | bundled external connectivity payload |
| `wiki/` | 当前实现文档 | current facts |
| `docs-codex/` | V2 计划与执行记录 | historical implementation context |
| `docs/archive/` | 更早历史文档 | not current evidence |

## 2. Product Process Architecture

```text
React UI
  ↕ invoke / harness:status
Tauri HarnessController
  ├─ RuntimeSupervisor ── JSONL control ── chatgpt-harness-runtime
  │                                      └─ 127.0.0.1:<ephemeral>/mcp
  └─ TunnelProcessManager ── tunnel-client ── OpenAI control plane
                                      └─ MCP target = Runtime /mcp
```

真实 tool-call 链：

```text
ChatGPT → Tunnel → Runtime MCP → effective tools → MCP adapter
        → ExecutionPipeline → WorkspaceGuard → Policy → executor
        → Audit / Activity
```

## 3. Harness Lifecycle

产品级入口位于 `ui/src-tauri/src/commands/**`，状态 owner 位于 `ui/src-tauri/src/harness.rs`。

```text
harness_start_eligibility
  → config/workspace/security/distribution/secret validation

harness_start
  → Runtime start
  → mcp.endpoint.get
  → SecretStore get
  → Tunnel launch
  → readiness discovery + /readyz
  → Harness Ready

harness_stop
  → Tunnel/cloudflared cleanup
  → Runtime cleanup
  → session/endpoint clear
  → Harness stopped

harness_restart
  → full stop
  → reload config/secret
  → full start
```

Runtime watcher 用 generation，Tunnel watcher 用 instance id；stale callback 不会改变新 active session。

## 4. Runtime Execution

`read_file` 与 `glob` 由 Runtime MCP 暴露，统一进入：

```text
McpService
 ↓
McpExecutionContext
 ↓
ExecutionPipeline
 ↓
WorkspaceGuard
 ↓
PolicyEngine
 ↓
ExecutionPermit
 ↓
Tool executor
 ↓
Audit / Activity
```

关键 source：

- `runtime/src/mcp/**`
- `runtime/src/execution/**`
- `runtime/src/workspace/**`
- `runtime/src/policy/**`
- `runtime/src/tools/**`

## 5. Tunnel and Secret Boundary

Tunnel launch 消费：

```text
Tunnel ID
Runtime MCP endpoint
API key secret
bundled cloudflared
health URL-file
```

Raw API key 只在 Tauri controlled secret read 和 child environment 中短暂存在，不进入 Runtime 或普通状态输出。

Tunnel Ready 需要 control-plane connection、configured MCP target reachability 和 `/readyz` evidence。

Windows 使用 per-session Job Object 管理 Tunnel process tree。

## 6. Distribution

Distribution payload 至少包含：

```text
chatgpt-harness-ui.exe
runtime/chatgpt-harness-runtime.exe
tunnel/tunnel-client.exe
tunnel/cloudflared.exe
distribution.json
```

仓库提供 NSIS 与 portable Windows 分发流程。当前 release baseline 为 `0.1.1`，源码与 release packaging 均在同一 repository 管理。

## 7. Verification and Acceptance

自动化 gate 已有记录：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm test
npm run build
npm run distribution:stage
npm run bundle:windows
```

V2 final acceptance 另外验证真实外部产品链，不把 local/mock test 冒充 ChatGPT E2E。

2026-09-27 的最终验收状态为 **Accepted**。真实 ChatGPT caller 已通过 Harness 连接访问 active Workspace，并成功调用 `glob` / `read_file`。完整接受边界见 [`ACCEPTANCE.md`](./ACCEPTANCE.md)。

## 8. Documentation Boundary

```text
source/tests/manifests  → implementation truth
wiki/**                 → current explanation/status
docs-codex/**           → completed V2 planning/execution records
docs/archive/**         → older history
```

后续修改当前架构时，应同步更新 `wiki/ARCHITECTURE.md`、`wiki/MCP-AND-TUNNEL.md` 与 `wiki/IMPLEMENTATION-MATRIX.md`，避免再次出现状态漂移。
