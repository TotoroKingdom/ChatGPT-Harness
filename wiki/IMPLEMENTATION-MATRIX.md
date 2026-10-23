# IMPLEMENTATION MATRIX

~~~yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN
~~~

判定基准是当前源码、Tests、Manifests 和构建脚本。外部 ChatGPT E2E 已完成 **manual acceptance（人工验收）**；它不是自动化测试列的 PASS，也不把 mock、进程存在或本地 Runtime HTTP 测试冒充真实外部调用。

| Capability | Core | Protocol | Tauri | UI | Automated evidence | Acceptance status |
| --- | --- | --- | --- | --- | --- | --- |
| Runtime lifecycle | Implemented | Implemented | Implemented | Implemented | PASS | V2 accepted |
| Runtime MCP endpoint handoff | Implemented | Implemented | Implemented | Internal only | PASS | V2 accepted |
| WorkspaceGuard / Policy / Execution | Implemented | Not Applicable | Not Applicable | Not Applicable | PASS | V2 accepted |
| SecretStore controlled read | Not Applicable | Reference implemented | Implemented | Transient input only | PASS | V2 accepted |
| API key boundary | Not Applicable | SecretReference only | Implemented | Implemented | PASS | V2 accepted |
| Tunnel distribution resolution | Not Applicable | Manifest metadata | Implemented | Dev override only | PASS | V2 accepted |
| Tunnel launch | Not Applicable | Not Applicable | Implemented | Harness action | PASS | V2 accepted |
| Tunnel readiness | Not Applicable | Not Applicable | Implemented | Real state consumed | PASS | V2 accepted |
| Windows process-tree ownership | Not Applicable | Not Applicable | Implemented | Not Applicable | PASS | V2 accepted |
| Harness eligibility | Not Applicable | Implemented statuses | Implemented | Remediation rendered | PASS | V2 accepted |
| Harness orchestration | Not Applicable | HarnessStatus payload | Implemented | Start / Stop / Restart | PASS | V2 accepted |
| Runtime crash convergence | Implemented | Not Applicable | Implemented | Event-driven state | PASS | V2 accepted |
| Tunnel crash convergence | Not Applicable | Not Applicable | Implemented | Event-driven state | PASS | V2 accepted |
| Real UI state | Not Applicable | HarnessStatus | Implemented | Implemented | PASS | V2 accepted |
| read_file / glob execution | Implemented | Not Applicable | Not Applicable | Activity view | PASS locally | External path manually accepted |
| Distribution staging | Not Applicable | Manifest | Implemented | Build scripts | PASS | V2 accepted |
| NSIS bundle verification | Not Applicable | Manifest | Implemented | Build script | PASS | V2 accepted |
| Development external E2E | Implemented | Implemented | Implemented | Implemented | Not an automated test | MANUAL ACCEPTANCE |
| Installed external E2E | Implemented | Implemented | Implemented | Implemented | Not an automated test | MANUAL ACCEPTANCE |
| Approval resume workflow | Out of V2 scope | DTOs only | Not applicable | Not applicable | Not implemented | V3 Phase 3 planned |
| write_file / edit_file / bash | Out of V2 scope | Not applicable | Not applicable | Not applicable | Not implemented | V3 planned |

## Automated validation boundary

自动化验证只声明已实际执行的本地门禁与组件覆盖，例如：

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cd ui
npm test
npm run build
npm run distribution:stage
npm run bundle:windows
~~~

这些门禁证明代码、contract、Runtime/Tauri/UI 测试和分发校验的相应覆盖；它们不自动证明当前 ChatGPT connector、真实 Tunnel credential 或人工外部连接。

## V2 acceptance boundary

人工验收覆盖：

~~~text
Configure Workspace / Tunnel ID / API Key
        ↓
Start Harness
        ↓
Runtime = ready
MCP = ready
Tunnel = ready
Harness = ready
        ↓
ChatGPT discovers tools
        ↓
ChatGPT calls read_file / glob
        ↓
WorkspaceGuard + Policy + Execution
        ↓
Activity appears in UI
        ↓
Stop Harness
        ↓
Tunnel and Runtime exit
~~~

人工验收仍然标记为 manual acceptance；不要将其改写成 automated PASS。

## Source-of-truth locations

| Question | Source |
| --- | --- |
| Runtime / MCP | runtime/src/** |
| Control protocol | protocol/src/** |
| Runtime supervision / endpoint handoff | ui/src-tauri/src/runtime/** |
| Tunnel launch / readiness / cleanup | ui/src-tauri/src/tunnel/** |
| Harness lifecycle | ui/src-tauri/src/harness.rs, ui/src-tauri/src/commands/** |
| React state adapter | ui/src/services/harness/**, ui/src/app/providers/** |
| V2 acceptance record | wiki/V2-ACCEPTANCE.md |
| V3 historical archive | docs/archive/v3-2026-10/README.md |
