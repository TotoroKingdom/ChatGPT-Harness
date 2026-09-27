# IMPLEMENTATION MATRIX

> Last accepted: **2026-09-27**. Automated verification and manual external acceptance are intentionally distinguished.

| Capability | Status | Verification | Current evidence |
| --- | --- | --- | --- |
| Runtime lifecycle | Implemented | PASS automated | `RuntimeSupervisor`, JSONL lifecycle, timeout/crash watcher |
| Runtime MCP endpoint handoff | Implemented | PASS automated | `mcp.endpoint.get`, loopback validation, generation-bound storage, stop/crash clear |
| WorkspaceGuard / Policy / Execution | Implemented | PASS automated | MCP tools enter the shared ExecutionPipeline |
| SecretStore controlled read | Implemented | PASS automated | Windows Credential Manager + MemorySecretStore; no frontend secret-read API |
| API key boundary | Implemented | PASS automated | Secret reference in config; raw key excluded from Runtime/status/argv/activity |
| Tunnel distribution resolution | Implemented | PASS automated | packaged client/companion resolution and validation |
| Tunnel launch | Implemented | PASS automated | Tunnel ID + Runtime MCP URL + env secret reference + cloudflared |
| Tunnel readiness | Implemented | PASS automated | health discovery, loopback probe, `/readyz`, control-plane/MCP evidence |
| Windows process-tree ownership | Implemented | PASS automated | per-session Job Object and tree cleanup |
| Harness eligibility | Implemented | PASS automated | workspace/security/runtime/Tunnel/secret/distribution/conflict checks |
| Harness orchestration | Implemented | PASS automated | serialized controller, rollback, ordering, session/generation stale suppression |
| Runtime crash convergence | Implemented | PASS automated | Runtime failure invalidates MCP/Harness and stops associated Tunnel |
| Tunnel crash convergence | Implemented | PASS automated | Tunnel failure removes Harness Ready; explicit restart recovers |
| Real UI state | Implemented | PASS automated | UI consumes `HarnessStatus` instead of projecting Runtime state |
| `read_file` / `glob` | Implemented | PASS automated + external acceptance | Runtime tests pass; real external caller accepted |
| Execution Activity / Diagnostics | Implemented | PASS automated + acceptance | Runtime-owned observability exposed to UI/control-read |
| NSIS distribution | Implemented | PASS automated build gate | staged and bundle-verified distribution path |
| Portable distribution | Implemented | Release evidence | portable build/verify path present in current 0.1.1 release baseline |
| Development external ChatGPT E2E | Implemented | PASS manual acceptance | final V2 external acceptance completed |
| Packaged external ChatGPT E2E | Implemented | PASS manual acceptance | final packaged-product acceptance user-confirmed |
| Approval resume workflow | Post-V2 | NOT IMPLEMENTED | `ASK` state exists; complete resume flow is intentionally deferred |
| Write / Shell / DB / Git-write tools | Post-V2 | NOT IMPLEMENTED | intentionally outside V2 security scope |

## Automated Gates

Recorded repository gates for the V2 implementation include:

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

These gates validate repository behavior but do not substitute for external ChatGPT E2E.

## External Acceptance

V2 final acceptance is recorded separately in [`ACCEPTANCE.md`](./ACCEPTANCE.md).

The accepted production path is:

```text
Harness config
→ Runtime start / endpoint handoff
→ secure Tunnel launch
→ authoritative Tunnel readiness
→ ChatGPT connection
→ read_file / glob
→ WorkspaceGuard / Policy / Execution
→ Activity / UI
→ controlled shutdown
```

Manual external acceptance is not represented as a CI test. The distinction is intentional so future regressions can be traced to either internal automated gates or real external integration.
