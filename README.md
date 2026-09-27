# ChatGPT-Harness

> **Status:** V2 accepted on 2026-09-27. The first real ChatGPT → Tunnel → Runtime MCP product chain is complete and accepted.

ChatGPT-Harness is a Windows desktop MCP Harness that lets ChatGPT access a selected local workspace through a controlled, observable Runtime instead of exposing the local machine directly.

The project uses Rust for the Runtime and Tauri host, with a React UI for configuration, lifecycle control, and observability.

## Product path

```text
ChatGPT
   ↕
Tunnel
   ↕
Runtime MCP
   ↓
ExecutionPipeline
   ↓
WorkspaceGuard
   ↓
PolicyEngine
   ↓
read_file / glob
   ↓
Audit / Activity
```

The desktop application owns the local control plane:

```text
React UI
   ↓ Tauri IPC
Tauri HarnessController
   ├── RuntimeSupervisor
   ├── TunnelProcessManager
   ├── ConfigStore
   └── SecretStore
```

Tauri orchestrates processes and state. It is not in the per-tool MCP data path.

## Current capabilities

- Product-level `Start / Stop / Restart` through `HarnessController`.
- Runtime MCP endpoint handoff from the active Runtime instance to Tauri.
- Managed Tunnel launch and authoritative readiness checks.
- Windows process-tree ownership and cleanup for Tunnel descendants.
- Windows Credential Manager backed API-key storage.
- Read-only MCP tools: `read_file` and `glob`.
- Workspace escape protection through `WorkspaceGuard`.
- Fail-closed policy evaluation through `PolicyEngine`.
- Execution audit/activity and Runtime diagnostics.
- NSIS and portable Windows distribution paths.
- Real external ChatGPT E2E accepted for V2.

## Security model

The main boundaries are deliberate:

```text
UI displays state; Runtime owns execution truth.
Tauri owns local process lifecycle; Runtime owns tool execution.
Runtime MCP binds loopback; Tunnel owns external connectivity.
API keys stay in SecretStore and controlled child-process injection.
WorkspaceGuard and PolicyEngine remain inside Runtime.
```

Higher-risk capabilities such as file writes, shell execution, database access, and Git write operations are intentionally outside the V2 scope.

## Documentation

Current implementation documentation lives under [`wiki/`](./wiki/):

- [`wiki/README.md`](./wiki/README.md) — documentation entry point and source-of-truth rules.
- [`wiki/ARCHITECTURE.md`](./wiki/ARCHITECTURE.md) — current process, control-plane, and MCP data-plane architecture.
- [`wiki/MCP-AND-TUNNEL.md`](./wiki/MCP-AND-TUNNEL.md) — current Runtime MCP ↔ Tunnel integration contract.
- [`wiki/CODEBASE-MAP.md`](./wiki/CODEBASE-MAP.md) — repository ownership and important source locations.
- [`wiki/IMPLEMENTATION-MATRIX.md`](./wiki/IMPLEMENTATION-MATRIX.md) — capability status and verification evidence.
- [`wiki/ACCEPTANCE.md`](./wiki/ACCEPTANCE.md) — V2 final acceptance record.

Planning/execution records are retained under [`docs-codex/`](./docs-codex/). They are not current implementation evidence.

Historical pre-code documentation is retained under [`docs/archive/`](./docs/archive/). It must not be used to infer current behavior.

## Current scope after V2

V2 is frozen as the minimal read-only product chain. The next security-sensitive expansion should begin with a complete approval/resume workflow before adding write, Git-write, shell, or database execution capabilities.
