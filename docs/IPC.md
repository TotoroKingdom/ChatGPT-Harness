# UI ↔ Runtime Control Protocol

## Topology and boundary

The React UI calls `HarnessControlClient`, which is implemented by the Tauri backend. The backend is the Config Authority, SecretStore Authority, and Runtime Process Supervisor. It spawns the Harness Runtime and communicates with that process over this Control Protocol.

`ChatGPT → tunnel-client → MCP → Harness Runtime` is a separate public protocol surface. MCP is not carried over, or represented by, the Control Protocol.

## Transport and framing

V1 transport is Runtime `stdin`/`stdout` only. Messages are UTF-8 JSON Lines: every line is one complete JSON message; stdin is client-to-Runtime and stdout is Runtime-to-client. Runtime stdout contains protocol messages only; logs go to stderr. Named pipes, HTTP, WebSocket, and gRPC are not part of M0.

## Envelope and serialization

The envelope follows JSON-RPC 2.0 semantics but deliberately omits `"jsonrpc":"2.0"` on the wire. It supports requests (`id`, `method`, optional `params`), results (`id`, `result`), notifications (`method`, optional `params`), and errors (`id`, `error`). Request IDs are JSON integers or strings.

Rust fields are `snake_case`; JSON fields are `camelCase`; enum values are `snake_case`; timestamps are RFC3339 UTC strings; IDs are opaque strings unless a JSON-RPC request ID; absent optional fields are omitted.

Standard error numbers are `-32700` parse error, `-32600` invalid request, `-32601` method not found, `-32602` invalid params, and `-32603` internal error. Harness errors use `-32000..-32099`. Every error has `error.data.code`, a stable machine-readable value: `NOT_INITIALIZED`, `ALREADY_INITIALIZED`, `PROTOCOL_VERSION_MISMATCH`, `RUNTIME_NOT_READY`, `INVALID_ARGUMENT`, `APPROVAL_NOT_FOUND`, `APPROVAL_ALREADY_RESOLVED`, or `INTERNAL_ERROR`.

## Initialization and lifecycle

`protocolVersion` is **1** and compatibility is strict equality. The sequence is: Tauri spawns Runtime; client sends `initialize`; Runtime returns `initialize` result; client sends `initialized`; Runtime is ready. Before initialization succeeds, Runtime rejects every business request except `initialize`.

`runtime.start` is a `HarnessControlClient`/Tauri facade operation, not a Runtime wire RPC; a stopped Runtime cannot receive requests. `runtime.stop` is a Runtime wire RPC for graceful shutdown. Process kill and cleanup are outside M0.

## Method registry

Runtime request methods are:

- `initialize`
- `runtime.status`, `runtime.stop`
- `tools.list`
- `policies.list`
- `activity.list`, `activity.subscribe`
- `approvals.list`, `approvals.resolve`
- `diagnostics.run`

`initialized` and `activity.event` are notifications. No `files.*`, `git.*`, `database.*`, `shell.*`, `skills.*`, `memory.*`, `agent.*`, or `tasks.*` methods are defined by M0.

`activity.list` accepts optional `afterSequence` and `limit`, and returns `items` with `latestSequence`. `activity.subscribe` accepts optional `afterSequence`; Runtime first replays events after that sequence, then sends subsequent `activity.event` notifications. Closing the connection cancels the subscription; no unsubscribe RPC exists.

## Canonical artifacts

The Rust source of truth is [protocol](../protocol). Generate client artifacts explicitly:

```text
cargo run -p harness-control-protocol --bin protocol-export -- generate-ts
cargo run -p harness-control-protocol --bin protocol-export -- generate-json-schema
cargo run -p harness-control-protocol --bin protocol-export -- generate-all
```

Generated TypeScript is at `protocol/generated/typescript/v1.ts`; JSON Schema is at `protocol/generated/json-schema/`. Files are deterministic and marked `GENERATED FILE — DO NOT EDIT` (the JSON schema marker is its legal JSON `$comment`). Generation is never performed from `build.rs`.
