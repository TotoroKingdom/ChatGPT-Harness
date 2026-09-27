# V2 Final Acceptance

## Status

**ACCEPTED**

- Acceptance date: **2026-09-27**
- Source baseline at documentation closeout: `18f0e8617b12b6c4045515231bf61fbab10082a2`
- Release baseline: `0.1.1`
- Scope: ChatGPT-Harness V2 read-only product chain

## Accepted Product Chain

```text
Configure Workspace / Tunnel ID / API Key
        ↓
Start Harness
        ↓
Runtime Ready
        ↓
Runtime MCP endpoint handoff
        ↓
Tunnel Connected + MCP Reachable
        ↓
Harness Ready
        ↓
ChatGPT discovers and invokes tools
        ↓
read_file / glob
        ↓
WorkspaceGuard
        ↓
Policy / ExecutionPipeline
        ↓
Audit / Activity
        ↓
Stop Harness / owned-process cleanup
```

## Evidence Classes

### Automated repository verification

The V2 implementation recorded passing repository gates for Rust formatting/check/tests, frontend tests/build, distribution staging, and Windows bundle verification. Exact gate counts are retained in [`IMPLEMENTATION-MATRIX.md`](./IMPLEMENTATION-MATRIX.md).

### Real external acceptance

External acceptance is separate from local/mock MCP testing.

During the final acceptance session, the real ChatGPT caller successfully reached the configured Harness workspace and invoked both exposed read-only capabilities:

```text
glob
read_file
```

This proves the connection crossed the external product boundary rather than stopping at a local Runtime HTTP test.

The final packaged-product acceptance status is user-confirmed. This document records that acceptance; it does not relabel manual integration checks as automated CI.

## Definition of Done

| Requirement | Result |
| --- | --- |
| Runtime and MCP start under product lifecycle | PASS |
| Runtime endpoint is handed to Tauri without entering normal config/UI state | PASS |
| Tunnel receives real Runtime target and secret reference | PASS |
| Tunnel readiness is stronger than process existence | PASS |
| Harness Ready represents Runtime + MCP + Tunnel readiness | PASS |
| ChatGPT can call `read_file` / `glob` through the external path | PASS |
| Workspace/Policy/Execution remain in the tool path | PASS |
| Raw API key remains outside Runtime and normal status/activity data | PASS |
| Runtime/Tunnel failures converge Harness out of Ready | PASS |
| Windows distribution path is packaged | PASS |
| V2 final external acceptance | PASS |

## V2 Freeze Boundary

Acceptance closes V2 around the smallest useful read-only Harness:

```text
read_file
glob
```

The following are explicitly **not** required for V2 acceptance:

- complete approval/resume workflow;
- `write_file` / edit operations;
- shell execution;
- database execution;
- Git write operations;
- broader computer-use capabilities.

## Post-V2 Entry Rule

Higher-permission tools should not be added by simply extending the MCP dispatcher.

The next security-sensitive foundation should be a complete approval lifecycle:

```text
Policy = ASK
   ↓
Pending Approval
   ↓
Allow / Deny
   ↓
Resume original request
   ↓
Execute / Reject
```

Only after this path is defined, testable, auditable, and recoverable should V3 add write or arbitrary execution capabilities.

## Documentation Closeout

After acceptance:

- `wiki/**` is the current implementation documentation.
- `docs-codex/**` is retained as completed V2 planning/execution history.
- `docs/archive/**` remains older historical material.
- New implementation work must update the current Wiki rather than reviving stale V2 planning text as current truth.
