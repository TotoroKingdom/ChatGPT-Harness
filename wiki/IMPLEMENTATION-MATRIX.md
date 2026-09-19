# IMPLEMENTATION MATRIX

> 判定基准：当前源码与自动化验证结果。`E2E` 只有在真实外部 caller 能走完整链路时才算通过；本表不把 mock 或本地 Runtime HTTP 测试冒充 ChatGPT E2E。

| Capability | Core | Protocol | Tauri | UI | Automated | Current evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Runtime lifecycle | Implemented | Implemented | Implemented | Implemented | PASS | `RuntimeSupervisor`、Runtime JSONL lifecycle、stop timeout、crash watcher |
| Runtime MCP endpoint handoff | Implemented | Implemented | Implemented | Internal only | PASS | `mcp.endpoint.get`、loopback/path validation、generation-bound storage、stop/crash clear |
| WorkspaceGuard / Policy / Execution | Implemented | Not Applicable | Not Applicable | Not Applicable | PASS | Runtime MCP `read_file`/`glob` 继续经过既有 ExecutionPipeline |
| SecretStore controlled read | Not Applicable | Reference implemented | Implemented | Transient input only | PASS | Windows Credential Manager + MemorySecretStore `get`; no frontend get command |
| API key boundary | Not Applicable | SecretReference only | Implemented | Implemented | PASS | config stores locator only；argv 使用 `env:HARNESS_TUNNEL_API_KEY`；raw key 不进状态/日志 |
| Tunnel distribution resolution | Not Applicable | Manifest metadata | Implemented | Override is dev-only | PASS | installed layout canonicalization、manifest schema 2、client/companion checks |
| Tunnel launch | Not Applicable | Not Applicable | Implemented | Harness action | PASS | verified 0.0.13 flags、MCP URL、Tunnel ID、health URL file |
| Tunnel readiness | Not Applicable | Not Applicable | Implemented | Real state consumed | PASS | discovery、loopback health、`/readyz`、control-plane/MCP/readiness evidence、loss monitor |
| Windows process-tree ownership | Not Applicable | Not Applicable | Implemented | Not Applicable | PASS | per-session Job Object、kill-on-close、graceful then forced tree cleanup code/tests |
| Harness eligibility | Not Applicable | Implemented statuses | Implemented | Rendered remediation | PASS | workspace/security/runtime/Tunnel/API key/distribution/conflict checks |
| Harness orchestration | Not Applicable | HarnessStatus payload | Implemented | Start/Stop/Restart | PASS | serialized controller、rollback、ordering、session/generation stale suppression |
| Runtime crash convergence | Implemented | Not Applicable | Implemented | Event-driven state | PASS | Runtime watcher invalidates MCP/Harness and stops associated Tunnel |
| Tunnel crash convergence | Not Applicable | Not Applicable | Implemented | Event-driven state | PASS | Tunnel watcher changes Harness to Error; explicit restart required |
| Real UI state | Not Applicable | HarnessStatus | Implemented | Implemented | PASS | Provider uses `harness_status`/`harness:status`; no Tunnel=stopped or MCP=Runtime projection |
| `read_file` / `glob` execution | Implemented | Not Applicable | Not Applicable | Activity view | PASS locally | Runtime MCP and security tests pass; external ChatGPT path requires credentials |
| Distribution staging | Not Applicable | Manifest | Implemented | Build scripts | PASS | staging copies UI, Runtime, Tunnel, cloudflared, metadata, licenses/notices/SBOM |
| NSIS bundle verification | Not Applicable | Manifest | Implemented | Build script | PASS | `npm run bundle:windows` generated and verified NSIS installer |
| Development external E2E | Implemented | Implemented | Implemented | Implemented | NOT RUN | requires real Tunnel ID/API Key and ChatGPT connector access |
| Installed external E2E | Implemented | Implemented | Implemented | Implemented | NOT RUN | bundle payload passed; clean installed ChatGPT flow still needs external credentials/access |

## Automated Gates

All repository gates passed in the final run:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
  Runtime: 155 passed
  Tauri: 48 passed
  Protocol: 9 passed
npm test
  5 files / 77 tests passed
npm run build
npm run distribution:stage
npm run bundle:windows
```

## External Acceptance Boundary

The code now provides the complete production path:

```text
Harness config
→ Runtime endpoint handoff
→ secure Tunnel launch
→ authoritative Tunnel readiness
→ ChatGPT connector
→ read_file / glob
→ WorkspaceGuard / Policy / Execution
→ Activity
```

The first five steps and all local automated evidence are implemented. The last external steps cannot be honestly marked PASS in this run because no real Tunnel credentials or ChatGPT connector session were supplied. A future acceptance run must prove both development and installed flows, then verify all owned Runtime/Tunnel/cloudflared processes exit after Stop.
