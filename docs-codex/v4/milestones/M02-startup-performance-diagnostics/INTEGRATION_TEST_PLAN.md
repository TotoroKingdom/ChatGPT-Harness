# M02 Integration Test Plan — Startup Measurement and One-click Diagnostics

> 存放规则遵守 AGENTS.md。本目录只保留六份正式文档；执行日志、benchmark archive、独立报告和临时数据不得写入本目录。  
> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M02 — Startup Performance and One-click Diagnostics  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M02 Development Complete  
> Scope: Host startup recorder, startup DTOs, DiagnosticsCoordinator, fault mapping, read-only/privacy, Overview/Settings client boundary and packaged regression.

## 1. Objective

Integration proves that M02 crosses the real protocol → Tauri Host → Runtime/Tunnel → React boundaries without creating a second lifecycle, readiness, Policy or credential authority.

The required proof chain is:

~~~text
accepted Harness Start
  → Host monotonic StartupAttempt
  → authoritative Runtime/MCP/Tunnel readiness
  → authoritative Harness Ready

read-only Tauri diagnostics command
  → Config/Workspace/SecretStore presence
  → Runtime/MCP/Tunnel/control-plane/proxy facts
  → typed PASS/WARNING/FAIL/UNKNOWN
  → shared Overview/Settings presentation
~~~

This plan is frozen before formal Integration execution. A failed or blocked external boundary must remain recorded as such; it must not be replaced with a weaker mock.

## 2. Entry Criteria

Before Integration execution:

- M01 is user-accepted and frozen.
- M02 T1–T6 are Development Complete.
- B0 SHA and candidate SHA are fixed.
- Startup recorder exists before the candidate optimization cohort.
- task-local Rust and frontend tests are PASS.
- cargo fmt, cargo check, cargo test, npm run typecheck, npm test and npm run build have been run for the same candidate.
- disposable config, SecretStore fixture and Workspace use system temporary locations; no real user profile is used for fault injection.

Minimum preflight:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cd ui
npm run typecheck
npm test
npm run build
~~~

If a required packaged/runtime boundary is unavailable:

~~~text
M02 Integration = BLOCKED
~~~

## 3. Evidence Header

Every execution record is delivered in the conversation or an existing approved evidence channel and records:

~~~text
Milestone:
Candidate commit:
Branch:
Dirty state:
Windows build:
Power mode:
Package type:
App/Runtime/Tunnel versions:
Package hash:
Workspace fixture identity:
Config revision:
Proxy source/route mode:
Network type:
Execution time:
QA owner:
Cleanup:
~~~

No secret, physical Workspace path, credential-bearing proxy URL, raw environment or unrestricted process output may be retained in evidence.

## 4. Required Integration Cases

### M02-INT-01 — Protocol additive startup and diagnostics contract

**AC:** M02-AC01, AC06, AC08  
**Task:** T1/T3

Validate generated TypeScript and JSON Schema against Rust models:

- all frozen startup phase IDs round-trip;
- cold/warm and all startup outcomes round-trip;
- phase status and bounded error code round-trip;
- DiagnosticStatus includes UNKNOWN;
- typed scope/evidence/remediation round-trip;
- old v1 diagnostic fixture remains readable;
- generated artifacts are current.

**PASS:** protocol v1 remains additive and no raw value is required by the new DTOs.

### M02-INT-02 — Successful startup recorder closes only on authoritative Ready

**AC:** M02-AC01, AC03, AC06  
**Task:** T1

Using the real Host controller integration surface:

- start an accepted Harness session;
- observe every frozen phase;
- delay Tunnel readiness after process launch;
- verify harness_ready and total duration close only after authoritative HarnessStatus Ready;
- verify phase offsets/durations are monotonic and latest-attempt returns one snapshot.

**PASS:** process exists, Runtime Running or Tunnel process spawn never closes the attempt as Ready.

### M02-INT-03 — Startup failure, cancellation and stale-generation isolation

**AC:** M02-AC01, AC03, AC05  
**Task:** T1/T2

Cover eligibility/config failure, Runtime spawn/initialize/MCP failure, Tunnel resolve/launch failure, Stop while Starting, stale old Runtime generation, stale old Tunnel generation/instance and repeated Ready event.

**PASS:** outcome is failed/cancelled/incomplete as applicable, later phases are skipped/incomplete, and old events cannot close a new attempt.

### M02-INT-04 — Startup instrumentation has no optimization or readiness shortcut

**AC:** M02-AC03, AC05, AC15  
**Task:** T1/T2/T6

Compare B0 and candidate startup call boundaries. Config/Workspace/security validation, SecretStore presence, Runtime version/initialize, MCP endpoint validation, Tunnel distribution/readiness/stale checks and the existing authoritative Harness Ready source must remain present.

**PASS:** instrumentation and later optimization do not replace a readiness check with process existence or spawn success.

### M02-INT-05 — Diagnostics command is read-only

**AC:** M02-AC07, AC10, AC11  
**Task:** T3/T4

Run the aggregate command with call spies/sentinels and assert it never calls config save/reset or migration-writing load, Workspace/client chooser, SecretStore get/set/delete, Harness start/stop/restart, approvals.resolve or filesystem mutation. For a legacy config, verify diagnostic inspection does not rewrite bytes or mtime.

**PASS:** diagnostics observes current facts only and cannot mutate product state.

### M02-INT-06 — Diagnostics healthy state and status precedence

**AC:** M02-AC08, AC09  
**Task:** T3/T4

Exercise all-healthy/Ready plus warning+unknown, unknown only, failure plus warning/unknown and a lifecycle transition while diagnostics runs. Verify exact precedence:

~~~text
FAIL > WARNING > UNKNOWN > PASS
~~~

**PASS:** UNKNOWN is never presented or treated as PASS.

### M02-INT-07 — Configuration and Workspace fault mapping

**AC:** M02-AC07, AC09, AC12  
**Task:** T3/T4

Cover missing, malformed, unsupported, inaccessible config and missing/inaccessible Workspace. Verify dependent checks become UNKNOWN, no physical path is returned and remediation targets are typed.

### M02-INT-08 — Runtime and MCP fault mapping

**AC:** M02-AC07, AC09, AC12  
**Task:** T3/T4

Cover Runtime distribution missing/incompatible, stopped, crashed, status/diagnostics unavailable, MCP endpoint unavailable and ready. Verify process/readiness/endpoint statuses follow SPEC §12.

### M02-INT-09 — Tunnel/control-plane/target fault mapping

**AC:** M02-AC07, AC09, AC12  
**Task:** T3/T4

Cover Tunnel distribution missing, stopped, starting, ready, degraded, failed, stale control plane, blocked recovery, target unreachable and recovery to Ready. Verify old readiness evidence cannot make a new instance PASS.

### M02-INT-10 — Proxy and credential privacy-safe mapping

**AC:** M02-AC08, AC09, AC11, AC12  
**Task:** T3/T4

Cover direct/configured proxy, observed route, typed proxy failure, no route evidence, credential reference present/missing, SecretStore unavailable, accepted connection, Unauthorized/Forbidden and no live proof.

Inject sentinels for API key, credential-bearing proxy URL, Workspace path, raw environment, stdout/stderr and long error bodies. Search serialized result and safe copy output.

**PASS:** only allowlisted bounded facts remain.

### M02-INT-11 — Shared client command and latest startup snapshot

**AC:** M02-AC06, AC07, AC13  
**Task:** T5

Verify the real client calls connection_diagnostics_run, latest attempt calls the Host latest-attempt command, Overview and Settings consume the same Provider snapshot, retry replaces the prior immutable result and one attempt is displayed as startup detail, never benchmark PASS.

### M02-INT-12 — Safe copy allowlist

**AC:** M02-AC06, AC11, AC13  
**Task:** T4/T5

Copy only after explicit user action. Assert output contains only overall/check status/scope/allowlisted evidence/remediation code/startup timing fields. Unknown future DTO fields, raw summaries, paths, URLs and secrets are excluded.

### M02-INT-13 — Overview/Settings/i18n presentation

**AC:** M02-AC06, AC13  
**Task:** T5

Verify en and zh-CN cover four diagnostic statuses, scopes, startup outcome/phase/status, running/error/retry/copy feedback and remediation navigation. No React code reclassifies or merges checks.

### M02-INT-14 — M01/V3 regression boundary

**AC:** M02-AC14  
**Task:** T6

Run affected Tray, Close/Hide/Quit, connection notification, AppConfig v3 desktop preference, Workspace, Policy/Approval, Bash, SecretStore and Start/Stop/Restart suites. Confirm diagnostics does not mutate lifecycle or authorization.

### M02-INT-15 — Packaged candidate wiring

**AC:** M02-AC13, AC14  
**Task:** T6

Use the same packaged candidate for UI/Host integration. Verify the diagnostics command, startup latest command and embedded frontend are present. Record Portable/NSIS capability differences truthfully.

### M02-INT-16 — Cleanup and candidate identity

**AC:** M02-AC02, AC04, AC14  
**Task:** T6

Verify all test-owned Runtime/Tunnel processes stop, system temp data is removed, candidate hash/manifest remains bound to the evidence and no milestone directory receives per-run artifacts.

## 5. Verdict Rules

- PASS — executed and all assertions satisfied.
- FAIL — executed and a required assertion failed.
- BLOCKED — required packaged/external component is unavailable.
- INCONCLUSIVE — execution occurred but evidence cannot support the required conclusion.

Integration PASS does not mark M02 ACCEPTED/FROZEN.

## 6. Exit Gate

M02 Integration is PASS only when M02-INT-01 through M02-INT-16 are PASS, all results bind to one candidate/environment, cleanup is complete and no required case was replaced by a weaker mock.

