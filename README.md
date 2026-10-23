# ChatGPT-Harness

> Current code version: **0.1.3**  
> Active development track: **V4**  
> Current milestone: **M00 — As-Is Baseline and Architecture Contract Gate**  
> M00 status: **implementation ready / validation plans frozen / not yet ACCEPTED**

ChatGPT-Harness is a Windows local Workspace Agent Runtime and desktop control plane. It connects external ChatGPT tool calls to a local MCP Runtime through the Tunnel, while keeping Workspace boundaries, Policy, Approval and execution authority inside the Runtime.

## Architecture at a Glance

```text
React UI
   ↕ Tauri IPC
Tauri Host / HarnessController
   ├── supervises Runtime process
   └── supervises Tunnel process

External ChatGPT
      ↕
    Tunnel
      ↕
 Runtime MCP
      ↓
WorkspaceGuard
      ↓
Policy / Approval / Session Grant
      ↓
 Tool Execution
      ↓
Audit / Activity
```

Ownership is intentionally split:

- **React UI** presents state and user controls.
- **Tauri Host** owns desktop lifecycle and Runtime/Tunnel supervision.
- **Runtime** is the authority for Tool availability, Workspace validation, Policy, Approval, Session Grants and execution.
- **Tunnel** bridges external ChatGPT traffic to the local Runtime; it is not an authorization authority.

## Current Implemented Baseline

The current 0.1.3 codebase already contains the V3-derived local Harness foundation, including:

- Workspace-scoped `read_file`, `glob`, `write` and `edit`;
- controlled `bash` execution;
- Tool enable/disable configuration;
- Policy rules and security modes;
- Human Approval and Session Grants;
- Runtime MCP and Tauri-managed Runtime/Tunnel lifecycle;
- Activity/Audit evidence;
- SecretStore/SecretReference credential handling;
- bilingual UI and appearance/theme support;
- Windows portable and NSIS packaging flows.

The behavioral source of truth remains the current source code, tests and manifests. Historical acceptance documents do not automatically prove the current candidate.

## V4 Development Status

V4 is being developed through explicit milestone gates. M00 is the current gate and exists to verify the architecture assumptions required by later milestones before production V4 features are added.

M00 specifically validates:

- current ownership/lifecycle facts;
- stable Runtime evidence identity and a safe evidence contract;
- Runtime-single-writer / Tauri-read-only SQLite feasibility;
- future AppConfig v2 → v3 migration compatibility while production remains schema v2;
- a real controllable Agent/Model evaluation path through the real Runtime MCP;
- current real ChatGPT → Tunnel → Runtime baseline connectivity;
- POC isolation so M00 does not accidentally ship future V4 features.

M00 does **not** yet ship production SQLite History, Tray/notifications, grep, Metrics/Trace or the full Agent Evaluation system.

## V4 Document Entry Points

Upstream V4 baseline:

- [V4 PRD](docs-codex/ChatGPT-Harness_V4_PRD.md)
- [V4 Architecture](docs-codex/ChatGPT-Harness_V4_ARCHITECTURE.md)
- [V4 Roadmap](docs-codex/ChatGPT-Harness_V4_ROADMAP.md)

Current M00 implementation contract:

- [M00 SPEC](docs-codex/v4/milestones/M00-baseline-contract-gate/SPEC.md)
- [M00 PLAN](docs-codex/v4/milestones/M00-baseline-contract-gate/PLAN.md)
- [M00 TASKS](docs-codex/v4/milestones/M00-baseline-contract-gate/TASKS.md)
- [M00 Integration Test Plan](docs-codex/v4/milestones/M00-baseline-contract-gate/INTEGRATION_TEST_PLAN.md)
- [M00 E2E Test Plan](docs-codex/v4/milestones/M00-baseline-contract-gate/E2E_TEST_PLAN.md)
- [M00 Manual Acceptance Checklist](docs-codex/v4/milestones/M00-baseline-contract-gate/MANUAL_ACCEPTANCE_CHECKLIST.md)

Other references:

- [Current implementation wiki](wiki/README.md)
- [Historical documentation index](docs/archive/README.md)
- [V3 archive](docs/archive/v3-2026-10/README.md)
- [Coding Agent repository rules](AGENTS.md)

## Documentation Authority

Use this order when documents disagree:

```text
Current source / tests / manifests
→ V4 PRD / Architecture / Roadmap
→ current milestone SPEC / PLAN / TASKS + validation plans
→ wiki implementation notes
→ archived historical documents
```

`docs-codex/**` describes active V4 work and may contain future contracts that are not implemented yet.  
`docs/archive/**` is historical reference only.

## Development and Acceptance Flow

Each V4 milestone follows:

```text
PRD / Architecture / Roadmap
→ SPEC / PLAN / TASKS
→ Integration / E2E / Manual plans frozen
→ Coding Agent development + Unit/task-local verification
→ independent Integration QA
→ controlled/authorized E2E
→ Human Manual Acceptance
→ ACCEPTED / FROZEN
→ next milestone
```

A failed architecture assumption is not fixed by weakening tests or silently rewriting upstream requirements. It becomes an explicit `UPSTREAM_DECISION_REQUIRED` decision and must be resolved separately.

## Build and Test

Rust workspace:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

Frontend/Desktop:

```powershell
cd ui
npm install
npm run typecheck
npm test
npm run build
npm run tauri:dev
```

Windows packaging:

```powershell
cd ui
npm run portable:windows
npm run tauri:build
```

## Version Boundary

The repository is currently **developing V4 on top of the 0.1.3 implementation baseline**. A document saying a V4 feature is planned, specified or tested does not mean it is shipped. Only milestone evidence and final acceptance can move a capability from planned to accepted.
