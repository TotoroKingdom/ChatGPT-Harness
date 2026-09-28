# CODEBASE MAP

> 基准：当前源码、manifest、Tests 与构建脚本。V2 已完成真实链路验收并冻结；V3 文档只描述计划。

~~~yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN
~~~

## 1. Repository Structure

| 区域 | 当前职责 | 主要边界 |
| --- | --- | --- |
| protocol/ | Tauri/Runtime 共用 control protocol、JSON Schema、TypeScript 生成物 | 只定义 contract，不实现进程或业务 |
| runtime/ | WorkspaceGuard、工具注册、Policy、ExecutionPipeline、Activity/Audit、Diagnostics、loopback MCP | 只拥有 Runtime 与本地 MCP，不拥有 Tunnel/Secret |
| ui/src-tauri/src/runtime/ | Runtime 分发解析、child 生命周期、MCP endpoint handoff | endpoint 只在 Tauri 内部保存，停止/崩溃时清除 |
| ui/src-tauri/src/tunnel/ | Tunnel 分发解析、CLI launch、健康发现、/readyz、进程树清理 | API Key 仅受控注入 child 环境；状态不含 secret 或 endpoint |
| ui/src-tauri/src/harness.rs | HarnessController、统一 start/stop/restart/status/eligibility、session/generation 收敛 | 是产品生命周期唯一 owner |
| ui/src-tauri/src/secrets/ | Windows Credential Manager 与测试内存 SecretStore | get 仅供 Tauri 受控 Tunnel launch 使用 |
| ui/src/ | React 页面、Provider、real/mock client、状态/事件适配 | 产品生命周期调用 Harness commands，不直接组合 Runtime |
| ui/scripts/ | release staging、NSIS bundle 验证 | 同时校验 UI、Runtime、Tunnel client、cloudflared 与 manifest |
| tunnel-client-v0.0.13-windows-amd64/ | 已验证的 Tunnel client 0.0.13 包 | staging 原样保留 executable、metadata、license/notice/SBOM |
| wiki/ | 当前代码事实、实现矩阵和验收边界 | 不记录 V3 未实现能力 |
| docs-codex/v3/ | V3 当前开发计划 | 不作为实现证据 |
| docs/archive/ | V2 和更早资料 | 只提供历史上下文 |

## 2. Product Process Architecture

~~~text
React UI
  ↕ invoke / harness:status
Tauri HarnessController
  ├─ RuntimeSupervisor ── JSONL control ── chatgpt-harness-runtime
  │                                      └─ 127.0.0.1:<ephemeral>/mcp
  └─ TunnelProcessManager ── tunnel-client ── OpenAI control plane
                                      └─ MCP target = Runtime /mcp
~~~

Runtime MCP 的真实调用链保持为：

~~~text
ChatGPT → Tunnel → Runtime MCP → effective tools → MCP adapter
        → ExecutionPipeline → WorkspaceGuard → Policy → executor
        → Audit / Activity → UI
~~~

Runtime 只绑定 loopback。Tauri 通过 mcp.endpoint.get 取回当前 Runtime 实例的 endpoint；该 URL 不进入 AppConfig、RuntimeState、React state 或 HarnessStatus。

## 3. Harness Lifecycle

产品级入口位于 ui/src-tauri/src/commands/mod.rs：

~~~text
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
~~~

HarnessController 用单 operation lock 串行 lifecycle command；Runtime watcher 带 Runtime generation，Tunnel watcher 带 instance id，stale callback 不会修改新 session。Runtime crash 会让 Harness 离开 Ready 并停止关联 Tunnel；Tunnel crash 会让 Harness 进入 Error，Runtime 可保留作诊断。

产品状态来自 HarnessStatus：

~~~text
lifecycle: stopped | starting | ready | degraded | error | stopping
runtime:   stopped | starting | ready | degraded | error | stopping
mcp:       stopped | starting | ready | degraded | error | stopping
tunnel:    stopped | starting | ready | degraded | error | stopping
~~~

Harness Ready 只有 Runtime、MCP、Tunnel 同时 Ready 且属于同一 active session 才成立。

## 4. Tunnel Launch and Security Boundary

实际 0.0.13 CLI contract 在 TunnelProcessManager 中固定为：

~~~text
run
--control-plane.tunnel-id <id>
--control-plane.api-key env:HARNESS_TUNNEL_API_KEY
--mcp.server-url url=<Runtime endpoint>,channel=main
--cloudflared.path <bundled companion>
--health.listen-addr 127.0.0.1:0
--health.url-file <temporary loopback health artifact>
~~~

原始 API Key 不进入 argv、AppConfig、Runtime snapshot、IPC response、Activity、diagnostics 或普通日志；只在 child launch 的环境变量中短暂存在。Tunnel stdout/stderr 不透传到用户界面。

Windows 使用每 managed session 一个 Job Object，并在 graceful timeout 后终止 Job Object，确保 tunnel-client.exe 与 cloudflared.exe descendants 一起退出；不按进程名杀 unrelated process。

Tunnel Ready 需要 control-plane connection、configured MCP target reachability 和 authoritative /readyz 健康检查。PID、spawn 成功、监听端口都不能单独产生 Ready。

## 5. Distribution

ui/src-tauri/src/runtime/manifest.rs 当前为 distribution schema 2，声明：

~~~text
runtime/chatgpt-harness-runtime.exe
tunnel/tunnel-client.exe
tunnel/cloudflared.exe
~~~

stage-distribution.ps1 保留 Tunnel 包中的 client、companion、metadata、license、notice、SBOM；tauri.conf.json 将 runtime、tunnel 和 distribution.json 作为资源；verify-nsis-bundle.ps1 验证完整 payload。

## 6. Runtime Execution and Acceptance

read_file 与 glob 仍由 Runtime MCP 进入既有安全执行链。Runtime 的 execution Activity 通过 control-read/API 和 UI Activity 页面展示。

V2 外部 ChatGPT E2E 已完成 manual acceptance（人工验收）。这表示真实 ChatGPT → Tunnel → Runtime MCP → tool call → Activity 链路已经验收；该验收不被写成自动化测试 PASS。

V2 仍不包含 write_file、edit_file、bash / shell 或 Approval resume；这些能力位于 docs-codex/v3/ROADMAP.md，均为 PLANNED / NOT IMPLEMENTED。

## 7. Source-of-Truth Locations

| Question | Source |
| --- | --- |
| Runtime MCP lifecycle | runtime/src/mcp/runtime.rs, runtime/src/mcp/http.rs |
| MCP service/tool adapter | runtime/src/mcp/service.rs |
| Runtime initialization | runtime/src/server.rs |
| Runtime state / execution | runtime/src/state.rs, runtime/src/execution/** |
| Workspace / policy | runtime/src/workspace/**, runtime/src/policy/** |
| Tunnel process lifecycle | ui/src-tauri/src/tunnel/mod.rs |
| Tauri command surface | ui/src-tauri/src/commands/mod.rs |
| Tauri app registration | ui/src-tauri/src/lib.rs |
| Harness controller | ui/src-tauri/src/harness.rs |
| Config persistence | ui/src-tauri/src/config/mod.rs |
| Secret storage | ui/src-tauri/src/secrets/mod.rs |
| Real UI adapter | ui/src/services/harness/local-client.ts |
| Current implementation status | wiki/IMPLEMENTATION-MATRIX.md |
| V2 acceptance | wiki/V2-ACCEPTANCE.md |
| V3 development plan | docs-codex/v3/ROADMAP.md |
