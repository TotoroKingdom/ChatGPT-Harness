# ChatGPT-Harness — ARCHITECTURE.md

> Status: V1 Architecture Specification  
> Audience: Codex / Claude / human implementers  
> Product contract: `PRODUCT.md` is authoritative for V1 scope. This document defines the implementation boundaries and mandatory architecture.

## 1. Architecture Goals

The V1 architecture must provide:

- a Rust Harness Runtime;
- a Tauri desktop control plane;
- an external OpenAI tunnel-client transport adapter;
- MCP tool exposure;
- a strict read-only default capability model;
- centralized workspace validation;
- centralized policy evaluation;
- optional human approval for Developer/Test capabilities;
- structured audit/activity events;
- machine-local configuration;
- clean process lifecycle management;
- clear separation of configuration, runtime state, and process state.

The architecture must remain simple enough for a V1 implementation. Logical modules do not imply separate Rust crates unless there is a concrete build or dependency reason.

## 2. System Context

```text
                       Agent Layer
                           │
                       ChatGPT
                           │
                           │ MCP
                           ↓
                ┌────────────────────┐
                │ Transport Adapter  │
                │   tunnel-client    │
                └─────────┬──────────┘
                          │
                          ↓
                ┌────────────────────┐
                │  Harness Runtime   │
                │                    │
                │ MCP Protocol       │
                │ Tool Registry      │
                │ Validation         │
                │ Workspace Guard    │
                │ Policy Engine      │
                │ Approval Manager   │
                │ Executor           │
                │ Audit/Event Bus    │
                │ Runtime State      │
                └─────────┬──────────┘
                          │
                    Local Machine
                          ↑
                          │ local IPC
                ┌─────────┴──────────┐
                │      Tauri UI      │
                │                    │
                │ Config             │
                │ Lifecycle          │
                │ Tools              │
                │ Policies           │
                │ Approval           │
                │ Activity           │
                │ Diagnostics        │
                └────────────────────┘
```

## 3. Mandatory Architecture Principles

1. ChatGPT owns the agent loop.
2. Harness owns local execution.
3. Every executable MCP Tool Request passes through validation and the Policy Engine.
4. Security decisions are part of the execution path, not optional hooks.
5. Normal Mode is read-only.
6. `bash` is an escape-hatch test capability and is not part of the default Tool Catalog.
7. UI is a control plane; Runtime core behavior must not depend on UI availability.
8. Runtime and tunnel-client are separate processes/components.
9. UI may orchestrate Runtime and tunnel-client but must not merge their identity or status.
10. Configuration state, runtime state, and process state are distinct concepts.
11. Security-relevant unknowns fail closed.
12. Tool results, policy results, and audit events are structured data.

## 4. Process Model

V1 contains three conceptual process roles:

### 4.1 Tauri UI

Responsibilities:

- first-run setup
- settings
- lifecycle orchestration
- status display
- Tool Catalog display
- Policy display
- approval interaction
- Activity display
- diagnostics
- local config/secret integration

The UI must not contain the canonical policy implementation.

### 4.2 Harness Runtime

Responsibilities:

- MCP protocol endpoint
- Tool Registry
- input validation
- workspace boundary enforcement
- Policy Engine
- Approval Manager
- tool execution
- structured results
- audit/activity event production
- runtime state

Runtime is the security and execution authority.

### 4.3 tunnel-client

Responsibilities:

- provide transport between ChatGPT and the local MCP Runtime
- run as an independently observable process
- expose exit/error information to the control plane

Harness must treat tunnel-client as an external executable dependency and must not depend on its internal implementation.

## 5. Repository Layout

Recommended V1 layout:

```text
ChatGPT-Harness/
├─ runtime/
│  ├─ Cargo.toml
│  └─ src/
│     ├─ app/
│     ├─ protocol/
│     │  └─ mcp/
│     ├─ tools/
│     │  ├─ read_file/
│     │  ├─ glob/
│     │  └─ bash/
│     ├─ workspace/
│     ├─ policy/
│     ├─ approval/
│     ├─ executor/
│     ├─ audit/
│     ├─ config/
│     └─ state/
│
├─ ui/
│  ├─ src/
│  └─ src-tauri/
│
├─ tunnel-client/
│  └─ README.md / managed binary location as permitted
│
├─ config/
│  ├─ defaults.*
│  └─ example.*
│
├─ docs/
│  ├─ PRODUCT.md
│  ├─ ARCHITECTURE.md
│  └─ SECURITY.md
│
├─ README.md
└─ .gitignore
```

Do not split each logical module into a separate crate in V1 unless required.

## 6. Runtime Request Pipeline

Every tool request must follow one canonical pipeline:

```text
MCP Tool Request
      ↓
Request Envelope Construction
      ↓
Schema / Argument Validation
      ↓
Workspace Resolution / Boundary Validation
      ↓
Policy Evaluation
      ↓
 ┌───────────────┬────────────────┬────────────────┐
 │ DENY          │ ASK            │ ALLOW          │
 ↓               ↓                ↓
Structured       Approval         Executor
Denied Result    Manager             │
                 │                   ↓
          approved / denied       Tool Result
                 │
                 └────approved──────→ Executor
```

No tool may call the Executor directly while bypassing Policy Engine.

## 7. Canonical Request Model

All tools use a common envelope.

Conceptual model:

```rust
struct ToolRequest {
    request_id: RequestId,
    session_id: Option<SessionId>,
    tool: ToolName,
    arguments: ToolArguments,
    workspace_id: WorkspaceId,
    created_at: Timestamp,
}
```

Requirements:

- `request_id` is unique.
- workspace identity is attached before execution.
- raw secrets must never appear in the envelope.
- Tool arguments are validated against tool-specific schemas.
- Audit events reference `request_id`.

## 8. Canonical Result Model

Conceptual model:

```rust
enum ToolStatus {
    Success,
    Denied,
    UserDenied,
    Expired,
    Cancelled,
    Failed,
    TimedOut,
}

struct ToolResult {
    request_id: RequestId,
    status: ToolStatus,
    data: Option<ToolResultData>,
    error: Option<ToolError>,
    started_at: Option<Timestamp>,
    finished_at: Timestamp,
}
```

Errors must use stable machine-readable codes.

Minimum error classes:

- `INVALID_ARGUMENT`
- `WORKSPACE_VIOLATION`
- `POLICY_DENIED`
- `USER_DENIED`
- `APPROVAL_EXPIRED`
- `EXECUTION_FAILED`
- `EXECUTION_TIMEOUT`
- `EXECUTION_CANCELLED`
- `RUNTIME_NOT_READY`
- `TOOL_NOT_EXPOSED`

Do not return only free-form natural-language failure text.

## 9. Workspace Model

Workspace is a first-class security concept.

Conceptual model:

```rust
struct Workspace {
    id: WorkspaceId,
    root_path: PathBuf,
}
```

### 9.1 Path Rules

All file-oriented tools must use one centralized `WorkspaceGuard` / `PathResolver`.

Required flow:

```text
workspace root
    +
requested path
    ↓
normalize
    ↓
resolve/canonicalize where possible
    ↓
symlink/junction-aware boundary validation
    ↓
approved local path
```

Requirements:

- Normal Mode must not read outside the active workspace.
- `..` path traversal must not escape workspace.
- absolute paths outside workspace must be denied.
- Windows symlink/junction behavior must be considered.
- individual tools must not implement independent boundary logic.

## 10. Tool Registry and Exposure

Runtime may contain more code than the active Tool Catalog exposes.

The Tool Registry must distinguish:

- implemented capability
- enabled capability
- exposed MCP tool

### 10.1 Normal Mode Catalog

Exactly:

- `read_file`
- `glob`

### 10.2 Developer/Test Mode Catalog

May additionally include:

- `bash`

### 10.3 V1 Prohibited Catalog Entries

Must not expose:

- `write_file`
- `edit_file`
- arbitrary database tools
- general process-management tools

The UI Tools page reads the effective catalog from Runtime state; it must not use a separately hard-coded list as the source of truth.

## 11. Tool Contract — `read_file`

Purpose:

Read textual file content inside the active workspace.

Conceptual request:

```text
read_file(
  path,
  offset?,
  limit?
)
```

Requirements:

- path must resolve inside workspace;
- `offset` and `limit` support bounded reads;
- binary or unsupported content must return a structured unsupported-content error rather than uncontrolled output;
- read size must be bounded by Runtime configuration;
- result returns normalized path, content, and read metadata;
- no file mutation.

## 12. Tool Contract — `glob`

Purpose:

Find files/directories inside the active workspace by pattern.

Conceptual request:

```text
glob(
  pattern,
  base_path?
)
```

Requirements:

- search root must remain inside workspace;
- results must be bounded;
- result ordering must be deterministic where practical;
- inaccessible paths produce structured diagnostics without aborting unrelated valid results when safe;
- no file mutation.

The tool is responsible for file discovery. Do not add redundant `list_files`, `find_files`, or `tree` tools in V1.

## 13. Tool Contract — `bash` (Developer/Test Mode Only)

`bash` is a generic high-risk command execution capability used for development/testing.

It is not part of Normal Mode.

Conceptual request:

```text
bash(
  command,
  cwd?
)
```

Requirements:

- default policy: ASK;
- only available when Developer/Test Mode is enabled;
- working directory defaults to active workspace;
- Runtime must not intentionally start the process with a cwd outside the active workspace;
- execution timeout is mandatory;
- cancellation is mandatory;
- stdout/stderr capture is mandatory;
- output size must be bounded;
- exit code must be returned;
- process-tree cleanup must be implemented where practical on the target OS;
- commands matching hard DENY policy remain denied even after user interaction;
- the feature must be documented as policy-gated execution, not an OS sandbox.

Runtime internals should use a generic `CommandExecutor` abstraction even if the MCP tool remains named `bash`.

This avoids coupling the architecture to one shell implementation.

## 14. Policy Engine

Policy decision types:

```text
DENY
ASK
ALLOW
```

Precedence:

```text
DENY > ASK > ALLOW
```

Policy evaluation must not use an unsafe `first match wins` behavior when a stronger decision also matches.

Conceptual result:

```rust
struct PolicyEvaluation {
    decision: Decision,
    reason: String,
    matched_rules: Vec<RuleId>,
}
```

### 14.1 Normal Mode Defaults

| Operation | Decision |
|---|---|
| `read_file` inside workspace | ALLOW |
| `glob` inside workspace | ALLOW |
| read outside workspace | DENY |
| shell execution | unavailable |
| write operation | unavailable |

### 14.2 Developer/Test Mode Defaults

| Operation | Decision |
|---|---|
| `read_file` inside workspace | ALLOW |
| `glob` inside workspace | ALLOW |
| `bash` | ASK |
| hard-blocked dangerous command | DENY |

Hard DENY rules are defense-in-depth and do not constitute a complete shell sandbox.

## 15. Approval Manager

Approval exists for `ASK` decisions.

Conceptual model:

```rust
struct ApprovalRequest {
    request_id: RequestId,
    tool: ToolName,
    sanitized_arguments: ApprovalArguments,
    created_at: Timestamp,
    expires_at: Timestamp,
    status: ApprovalStatus,
    resolved_at: Option<Timestamp>,
    decision: Option<ApprovalDecision>,
}
```

Statuses:

- `pending`
- `approved`
- `denied`
- `expired`
- `cancelled`

V1 UI actions:

- `Allow Once`
- `Deny`

There is no `Always Allow` in V1.

The approval path must never override a `DENY` evaluation.

## 16. Request State Machine

Canonical lifecycle:

```text
RECEIVED
   ↓
VALIDATING
   ├── invalid ─────────────→ FAILED
   ↓
WORKSPACE_CHECK
   ├── violation ───────────→ DENIED
   ↓
POLICY_EVALUATING
   ├── DENY ────────────────→ DENIED
   ├── ALLOW ───────────────→ EXECUTING
   └── ASK
         ↓
   WAITING_APPROVAL
     ├── denied ────────────→ USER_DENIED
     ├── expired ───────────→ EXPIRED
     ├── cancelled ─────────→ CANCELLED
     └── approved
             ↓
          EXECUTING
             ├── success ───→ DONE
             ├── failed ────→ FAILED
             ├── timeout ───→ TIMED_OUT
             └── cancelled ─→ CANCELLED
```

Runtime and UI must derive displayed request state from this canonical lifecycle.

## 17. Audit and Activity Event Model

Audit is structured event production, not only text logging.

Minimum event types:

- `runtime_started`
- `runtime_stopped`
- `tool_requested`
- `validation_failed`
- `workspace_violation`
- `policy_evaluated`
- `approval_requested`
- `approval_resolved`
- `execution_started`
- `execution_completed`
- `execution_failed`
- `execution_timed_out`
- `execution_cancelled`

Conceptual event:

```rust
struct AuditEvent {
    event_id: EventId,
    event_type: EventType,
    request_id: Option<RequestId>,
    timestamp: Timestamp,
    summary: String,
    metadata: SanitizedMetadata,
}
```

Requirements:

- no API Key;
- no unredacted secrets;
- Activity UI consumes structured events;
- raw debug logs are secondary;
- audit output is stored outside the repository.

## 18. Configuration Model

Configuration is persistent desired state.

It is not runtime state.

Conceptual categories:

```text
AppConfig
├─ workspace
├─ tunnel
│  ├─ tunnel_id
│  └─ tunnel_client_path
├─ security
│  ├─ mode
│  └─ developer_test_mode
└─ ui
```

API Key must be referenced through secret storage rather than treated as normal serializable config data.

Configuration files must live under OS application data, not under the cloned Git repository.

## 19. Runtime State Model

Runtime state is ephemeral observed state.

Conceptual model:

```text
RuntimeState
├─ runtime_id
├─ started_at
├─ readiness
├─ active_workspace
├─ exposed_tool_catalog
├─ active_requests
├─ pending_approvals
└─ running_executions
```

Do not infer Runtime State merely from saved Config.

Example:

```text
Config workspace = E:\project
Runtime workspace = D:\project
```

must be represented as mismatch/restart-required state rather than `already running`.

## 20. Process State and Identity

Runtime process identity must not rely only on a mutable PID file.

Where persistent process metadata is used, include enough identity to prevent confusing unrelated or stale processes, such as:

- process id
- runtime instance id
- executable identity/version where available
- workspace
- started_at
- tunnel identity where relevant to orchestration

Process liveness and expected configuration must both be checked before reporting `already running`.

## 21. UI ↔ Runtime Control Channel

UI requires a local control channel distinct from the public MCP tool surface.

Requirements:

- local-machine only;
- no unauthenticated LAN binding;
- provides Runtime state, Tool Catalog, Policy view, approvals, events, and lifecycle control as appropriate;
- does not permit bypassing Policy Engine;
- failure of the control channel must not silently weaken Runtime policy.

For Windows V1, a local IPC mechanism such as a named pipe is preferred over a network-listening control API. The implementation may use another local-only mechanism if it preserves the same security properties.

## 22. Lifecycle Orchestration

The UI exposes one product-level `Start Harness` action while maintaining separate component states.

### Start sequence

```text
Validate Config
    ↓
Locate Runtime
    ↓
Start Runtime
    ↓
Wait Runtime Ready
    ↓
Locate tunnel-client
    ↓
Start tunnel-client
    ↓
Wait Tunnel Healthy
    ↓
Verify MCP/Tool Catalog
    ↓
Harness Ready
```

### Stop sequence

```text
Stop tunnel-client
    ↓
Resolve/cancel active test execution as required
    ↓
Stop Runtime
    ↓
Persist final events
    ↓
Harness Stopped
```

If one stage fails, the UI must show the failed component and must not display Ready.

## 23. tunnel-client Discovery

Implement detection as a dedicated service, not UI-only ad hoc logic.

Detection priority:

1. managed/bundled location if permitted;
2. application directory;
3. PATH;
4. known compatible installation locations;
5. manually configured path.

Validation should check:

- file exists;
- file is executable;
- expected executable identity;
- version when queryable.

The user-selected path is local machine configuration and must not be committed to Git.

## 24. Diagnostics Architecture

Diagnostics should run named checks and return structured results.

Conceptual:

```rust
struct DiagnosticCheck {
    id: DiagnosticId,
    status: Pass | Warning | Fail,
    summary: String,
    remediation: Option<String>,
}
```

Minimum checks:

- configuration load
- secret presence
- workspace validation
- Runtime executable/startup
- Tool Registry
- Policy initialization
- tunnel-client discovery
- tunnel-client version
- tunnel authentication/startup
- MCP readiness

The UI must be able to show exactly which check failed.

## 25. UI Architecture and Source of Truth

Pages:

- Overview
- Tools
- Policies
- Activity
- Settings

### Source-of-truth rules

- Tools page: Runtime effective Tool Catalog
- Policies page: Runtime effective Policy representation
- Activity page: structured Audit/Event stream
- Settings page: persistent local Config
- Overview: composed observed state from Runtime + tunnel-client + MCP orchestration

Do not duplicate security logic in frontend code.

## 26. Local Data Layout

Conceptual Windows layout:

```text
%APPDATA%/ChatGPT-Harness/
├─ config.*
├─ state/
└─ logs/
```

Secret storage is separate through the selected OS secret-store adapter.

Repository data and local runtime data must never share the same default path.

## 27. Security Model

V1 provides:

- capability minimization;
- read-only Normal Mode;
- workspace boundary validation;
- central policy evaluation;
- explicit approval for test shell execution;
- hard deny rules;
- bounded execution;
- auditability;
- secret redaction.

V1 does NOT provide:

- complete command semantic analysis;
- container isolation;
- VM isolation;
- a hardened OS sandbox;
- protection from every behavior possible once high-risk shell execution is approved.

Documentation and UI must not claim otherwise.

## 28. Failure Policy

Security-relevant failures fail closed.

Examples:

- unknown Workspace → deny tool request
- Policy Engine unavailable → deny tool request
- invalid path resolution → deny
- approval subsystem unavailable for ASK → do not execute
- developer mode state unknown → do not expose bash
- malformed tool arguments → reject before policy/execution

Operational failures must be visible:

- Runtime crash
- tunnel-client crash
- MCP connection loss
- config mismatch
- diagnostics failure

## 29. Implementation Order

Codex/Claude should implement V1 in this order unless a dependency requires a small adjustment:

### Phase 1 — Skeleton

- repository/workspace setup
- Runtime process
- Tauri UI process
- local config path
- basic Runtime ↔ UI state channel
- structured logging/event primitives

### Phase 2 — Read-Only Runtime

- Workspace model
- WorkspaceGuard/PathResolver
- ToolRequest/ToolResult models
- Tool Registry
- `read_file`
- `glob`
- Policy Engine
- MCP registration
- Normal Mode Tool Catalog

### Phase 3 — Control Plane

- Overview
- Settings
- Tools
- Policies
- Activity
- first-run setup
- Runtime lifecycle management

### Phase 4 — Tunnel Integration

- tunnel-client detection
- start/stop orchestration
- connection state
- credential integration
- diagnostics

### Phase 5 — Developer/Test Mode

- `bash`
- ASK policy
- Approval Manager
- approval UI
- timeout
- cancellation
- bounded output
- hard DENY rules

### Phase 6 — Hardening

- crash recovery
- stale process detection
- state mismatch handling
- secret redaction verification
- symlink/junction boundary tests
- end-to-end acceptance tests
- release packaging

## 30. Required Tests

At minimum:

### Workspace

- valid relative file
- nested path
- `../` traversal
- absolute outside path
- symlink/junction escape where test environment supports it

### Policy

- ALLOW read inside workspace
- DENY outside workspace
- DENY precedence over ASK/ALLOW
- bash unavailable in Normal Mode
- bash ASK in Developer/Test Mode

### Approval

- allow once
- deny
- expire
- cancel
- DENY cannot enter approval path

### Lifecycle

- clean start
- missing config
- Runtime crash
- tunnel-client crash
- stale process metadata
- config/runtime workspace mismatch
- clean stop

### Security

- secrets absent from Activity
- secrets absent from audit logs
- secrets absent from process log rendering
- write tools absent from V1 Tool Catalog

## 31. V1 Architectural Definition of Done

Architecture is correctly implemented when:

1. Runtime can operate as an independent core process.
2. UI controls but does not own Runtime security logic.
3. tunnel-client is independently detected, started, stopped, and observed.
4. Normal Mode exposes only `read_file` and `glob`.
5. all file access passes through a single workspace boundary implementation.
6. all executable tool requests pass through Policy Engine.
7. Developer/Test `bash` is absent unless explicitly enabled.
8. ASK requests use Approval Manager and cannot bypass DENY.
9. Activity is generated from structured events.
10. config, runtime state, process state, and secrets are kept separate.
11. no machine-specific runtime configuration is required in Git.
12. a clean-machine release can be configured and run through the UI.

If an implementation decision conflicts with these requirements, `PRODUCT.md` and this architecture document take precedence over convenience or framework defaults.
