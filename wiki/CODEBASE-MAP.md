# CODEBASE MAP

> 基准：当前源码、manifest、测试与构建脚本。`docs/archive/**` 不参与当前实现判断；`docs-codex/**` 是 V2 执行记录，不是现状证据。

## 1. Repository Structure

| 区域 | 当前职责 | 主要边界 |
| --- | --- | --- |
| `protocol/` | Tauri/Runtime 共用 control protocol、JSON Schema、TypeScript 生成物 | 只定义 contract，不实现进程或业务 |
| `runtime/` | WorkspaceGuard、工具注册、Policy、ExecutionPipeline、Activity/Audit、Diagnostics、loopback MCP | 只拥有 Runtime 与本地 MCP，不拥有 Tunnel/Secret |
| `ui/src-tauri/src/runtime/` | Runtime 分发解析、child 生命周期、MCP endpoint handoff | endpoint 只在 Tauri 内部保存，停止/崩溃时清除 |
| `ui/src-tauri/src/tunnel/` | Tunnel 分发解析、CLI launch、健康发现、`/readyz`、进程树清理 | API Key 仅受控注入 child 环境；状态不含 secret 或 endpoint |
| `ui/src-tauri/src/harness.rs` | HarnessController、统一 start/stop/restart/status/eligibility、session/generation 收敛 | 是产品生命周期唯一 owner |
| `ui/src-tauri/src/secrets/` | Windows Credential Manager 与测试内存 SecretStore | `get` 仅供 Tauri 受控 Tunnel launch 使用 |
| `ui/src/` | React 页面、Provider、real/mock client、状态/事件适配 | 产品生命周期调用 Harness commands，不直接组合 Runtime |
| `ui/scripts/` | release staging、NSIS bundle 验证 | 同时校验 UI、Runtime、Tunnel client、cloudflared 与 manifest |
| `tunnel-client-v0.0.13-windows-amd64/` | 已验证的 Tunnel client 0.0.13 包 | staging 原样保留可执行文件、metadata、license/notice/SBOM |

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

Runtime MCP 的真实调用链保持为：

```text
ChatGPT → Tunnel → Runtime MCP → effective tools → MCP adapter
        → ExecutionPipeline → WorkspaceGuard → Policy → executor
        → Audit / Activity → UI
```

Runtime 只绑定 loopback。Tauri 通过 `mcp.endpoint.get` 取回当前 Runtime 实例的 endpoint；该 URL 不进入 AppConfig、RuntimeState、React state 或 `HarnessStatus`。

## 3. Harness Lifecycle

产品级入口位于 `ui/src-tauri/src/commands/mod.rs`：

```text
harness_start_eligibility
  → config/workspace/security/runtime distribution
  → Tunnel ID/API key reference/secret retrieval
  → Tunnel distribution resolution

harness_start
  → eligibility re-check
  → Runtime start
  → mcp.endpoint.get + loopback validation
  → SecretStore get
  → tunnel-client launch
  → health URL discovery + /readyz
  → Harness Ready

harness_stop
  → Tunnel/cloudflared cleanup
  → Runtime cleanup
  → session/endpoint clear
  → Harness stopped

harness_restart = complete stop → reload config/secret → full start
```

`HarnessController` 用单 operation lock 串行 lifecycle command；Runtime watcher 带 Runtime generation，Tunnel watcher 带 instance id，stale callback 不会修改新 session。Runtime crash 会让 Harness 离开 Ready 并停止关联 Tunnel；Tunnel crash 会让 Harness 进入 Error，Runtime 可保留作诊断。

产品状态来自 `HarnessStatus`：

```text
lifecycle: stopped | starting | ready | degraded | error | stopping
runtime:   stopped | starting | ready | degraded | error | stopping
mcp:       stopped | starting | ready | degraded | error | stopping
tunnel:    stopped | starting | ready | degraded | error | stopping
```

`Harness Ready` 只有 Runtime、MCP、Tunnel 同时 Ready 且属于同一 active session 才成立。

## 4. Tunnel Launch and Security Boundary

实际 0.0.13 CLI contract 在 `TunnelProcessManager` 中固定为：

```text
run
--control-plane.tunnel-id <id>
--control-plane.api-key env:HARNESS_TUNNEL_API_KEY
--mcp.server-url url=<Runtime endpoint>,channel=main
--cloudflared.path <bundled companion>
--health.listen-addr 127.0.0.1:0
--health.url-file <temporary loopback health artifact>
```

原始 API Key 不进入 argv、AppConfig、Runtime snapshot、IPC response、Activity、diagnostics 或普通日志；只在 child launch 的环境变量中短暂存在。Tunnel stdout/stderr 不透传到用户界面。

Windows 使用每 managed session 一个 Job Object，并在 graceful timeout 后终止 Job Object，确保 `tunnel-client.exe` 与 `cloudflared.exe` descendants 一起退出；不按进程名杀 unrelated process。

Tunnel Ready 需要：control-plane connection、configured MCP target reachability、authoritative `/readyz` 健康检查。PID、spawn 成功、监听端口都不能单独产生 Ready。

## 5. Distribution

`ui/src-tauri/src/runtime/manifest.rs` 当前为 distribution schema 2，声明：

```text
runtime/chatgpt-harness-runtime.exe
tunnel/tunnel-client.exe
tunnel/cloudflared.exe
```

`stage-distribution.ps1` 保留 Tunnel 包中的 client、companion、metadata、license、notice、SBOM；`tauri.conf.json` 将 `runtime`、`tunnel` 和 `distribution.json` 作为资源；`verify-nsis-bundle.ps1` 验证完整 payload。已验证命令：`npm run distribution:stage`、`npm run bundle:windows`。

## 6. Runtime Execution and Activity

`read_file` 与 `glob` 没有移动到 Tauri/Tunnel，仍由 Runtime MCP 进入既有安全执行链。Runtime 的 execution Activity 通过 control-read/API 和 UI Activity 页面展示；Tunnel/ChatGPT 真实外部调用需要真实凭据才能在当前机器完成最终验收。

## 7. Verification Boundary

已自动验证：

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `npm test`（77 tests）
- `npm run build`
- `npm run distribution:stage`
- `npm run bundle:windows`

未在本次运行中宣称通过的项目：使用真实 Tunnel ID/API Key 从 ChatGPT 连接、发现工具、调用 `read_file`/`glob`、查看 Activity，并从已安装 NSIS 包完成同一流程。这些步骤需要当前环境之外的真实凭据、ChatGPT connector 配置和人工外部连接权限。
