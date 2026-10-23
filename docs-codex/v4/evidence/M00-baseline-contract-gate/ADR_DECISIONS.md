# M00 ADR Decisions — Development Evidence and Handoff

> Status: DEVELOPMENT COMPLETE HANDOFF; not `ACCEPTED` / `FROZEN`.
> Candidate source observed during T1: `7f2a482b1babfdb08b5e80cabfa430d3d0227dac`; later local-only test/doc changes are listed in the final report.
> Integration, external E2E and Manual Acceptance remain unexecuted by request.

M00 does not silently revise PRD/Architecture/Roadmap. Every decision below preserves the upstream semantics; a BLOCKED result means the relevant acceptance path is not unlocked.

## 1. Decision matrix

| ADR | Planning-baseline decision | M00 evidence | Verdict | Downstream constraints | Open risk / owner |
| --- | --- | --- | --- | --- | --- |
| A01 | Keep React / Tauri / Runtime / Tunnel ownership boundaries. | `AS_IS_BASELINE.md` ASIS-01–18; existing Runtime/Tauri tests; no production POC imports. | **ACCEPTED** | M01+ keep Tauri as lifecycle owner, Runtime as authorization owner, Tunnel as bridge, React as projection. | Real desktop/external boundary remains M00 E2E work; owner: M01/M06 QA. |
| A04 | Runtime `AuditEvent` is the canonical evidence source. | `runtime/src/audit/event.rs`; `runtime/src/audit/mod.rs`; T2 `m00_contract` projection tests; existing Activity/EventBus fan-out. | **ACCEPTED** | M03/M08 must project from one canonical event source and must not recompute Policy. | Current production has only bounded live projections; durable writer is future M03. |
| A05 | Runtime is the normal-running SQLite writer; Tauri reads read-only. | `runtime/tests/m00/evidence-store-poc/src/lib.rs`; `SQLITE_POC_REPORT.md`; 7/7 POC tests. | **ACCEPTED** | Runtime writer owns online migration/write; Tauri read path cannot write/migrate/clear; offline maintenance requires writer stop. | Final cross-process lock and production DAL belong to M03. |
| A06 | Shared evidence-store library may own DTO/migration/query/maintenance, never authorization. | Isolated POC has no root workspace membership and exposes only data/maintenance APIs; historical approval test has no authorization conversion. | **ACCEPTED** | M03 may extract a shared library only if it remains outside PolicyEngine/ExecutionPermit authority. | Production schema/API not designed or shipped in M00. |
| A07 | WAL + versioned migration + bounded writer/retention is viable. | POC WAL, explicit checksum history, transactional event+summary, idempotency, injected migration failure, offline checkpoint/clear. | **ACCEPTED** | M03 must re-specify queue size, batching, retention and full/busy/corrupt behavior; POC defaults are not production defaults. | Crash/process-kill and real disk-full integration remain M03 validation. |
| A08 | `(runtime_id, request_id)` is the primary request correlation key; other IDs need explicit mapping. | T2 projection carries `request_id` but sets `runtime_id=null` with explicit `not_available_in_current_audit_event`; near-simultaneous request test stays distinct. | **ACCEPTED** | M03 must add a confirmed Runtime identity mapping before durable cross-feature joins. Never join by timestamp, Host generation, or event order alone. | Current AuditEvent lacks runtime_id; owner: M03 contract implementation. |
| A10 | Controlled Evaluation requires a real controllable Agent/Model adapter and ground truth. | `runtime/tests/m00/agent-evaluation-poc/` has real loopback HTTP MCP client, provider-neutral HTTP adapter path, versioned case, fixture hash and independent final-state scorer; local mechanics 7/7. | **BLOCKED for M00 acceptance** | M04 may use the runner boundary only after authorized real model/adapter execution, actual Runtime/MCP case and frozen quality thresholds. No mock-only PASS. | User requested no E2E in this round; M00-E2E-02 remains `NOT_VERIFIED/BLOCKED`. Owner: authorized M00 E2E / M04. |
| A15 | Persistent AppConfig direction is v2→v3; control protocol stays v1 additive. | T4 fixtures and 6/6 UI contract tests prove v2 preservation, host-only fields, opaque SecretReference, non-destructive unsupported schema; production `CURRENT_SCHEMA_VERSION=2`, protocol version=1. | **ACCEPTED** | Downstream SPECs choose feature defaults; production migration must be atomic and preserve existing fields. No protocol/v2 implication. | Actual production v3 migration belongs M01/M03/M07 and is not shipped here. |
| A17 | History/Trace are observational by default, not authorization dependencies. | Evidence fixtures/negative tests, SQLite POC observational-failure test, no Permit/Approval/Grant conversion. | **ACCEPTED** | Telemetry/DB failure cannot add ALLOW, cannot mutate Policy, and cannot fabricate terminal outcomes. | Future “audit required to execute” semantics would require a new PRD/ADR. |

## 2. Explicit non-verdicts

- `M00-INT-01`–`M00-INT-12`: **NOT EXECUTED** in this development round.
- `M00-E2E-01` real ChatGPT → Tunnel → Runtime baseline: **NOT EXECUTED / NOT_VERIFIED**.
- `M00-E2E-02` real controllable Agent/Model → runner → Runtime MCP: **NOT EXECUTED / BLOCKED pending explicit authorization**.
- `M00-E2E-03` production black-box isolation: **NOT EXECUTED**.
- `M00-MAN-01`–`M00-MAN-05`: **NOT EXECUTED**; Human Acceptance remains outside this round.
- No line above is a claim that M00 is `ACCEPTED` or `FROZEN`.

## 3. Isolation audit

| Check | Result | Evidence |
| --- | --- | --- |
| Root production workspace depends on SQLite POC | PASS (source check) | Root `Cargo.toml` still lists only `protocol`, `runtime`, `ui/src-tauri`; POC has its own empty workspace. |
| Production Runtime/Tauri invokes Eval spike | PASS (source check) | `rg` over production modules finds no `runtime/tests/m00/agent-evaluation-poc` import/path. |
| Production `CURRENT_SCHEMA_VERSION` | PASS | `protocol/src/v1/config.rs` remains `2`; T4 test asserts it. |
| Production control protocol | PASS | `protocol/src/lib.rs` remains `PROTOCOL_VERSION=1`; no `protocol/v2` created. |
| New user-facing Tool / grep | PASS | M00 changed no ToolRegistry/catalog/schema; grep remains M05 scope. |
| Production History/Trace API | PASS | M00 evidence DTO is test-only; POC is isolated; no production SQLite path/API added. |
| Policy/Approval/Workspace semantics | PASS (local regressions) | Only `#[cfg(test)]` projection/config tests plus isolated POCs changed; affected tests pass. |
| Raw credentials | PASS for M00 additions | Fixtures use synthetic opaque references/sentinels; no provider credential/token was committed. |
| Temp DB/Workspace/process cleanup | PASS for task-local tests | POC tests use temp directories and drop resources; no long-lived test process was started. |

## 4. Handoff constraints

1. M01/M02/M03 must consume the identity and authority boundary here without redefining it in downstream TASKS.
2. M03 must resolve the confirmed `runtime_id` mapping before durable History/Trace joins.
3. M04 remains gated on a real, authorized controllable Agent/Model run; the local scripted adapter is mechanics evidence only.
4. Formal Integration/E2E/Manual plans remain frozen as authored. They are not rewritten or marked PASS by T6.
5. Because A10 has an acceptance blocker, this document does not unlock M01/M02/M03 and does not declare M00 acceptance.

## File layout relocation — 2026-10-24

User-authorized layout-only change: reports and decisions now live in `docs-codex/v4/evidence/M00-baseline-contract-gate/`; contract fixtures moved to `fixtures/v4/m00/`. Runtime and Tauri test references were updated. Fixtures remain test-only and outside production imports; no authorization or acceptance criteria changed. Historical results above apply to their original candidates, not automatically to a rebuilt candidate.

The isolated prototypes now reside in `runtime/tests/m00/evidence-store-poc/` and `runtime/tests/m00/agent-evaluation-poc/`. Both remain independent Cargo workspaces; production workspace membership is unchanged. Compile caches and disposable validation copies are not maintained source. `artifacts/` is reserved for Portable delivery packages and their manifests, hashes and package-verification records.
