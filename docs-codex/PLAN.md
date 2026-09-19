# V2 PLAN

## 1. Purpose

This plan implements the complete V2 Roadmap in one development effort.

It is not a new milestone hierarchy and does not split V2 into sub-versions.

The later execution TASK should instruct Codex to implement this plan end to end, in order, until the V2 Definition of Done in `SPEC.md` is satisfied.

## 2. Constraints

During implementation:

- treat current production code as implementation truth;
- do not use `docs/archive/**` as authority;
- use `docs-codex/ROADMAP.md` as the product target;
- use `docs-codex/SPEC.md` as the required behavior contract;
- use this PLAN as implementation order;
- preserve existing Runtime WorkspaceGuard / Policy / Execution behavior unless a change is strictly required for V2 integration;
- do not add Bash/database/write/Git-write capabilities;
- do not replace working subsystems merely for stylistic cleanup;
- do not report V2 complete from mocked UI or process existence alone.

## 3. Implementation Strategy

The implementation should close the product chain from the inside out:

```text
Runtime MCP endpoint
        ↓
Tauri endpoint handoff
        ↓
Secret retrieval
        ↓
real Tunnel launch
        ↓
Tunnel readiness
        ↓
Harness orchestration
        ↓
real UI state
        ↓
distribution
        ↓
real ChatGPT E2E
```

Every earlier layer must be usable by the next layer before moving on.

## 4. Preflight: Verify the Actual Bundled Tunnel Client

Before changing launch code, inspect the actual repository-bundled `tunnel-client.exe`.

Run at minimum:

```text
tunnel-client.exe --version
tunnel-client.exe run --help
```

Confirm support for all required semantic inputs:

- control-plane Tunnel ID;
- Runtime API key through a non-literal secret reference;
- MCP server URL;
- loopback health listener;
- a discoverable health/readiness URL or equivalent;
- `/readyz` or an equivalent authoritative readiness signal.

Do not guess flag names from memory.

If the bundled version does not support the V2 contract:

1. replace it with a supported official release;
2. replace the matching `cloudflared.exe` and package metadata as required;
3. update license/notice/SBOM artifacts consistently;
4. record the exact supported client version in distribution metadata;
5. run the same preflight against the replacement binary.

Do not proceed with a launch implementation that cannot prove readiness.

The repository preflight has confirmed the bundled `tunnel-client` 0.0.13
supports Tunnel ID, non-literal API key references, MCP server URL, loopback
health binding, health URL-file discovery, and `/readyz`. Record the exact
documented invocation and its machine-readable readiness semantics in the
implementation tests. Do not infer flags or treat a successful process spawn,
health listener, or PID as proof of Tunnel Ready.

## 5. Add Runtime MCP Endpoint Handoff

### Protocol

Add an additive typed control method:

```text
mcp.endpoint.get
```

Add a typed result containing only:

```text
url
```

Update:

- protocol method registry;
- Rust models;
- schema/generated TypeScript artifacts where applicable;
- contract tests/fixtures required by the repository generation model.

Do not put the endpoint into AppConfig.

### Runtime

In `runtime/src/server.rs`:

- handle `mcp.endpoint.get`;
- read the URL from the active `McpRuntimeService`;
- return it only while Runtime is initialized/ready and MCP is active;
- fail closed if MCP is unavailable.

Keep MCP loopback-only.

### Tauri RuntimeSupervisor

After successful Runtime initialize + `initialized` acknowledgement:

- request `mcp.endpoint.get`;
- validate that the URL is loopback HTTP and identifies `/mcp`;
- associate it with the active Runtime process generation/identity;
- clear it on Runtime stop/crash/replacement.

Expose it only through an internal Rust method needed by Harness orchestration.

Do not add a normal frontend command that returns the endpoint.

### Tests

Add coverage proving:

- endpoint retrieval succeeds for a running Runtime;
- URL belongs to the active Runtime generation;
- endpoint is unavailable after stop;
- malformed/non-loopback endpoint values are rejected by Tauri.

## 6. Extend SecretStore for Controlled Read

Add a read operation to the SecretStore abstraction.

Expected behavior:

```text
get(key) -> value or not-found/error
```

The Settings input is the only short-lived UI exception for the raw API key.
The allowed path is `Settings → Tauri → SecretStore → OS Credential Storage`.
After submission, the raw value must not remain in React Provider state, global
state, AppConfig, RuntimeConfigSnapshot, Runtime, any IPC response, Activity,
logs, or errors. Tauri may materialize it only for the controlled child launch
environment and must clear the input after the save operation completes.

Implement for:

- Windows Credential Store;
- MemorySecretStore tests.

Keep existing `set/exists/delete`.

Add tests proving:

- value can be retrieved after set;
- replacement returns latest value;
- delete removes it;
- serialized SecretReference still contains no secret;
- no command/API response returns the raw value.

Do not expose a `secret_get` Tauri command to React.

## 7. Build Deterministic Tunnel Distribution Resolution

Create a Tunnel resolver parallel in intent to Runtime distribution resolution.

### Installed mode

Resolve the packaged Tunnel from the installed application layout.

Validate:

- expected directory;
- expected `tunnel-client.exe`;
- required sibling `cloudflared.exe`;
- required package metadata if used by the client;
- canonical location remains inside the installed Tunnel directory;
- no unexpected symlink/reparse escape;
- distribution manifest matches the expected Tunnel relative path/version.

### Development mode

Resolve in deterministic order:

1. explicit configured override, when present;
2. repository-bundled Tunnel package.

Use existing auto-detect behavior only where it is intentionally retained for development UX.

Normal installed V2 startup must not depend on PATH or a manually chosen folder.

Add resolver tests for valid/missing/escaped/wrong-layout cases.

## 8. Upgrade TunnelProcessManager from Lifecycle-Only to Real Launch

Refactor `TunnelLaunch` so it contains all non-secret launch data plus a controlled secret input that is never copied into status objects.

Conceptually separate:

```text
TunnelLaunchPublic
  executable
  tunnelId
  runtimeMcpUrl
  health discovery path

TunnelCredential
  apiKey
```

Do not place API key in `TunnelInstance`, `TunnelStatus`, errors, or debug serialization.

### Command construction

Construct the verified Tunnel CLI invocation using:

- `run`;
- Tunnel ID;
- Runtime MCP URL as the `main` MCP binding;
- API key through a supported secret reference;
- loopback ephemeral health listener;
- health URL discovery mechanism.

Pass the raw API key only through the chosen child-scoped secret carrier.

Never put the raw API key in argv.

### Output handling

Do not pipe unrestricted raw Tunnel stdout/stderr into Activity or user logs.

If output is needed for diagnostics/tests:

- capture only bounded data;
- redact secret-bearing values;
- convert to classified errors rather than storing arbitrary raw output.

### Process tree

Preserve existing single-owner semantics.

On Windows, create a per-session Job Object with kill-on-close semantics and
assign `tunnel-client.exe` to it before the child can create owned
descendants. Keep the Job Object handle tied to the Tunnel instance/generation
and outside serialized status objects.

Normal stop requests graceful termination of `tunnel-client.exe`, waits for
the owned process tree, and escalates to Job Object termination after the
bounded timeout. Verify that `tunnel-client.exe`, `cloudflared.exe`, and all
other descendants owned by that Job Object have exited before reporting
cleanup complete.

Managed stop must not leave owned Tunnel/Cloudflared descendants running.

Do not kill unrelated processes by name.

### Tests

Retain existing lifecycle tests and add tests for:

- command arguments contain Tunnel ID and MCP URL;
- command arguments do not contain the raw API key;
- child secret carrier receives the key;
- duplicate start is rejected;
- stop is instance-safe;
- crash state remains observable;
- owned descendant cleanup behavior is verified with a Windows process fixture
  and Job Object ownership assertions.

## 9. Add Tunnel Readiness Discovery and Monitoring

Extend Tunnel management with a readiness layer separate from process state.

### Startup readiness

After spawn:

1. wait for health URL discovery with a bounded timeout;
2. require loopback health URL;
3. use the documented tunnel-client health contract, including the
   control-plane-poll requirement where supported;
4. require evidence that the configured MCP target is reachable;
5. only then mark Tunnel product state `ready`.

A spawned process that never becomes ready is a startup failure.

Tunnel Ready is not equivalent to process existence, a listening health port,
or a successful control-plane process start. The exact 0.0.13 command/endpoint
semantics must be captured in tests rather than guessed by the implementation.

### Runtime monitoring

While a managed Tunnel session exists:

- detect process exit immediately;
- perform bounded readiness monitoring;
- update product state when readiness is lost;
- do not run an infinite restart loop.

Use conservative timing constants and make them testable.

### Cleanup

Delete health URL files/temp artifacts on:

- failed startup;
- stop;
- restart;
- crash cleanup where possible.

Add fixture-driven tests for:

- ready success;
- timeout;
- malformed health URL;
- non-loopback health URL;
- readiness loss;
- process exit before readiness.

## 10. Introduce the Tauri Harness Orchestrator

Create a single product-level orchestration owner.

Recommended structure:

```text
HarnessController
  shared RuntimeSupervisor
  shared TunnelProcessManager
  Harness session/state
```

Reuse the existing supervisors rather than duplicating process ownership.

If necessary, make the supervisors clonable wrappers over their existing shared `Arc<Mutex<...>>` state so Runtime read commands can continue using the same Runtime instance.

### Harness state

Maintain authoritative:

```text
lifecycle
runtime component state
mcp component state
tunnel component state
active config revision
active session identity
last structured error
```

A Harness session identity should prevent stale callbacks from an earlier Runtime/Tunnel generation mutating a newer session.

Lifecycle commands must be serialized by the HarnessController. The controller
is the only lifecycle state writer. Runtime and Tunnel watcher events must be
delivered through the controller with the active session identity and the
component generation/instance identity; stale events are ignored.

### Lifecycle observation

Wire existing Runtime waiter and Tunnel watcher events into the Harness controller/shared lifecycle state.

Do not rely solely on React polling for crash correctness.

Runtime/Tunnel asynchronous exits must update Harness state even when the UI is idle.

## 11. Implement Harness Eligibility

Replace product-level reliance on `runtime_start_eligibility`.

Implement:

```text
harness_start_eligibility
```

Validate:

- config load/schema;
- Workspace;
- security consistency;
- Runtime distribution;
- Tunnel distribution;
- Tunnel ID;
- API key reference;
- API key retrieval;
- reject an already-running Runtime that is not owned by the active Harness session;
- lifecycle conflicts.

Return structured statuses/messages that the current Overview/first-run UI can render.

Runtime-specific eligibility may remain internal or be retained for tests, but Start Harness must use complete Harness eligibility.

## 12. Implement Harness Start

Add:

```text
harness_start
```

Execute the SPEC sequence exactly:

1. re-check eligibility;
2. allocate a new Harness session;
3. mark Harness starting;
4. start Runtime;
5. confirm Runtime Ready;
6. obtain/validate current Runtime MCP endpoint;
7. mark MCP Ready;
8. retrieve API key;
9. resolve Tunnel executable;
10. start Tunnel against this Runtime endpoint;
11. wait for authoritative Tunnel readiness;
12. mark Tunnel Ready;
13. mark Harness Ready.

### Rollback

Any failure after Runtime start must run rollback before returning.

Rollback must:

- stop partially started Tunnel;
- stop Runtime started by this session;
- clear endpoint/session state;
- remove temp health artifacts;
- return a structured non-secret error;
- leave Harness in `error` with Runtime/Tunnel cleanup complete and a safe
  structured `lastError`.

Write integration tests for failure injection at every major boundary.

## 13. Implement Harness Stop and Restart

Add:

```text
harness_stop
harness_restart
harness_status
```

### Stop

Perform Tunnel stop before Runtime stop.

Always make a best effort to stop both.

Clear active endpoint/session state only after shutdown has converged.

Make repeated stop safe. A user-requested stop converges Harness to
`stopped`; a failed start remains `error` until an explicit successful start
or explicit stop clears the failed session.

### Restart

Do not implement restart as a frontend-only composition.

Tauri should own:

```text
stop complete
→ reload latest config/secret reference
→ full start
```

Write ordering and failure tests.

## 14. Implement Crash Convergence

### Runtime exit callback

When the active Runtime generation exits unexpectedly:

- update Runtime/MCP state;
- invalidate Harness Ready;
- stop the Tunnel belonging to that Harness session;
- clear the active endpoint.

### Tunnel exit callback

When the active Tunnel instance exits unexpectedly:

- update Tunnel state;
- invalidate Harness Ready;
- keep Runtime available for diagnostics unless explicit shutdown policy requires otherwise;
- require explicit Restart Harness for full recovery.

Protect both callbacks with session/generation identity so stale watcher events cannot affect a newer session.

## 15. Expose Harness State to the Frontend

Add a Tauri state/event surface for real Harness state.

Prefer a single authoritative state payload/event rather than reconstructing truth independently in React.

Expose enough data for:

```text
Harness
Runtime
MCP
Tunnel
```

status plus safe structured last error.

Do not expose:

- API key;
- child environment;
- Runtime MCP endpoint URL;
- raw Tunnel stdout/stderr.

Update `HarnessControlClient` to use Harness lifecycle methods.

Remove the real-client hardcoded Tunnel `stopped` behavior.

Remove MCP = Runtime status projection.

Keep MockClient behavior useful for isolated UI tests, but ensure real adapter tests assert actual invoke mappings.

## 16. Update HarnessDataProvider and UI

### Provider

Change:

```text
startHarness
stopHarness
restartHarness
```

to call product-level Harness commands.

Rehydrate from `harness_status`, not by reconstructing the complete product state from Runtime process status.

Lifecycle state must come from the Harness status/event surface. React must not
infer Harness, MCP, or Tunnel readiness from Runtime status or process
existence.

Continue loading Runtime tools/policies/activity/diagnostics through existing Runtime read APIs after Runtime is available.

### Overview

Show authoritative Harness/Runtime/MCP/Tunnel state.

Do not show Ready when only processes are alive.

### Settings / first-run

Change the product wording from “external connection optional” to the V2 requirement:

- Workspace required.
- Tunnel ID required.
- API key required.
- installed Tunnel binary is bundled/resolved automatically.

Retain explicit Tunnel path control only if it is useful as a development/override setting.

A normal installed first run should not ask the user to locate `tunnel-client.exe`.

### Restart required

Preserve config revision behavior.

When config changes during an active Harness session, Restart Harness applies the new complete Runtime + Tunnel configuration.

## 17. Extend Distribution Manifest and Staging

Update the distribution model to include Tunnel payload metadata.

Example conceptual manifest:

```json
{
  "distribution_schema_version": 2,
  "distribution_id": "chatgpt-harness",
  "app_version": "...",
  "runtime_version": "...",
  "protocol_version": 1,
  "runtime": {
    "relative_path": "runtime/chatgpt-harness-runtime.exe"
  },
  "tunnel": {
    "version": "...",
    "relative_path": "tunnel/tunnel-client.exe",
    "cloudflared_relative_path": "tunnel/cloudflared.exe"
  }
}
```

Exact schema may differ, but it must be explicit and validated.

The Tunnel manifest section must identify the supported `tunnel-client` version,
the Tunnel client relative path, and the `cloudflared` companion relative
path. Staging must preserve the client metadata plus the applicable
license/notice/SBOM artifacts needed by the installed product.

### `stage-distribution.ps1`

Stage:

- UI executable;
- Runtime executable;
- Tunnel client executable;
- Cloudflared companion;
- required Tunnel package metadata;
- notices/licenses;
- applicable Tunnel SBOM/provenance artifacts;
- updated distribution manifest.

### `tauri.conf.json`

Bundle the staged Tunnel resources.

### `verify-nsis-bundle.ps1`

Verify the installed bundle contains the complete expected Tunnel payload and distribution manifest.

Update manifest unit tests and staging verification tests/scripts accordingly.

## 18. Preserve Existing Runtime Execution Path

Do not bypass or duplicate Runtime execution.

Real external tool calls must continue through:

```text
Runtime MCP
→ effective tools
→ MCP adapter
→ ExecutionPipeline
→ WorkspaceGuard
→ Policy
→ executor
→ Audit / Activity
```

Do not add a direct Tauri tool execution shortcut.

Do not move `read_file` or `glob` into the Tunnel/Tauri layer.

## 19. Automated Verification

Before real external acceptance, the full automated suite must pass.

Run:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm test
npm run build
npm run distribution:stage
```

Then run Windows bundle verification:

```text
npm run bundle:windows
```

If new tests reveal existing unrelated failures, document them explicitly; do not silently disable tests.

The V2-specific test gate must additionally cover transient API key handling,
documented Tunnel readiness semantics, Windows Job Object descendant cleanup,
rejection of Runtime ownership conflicts, serialized lifecycle commands, and
stale session/generation callback suppression.

## 20. Real Development E2E

With real credentials and the development build:

1. configure a real Workspace;
2. configure valid Tunnel ID;
3. save Runtime API key;
4. Start Harness;
5. confirm UI shows Runtime/MCP/Tunnel/Harness Ready;
6. confirm Tunnel Ready evidence includes control-plane connection, configured
   MCP target reachability, and the authoritative tunnel-client health check;
7. configure/select the Tunnel in ChatGPT;
8. confirm ChatGPT tool discovery;
9. call `read_file`;
10. call `glob`;
11. verify WorkspaceGuard/Policy/Execution behavior;
12. verify corresponding Activity in UI;
13. Stop Harness;
14. confirm Runtime/Tunnel/Cloudflared owned processes exit.

Do not mark E2E passed from local Runtime HTTP tests alone.

## 21. Real Installed E2E

Build the NSIS package and test from an installed layout that does not rely on repository paths.

Repeat the complete Development E2E.

Additionally verify:

- Tunnel executable is resolved from installed resources;
- no manual Tunnel directory selection is needed;
- Runtime is resolved from installed resources;
- config/secret remain machine-local;
- uninstall/reinstall does not accidentally package user config or secrets.

## 22. Documentation Update at Completion

After code and acceptance are complete:

- update `wiki/CODEBASE-MAP.md` from the finished code;
- update `wiki/IMPLEMENTATION-MATRIX.md` from the finished code;
- keep `docs-codex/ROADMAP.md`, `SPEC.md`, and `PLAN.md` as forward-looking/execution records until V2 is accepted;
- do not restore archived historical docs as implementation authority.

## 23. Completion Gate

The execution TASK is complete only when all of these are true:

```text
real Runtime endpoint handoff          PASS
SecretStore controlled read            PASS
real Tunnel launch                     PASS
Tunnel authoritative readiness         PASS
Harness orchestration                  PASS
real UI state                          PASS
Runtime crash convergence              PASS
Tunnel crash convergence               PASS
development E2E                        PASS
installed distribution contains Tunnel PASS
installed E2E                          PASS
read_file via ChatGPT                  PASS
glob via ChatGPT                       PASS
Activity visible                       PASS
all required automated tests           PASS
```

A completion report must also prove that no owned Tunnel/cloudflared
descendants remain, no raw API key appears in errors/logs/IPC responses, and
Tunnel Ready was based on the three required readiness conditions rather than
process existence.

A partially working chain must be reported as incomplete rather than redefining V2 readiness.
