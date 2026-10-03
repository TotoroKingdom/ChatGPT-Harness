# M5 Development Report

Status: M5 Development Complete

Implementation authority: `SPEC.md` → `PLAN.md` → `TASKS.md`.

This report counts unit tests and code-level automated development verification only. M5 Integration, E2E, and Manual Acceptance have not been executed. No acceptance or freeze status is claimed.

Phase-boundary incidents: during T2 a sub-agent inadvertently used a Runtime test command without `--bin`, which triggered the repository's existing M1 local black-box E2E entry. Earlier Runtime binary-target regression commands also included the existing `m3_runtime_integration_cases` wrapper. These were outside the authorized development phase and are excluded from M5 Unit/QA conclusions. Subsequent task commands use targeted unit filters; T6 explicitly skips `m1_local_black_box_e2e`, `m4_local_black_box_e2e`, `m3_runtime_integration_cases`, and `m3_config_integration_cases`. No Integration/E2E success claim is made.

The unintended command was `cargo test -p chatgpt-harness-runtime`; entry: `runtime/tests/m1_e2e.rs::m1_local_black_box_e2e`. It spawned a Runtime subprocess and exercised Control RPC / loopback MCP. Cargo hid that entry's stdout, so its exact binary/build identity was not captured or independently verified. Its source contains graceful-stop and fixture cleanup plus Drop fallbacks; no separate cleanup evidence was collected. This incident supplies no M5 verification evidence.

## Task evidence

| Task | State | Development evidence |
| --- | --- | --- |
| T1 Config / Discovery / Registry | Complete | Runtime binary test target: 261 tests (included the existing M3 wrapper; excluded from M5 Unit evidence); discovery after final path hardening: 8 passed; protocol: 12 passed; Tauri ConfigStore: 28 passed. |
| T2 Analyzer / Policy | Complete | Runtime binary test target: 288 tests (included the existing M3 wrapper; excluded from M5 Unit evidence); protocol: 16 passed; ConfigStore: 28 passed; analyzer subset: 10 passed; independent security subset: 5 passed; fmt/diff checks passed. |
| T3 Process runtime / Limits | Complete | Bash module targeted tests: 40 passed; independent combined-output security tests: 2 passed. Native process handles confirm tree exit on timeout/output/cancel/drop/stop; actual production output cap tested. |
| T4 Pipeline / MCP / Approval / Evidence | Complete | Bash pipeline: 8 passed; MCP service: 40; Approval: 9; Execution: 18; Audit: 4; effective metadata: 14; Activity: 9; Runtime check passed. |
| T5 UI | Complete | Tools/services: 79 passed; Policies/Approvals: 19 passed. Full frontend suite: 112 passed; TypeScript checks and Vite build passed. |
| T6 Full regression | Complete | Rust: 424 passed, 0 failed; frontend: 112 passed, 0 failed. fmt/check/build/diff checks passed. Historical QA entry points excluded as required below. |

T1 adds schema-v1 additive BashConfig defaults, immutable startup snapshot wiring, Runtime executable discovery/validation, DeveloperShell capability, and a five-tool stable registry with Bash last. Manual invalid paths never fall back. Local Windows verbatim disk paths remain valid; network/device namespaces are rejected. Git installation layout markers support custom installation roots. File tool enabled state is applied consistently across registry constructors.

Tauri code-level tests require a process-local build override when bundle resources have not been staged:

```powershell
$env:TAURI_CONFIG = '{"bundle":{"resources":[]}}'
cargo test -p chatgpt-harness-ui config::tests
```

The override changes test build resources only. It neither stages a distribution nor starts or packages an application.

## Final checks

All checks below completed with exit code 0 against the final source. Changes remain uncommitted.

```powershell
cargo fmt --all -- --check
$env:TAURI_CONFIG = '{"bundle":{"resources":[]}}'
cargo check --workspace
cargo test --workspace -- --skip m1_local_black_box_e2e --skip m4_local_black_box_e2e --skip m3_runtime_integration_cases --skip m3_config_integration_cases
# From ui/:
npm test -- --reporter=verbose
npm run build # includes both TypeScript noEmit checks
# From repository root:
git diff --check
```

Rust totals: Runtime 336; Tauri 72; protocol 16. Two pre-existing M2/M3 black-box E2E tests remain ignored. Four historical Integration/E2E entry points are explicitly filtered. None of these excluded/ignored tests is counted as passing development evidence.

The first T6 Rust run exposed two outdated wire-contract assertions. The repaired diagnostics test now verifies additive Bash observations while preserving the file-tool shape. The protocol decoder retains fail-closed rejection of Bash-only observation fields on file tools; the original contamination test remains enforced. Diagnostics 8/8 and Control Read 4/4 passed before the full regression rerun.

Logs and test binary SHA-256 identities: `artifacts/M5-Controlled-Bash/development/`. Key files: `cargo-check.log`, `cargo-test-workspace.log`, `frontend-tests.log`, `frontend-build.log`, `cargo-fmt.log`, `git-diff-check.log`, `test-binary-identity.json`. The initial failed Rust log is retained separately as `cargo-test-initial-failures.log`.

Build warnings include existing unused interfaces/imports and ts-rs's notice about `deny_unknown_fields`. Serde's closed-field validation remains enforced and tested; this notice does not disable it.

## Resulting behavior

- Bash defaults remain OFF / ASK; Normal mode returns mode unavailability. The legacy developerTestMode boolean supplies no separate authorization.
- Bash uses immutable generation-owned executable configuration, initial canonical Workspace cwd, the existing Policy/Approval/SessionGrant chain, and Pipeline-only ExecutionPermit.
- Classified grants match the full normalized command shape; unclassified commands retain an ASK floor and cannot receive reusable session grants.
- Windows suspended creation / Job assignment / active registration / cancellation recheck / resume keeps process ownership race-safe. Timeout, output overflow, cancellation, request drop and shutdown terminate the invocation tree; native PID handles verify exit.
- MCP exposes truthful high-risk annotations and structured process results. Nonzero exits remain normal process results.
- Public evidence includes sanitized previews and scalar execution facts, excluding argv, private grant scope, environment and complete stdout/stderr.
- Tools, Policies and Approval UI follow Runtime observations and desired/effective restart semantics. File-tool behavior and wire shapes remain compatible.

No outstanding development blocker remains. Git Bash remains Windows-only and provides controlled cwd plus compensating controls, without an OS/filesystem/network sandbox claim.

## Dependency review

- `windows-sys 0.61.2`, Windows target only: Foundation, Security, ToolHelp, JobObjects, Threading. Needed for suspended creation ownership, Job assignment/termination, and process-tree queries; std/Tokio alone do not supply race-safe Job ownership.
- Tokio `process` feature: asynchronous process waits and stdout/stderr drains so the MCP event loop is not blocked by shell execution.
- `signal-hook-registry 1.4.8`: transitive dependency enabled by Tokio process support, retained in Cargo.lock.
- No parser dependency: a bounded, conservative typed scanner models only the supported grammar and applies the ASK floor to uncertainty.

## Follow-up QA

Required follow-up, not executed or claimed by this development run:

1. Independent Integration: real Runtime generation / Control / MCP wiring, manual and automatic discovery, mode/enablement/readiness exposure, approval resume, exact grants and post-approval revalidation.
2. Independent black-box E2E: freshly built Runtime identity, real Git Bash commands, actual 30-second timeout, combined output cap, concurrent calls, cancellation/disconnect/shutdown and descendant PID exit; correlate Activity without secret/output retention.
3. After automated QA passes, rebuild and package the latest Portable into a dedicated M5 artifact directory before Manual Acceptance.
4. Manual Acceptance: native file picker, desired/effective restart convergence, high-risk approval review, unclassified session restriction, safe Create Rule drafts, and UI clarity about initial cwd and external file/network access.

Only M5 Development Complete is recorded. No M5 acceptance/freeze or Integration/E2E/Manual Acceptance result is assigned.
