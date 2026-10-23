# Repository Guidelines

This file contains **long-lived repository rules** for Coding Agents. It must remain useful across product versions and milestones. Do not put active milestone names, release numbers, temporary acceptance states, or phase-specific implementation instructions here; those belong in the relevant milestone documents.

## Sources of Truth

- For **implemented behavior**, inspect source code, tests, manifests, and build scripts. Do not infer shipped features from requirements or archived reports.
- For **intended behavior**, read the active PRD, Architecture, Roadmap, and the current milestone's SPEC / PLAN / TASKS / verification plans under `docs-codex/`.
- `wiki/` may help navigate the codebase, but verify its statements against code because it can lag behind.
- `docs/archive/` is historical. Do not consult or treat it as authoritative for current work unless a task explicitly requires historical comparison or migration.
- When an implementation contradicts an approved requirement, surface the discrepancy instead of silently changing either side.

## Repository Structure and Ownership

- `protocol/`: shared control contracts, schema, fixtures, and contract tests.
- `runtime/`: MCP endpoints, Workspace enforcement, Tool execution, Policy, Approval, Activity, and Audit.
- `ui/src/`: React application and frontend tests.
- `ui/src-tauri/`: Tauri Host, desktop lifecycle, configuration, secrets, and managed Runtime/Tunnel processes.
- `ui/scripts/`: distribution and packaging automation.
- `fixtures/`, `assets/`, `wiki/`: test fixtures, media, and implementation references.
- `docs-codex/`: active product and milestone documentation.
- `docs/archive/`: archived historical documents.

Respect process and authority boundaries: UI expresses intent and displays observed state; Tauri manages desktop processes, configuration, and secrets; Runtime authorizes and executes tools. Tunnel transports traffic and must not become an authorization authority. Verify the exact ownership in code before altering a boundary.

## Milestone Development Process

For milestone work, follow the active project's delivery contract:

```text
PRD / Architecture / Roadmap
→ SPEC / PLAN / TASKS
→ Integration / E2E / Manual Acceptance plans
→ Task implementation + Unit/contract tests
→ independent Integration and E2E verification
→ human Manual Acceptance
→ evidence and explicit milestone acceptance/freeze
```

- Read the *current* milestone documents and their entry/exit criteria before changing production code.
- Define and review test/acceptance criteria **before** implementing against them. Do not weaken them to fit an implementation.
- Keep tasks scoped, verify each task locally, and separate development-complete from verified and accepted.
- Follow the milestone's definitions of automated E2E, external-system E2E, and manual checks. Do not replace a required real boundary with mocks or call an unexecuted check PASS.
- Record actual commands, versions, evidence, skipped checks, and failures. Use PASS, FAIL, BLOCKED, or NOT_EXECUTED accurately.
- Only human reviewers can attest to manual acceptance; the Coding Agent must not self-certify it.
- If an upstream product or architecture decision must change, record the affected requirement, evidence, and blocker; do not edit an approved upstream baseline just to make a task pass. Obtain explicit direction and re-verify affected contracts.

## Security and Runtime Invariants

- Never bypass Workspace confinement, path normalization, Policy, Approval, or Tool authorization to make a test succeed.
- Fail closed on invalid, missing, or inconsistent security state. Preserve cancellation, timeout, process cleanup, and generation/session isolation.
- Keep API keys and credentials in the established secret store/secret-reference flow; do not commit secrets or expose them in logs, argv, IPC responses, error messages, fixtures, or evidence.
- Treat external inputs, MCP tool arguments, persisted configuration, and diagnostic payloads as untrusted.
- Preserve compatibility and migration behavior unless a separately approved contract change explicitly revises it.
- Use disposable directories and synthetic credentials for tests/POCs. Keep experiments separate from production dependencies and persistent user data.
- Avoid unrelated refactors, destructive cleanup, or global process termination. Protect unrelated local changes.

## Development and Verification Commands

From repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

From `ui/`:

```powershell
npm install
npm run dev
npm run tauri:dev
npm run typecheck
npm test
npm run build
```

Windows packaging (from `ui/`):

```powershell
npm run portable:windows
npm run tauri:build
```

Run focused tests first and then all relevant regression suites. Rust protocol contract tests belong with `protocol/`; Runtime black-box/integration tests live under `runtime/tests/`; frontend tests use the established `*.test.ts` / `*.test.tsx` conventions. Packaging or native behavior must be verified on an appropriate Windows environment; do not claim success without running it.

## Changes, Documentation, and Evidence

- Follow existing Rust formatting and TypeScript conventions. Keep changes narrow and update schemas, protocol fixtures, generated types, and tests when contracts change.
- Keep `README.md` **Chinese-first and reader-facing**: what the product does, the currently available features, how to get started, and concise development links. Avoid internal milestone task lists and lengthy acceptance reports in README.
- Keep milestone-specific rules, thresholds, proofs, and blockers in their own `docs-codex/` files, not in this AGENTS.md.
- Explain assumptions, risks, and verification gaps in handoffs. Separate implemented, tested, externally verified, and manually accepted states.
- Use existing commit conventions such as `feat(scope):`, `fix(scope):`, `docs:`, `test:`, and `release:`. Do not force-push, rewrite history, or discard unrelated changes without explicit authorization.
