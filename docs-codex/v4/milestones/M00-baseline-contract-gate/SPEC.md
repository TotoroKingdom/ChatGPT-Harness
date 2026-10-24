# M00 SPEC — As-Is Baseline and Architecture Contract Gate

> 存放规则遵守 [AGENTS.md](../../../../AGENTS.md)。本目录只保留六份正式文档，固定测试数据沿用 `fixtures/v4/m00/`。独立报告、独立 ADR、Evidence Index 与独立 POC 工程已按用户要求删除，不恢复；删除不改写已经实际执行的 Development / Integration / E2E-03 结果。未具备真实边界证据的验收项保持 BLOCKED，未执行项保持 NOT EXECUTED，不得记为 PASS。

> Status: M00 overall BLOCKED — Development Complete PASS; Integration PASS (12/12); E2E-01 BLOCKED; E2E-02 BLOCKED; E2E-03 PASS; Manual Acceptance NOT EXECUTED; not ACCEPTED / FROZEN
> Milestone: M00 — As-Is Baseline and Architecture Contract Gate  
> Upstream baseline: V4 PRD + V4 Architecture + V4 Roadmap, FROZEN FOR M00  
> Target: validate current code facts, prove critical V4 architecture assumptions, and freeze the implementation contract consumed by M01+  
> Important: M00 is a contract/feasibility gate. It does not deliver production SQLite History, Tray, Notifications, grep, Metrics/Trace, or the full Agent Evaluation system.

---

## 1. Objective

M00 converts the V4 upstream planning baseline into an implementation-frozen architecture contract.

The milestone must establish five facts with executable evidence:

1. the current main branch behavior and ownership boundaries are accurately documented;
2. a single canonical Runtime evidence source can be projected into a typed, bounded, sanitized contract without creating a second authorization truth;
3. the proposed Runtime-single-writer / Tauri-read-only SQLite ownership model is technically viable under WAL, locking, migration and crash conditions;
4. AppConfig v2 → v3 can be migrated without losing existing user configuration or SecretReference semantics, without performing the production schema bump in M00;
5. a real controllable Agent/Model adapter can execute at least one isolated MCP task with ground truth and a final-state assertion, proving that F12 is feasible before M04 depends on it.

M00 is successful only when these claims are supported by code inspection, runnable POCs/tests and the required real baseline E2E. Research notes or architecture opinion alone are insufficient.

### Current execution closeout

- M00 Development Complete: PASS.
- M00 Integration: PASS (12/12).
- M00-E2E-01: BLOCKED — no real ChatGPT → Tunnel → Runtime MCP evidence was produced.
- M00-E2E-02: BLOCKED — no real controllable Agent/Model → Eval Runner → Runtime MCP evidence was produced.
- M00-E2E-03: PASS; this previously executed result was not rerun during the closeout.
- Manual Acceptance: NOT EXECUTED.
- M00 overall: BLOCKED; it cannot be marked ACCEPTED / FROZEN.

The deleted standalone reports, standalone ADR, Evidence Index and POC source trees are not recreated. They do not turn the completed Development, Integration or E2E-03 execution into “not executed”; the remaining acceptance blockers are M00-E2E-01, M00-E2E-02 and the subsequent Manual Acceptance.

---

## 2. Upstream Authority

M00 must not redefine the V4 product.

Authoritative inputs:

- docs-codex/ChatGPT-Harness_V4_PRD.md
- docs-codex/ChatGPT-Harness_V4_ARCHITECTURE.md
- docs-codex/ChatGPT-Harness_V4_ROADMAP.md
- current main source code, tests and manifests

Relevant canonical PRD acceptance criteria:

- SEC-AC01 — stable request identity across Activity / Approval / History / Metrics / Trace
- SEC-AC02 — typed, bounded, sanitized durable evidence; no second Policy truth
- SEC-AC03 — historical evidence never restores live authorization
- F07-AC02 — History/live authorization separation
- F07-AC03 — persistence failure/migration degradation is observable and does not mutate Policy truth
- F12-AC01 — controlled Agent/Model adapter + versioned ground truth + isolated real MCP Runtime
- F12-AC03 — insufficient/unavailable controlled evaluation must be BLOCKED or INCONCLUSIVE
- F12-AC04 — passive telemetry must not be presented as selection accuracy or hidden reasoning

Primary Architecture decisions under M00 verification:

- A01 — preserve React / Tauri / Runtime / Tunnel ownership boundaries
- A04 — Runtime AuditEvent is canonical evidence source
- A05 — Runtime is the only normal-running SQLite writer; Tauri reads through a read-only path
- A06 — future shared evidence-store library contains DTO / migration / query / maintenance only, never authorization
- A07 — SQLite WAL + versioned migration + bounded writer + retention design is feasible
- A08 — runtime_id + request_id is the request correlation key; other identities require explicit mapping
- A10 — controlled offline evaluation requires an actual controllable Agent/Model adapter and ground truth
- A15 — V4 persistent AppConfig migration direction is v2 → v3; wire protocol remains v1 additive
- A17 — History/Trace are observational by default, not execution authorization dependencies

If evidence proves an upstream decision infeasible, M00 must stop and revise the affected PRD / Architecture / Roadmap before downstream implementation continues.

---

## 3. Verified Starting Facts

The following current-main facts are treated as the baseline to re-verify in T1, not as V4 completion claims:

- Cargo workspace currently contains protocol, runtime and Tauri host crates.
- CURRENT_SCHEMA_VERSION is 2.
- ConfigStore already has explicit v1 → v2 migration behavior and typed migration errors.
- RuntimeSupervisor owns Runtime child process lifecycle, generation and control RPC.
- HarnessController serializes lifecycle operations.
- Runtime AuditEvent already contains event_id and request_id plus typed policy / approval / execution evidence.
- AuditSink is observation-only and cannot return an authorization or execution result.
- ActivityStore is an in-memory bounded projection with capacity 256.
- Pending Approval and Session Grant authority remains Runtime-owned.
- Runtime MCP uses loopback Streamable HTTP.
- V4 production SQLite History, grep, full controlled Evaluation, Tray and Windows notifications are not established by M00 input documents as implemented facts.

T1 must classify every material V4 prerequisite as one of:

- IMPLEMENTED
- LOCAL_TESTED
- EXTERNALLY_ACCEPTED
- NOT_VERIFIED
- KNOWN_ISSUE
- NOT_IMPLEMENTED

Historical documentation is evidence context only; source/tests/manifests and current executable behavior take precedence.

---

## 4. Scope

### 4.1 Included

M00 includes:

1. current-code ownership and lifecycle audit;
2. request/event/session identity contract;
3. terminal/incomplete/unknown outcome semantics;
4. safe evidence field allowlist and explicit prohibited data;
5. proof that real AuditEvent observations can produce the intended safe contract;
6. SQLite ownership POC covering writer/read/maintenance boundaries;
7. WAL, transaction, idempotency, schema migration and crash/incomplete behavior POC;
8. AppConfig v2 → v3 compatibility contract and migration fixtures;
9. proof that SecretReference remains opaque and raw secrets never enter migration fixtures;
10. controlled Agent/Model evaluation feasibility spike against an isolated real Runtime/MCP;
11. versioned ground-truth case and final-state grader for the spike;
12. current-baseline real ChatGPT → Tunnel → Runtime MCP E2E using a non-destructive existing tool;
13. 基线事实与源码引用（在既有 SPEC 中维护） and 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） outputs;
14. decision whether each architecture assumption is ACCEPTED, REVISED or BLOCKED.

### 4.2 Explicit non-goals

M00 does not:

- ship production SQLite History;
- add a production database path to Runtime launch configuration;
- add production evidence-store APIs;
- change current Activity behavior;
- persist production AuditEvent records;
- change Policy / Approval / Workspace / Execution semantics;
- make audit persistence mandatory for execution;
- implement retention UI or query UI;
- upgrade production AppConfig to schema v3;
- add desktop/history production config fields;
- add Tray or Windows notifications;
- implement Startup performance instrumentation or Diagnostics;
- add grep or any new user-facing tool;
- implement production Metrics or Trace;
- implement the full M04 evaluation dataset/runner;
- select a permanent model vendor for the product;
- create protocol/v2;
- claim MCP conformance;
- modify V3 authorization semantics.

Any POC code that could be mistaken for product functionality must be isolated and must not be imported by production Runtime/Tauri modules.

---

## 5. M00 Deliverables

Required outputs:

1. SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md, E2E_TEST_PLAN.md and MANUAL_ACCEPTANCE_CHECKLIST.md — the six pre-development contract documents for M00.
2. 基线事实与源码引用（在既有 SPEC 中维护） — current source/test/runtime truth with status and evidence path.
3. 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） — M00 verdict for each relevant ADR and unresolved risks.
4. Shared Evidence Contract v1 fixture(s), including valid and invalid/redaction cases.
5. SQLite ownership POC source and automated tests used by Development/Integration; the standalone POC source is not retained after the requested cleanup and its deletion is not a new acceptance blocker.
6. SQLite POC execution evidence containing commands, environment and actual results; the evidence is maintained in the existing six documents or dialogue, not a standalone report.
7. Config v2 → v3 compatibility fixtures and migration-contract tests.
8. Controlled Eval feasibility POC, one or more ground-truth cases, and reproducible smoke evidence used by Development/Integration; standalone POC source/report is not retained and is not recreated.
9. Evidence references consumed by the frozen M00 validation plans, maintained in the existing six documents, fixtures and dialogue; no standalone Evidence Index is recreated.

INTEGRATION_TEST_PLAN.md, E2E_TEST_PLAN.md and MANUAL_ACCEPTANCE_CHECKLIST.md are part of the M00 implementation baseline and must be reviewed/frozen before T1 begins. Implementation may add evidence and executable fixtures, but it must not silently weaken test cases, acceptance criteria, or blocker semantics. Any required validation-contract change follows §15 Change Control.

---

## 6. Identity and Evidence Contract

### 6.1 Identity model

M00 freezes the following semantic distinction:

| Identity | Meaning | Correlation rule |
| --- | --- | --- |
| event_id | globally unique observation | used for dedupe/idempotent persistence |
| runtime_id | identity of one Runtime process instance | never inferred from wall-clock time |
| request_id | one Runtime/MCP Tool request identity | paired with runtime_id for request correlation |
| host_session_id | one Host/Harness lifecycle session | only linked after an explicit confirmed mapping |
| supervisor_generation | Tauri RuntimeSupervisor generation | not interchangeable with runtime_id |
| approval_id | one approval object | linked to request only through Runtime evidence |
| trace_id / span_id | future request-local observability IDs | local Harness trace only; not upstream ChatGPT traceparent |

Canonical request correlation is:

~~~text
(runtime_id, request_id)
~~~

M00 must prove that current request evidence can be adapted to this model or explicitly identify the missing mapping that M03 must add.

Time proximity must never be used as the primary identity join.

### 6.2 Observation stages

The safe contract must be capable of representing, when observed:

- request received;
- tool/capability resolution;
- Workspace validation;
- Policy evaluation;
- Approval required/resolved/expired/cancelled;
- Session Grant match where relevant;
- execution started;
- execution completed;
- execution failed/cancelled;
- terminal denied;
- incomplete request after crash/disconnect.

The contract must distinguish:

- requested from attempted;
- ASK/Pending from execution;
- Policy DENY from executor failure;
- execution success from Agent task completion;
- missing terminal evidence from a known failure.

### 6.3 Coverage state

Every request summary that can be durable later must support:

- COMPLETE
- INCOMPLETE
- UNKNOWN

Crash/disconnect/telemetry loss may produce INCOMPLETE. Missing information must not be synthesized into a terminal success/failure.

### 6.4 Safe field policy

Allowed classes:

- stable IDs;
- logical tool identity from the registry;
- typed capability;
- logical Workspace identifier;
- typed Workspace validation result;
- typed Policy decision/reason;
- bounded matched rule IDs;
- approval ID and typed decision;
- typed lifecycle stage/outcome/failure category;
- bounded duration/count/size values;
- bounded tool-specific sanitized argument/result facts;
- hashes/fingerprints where explicitly useful;
- coverage state.

Prohibited by default:

- API keys or SecretStore values;
- raw environment;
- unrestricted argv or command text;
- raw stdout/stderr;
- complete file contents;
- arbitrary unbounded JSON payloads;
- physical filesystem paths;
- OS error strings containing sensitive paths/data;
- model hidden reasoning;
- unverified upstream trace IDs.

A new future tool must define its own typed safe evidence variant; it cannot inherit a generic “serialize all args” path.

### 6.5 Authorization separation

The evidence contract is strictly observational.

No DTO, DB row, fixture, query result or historical approval may:

- create ExecutionPermit;
- create/restore Pending Approval;
- create/restore Session Grant;
- bypass WorkspaceGuard;
- alter PolicyEngine result;
- mark current Tool availability;
- authorize retry/replay.

This is the direct M00 contract for SEC-AC02 / SEC-AC03.

---

## 7. SQLite Ownership POC Contract

### 7.1 Purpose

The POC validates A05/A06/A07. It is not the M03 production database implementation.

The POC must run in an isolated temporary directory and must not use the real AppData database path.

### 7.2 Ownership rules

While the simulated Runtime writer is active:

- exactly one writer connection owns migration/write/online maintenance;
- a Tauri-like reader may open read-only and query committed data;
- the reader may not migrate, delete or perform online maintenance;
- a second writer attempt must fail deterministically.

When the simulated Runtime writer is stopped:

- an offline maintenance actor may acquire an exclusive maintenance lock;
- maintenance may run migration/clear/checkpoint operations defined by the POC;
- writer restart must observe the post-maintenance schema/data consistently.

### 7.3 Required POC behaviors

The POC must prove:

- SQLite WAL can support the intended one-writer + read-only-reader pattern;
- schema_migrations uses explicit version/checksum semantics;
- event_id insertion can be idempotent;
- request identity can be queried without time-based matching;
- a transaction can atomically persist an event plus request-summary update;
- forced interruption leaves no invented terminal success;
- incomplete request state can be represented;
- schema version mismatch/migration failure preserves the source DB for diagnosis;
- writer-lock contention is surfaced rather than silently creating another writer;
- evidence storage failure is observational only and cannot affect a Policy/Permit decision;
- no approval/session grant can be reconstructed into a live authorization object from persisted data.

The POC may use a minimal schema. Production physical DDL, indexes, retention implementation and query SLA belong to M03.

### 7.4 POC isolation rule

Follow the existing development/test layout in root AGENTS.md. POC describes the verification purpose; it does not authorize creating a separate Cargo project, adding a top-level directory, or putting an independent experiment in a core module's tests directory.

If verification cannot fit an existing module-owned test, obtain explicit user approval for the necessity, exact location, file scope and cleanup plan before creating or relocating an independent project. If approved as a Rust crate, keep its Cargo workspace ownership explicit and exclude it from production dependencies. Do not silently change production workspace membership.

The removed prototypes' historical locations and commands are recorded in the existing plans and manual checklist for traceability only. Any remaining cache cleanup is a separate repository-hygiene matter, not a current M00 acceptance blocker; neither this SPEC nor an ADR entry authorizes reuse of the mistaken layout. Record any approved disposition in the existing SPEC/PLAN and manual checklist.

---

## 8. AppConfig v2 → v3 Contract POC

### 8.1 Purpose

M00 proves migration compatibility and freezes migration semantics. M00 does not set CURRENT_SCHEMA_VERSION to 3.

### 8.2 Source contract

The source fixture is a valid current schema v2 configuration containing representative non-default values for:

- configRevision;
- Workspace;
- Tunnel ID;
- proxy;
- tunnel-client selection;
- autoDetect;
- apiKeyRef as SecretReference;
- Security mode;
- UI theme/language;
- Tool enable/disable state;
- Policy defaults and user rules;
- Bash configuration.

The fixture must never contain a raw API key.

### 8.3 Candidate v3 rules

The POC must demonstrate that a future v2 → v3 migration can:

- preserve all existing v2 fields losslessly except deliberate normalization already allowed by current validation;
- preserve configRevision unless a future SPEC explicitly defines a migration revision rule;
- preserve SecretReference as an opaque reference;
- add future host-only desktop settings without placing them in RuntimeConfigSnapshot;
- add future history settings without making DB state an authorization source;
- reject malformed known fields instead of silently resetting user choices;
- reject unsupported future schema versions without rewriting the original;
- perform validation before persistence;
- be atomic and idempotent once implemented.

Exact production desktop/history field defaults may be frozen by M01/M03/M07 SPECs. M00 freezes compatibility and ownership, not feature-level UX defaults that belong downstream.

### 8.4 Wire protocol rule

AppConfig schema v3 does not imply protocol/v2.

M00 preserves:

~~~text
persistent config schema: future v3
control wire protocol: v1 additive unless a real wire incompatibility is proven
~~~

---

## 9. Controlled Agent Evaluation Feasibility Contract

### 9.1 Purpose

M00 must prove that F12 can be implemented honestly before M04 becomes a hard dependency for tool expansion.

### 9.2 Required spike

A controlled runner must execute at least one versioned case through an actual controllable Agent/Model adapter and an isolated real Runtime MCP endpoint.

The case must define:

- case ID/version;
- prompt/task;
- isolated Workspace fixture hash;
- allowed/expected tool set;
- argument constraints;
- Policy/Approval expectation;
- expected final observable state;
- max steps/time/spend;
- model/adapter/version;
- tool schema hash or equivalent pinned identity;
- scorer version.

The runner must capture real MCP tool calls and grade the final state independently from “tool returned success”.

Current E2E-02 result: BLOCKED. The local/deterministic runner mechanics remain Integration evidence and do not substitute for a real controllable Agent/Model turn through the real Runtime MCP.

### 9.3 Truthfulness boundary

The spike may report only what it controls/observes.

It must not infer:

- hidden model reasoning;
- tools the model considered but never emitted;
- provider billed tokens unless actually supplied with verified semantics;
- task success merely from HTTP/MCP success.

If no real controllable Agent/Model adapter can be executed, M00 is BLOCKED. A mocked model or hand-authored tool sequence may test runner mechanics but cannot satisfy the M00 exit gate.

### 9.4 External authorization

Use of a paid/networked model requires explicit user authorization at execution time. Lack of authorization/environment is recorded as BLOCKED, not converted into a weaker acceptance definition.

---

## 10. Current-Baseline External E2E Contract

M00 E2E proves the current chain still exists before V4 implementation begins.

Required path:

~~~text
real ChatGPT
→ Tunnel
→ current Runtime MCP
→ existing read_file or glob
→ Workspace result
→ current Activity / Runtime Ready observation
~~~

Requirements:

- use current-main or the exact M00 candidate build;
- use a controlled Workspace;
- prefer a non-destructive read/glob request;
- record commit, app/runtime/tunnel identity and effective config;
- do not claim SQLite History, Trace or other V4 functionality from this test;
- external execution requires explicit authorization when the E2E plan is run.

Current E2E-01 result: BLOCKED. No real ChatGPT → Tunnel → Runtime MCP chain evidence is available; local UI/process or Runtime smoke evidence cannot substitute for that boundary.

---

## 11. Derived Acceptance Criteria

| M00 AC | Requirement |
| --- | --- |
| M00-AC01 | 基线事实与源码引用（在既有 SPEC 中维护） maps current Runtime/Tauri/Tunnel/Protocol/Config/Approval/Audit behavior to source/tests and uses explicit implementation/verification statuses. |
| M00-AC02 | Ownership map confirms Tauri lifecycle authority, Runtime authorization authority, Tunnel bridge-only role and React presentation role, with no new second authority. |
| M00-AC03 | Shared Evidence Contract defines stable identity, lifecycle semantics, COMPLETE/INCOMPLETE/UNKNOWN and explicitly forbids time-based primary correlation. |
| M00-AC04 | Real current AuditEvent/AuditSink observations can be projected into typed bounded safe evidence; redaction fixtures prove prohibited raw data is not emitted. |
| M00-AC05 | Evidence/history contract cannot create or restore Pending Approval, Session Grant or ExecutionPermit and cannot alter Policy/Workspace/Tool authority. |
| M00-AC06 | SQLite POC proves one normal-running writer + read-only reader + stopped-runtime exclusive maintenance, including deterministic second-writer rejection. |
| M00-AC07 | SQLite POC proves versioned migration, atomic event+summary transaction, event-id idempotency and crash/incomplete semantics without fabricated terminal outcomes. |
| M00-AC08 | Config contract fixtures prove future v2→v3 preservation of current settings and SecretReference, while production CURRENT_SCHEMA_VERSION remains 2 in M00. |
| M00-AC09 | M00 confirms config-schema evolution remains independent from additive control protocol v1 and records any actual incompatibility as a blocker rather than silently creating protocol/v2. |
| M00-AC10 | A real controllable Agent/Model adapter completes at least one isolated Runtime MCP ground-truth case and the scorer validates final state independently of tool transport success. |
| M00-AC11 | Real ChatGPT→Tunnel→Runtime current-baseline E2E succeeds for a controlled existing tool, with build/config evidence and without claiming unimplemented V4 features. |
| M00-AC12 | 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） records ACCEPTED / REVISED / BLOCKED for relevant ADRs; any REVISED upstream decision is committed to PRD/Architecture/Roadmap before M00 can pass. |
| M00-AC13 | M00 introduces no production V4 feature path: POC/eval spike code is isolated and not imported by production Runtime/Tauri. |
| M00-AC14 | Required local/POC verification is reproducible from documented commands and leaves no persistent test DB, processes, secrets, Workspace mutations or config changes. |

---

## 12. Traceability

The test IDs below are frozen in the pre-development M00 validation documents and define the implementation-to-validation traceability contract.

| Upstream | M00 AC | Implementation Task | Integration | E2E | Manual |
| --- | --- | --- | --- | --- | --- |
| A01, Runtime authority | M00-AC01, AC02 | T1 | M00-INT-01, 02 | M00-E2E-01 | M00-MAN-01 |
| SEC-AC01, A04, A08 | M00-AC03, AC04 | T2 | M00-INT-03, 04 | M00-E2E-01 | M00-MAN-02 |
| SEC-AC02, SEC-AC03, F07-AC02, A17 | M00-AC04, AC05 | T2, T3 | M00-INT-04, 05 | M00-E2E-01 | M00-MAN-02 |
| A05, A06, A07, F07-AC03 | M00-AC06, AC07 | T3 | M00-INT-06, 07, 08 | — | M00-MAN-03 |
| A15 | M00-AC08, AC09 | T4 | M00-INT-09, 10 | — | M00-MAN-03 |
| F12-AC01, F12-AC03, F12-AC04, A10 | M00-AC10 | T5 | M00-INT-11 | M00-E2E-02 | M00-MAN-04 |
| Roadmap M00 current-baseline gate | M00-AC11 | T1, T6 | — | M00-E2E-01 | M00-MAN-01 |
| Architecture freeze/change control | M00-AC12, AC13, AC14 | T6 | M00-INT-12 | M00-E2E-03 | M00-MAN-05 |

A dash means the AC is not independently proven by that layer; it is still covered by its required layers above.

---

## 13. Implementation Entry Gate

Codex / the Coding Agent may start T1 only when all six M00 contract documents are present and reviewed:

- SPEC.md;
- PLAN.md;
- TASKS.md;
- INTEGRATION_TEST_PLAN.md;
- E2E_TEST_PLAN.md;
- MANUAL_ACCEPTANCE_CHECKLIST.md.

The validation documents are frozen as targets before implementation. T1–T6 may satisfy them with code, fixtures and evidence, but may not rewrite them merely because an implementation result is inconvenient. If implementation evidence invalidates an upstream product/architecture assumption, follow §15 Change Control instead of weakening the validation target.

---

## 14. Exit Gate

M00 may be marked ACCEPTED / FROZEN only when all of the following are true:

- M00-AC01 through M00-AC14 pass;
- 基线事实与源码引用（在既有 SPEC 中维护） and 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） are complete;
- Shared Evidence Contract fixtures and sanitization tests pass;
- SQLite ownership/migration/crash POC passes;
- future Config v2→v3 compatibility fixtures pass while production schema remains v2;
- at least one real controllable evaluation case actually executes and grades final state;
- real ChatGPT→Tunnel→Runtime baseline E2E passes;
- required Integration/E2E/Manual plans have been executed, not merely authored;
- no unresolved REVISED ADR exists only in downstream docs;
- no production module depends on an M00-only POC;
- no current-scope blocker remains.

If real external ChatGPT access or the controlled Agent/Model adapter cannot be executed, verdict is BLOCKED or INCONCLUSIVE as specified by the relevant test plan. It must not be replaced by a mock and marked PASS.

### Current exit status

The local development and Integration gates are complete, and E2E-03 is PASS. The M00 exit gate remains unsatisfied solely because E2E-01 and E2E-02 are BLOCKED; Manual Acceptance therefore remains NOT EXECUTED. No ACCEPTED / FROZEN claim is valid until those two real-boundary cases pass and the Human Reviewer completes the manual checklist.

---

## 15. Change Control

During M00:

- implementation detail may change inside POC/test-only code;
- module names may be refined if ownership semantics remain unchanged;
- any change to PRD canonical AC, security semantics, authority, release gates, AppConfig migration direction, or M00 dependency graph requires an explicit upstream document revision;
- the M00 implementation agent must not edit V4 PRD/Architecture/Roadmap merely to make a failed POC or test pass; when evidence invalidates an upstream decision, record `UPSTREAM_DECISION_REQUIRED`, mark the affected ADR REVISED/BLOCKED as appropriate, and stop the affected acceptance path;
- upstream revision is a separate architecture/product decision. After it is explicitly approved and committed, re-freeze the affected M00 contract documents and re-run the impacted verification;
- downstream M01/M02/M03 implementation must not begin from a silently revised M00 contract.

M00 ends by freezing facts and contracts, not by maximizing code output.
