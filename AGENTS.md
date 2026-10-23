# Repository Guidelines

## Current Development State

ChatGPT-Harness is currently on the **V4 development track**. The production code/manifests are version **0.1.3**, while V4 features are being developed milestone-by-milestone.

The active milestone is:

```text
M00 — As-Is Baseline and Architecture Contract Gate
```

M00 is an implementation/feasibility gate, not a shipped V4 feature milestone. Its pre-development contract is:

- `docs-codex/v4/milestones/M00-baseline-contract-gate/SPEC.md`
- `docs-codex/v4/milestones/M00-baseline-contract-gate/PLAN.md`
- `docs-codex/v4/milestones/M00-baseline-contract-gate/TASKS.md`
- `docs-codex/v4/milestones/M00-baseline-contract-gate/INTEGRATION_TEST_PLAN.md`
- `docs-codex/v4/milestones/M00-baseline-contract-gate/E2E_TEST_PLAN.md`
- `docs-codex/v4/milestones/M00-baseline-contract-gate/MANUAL_ACCEPTANCE_CHECKLIST.md`

These six documents are frozen as the implementation target before T1 starts. M00 is not ACCEPTED/FROZEN until its actual Integration, E2E and Manual Acceptance evidence passes.

## Authority and Documentation Order

Use the following authority order:

1. **Source code, tests and manifests** — truth for what the current product actually implements.
2. **V4 PRD / Architecture / Roadmap** — frozen upstream product and architecture baseline for the current V4 milestone.
3. **Current milestone SPEC / PLAN / TASKS + validation plans** — executable implementation and acceptance contract.
4. **`wiki/**`** — current implementation notes only when consistent with source/tests.
5. **`docs/archive/**`** — historical reference only; never use archived acceptance as proof for the current candidate.

Key upstream V4 documents:

- `docs-codex/ChatGPT-Harness_V4_PRD.md`
- `docs-codex/ChatGPT-Harness_V4_ARCHITECTURE.md`
- `docs-codex/ChatGPT-Harness_V4_ROADMAP.md`

Do not treat future V4 documentation as proof that a feature is already implemented.

## V4 Development Workflow

The required milestone flow is:

```text
PRD / Architecture / Roadmap
→ SPEC / PLAN / TASKS
→ Integration / E2E / Manual Acceptance plans frozen
→ Coding Agent executes Tasks + task-local Unit/POC tests
→ independent QA executes Integration
→ controlled/authorized E2E
→ Human Manual Acceptance
→ ACCEPTANCE + EVIDENCE_INDEX
→ Milestone ACCEPTED / FROZEN
→ next milestone
```

Rules:

- Do not start milestone implementation before its engineering and validation documents are frozen.
- Coding Agents own Task implementation and task-local automated verification only.
- Do not mark Integration/E2E/Manual cases PASS from development-task results.
- Unexecuted, blocked or unavailable external checks remain BLOCKED/INCONCLUSIVE; never convert them to PASS.
- Paid/networked model execution or real external ChatGPT execution must follow the explicit authorization rules in the milestone documents.
- A milestone may unlock downstream work only after its defined acceptance gate is satisfied.

## Upstream Change Control

A Coding Agent must not edit the V4 PRD, Architecture or Roadmap merely to make an implementation or test pass.

If implementation evidence invalidates an upstream decision:

1. record the evidence and affected ADR/requirement;
2. mark `UPSTREAM_DECISION_REQUIRED` and REVISED/BLOCKED as appropriate;
3. stop the affected acceptance path;
4. perform any PRD/Architecture/Roadmap revision as a separate explicitly approved architecture/product change;
5. re-freeze the affected milestone documents and re-run impacted verification.

Never weaken acceptance criteria after observing a failing result.

## M00-Specific Guardrails

During M00:

- do not ship production SQLite History, Trace, Metrics, Tray, notifications, grep or the full Agent Evaluation system;
- production `CURRENT_SCHEMA_VERSION` remains **2**;
- do not introduce `protocol/v2`; current control protocol remains v1 unless a separately approved incompatibility requires otherwise;
- M00 evidence/history objects are observational only and must never restore or create `Pending Approval`, `Session Grant` or `ExecutionPermit`;
- SQLite and evaluation POCs must be isolated from production Runtime/Tauri dependencies;
- if a Rust POC lives below the repository root, it must either declare its own empty `[workspace]` or be listed in root `workspace.exclude`; do not let it become an implicit production workspace member;
- POC databases/config/workspaces must use disposable paths, never the real user AppData/config;
- do not add a new user-facing Tool or weaken Workspace/Policy/Approval semantics to satisfy a POC;
- a mocked model/provider may test runner mechanics but cannot satisfy the real controlled-evaluation acceptance gate;
- never commit API keys, raw SecretStore values, machine-specific credentials or sensitive raw payloads.

## Project Structure

The repository combines a Rust workspace with a React/Vite frontend and Tauri desktop shell:

- `protocol/` — shared control protocol types, schemas, fixtures and contract tests.
- `runtime/` — Runtime process, MCP server, tools, Workspace security, Policy, Approval, Session Grants, Activity and Audit.
- `runtime/tests/` — Runtime black-box/integration tests.
- `ui/src/` — React UI and colocated frontend tests.
- `ui/src-tauri/` — Tauri Host, `HarnessController`, Runtime/Tunnel supervision, config and SecretStore integration.
- `ui/scripts/` — Windows packaging/distribution scripts.
- `fixtures/`, `assets/`, `wiki/` — test data, media and current implementation notes.
- `docs-codex/` — active V4 product/milestone documentation.
- `docs/archive/` — historical version documentation.
- `poc/` / `evaluation/` — only when explicitly introduced by a milestone; keep isolated from production dependency graphs.

## Build and Test Commands

Run frontend commands from `ui/`:

```powershell
npm install
npm run dev
npm run tauri:dev
npm run typecheck
npm test
npm run build
```

At repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

Packaging:

```powershell
cd ui
npm run portable:windows
npm run tauri:build
```

Run the narrowest relevant test first, then the applicable suite. Protocol contracts belong in `protocol/tests/`; Runtime integration/black-box tests belong in `runtime/tests/`; frontend tests use `*.test.ts` / `*.test.tsx`.

## Coding and Evidence Conventions

Use rustfmt-compatible Rust, the repository's existing TypeScript/JSON formatting, `PascalCase` for React components/types, `camelCase` for TypeScript values and `snake_case` for Rust modules/functions.

Synchronize protocol changes with schemas, fixtures and tests. Store credentials through `SecretStore`.

Every milestone report must separate:

- implementation completed;
- local/task-level tests passed;
- Integration results;
- E2E results;
- Manual Acceptance results;
- skipped/BLOCKED/INCONCLUSIVE checks.

Do not reuse an old milestone's ACCEPTED label as current evidence without proving the exact candidate is covered.

## Commit and Pull Request Guidelines

Use the existing commit prefixes such as `feat(scope):`, `fix(scope):`, `docs:`, `test:` and `release:`.

Keep commits behavior-focused. PRs should reference the relevant SPEC/TASKS, list verification commands and outcomes, and clearly call out unexecuted external/manual checks. Packaging changes should include artifact path/version and verification result.
