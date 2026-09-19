# ChatGPT-Harness — PRODUCT.md

> Status: V1 Product Specification  
> Audience: Codex / Claude / human implementers  
> Scope: Windows-first desktop application, with architecture that does not intentionally prevent later cross-platform support.

## 1. Product Definition

ChatGPT-Harness is a local desktop control plane and secure execution runtime that connects ChatGPT to a user-selected local workspace through MCP.

The V1 product is intentionally narrow:

- ChatGPT owns the agent loop.
- ChatGPT-Harness owns controlled local execution.
- The default product is read-only.
- High-risk execution capabilities are not exposed by default.
- The user must be able to install or copy the Harness to another computer and complete all machine-specific setup through the UI.
- Machine-specific configuration and secrets must never be committed to the Git repository.

V1 is not an autonomous agent framework and must not attempt to reproduce Claude Code, Codex, or similar agent runtimes.

## 2. Product Goals

V1 must solve the following problems:

1. Connect ChatGPT to the local Harness Runtime through the OpenAI tunnel client and MCP.
2. Allow ChatGPT to safely inspect a user-selected local workspace.
3. Make the currently exposed capabilities visible to the user.
4. Make security policy visible and understandable.
5. Allow the user to start, stop, diagnose, and configure the Harness from a polished desktop UI.
6. Keep Runtime, tunnel-client, local configuration, and UI responsibilities clearly separated.
7. Make the repository and release distributable to another computer without carrying the original machine's configuration.
8. Record an auditable activity trail of tool calls, policy decisions, approvals, connection events, and failures.

## 3. Non-Goals for V1

The following features are explicitly out of scope:

- Autonomous Agent Loop
- Skills system
- Memory system
- Todo/task planning system
- Context compression
- SubAgents
- Agent Teams
- Background Tasks
- Cron / scheduled tasks
- Database tools
- Git-specific tools
- General write access
- General shell access
- OS-level sandbox implementation
- Remote administration
- Multi-user permission system
- Plugin marketplace
- User-defined hooks or executable policy plugins

Implementers MUST NOT add these features unless the product specification is explicitly revised.

## 4. Target User

Primary target:

A developer who wants ChatGPT to inspect and reason about local source code or project files while keeping local-machine access visible, constrained, and controllable.

The user should not need to:

- edit JSON/TOML configuration files manually;
- modify source code;
- write startup scripts;
- copy configuration from the developer's machine;
- understand the internal process topology;
- keep terminal windows open manually.

## 5. Core Product Principles

1. ChatGPT owns the agent loop.
2. Harness owns local execution.
3. All executable tool requests pass through validation and policy evaluation.
4. Permission is a security boundary, not a hook.
5. The safest capability is one that is not exposed.
6. Read-only is the V1 default.
7. Bash/shell execution is a test capability, not a normal product capability.
8. UI is the local control plane; Runtime must not depend on UI to perform core MCP work.
9. Runtime and tunnel-client are separate components even if the UI controls them through one Start/Stop action.
10. Machine configuration is local data, not repository data.
11. Activity shown to users must be understandable without reading raw logs.
12. The product must fail closed when security-relevant state is unknown.

## 6. V1 Capability Model

### 6.1 Normal Mode

The default tool catalog exposed to ChatGPT contains only:

- `read_file`
- `glob`

These tools operate only inside the active workspace.

Default decisions:

| Capability | Exposure | Policy |
|---|---:|---|
| `read_file` inside workspace | Enabled | ALLOW |
| `glob` inside workspace | Enabled | ALLOW |
| read outside workspace | Not allowed | DENY |
| write operations | Not present | DENY by absence |
| shell execution | Not present | DENY by absence |

### 6.2 Developer/Test Mode

Developer/Test Mode is an explicit local setting intended only for development and diagnostics.

When enabled, the tool catalog may additionally expose:

- `bash`

`bash` MUST:

- be disabled by default;
- be visually marked as high risk;
- use policy `ASK` by default;
- require `Allow Once` or `Deny` for each request;
- have timeout, cancellation, bounded output, working-directory controls, and audit logging;
- never be treated as an OS sandbox;
- remain subject to hard DENY rules.

### 6.3 Deferred Capabilities

The following are not part of V1:

- `write_file`
- `edit_file`

They may be introduced in a later version only after a separate write-policy and recovery design is approved.

## 7. Primary User Flows

### 7.1 First Launch

A new computer must support the following flow:

1. Launch ChatGPT-Harness.
2. Harness checks local prerequisites.
3. Harness detects `tunnel-client` automatically when possible.
4. User selects a Workspace.
5. User enters Tunnel ID.
6. User enters API Key.
7. User runs connection/diagnostic validation.
8. Harness stores machine-specific configuration locally.
9. User starts Harness.
10. UI reports Runtime, Tunnel, MCP, Workspace, Tool Catalog, and Security Mode state.

No manual config-file editing is allowed in the normal flow.

### 7.2 Start Harness

The product exposes one primary action:

`Start Harness`

Internally it must orchestrate separate components:

1. Validate required configuration.
2. Start Harness Runtime.
3. Wait until Runtime reports ready.
4. Start tunnel-client.
5. Wait until tunnel connection is healthy.
6. Verify MCP/tool registration state.
7. Report Harness as Ready.

A partial startup must never be reported as fully connected.

### 7.3 Stop Harness

`Stop Harness` must:

1. Stop tunnel-client.
2. Stop or cancel active test executions when required by product policy.
3. Stop Runtime cleanly.
4. Update UI state.
5. Preserve audit data.

The implementation must not rely only on PID files to determine process identity.

### 7.4 Inspect Capabilities

The user must be able to answer:

- Which tools can ChatGPT currently call?
- Which tools exist but are disabled?
- Which capabilities are blocked by design?
- Which security mode is active?

The Tools page is the authoritative UI for the current exposed Tool Catalog.

### 7.5 Inspect Policy

The user must be able to see:

- ALLOW rules
- ASK rules
- DENY rules
- whether a rule is a fixed system rule or configurable user rule
- the scope/reason of the rule

System DENY rules must not expose an override control.

### 7.6 Human Approval

When an `ASK` request is pending:

- UI must surface it prominently;
- user can choose `Allow Once` or `Deny`;
- there is no `Always Allow` action in V1;
- approval requests can expire;
- the result is audited;
- the originating tool request receives a structured result.

## 8. Desktop UI Information Architecture

V1 contains five main pages.

### 8.1 Overview

Purpose: answer “Is Harness working and what is active?”

Must show:

- Harness overall state
- Runtime state
- tunnel-client state
- MCP state
- active Workspace
- active Security Mode
- number of exposed tools
- pending approval count
- last meaningful error, when present
- Start Harness / Stop Harness primary action
- Run Diagnostics action

Recommended states:

- Stopped
- Starting
- Ready
- Degraded
- Error
- Stopping

### 8.2 Tools

Purpose: answer “What can ChatGPT do right now?”

For every Runtime capability, show:

- tool name
- description
- current exposure state
- effective policy
- scope
- readiness

Normal Mode must clearly show that `bash` is not exposed.

### 8.3 Policies

Purpose: answer “What is automatically allowed, what requires me, and what is blocked?”

Show effective policy grouped by:

- DENY
- ASK
- ALLOW

Each entry must show:

- rule name
- scope
- reason
- source (`SYSTEM` or `USER`)
- whether it is editable

### 8.4 Activity

Purpose: provide human-readable operational history.

Show structured events such as:

- tool requested
- tool allowed
- tool denied
- approval requested
- approval resolved
- execution completed
- execution failed
- Runtime started/stopped
- tunnel connected/disconnected
- configuration validation failed

Raw logs may be available behind a secondary action, but the default view is structured Activity.

### 8.5 Settings

Must support configuration of:

- Workspace
- Tunnel ID
- API Key
- tunnel-client executable path
- automatic tunnel-client detection
- Security Mode
- Developer/Test Mode
- local data/log location display

User must not need to edit config files directly.

## 9. tunnel-client Product Requirements

`tunnel-client` is a transport dependency, not part of Runtime business logic.

Detection priority:

1. bundled/managed location, if distribution rights allow;
2. ChatGPT-Harness installation directory;
3. system PATH;
4. known installation locations;
5. user-selected executable.

The UI must show:

- detected path
- detection source
- version when available
- health/readiness
- Re-detect
- Choose Manually

If the compatible tunnel-client can legally and technically be bundled, release packaging should prefer a self-contained experience. Otherwise automatic detection is the fallback.

## 10. Local Configuration and Secret Handling

Repository and machine data are separate.

### Repository may contain

- source code
- default policy definitions
- config schema
- config examples without machine values
- documentation
- build/release metadata

### Repository and release MUST NOT contain

- real API keys
- real Tunnel IDs belonging to the developer
- developer machine Workspace paths
- developer-specific executable paths
- runtime state
- audit logs
- approval history

### Local machine storage

Machine configuration must be stored under the operating system's application-data location, outside the Git working tree.

Secrets such as API Key must be abstracted behind a secret-storage interface and must never appear in:

- Git
- Activity
- audit payloads
- raw process command logging
- crash/error text
- screenshots produced by the app

## 11. Distribution Requirement

V1 is considered distributable only when the following is possible on a clean Windows computer:

1. Download/install/extract a ChatGPT-Harness release.
2. Launch the application.
3. Configure the machine through the UI.
4. Select a Workspace.
5. Provide connection credentials.
6. Detect or select tunnel-client.
7. Click Start Harness.
8. Reach Ready state without editing source code or repository configuration.

The developer's original local configuration must not be required.

## 12. Diagnostics

`Run Diagnostics` must verify at minimum:

- Runtime binary is available
- Runtime can start
- Workspace exists
- Workspace is readable
- configured workspace boundary is valid
- tunnel-client executable is available
- tunnel-client version can be queried when supported
- Tunnel ID is present
- API Key is present
- tunnel startup succeeds
- MCP Runtime is reachable through the configured transport
- expected Tool Catalog is registered
- Policy Engine is initialized

Diagnostics must return actionable failures rather than only `success/failed`.

## 13. Visual Design Direction

The application should look like a modern developer desktop control plane rather than a terminal launcher.

Design priorities:

1. state clarity
2. security clarity
3. hierarchy
4. low visual noise
5. polished desktop interaction

Use color primarily for semantic state:

- success / connected
- warning / approval
- danger / denied or failure
- neutral / disabled

Do not make raw logs the dominant visual element.

## 14. Product Acceptance Criteria

V1 is complete only when all criteria below pass.

### Connectivity

- ChatGPT can reach Harness through tunnel-client and MCP.
- UI accurately distinguishes Runtime, Tunnel, and MCP state.
- A failed tunnel does not appear as Ready.

### Read-Only Safety

- Normal Mode exposes only `read_file` and `glob`.
- Workspace-outside access is denied.
- No write tool is exposed.
- No shell tool is exposed.

### Developer/Test Safety

- `bash` is disabled by default.
- Enabling Developer/Test Mode changes the exposed Tool Catalog visibly.
- Each bash request defaults to ASK.
- Hard DENY rules cannot be bypassed by approval.
- Bash supports timeout and cancellation.

### UI

- User can configure all required machine settings through UI.
- User can Start and Stop Harness.
- User can inspect current tools.
- User can inspect effective policy.
- User can inspect structured Activity.
- User can run diagnostics.

### Configuration Isolation

- No real local configuration is required inside the repository.
- API Key is never committed to Git.
- Runtime state and logs are outside the repository.
- A clean computer can configure Harness without copying another machine's config.

### Failure Handling

- Invalid Workspace produces an actionable error.
- Missing tunnel-client produces an actionable error.
- Invalid credentials produce an actionable error.
- Runtime crash updates UI state.
- Tunnel crash updates UI state.
- Expired approval requests resolve cleanly.

## 15. Definition of Done

V1 is done when ChatGPT-Harness can be given to another developer as a release, configured entirely through the UI, started with one primary action, and used to let ChatGPT safely inspect a selected local workspace with only the currently visible and policy-constrained capabilities.

Anything beyond that definition is a later version unless explicitly approved.
