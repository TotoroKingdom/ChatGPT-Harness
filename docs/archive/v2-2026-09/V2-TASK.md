# V2 TASK --- Complete ChatGPT-Harness V2 Roadmap

## Objective

Implement the complete V2 Roadmap defined by:

-   `docs-codex/ROADMAP.md`
-   `docs-codex/SPEC.md`
-   `docs-codex/PLAN.md`

This is a single end-to-end implementation task.

Do not split V2 into smaller milestones or stop after partial
infrastructure work.

The task is complete only when the V2 Definition of Done is satisfied.

------------------------------------------------------------------------

# Execution Rules

## Source of Truth

Use the following priority:

1.  Current production code
2.  `docs-codex/SPEC.md`
3.  `docs-codex/PLAN.md`
4.  `docs-codex/ROADMAP.md`

Do not use:

-   `docs/archive/**`
-   outdated design notes
-   assumptions from previous implementations

The repository code is authoritative for existing behavior.

The following lifecycle and security decisions are frozen for this task:

- The Settings API key input is the only short-lived UI holding point for the
  raw value. The submission path is `Settings → Tauri → SecretStore → OS
  Credential Storage`; the raw value must not remain in Provider/global state,
  AppConfig, Runtime config, Runtime, IPC responses, Activity, logs, or errors.
- `Tunnel Process Running` is not `Tunnel Ready`, and `Tunnel Ready` is not
  `Harness Ready`. Tunnel Ready requires control-plane connection, configured
  MCP target reachability, and the authoritative documented tunnel-client
  readiness check.
- Startup failure ends in Harness `error`; user-requested stop ends in
  Harness `stopped`.
- Only a Runtime started by the active Harness session belongs to that
  lifecycle. Existing Runtime processes are not adopted or bound to Tunnel.
- Harness lifecycle commands are serialized and watcher callbacks are routed
  through HarnessController with session/generation identity.
- Windows Tunnel ownership uses a per-session Job Object; direct-child kill is
  insufficient for cleanup.

------------------------------------------------------------------------

# Core Goal

Transform the current local Runtime prototype into a complete product
chain:

    ChatGPT
        ↓
    Tunnel
        ↓
    Runtime MCP
        ↓
    Tools
        ↓
    WorkspaceGuard
        ↓
    Policy
        ↓
    ExecutionPipeline
        ↓
    Activity
        ↓
    UI

The final system must work through the real external path.

A local Runtime-only success is not sufficient.

------------------------------------------------------------------------

# Mandatory Implementation Order

Execute in this order.

Do not skip ahead.

------------------------------------------------------------------------

# 1. Validate Tunnel Client Capability

Before implementing Tunnel integration:

Inspect the actual bundled Tunnel client.

Run:

    tunnel-client.exe --version
    tunnel-client.exe run --help

Verify support for:

-   Tunnel ID
-   MCP server URL
-   API Key secret reference
-   health/readiness endpoint
-   readiness discovery

Do not guess CLI flags.

If the bundled client cannot satisfy the SPEC:

-   upgrade the bundled client;
-   update cloudflared companion;
-   update license/notice artifacts;
-   update distribution metadata.

Do not weaken the security contract.

The repository preflight has confirmed tunnel-client 0.0.13 support for Tunnel
ID, non-literal API key references, MCP URL, loopback health, health URL-file
discovery, and `/readyz`. Record the exact documented invocation and readiness
semantics in tests; do not guess flags or treat process existence as readiness.

------------------------------------------------------------------------

# 2. Runtime MCP Endpoint Contract

Implement:

    mcp.endpoint.get

Requirements:

-   Runtime owns MCP endpoint creation.
-   Tauri retrieves the active endpoint.
-   Endpoint is loopback-only.
-   Endpoint is instance-specific.
-   Endpoint disappears after Runtime stop/crash.
-   Endpoint is never stored in config.
-   Endpoint is never exposed to React.

Add tests.

------------------------------------------------------------------------

# 3. SecretStore Extension

Extend SecretStore:

Current:

    set
    exists
    delete

Required:

    set
    get
    exists
    delete

Requirements:

-   Windows Credential Manager implementation.
-   Memory implementation for tests.
-   Settings may hold the raw API key only transiently during submission.
-   Submission path is Settings → Tauri → SecretStore → OS Credential Storage.
-   No raw secret serialization.
-   No raw API key in React Provider/global state, AppConfig, Runtime config,
    Runtime, IPC responses, Activity, logs, or errors.
-   No frontend secret retrieval API.
-   No Runtime access to API key.
-   No API key in logs/errors/activity.

------------------------------------------------------------------------

# 4. Tunnel Distribution Resolution

Implement deterministic Tunnel discovery.

Installed mode:

Resolve from installed package resources.

Validate:

-   tunnel-client.exe exists;
-   cloudflared exists;
-   paths are canonical;
-   no path escape;
-   distribution manifest matches.

Development mode:

Allowed:

1.  explicit developer override;
2.  repository bundled Tunnel package.

Do not depend on PATH.

------------------------------------------------------------------------

# 5. Real Tunnel Process Management

Upgrade TunnelProcessManager.

It must move from:

    spawn executable

to:

    Tunnel ID
    +
    Runtime MCP endpoint
    +
    secure API key reference
    +
    health configuration
    →
    managed tunnel-client

Requirements:

-   one managed Tunnel instance only;
-   no duplicate starts;
-   no raw API key in argv;
-   no raw API key in status;
-   no raw API key in errors;
-   child processes are owned and cleaned.
-   On Windows, create a per-session Job Object with kill-on-close semantics
    and assign tunnel-client before it can create owned descendants.
-   Graceful stop must wait for the owned tree; timeout must terminate the Job
    Object and verify tunnel-client/cloudflared descendants have exited.
-   Do not kill unrelated processes by name.

------------------------------------------------------------------------

# 6. Tunnel Readiness

Implement real readiness.

Never:

    process exists = ready

Required:

    spawn
     ↓
    health discovery
     ↓
    readiness probe
     ↓
    Tunnel ready

Support:

-   startup timeout;
-   failed readiness;
-   readiness loss;
-   process crash;
-   cleanup.

Tunnel Ready requires all three signals: control-plane connected, configured
MCP target reachable, and authoritative documented tunnel-client readiness.
PID existence, a spawned process, or a listening health port is insufficient.

------------------------------------------------------------------------

# 7. Harness Controller

Create product-level lifecycle ownership.

Required operations:

    harness_start_eligibility
    harness_start
    harness_status
    harness_stop
    harness_restart

Harness owns:

-   Runtime state;
-   MCP state;
-   Tunnel state;
-   lifecycle state;
-   session identity;
-   last structured error.

HarnessController is the only lifecycle state owner. Lifecycle commands are
serialized. Runtime/Tunnel watcher callbacks must flow through the controller
and carry session/generation identity so stale callbacks cannot mutate a newer
Harness session.

Do not duplicate Runtime/Tunnel ownership logic.

------------------------------------------------------------------------

# 8. Harness Start Flow

Implement exactly:

    validate eligibility
            ↓
    Harness starting
            ↓
    Runtime start
            ↓
    Runtime ready
            ↓
    get MCP endpoint
            ↓
    MCP ready
            ↓
    retrieve secret
            ↓
    start Tunnel
            ↓
    Tunnel readiness
            ↓
    Tunnel ready
            ↓
    Harness ready

Failure anywhere:

Rollback.

Rollback must:

-   stop Tunnel;
-   stop Runtime started by this session;
-   clear endpoint/session state;
-   remove temporary artifacts;
-   return structured non-secret errors;
-   leave Harness in `error` with safe `lastError` after cleanup.

------------------------------------------------------------------------

# 9. Stop / Restart

Stop:

    Tunnel
     ↓
    confirm Tunnel cleanup
     ↓
    Runtime
     ↓
    clear session
     ↓
    Stopped

Restart:

    complete stop
     ↓
    reload config
     ↓
    full start

Repeated stop must be safe.

User-requested stop converges Harness to `stopped` only after Tunnel,
cloudflared, and Runtime owned processes have exited and session state is
cleared. A startup failure remains `error` until an explicit stop or
successful restart changes it.

------------------------------------------------------------------------

# 10. Crash Handling

Runtime crash:

-   Runtime error;
-   MCP invalid;
-   Harness leaves Ready;
-   stop associated Tunnel;
-   clear endpoint.

Tunnel crash:

-   Tunnel error;
-   Harness leaves Ready;
-   Runtime may remain for diagnostics;
-   explicit restart required.

Prevent stale watcher events from modifying newer sessions.

An already-running Runtime that was not started by the active Harness session
must not be adopted, bound to Tunnel, or counted toward Harness Ready. The
legacy `runtime_start` operation remains an internal/development capability.

------------------------------------------------------------------------

# 11. Frontend Integration

Replace fake lifecycle state.

Remove:

    Tunnel always stopped

Remove:

    MCP = Runtime projection

UI must show:

    Harness
    Runtime
    MCP
    Tunnel

from authoritative state.

Product lifecycle actions must use `harness_start`, `harness_stop`, and
`harness_restart`. UI code must not derive Harness, MCP, or Tunnel lifecycle
from Runtime status or process existence.

Update:

-   HarnessControlClient;
-   HarnessDataProvider;
-   Overview;
-   Settings;
-   First Run flow.

------------------------------------------------------------------------

# 12. Configuration Rules

Required V2 configuration:

-   Workspace
-   Tunnel ID
-   API Key

Installed users:

must NOT manually locate:

    tunnel-client.exe

Development override may remain.

------------------------------------------------------------------------

# 13. Distribution

Update:

-   distribution manifest;
-   staging script;
-   Tauri bundle resources;
-   bundle verification.

Package:

    UI
    Runtime
    Tunnel client
    cloudflared
    Tunnel metadata and manifest
    licenses/notices/SBOM artifacts

Installed build must run without source tree dependencies.

------------------------------------------------------------------------

# 14. Preserve Existing Runtime Execution

Do not bypass:

    Runtime MCP
     ↓
    Tool Registry
     ↓
    ExecutionPipeline
     ↓
    WorkspaceGuard
     ↓
    Policy
     ↓
    Executor
     ↓
    Activity

Do not move tools into Tauri.

Do not add:

-   Bash;
-   database tools;
-   write operations;
-   Git write.

------------------------------------------------------------------------

# Validation Requirements

Before completion run:

    cargo fmt --all -- --check
    cargo check --workspace
    cargo test --workspace

    cd ui
    npm test
    npm run build
    npm run distribution:stage
    npm run bundle:windows

Automated validation must also cover transient API key boundaries, documented
Tunnel Ready semantics, Windows Job Object descendant cleanup, Runtime
ownership rejection, serialized lifecycle commands, and stale
session/generation callback suppression.

------------------------------------------------------------------------

# Real E2E Acceptance

Must verify:

    Configure:
    Workspace
    Tunnel ID
    API Key

    ↓

    Start Harness

    ↓

    Runtime Ready

    ↓

    MCP Ready

    ↓

    Tunnel Ready

    Tunnel Ready must have evidence of control-plane connection, configured MCP
    target reachability, and the authoritative tunnel-client readiness check.

    ↓

    Harness Ready

    ↓

    ChatGPT connects

    ↓

    Tool discovery succeeds

    ↓

    read_file works

    ↓

    glob works

    ↓

    WorkspaceGuard executes

    ↓

    Policy executes

    ↓

    Activity appears in UI

    ↓

    Stop Harness

    ↓

    Runtime/Tunnel/cloudflared exit

Repeat from installed package.

------------------------------------------------------------------------

# Completion Criteria

Do not declare completion unless:

    Runtime endpoint handoff        PASS
    SecretStore get                 PASS
    Real Tunnel launch              PASS
    Tunnel readiness                PASS
    Harness lifecycle               PASS
    Real UI state                   PASS
    Crash convergence               PASS
    Distribution packaging          PASS
    Development E2E                 PASS
    Installed E2E                   PASS
    ChatGPT read_file                PASS
    ChatGPT glob                     PASS
    Activity visibility              PASS
    Automated tests                  PASS

Completion also requires proof that no owned tunnel-client/cloudflared
descendants remain after stop or failure, that no raw API key appears in
errors, logs, IPC responses, or serialized state, and that Tunnel Ready was
based on the three required readiness conditions rather than process
existence.

If any item is incomplete:

continue implementation or report the exact blocker.

Do not redefine success criteria.
