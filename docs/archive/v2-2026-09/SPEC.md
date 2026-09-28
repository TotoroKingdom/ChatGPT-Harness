# V2 SPEC

## 1. Purpose

This specification defines the required end state for ChatGPT-Harness V2.

The V2 Roadmap is the product milestone:

```text
ChatGPT ⇄ Tunnel ⇄ Runtime MCP ⇄ Tools / Policy / Execution
                                  ↓
                           Activity / Diagnostics
                                  ↓
                               Tauri
                                  ↓
                                 UI
```

V2 is complete only when the real external path works end to end in both development and installed builds.

This document defines **what must be true**. Implementation sequencing belongs in `PLAN.md`.

## 2. Source of Truth and Current Baseline

Implementation decisions must be derived from the current production code.

`docs/archive/**` is historical material and must not be used as implementation authority.

`docs-codex/**` is forward-looking development material and must not be treated as evidence of existing behavior.

Current source baseline:

- Runtime lifecycle is already owned by Tauri through `RuntimeSupervisor`.
- Runtime starts an MCP Streamable HTTP endpoint on loopback during initialization.
- The MCP endpoint URL currently remains private inside `McpRuntimeService`.
- `read_file` and `glob` already execute through the Runtime MCP path and shared execution pipeline.
- `WorkspaceGuard`, policy evaluation, execution, audit/activity, and Runtime diagnostics already exist.
- `TunnelProcessManager` already owns process start/stop/restart/crash detection, but is internal and not wired into the product lifecycle.
- Tunnel production spawn currently launches only the executable and does not pass Tunnel ID, Runtime MCP endpoint, or credential material.
- `SecretStore` can save/check/delete the API key but currently cannot read it back for Tunnel launch.
- The real frontend client currently projects Tunnel as `stopped`; MCP status is derived from Runtime status.
- Installed distribution currently stages Runtime but not the Tunnel payload.

V2 must close those gaps without redesigning already-working Runtime execution/security behavior.

## 3. Scope

V2 includes:

1. Runtime MCP endpoint handoff to Tauri.
2. Secure retrieval and launch-time use of the stored Tunnel API key.
3. Real Tunnel launch configuration using Tunnel ID + Runtime MCP endpoint.
4. Real Tunnel readiness/connection determination distinct from process existence.
5. A unified Harness lifecycle owned by Tauri.
6. Real Runtime / MCP / Tunnel / Harness state in the UI.
7. Failure and crash convergence without false Ready states or orphaned managed processes.
8. Development and installed distribution support for the Tunnel payload.
9. Real external validation from ChatGPT through Tunnel to `read_file` / `glob`.
10. UI Activity evidence for the real tool execution.

## 4. Non-Goals

V2 does not add:

- Bash or shell execution.
- Database tools.
- File write/delete/move operations.
- Git write operations.
- New approval workflows.
- New policy language.
- Multi-Runtime support.
- Multiple simultaneous managed tunnels.
- Background auto-restart loops.
- Remote administration of Tunnel IDs.
- Long-lived plaintext secret files.
- A second MCP server outside Runtime.

Existing unrelated placeholders may remain placeholders unless they block the V2 product path.

## 5. Target Ownership Model

### Runtime

Runtime owns:

- Workspace initialization.
- WorkspaceGuard.
- Tool registry and effective tools.
- Policy evaluation.
- ExecutionPipeline.
- `read_file` and `glob`.
- Audit / Activity projection.
- Runtime diagnostics.
- The local MCP Streamable HTTP endpoint.

Runtime does **not** own:

- Tunnel lifecycle.
- Tunnel ID.
- Tunnel API key.
- Tunnel process credentials.
- Installed Tunnel discovery.

### Tauri

Tauri is the local control plane and owns:

- AppConfig.
- OS-backed SecretStore.
- Runtime process lifecycle.
- Tunnel process lifecycle.
- Runtime MCP endpoint handoff.
- Tunnel launch configuration.
- Harness start/stop/restart sequencing.
- Harness start eligibility.
- Tunnel readiness probing.
- Aggregated Runtime / MCP / Tunnel / Harness state.
- Installed distribution discovery for Runtime and Tunnel.

Harness ownership is session-scoped. Only a Runtime generation started by the
active Harness start operation belongs to that Harness session. Tauri must not
adopt an already-running Runtime, bind a Tunnel to it, or count it toward
Harness Ready. The existing `runtime_start` operation may remain available as
an internal/development capability, but the product lifecycle is owned by the
Harness operations.

### Tunnel

The managed `tunnel-client` process owns:

- Authentication to the OpenAI tunnel control plane.
- Tunnel connection.
- Forwarding ChatGPT MCP traffic to Runtime MCP.
- Its own managed Cloudflare companion behavior.

Tauri manages the Tunnel process but must not reinterpret “PID exists” as “Tunnel connected”.

### UI

UI owns:

- Configuration input.
- Start / Stop / Restart user actions.
- Display of real component state.
- Display of Runtime diagnostics and execution Activity.

UI must not receive or retain the raw API key outside the transient Settings
input submission described in the Secret Contract. UI must never receive the
Runtime MCP endpoint URL.

## 6. Runtime MCP Endpoint Contract

V2 must add an explicit internal Runtime → Tauri contract for the active MCP endpoint.

### Required behavior

- Runtime continues to bind MCP only to loopback.
- Runtime initialization is not successful unless the MCP endpoint has successfully bound.
- Tauri can retrieve the active endpoint URL after Runtime initialization.
- The returned endpoint identifies the current Runtime instance only.
- The endpoint must be cleared when the Runtime stops or crashes.
- Tauri must not persist the endpoint to `config.json`.
- Tauri must not expose the endpoint to normal React application state or UI.

### Contract shape

Add an additive control RPC:

```text
mcp.endpoint.get
```

Result:

```json
{
  "url": "http://127.0.0.1:<ephemeral-port>/mcp"
}
```

The result is non-secret but instance-scoped.

`mcp.endpoint.get` must fail closed when Runtime MCP is not active.

The existing `InitializeResult` and `RuntimeState` do not need to expose the endpoint publicly.

## 7. Secret Contract

The existing API key remains persisted only through the OS-backed SecretStore reference in `AppConfig`.

### Required change

`SecretStore` must support retrieving the stored value for controlled launch-time use.

Conceptual interface:

```text
set(key, value)
get(key) -> secret
exists(key)
delete(key)
```

### Security requirements

The Settings API key input is the only permitted short-lived UI holding point
for the raw value. The intended submission path is:

```text
Settings input
    ↓
Tauri save operation
    ↓
SecretStore
    ↓
OS Credential Storage
```

After submission, the raw value must not be retained in a React Provider,
global/application state, or any persisted UI state. The input must be cleared
after the save operation completes or fails.

The raw Tunnel API key:

- must not be written to `config.json`;
- must not be placed in `AppConfig`;
- must not enter `RuntimeConfigSnapshot`;
- must not be sent to Runtime;
- must not enter React Provider state, global state, or normal application state;
- must not be returned by a Tauri command;
- must not be emitted in Activity, diagnostics, error messages, stdout/stderr capture, or normal logs;
- must not be passed as a literal command-line argument.

For V2, Tauri may materialize the secret only long enough to launch the managed Tunnel process through a secret-reference mechanism supported by the bundled `tunnel-client`.

The preferred launch contract is a **child-scoped environment value referenced by name from the Tunnel CLI**, so the raw key is not present in argv or persistent configuration.

The environment variable name and its lifetime must be launch-scoped and must
not be copied into product status, errors, diagnostics, or serialized launch
objects. The controlled credential value exists only for the managed child
launch and is not part of the Harness state payload.

If the repository-bundled Tunnel client version cannot satisfy the required Tunnel ID, MCP server URL, secret-reference, and health/readiness contract, the bundled client must be upgraded as part of V2 rather than weakening the above requirements.

## 8. Tunnel Launch Contract

The managed Tunnel launch requires these inputs:

```text
Tunnel executable
Tunnel ID
Runtime MCP endpoint URL
Tunnel API key
Loopback health/readiness configuration
```

Normal V2 launch semantics must configure the Tunnel client equivalent of:

```text
run
control-plane tunnel id
control-plane API key via secret reference
main MCP server URL = Runtime MCP endpoint
health listener = loopback ephemeral port
health URL discovery file
```

Exact flags must be verified against the actual bundled binary before implementation.

The repository-verified `tunnel-client` 0.0.13 supports the required semantic
inputs: Tunnel ID, API key references using an environment variable or file
reference, MCP server URL, loopback health binding, health URL-file discovery,
and `/readyz`. The implementation must use the documented client contract and
must not invent or infer unsupported flags or wire semantics.

### Launch rules

- Only one managed Tunnel instance may exist.
- Tunnel ID must be non-empty and valid for the client contract.
- Runtime MCP endpoint must be available before Tunnel launch.
- API key must exist in SecretStore before Tunnel launch.
- Health listener must bind to loopback.
- Health discovery artifacts must contain no secret.
- Temporary health-discovery artifacts must be removed on stop, startup failure, and normal cleanup.
- Tunnel stdout/stderr must not be surfaced raw into user-visible or audit channels.
- Managed shutdown must not leave owned `tunnel-client.exe` or `cloudflared.exe` descendants running.

### Process ownership and shutdown

On Windows, Tauri must create one process-ownership Job Object per managed
Tunnel session and assign `tunnel-client.exe` to it before the child can create
owned descendants. The Job Object must use kill-on-close semantics and remain
associated with the Tunnel instance/generation, never with an unrelated
process name.

Normal stop first requests graceful termination of `tunnel-client.exe` and
waits for the owned process tree. If the bounded timeout expires, Tauri must
terminate the Job Object, wait for all owned descendants to exit, and verify
that the managed `tunnel-client.exe` and `cloudflared.exe` descendants are no
longer running. Direct-child termination alone is insufficient.

## 9. Tunnel State Semantics

V2 must distinguish process lifecycle from connectivity.

The following statements are not equivalent:

```text
Tunnel process exists
Tunnel process is running
Tunnel is ready
Tunnel can forward MCP traffic
```

### Internal process state

The existing process lifecycle may remain conceptually:

```text
Stopped
Starting
Running
Stopping
Failed
```

### Product Tunnel state

The product-facing Tunnel component state is:

```text
stopped
starting
ready
degraded
error
stopping
```

Rules:

- `stopped`: no managed Tunnel.
- `starting`: launch accepted, process may exist, readiness not yet proven.
- `ready`: the managed Tunnel is alive, the control plane connection is
  established, the configured MCP target is reachable, and the authoritative
  tunnel-client readiness check is healthy.
- `degraded`: process is alive but readiness was previously established and is temporarily unhealthy.
- `error`: startup failed, process exited unexpectedly, readiness timed out, or continued readiness is lost beyond the bounded tolerance.
- `stopping`: managed shutdown is in progress.

A live PID alone can never produce `ready`.

The authoritative check must use the documented health contract of the
bundled tunnel-client, including its loopback `/readyz`/health probe and the
control-plane-poll requirement where supported. A listening health port or a
successful process spawn alone does not prove configured MCP target reachability
and must not produce Tunnel Ready.

## 10. MCP State Semantics

The UI `MCP` component represents the Runtime-local MCP service, not the Tunnel process.

MCP becomes `ready` when:

- Runtime initialization succeeded; and
- Tauri successfully obtained the active loopback MCP endpoint for the current Runtime instance.

MCP becomes non-ready when Runtime stops, crashes, changes instance, or the endpoint contract is unavailable.

No UI path may infer external Tunnel readiness merely because MCP is ready.

## 11. Harness State

Introduce a single authoritative Harness state in Tauri.

Conceptual shape:

```text
HarnessState
  lifecycle
  runtime
  mcp
  tunnel
  activeSessionId
  configRevision
  lastError
```

HarnessController is the single lifecycle state owner. Lifecycle commands are
serialized. Runtime and Tunnel watcher callbacks update state only through the
controller and must carry the active Harness session identity plus the Runtime
generation or Tunnel instance generation. Stale callbacks must be ignored.

Product lifecycle:

```text
stopped
starting
ready
degraded
error
stopping
```

### Ready definition

`Harness Ready` is true only when all of the following belong to the same active Harness session:

```text
Runtime = ready
MCP = ready
Tunnel = ready
```

No partial condition may be promoted to Ready.

### Degraded / Error

- Recoverable temporary readiness loss may be `degraded`.
- Startup failure, managed process crash, unrecoverable health loss, or orchestration failure becomes `error`.
- A component failure must immediately remove Harness from `ready`.

## 12. Harness Start Eligibility

V2 Start Harness is not “start local Runtime”.

Start eligibility must validate the complete product path.

Required checks:

- valid saved config;
- valid Workspace;
- consistent security mode;
- Runtime distribution is resolvable;
- Tunnel ID is configured;
- API key reference exists and the secret is retrievable;
- a usable Tunnel distribution/executable is resolvable;
- no Runtime already running outside the active Harness session may be adopted;
- no conflicting Runtime/Tunnel/Harness lifecycle transition is active.

If any required V2 connection input is absent, Start Harness is disabled and the UI must explain the specific remediation.

The installed product must not require the user to manually select the Tunnel executable directory.

Existing explicit Tunnel path configuration may remain as a development/override mechanism, but normal installed operation uses the bundled Tunnel distribution.

## 13. Harness Lifecycle Commands

Tauri must expose product-level lifecycle operations:

```text
harness_start_eligibility
harness_start
harness_status
harness_stop
harness_restart
```

Frontend product lifecycle must use these operations instead of directly composing Runtime lifecycle operations.

Lifecycle operations are serialized by the HarnessController. The frontend
must not reconstruct lifecycle truth from Runtime or Tunnel process status.

Runtime-specific read/diagnostic/tool/policy commands may continue to use `RuntimeSupervisor`.

## 14. Start Sequence

The authoritative sequence is:

```text
validate Harness eligibility
        ↓
mark Harness starting
        ↓
start Runtime
        ↓
Runtime Ready
        ↓
get Runtime MCP endpoint
        ↓
MCP Ready
        ↓
retrieve API key from SecretStore
        ↓
start Tunnel against Runtime MCP
        ↓
wait for Tunnel readiness
        ↓
Tunnel Ready
        ↓
Harness Ready
```

### Start rollback

If any stage after Runtime start fails:

1. Harness must never enter Ready.
2. Any partially started Tunnel must be stopped/cleaned.
3. Runtime must be stopped if it was started for this failed Harness start.
4. Temporary health artifacts must be removed.
5. Final state must be deterministic and user-visible.
6. Returned errors must be structured and must not contain secrets.

The final Harness lifecycle state for a failed start is `error`, with all
components started by that session cleaned up and a safe structured
`lastError` retained. A failed start must never be reported as `stopped` while
silently discarding the failure.

## 15. Stop Sequence

The authoritative sequence is:

```text
mark Harness stopping
        ↓
stop Tunnel
        ↓
confirm managed Tunnel/descendants exit
        ↓
stop Runtime
        ↓
confirm Runtime exit
        ↓
clear endpoint/session state
        ↓
Harness stopped
```

Stop must make a best effort to clean both owned components even if one stop operation reports an error.

Repeated stop must be safe and converge to `stopped`.

Only a user-requested or completed Harness stop converges to `stopped`. A
startup failure remains `error` until an explicit successful start or an
explicit stop clears the session error according to the controller contract.

## 16. Restart Sequence

Restart means:

```text
complete Harness stop
        ↓
reload latest saved config/secret references
        ↓
run full Harness start
```

A changed configuration is not active until restart completes successfully.

## 17. Crash and Failure Convergence

Tauri must react to asynchronous Runtime and Tunnel exits.

Watcher events must be delivered through HarnessController and validated
against the active session/generation before mutating aggregated state. The UI
must not be responsible for detecting crashes or repairing lifecycle state.

### Runtime crash

When the active Runtime crashes:

- Runtime -> error.
- MCP -> error/stopped.
- Harness leaves Ready immediately.
- The managed Tunnel for that Runtime instance must be stopped.
- Stale Runtime endpoint state must be cleared.

### Tunnel crash

When the active Tunnel crashes:

- Tunnel -> error.
- Harness leaves Ready immediately.
- Runtime may remain alive for diagnostics/recovery, but Harness is not Ready.
- A later explicit Restart Harness must rebuild the full chain.

### No automatic infinite restart

V2 does not silently loop/restart failed processes indefinitely.

## 18. UI Requirements

### Overview

Overview must display authoritative:

```text
Harness
Runtime
MCP
Tunnel
```

state.

The existing behavior that permanently hardcodes real Tunnel state as `stopped` must be removed.

The existing behavior that equates MCP state with Runtime state must be replaced by the explicit MCP endpoint/session state.

### Lifecycle actions

- Start Harness uses Harness eligibility and `harness_start`.
- Stop Harness uses `harness_stop`.
- Restart Harness uses `harness_restart`.
- Buttons are disabled during incompatible transitions.

React consumes the authoritative Harness state/event surface. It must not
derive Harness, MCP, or Tunnel readiness from Runtime state or process
existence.

### Settings

V2 configuration must support:

- Workspace.
- Tunnel ID.
- API key.
- optional development/override Tunnel client path if retained.

In an installed build, the user must not need to browse for `tunnel-client.exe`.

Tunnel ID and API key are required for V2 Start Harness.

The API key field may hold the raw value only transiently while the Settings
submission is in progress. After submission, the value must exist only in the
OS-backed SecretStore path and the short-lived controlled child launch
environment; it must not be copied into Provider/global state or returned to
the frontend.

### Activity

A successful real `read_file` / `glob` request coming from ChatGPT must produce the existing execution Activity evidence visible in UI.

V2 does not require replacing the existing Activity storage model if the real tool execution is visible and attributable.

## 19. Distribution Requirements

The installed package must contain everything needed for the V2 product chain.

Required installed payload:

```text
ChatGPT-Harness UI
Runtime
tunnel-client
cloudflared companion
required Tunnel client manifest/runtime artifacts
required notices/licenses
distribution manifest
```

Distribution staging and verification must include Tunnel payload validation.

The staged Tunnel payload must include the bundled `tunnel-client`, its
`cloudflared` companion, the client/package metadata, and the applicable
license, notice, and SBOM artifacts. The distribution manifest must describe
the supported Tunnel version and both executable relative paths.

Installed Tunnel resolution must be deterministic and fail closed in the same spirit as installed Runtime resolution.

The distribution manifest must be extended to describe the managed Tunnel payload and supported version/path.

The installed product must not depend on:

- repository checkout paths;
- developer PATH entries;
- the source-tree Tunnel package;
- a manually selected executable directory.

## 20. Development Requirements

Development mode must remain usable.

A development build must resolve a known Tunnel client through one of the approved development paths:

1. explicitly configured override; or
2. repository-bundled development Tunnel package.

Development behavior must not weaken production secret handling or readiness semantics.

## 21. Validation Requirements

V2 validation has three levels.

### Automated component/integration validation

Must cover:

- Runtime MCP endpoint RPC contract.
- SecretStore read behavior without secret serialization.
- Tunnel launch argument/reference construction.
- Tunnel process lifecycle.
- Tunnel readiness timeout/success/failure.
- Tunnel Ready evidence for control-plane connection, configured MCP target
  reachability, and the authoritative health check.
- API key transient-input and no-leak boundaries.
- Windows managed process-tree ownership and descendant cleanup.
- Harness start success.
- Harness start rollback.
- Harness stop ordering.
- Rejection of an already-running Runtime that is not owned by the active
  Harness session.
- Serialized lifecycle commands and stale session/generation callbacks.
- Runtime crash convergence.
- Tunnel crash convergence.
- UI state mapping.
- distribution manifest/path validation.
- staging/bundle verification.

Tunnel tests should use controlled fixtures/fake processes where real control-plane credentials are not appropriate.

### Build validation

Required commands must succeed:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm test
npm run build
npm run distribution:stage
```

Windows bundle validation must also succeed for the final release path:

```text
npm run bundle:windows
```

### Real external acceptance

With a valid Tunnel ID and Runtime API key:

```text
Configure Workspace / Tunnel ID / API Key
        ↓
Start Harness
        ↓
Runtime = ready
MCP = ready
Tunnel = ready
Harness = ready
        ↓
Connect/select the Tunnel in ChatGPT
        ↓
ChatGPT discovers tools
        ↓
ChatGPT calls read_file
        ↓
ChatGPT calls glob
        ↓
WorkspaceGuard + Policy + Execution run
        ↓
Activity appears in UI
        ↓
Stop Harness
        ↓
Tunnel and Runtime exit
```

The same flow must be verified from an installed build.

## 22. V2 Definition of Done

V2 is complete only when:

- ChatGPT can reach the real Runtime MCP through the managed Tunnel.
- `read_file` and `glob` work through that external path.
- WorkspaceGuard / Policy / Execution remain in the real call path.
- execution Activity is visible in UI.
- Runtime, MCP, Tunnel, and Harness state are real rather than inferred placeholders.
- Harness lifecycle is a single Tauri-owned product lifecycle.
- startup failure cannot leave a false Ready state.
- managed stop does not leave owned Tunnel/Cloudflared/Runtime processes behind.
- the raw API key remains outside config, Runtime, protocol responses, React
  Provider/global state, Activity, diagnostics, errors, logs, and literal argv.
- Tunnel Ready proves control-plane connection, configured MCP target
  reachability, and authoritative tunnel-client readiness.
- installed distribution contains and resolves both Runtime and Tunnel dependencies.
- a clean installed build can complete the same external ChatGPT flow.
- Bash, database tools, write tools, and Git write operations remain out of scope until this chain is complete.
