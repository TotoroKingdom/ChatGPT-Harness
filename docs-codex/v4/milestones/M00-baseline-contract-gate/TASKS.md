# M00 TASKS — As-Is Baseline and Architecture Contract Gate

> Status: IMPLEMENTATION READY / VALIDATION FROZEN  
> Inputs: SPEC.md, PLAN.md, INTEGRATION_TEST_PLAN.md, E2E_TEST_PLAN.md, MANUAL_ACCEPTANCE_CHECKLIST.md  
> Executor: Codex / Coding Agent  
> Upstream: V4 planning baseline FROZEN FOR M00  
> Rule: every Task includes implementation + task-local automated verification. Integration/E2E/Manual acceptance are separate frozen documents and must not be marked PASS here. Do not start T1 unless all six M00 contract documents are present.

---

## 1. Dependency

~~~text
T1  As-Is Baseline Audit
├──────────────┬───────────────┐
↓              ↓               ↓
T2 Evidence    T4 Config       T5 Eval preparation
Contract       Contract
↓                              ↑
T3 SQLite POC ─────────────────┘
              |              /
              |             /
        T6 ADR Freeze
~~~

Acceptance ordering:

~~~text
T1 → T2
T2 → T3
T1 → T4
T1 + T2 → T5 acceptance
T2 + T3 + T4 + T5 → T6
~~~

---

## T1 — Current-Main As-Is Baseline and Ownership Audit

**Primary areas:** repository manifests, Runtime/Tauri/Tunnel lifecycle code, protocol/config, execution/policy/approval/audit/activity code, current tests, existing wiki only when confirmed by source.

**AC:** M00-AC01, M00-AC02, supports M00-AC11.

### Implement

Create:

~~~text
docs-codex/v4/milestones/M00-baseline-contract-gate/AS_IS_BASELINE.md
~~~

The document must contain a stable Fact ID table covering at least:

- workspace/package topology;
- Tauri Host lifecycle owner;
- HarnessController operation serialization;
- RuntimeSupervisor process/generation/identity;
- Tunnel process owner and status inputs;
- MCP transport and reachability boundary;
- ExecutionPipeline order;
- ExecutionPermit construction boundary;
- WorkspaceGuard;
- PolicyEngine;
- ApprovalManager;
- SessionGrantStore;
- AuditEvent production and AuditSink;
- ActivityStore/EventBus projection;
- current AppConfig schema = 2;
- existing v1→v2 migration;
- SecretReference/SecretStore boundary;
- ToolRegistry/ToolConfig/Policy defaults;
- desired/effective config boundary;
- current external ChatGPT/Tunnel baseline evidence if still applicable;
- absence/not-verification of V4 SQLite/Tray/notification/grep/full Eval.

Each fact records:

~~~text
Fact ID
claim
source path(s)
test path(s)
verification status
V4 dependency
known gap / note
~~~

Allowed status values only:

- IMPLEMENTED
- LOCAL_TESTED
- EXTERNALLY_ACCEPTED
- NOT_VERIFIED
- KNOWN_ISSUE
- NOT_IMPLEMENTED

Do not convert an old milestone ACCEPTED label into current EXTERNALLY_ACCEPTED without evidence that the exact behavior/current candidate is still covered.

### Source checks

At minimum inspect:

- Cargo.toml
- protocol/Cargo.toml
- runtime/Cargo.toml
- ui/src-tauri/Cargo.toml
- ui/src-tauri/src/harness.rs
- ui/src-tauri/src/runtime/*
- tunnel manager/manifest/discovery modules
- runtime/src/execution/*
- runtime/src/workspace/*
- runtime/src/policy/*
- runtime/src/approval*
- runtime/src/session_grant*
- runtime/src/audit/*
- runtime/src/activity/*
- runtime/src/mcp/*
- protocol/src/v1/config.rs
- ui/src-tauri/src/config/*

### Required task-local verification

Run focused existing tests required to substantiate the facts. At minimum:

~~~text
cargo check --workspace
cargo test -p harness-control-protocol
~~~

For Runtime/Tauri, run affected/focused suites sufficient to validate the documented lifecycle/config/audit claims; record exact commands and counts in AS_IS_BASELINE.md.

Do not run real ChatGPT E2E in this task. That belongs to M00 E2E execution and requires its own authorization.

### Done

- AS_IS_BASELINE.md exists and is source-backed.
- no architecture claim relies only on historical docs.
- contradictions with V4 Architecture are listed as explicit ADR issues.
- no production behavior changed.

---

## T2 — Shared Evidence Contract v1 and Safe Projection Proof

**Primary areas:** runtime/src/audit/* test modules, current Activity projection, M00 fixtures.  
**AC:** M00-AC03, M00-AC04, M00-AC05.  
**Upstream:** SEC-AC01/02/03, A04, A08, A17.

### Implement

Create a versioned M00 fixture contract, recommended path:

~~~text
docs-codex/v4/milestones/M00-baseline-contract-gate/fixtures/evidence/
├── README.md
├── success.json
├── policy-denied.json
├── approval.json
├── execution-failed.json
├── incomplete.json
└── unsafe-input-cases.json
~~~

The fixture schema must express:

- schema version;
- event_id;
- runtime_id;
- request_id;
- optional confirmed host_session_id;
- optional trace/span placeholders;
- approval_id where applicable;
- stage;
- outcome;
- failure category;
- policy decision/reason;
- bounded matched rule IDs;
- duration/count/size facts where available;
- coverage COMPLETE/INCOMPLETE/UNKNOWN;
- typed sanitized tool evidence.

### Real AuditEvent projection proof

Add test-only code adjacent to Runtime audit tests that consumes actual current AuditEvent values and produces/validates the M00 fixture shape.

Requirements:

- do not expose ExecutionPermit;
- do not add a persistence side effect to AuditSink;
- do not change current Activity UI semantics;
- do not add a generic arbitrary JSON passthrough;
- use explicit allowlist mapping;
- identify where runtime_id is available or where M03 must add confirmed mapping.

If current AuditEvent lacks one target field, the proof must mark it as a known contract gap instead of manufacturing a value.

### Sanitization tests

Deliberately attempt candidate evidence containing:

- physical path;
- raw file body;
- long rule ID;
- raw command/argv;
- raw environment;
- secret-looking value;
- unbounded error text.

Prove the safe fixture either excludes, redacts, hashes or bounds the value according to SPEC.

### Authorization-separation test

Add a compile-time/API-level or structural test proving the M00 evidence object cannot be converted into:

- ExecutionPermit;
- Pending Approval;
- Session Grant.

A historical approval fixture must remain data only.

### Required task-local verification

Run focused Runtime audit/activity tests, plus any fixture validator introduced.

Representative command family:

~~~text
cargo test -p chatgpt-harness-runtime audit
cargo test -p chatgpt-harness-runtime activity
~~~

Use exact filters that exist after implementation and record them.

### Done

- versioned safe evidence fixtures exist;
- real current AuditEvent → safe contract feasibility is executable;
- sensitive negative cases pass;
- identity gaps are explicit;
- no production persistence or authorization behavior changed.

---

## T3 — SQLite Single-Writer / Read-Only / Maintenance POC

**Primary area:** isolated POC only.  
**AC:** M00-AC06, M00-AC07, supports M00-AC05 and AC14.  
**Upstream:** A05, A06, A07, F07-AC03.

### Implement

Preferred structure:

~~~text
poc/v4-m00-evidence-store/
├── Cargo.toml
├── src/
└── tests/
~~~

The POC may use SQLite libraries suitable for the future implementation, but must not be imported by production chatgpt-harness-runtime or chatgpt-harness-ui.

Cargo workspace isolation is mandatory. If this POC is a Rust crate under the repository root, either declare an empty `[workspace]` in its Cargo.toml or add `poc/v4-m00-evidence-store` to root `workspace.exclude`. The chosen mechanism must be documented and verified so the POC is never an implicit production workspace member.

Use temporary directories in every automated test.

### POC cases

#### Writer ownership

- open writer A;
- establish explicit writer/maintenance ownership lock;
- open read-only reader;
- confirm reader sees committed events;
- attempt writer B and confirm deterministic ownership rejection.

Do not treat SQLite’s incidental timing/busy behavior as the sole ownership protocol; model the intended ownership contract explicitly.

#### WAL/read path

- enable/test WAL behavior;
- writer commits while read-only connection is present;
- reader refreshes and observes committed state;
- reader cannot write/migrate/clear.

#### Transaction atomicity

In one transaction:

~~~text
insert evidence_event
+ update request_summary
~~~

Inject failure before commit and prove neither half is visible.

#### Idempotency

Insert same event_id twice and prove one logical event.

#### Correlation

Insert multiple requests close in time and prove query/correlation uses runtime_id + request_id, not timestamp proximity.

#### Crash/incomplete

Persist received/policy/execution-started without terminal event, simulate abrupt writer stop, reopen, and report request as INCOMPLETE/UNKNOWN.

Never synthesize a failure/success just because the process disappeared.

#### Migration

Exercise:

- initial schema version;
- forward POC migration;
- checksum/version mismatch;
- injected migration failure;
- source DB remains diagnosable.

#### Offline maintenance

After writer shutdown:

- acquire exclusive maintenance lock;
- perform a bounded maintenance action;
- release;
- reopen writer and read-only reader consistently.

#### Authorization boundary

Persist approval/session-like historical facts and prove POC exposes no API that reconstructs live Runtime authorization objects.

### POC report

Create:

~~~text
docs-codex/v4/milestones/M00-baseline-contract-gate/SQLITE_POC_REPORT.md
~~~

Include library/version, pragmas, lock strategy, test matrix, commands, actual results, limitations and recommendation for M03.

### Required task-local verification

Primary command:

~~~text
cargo test --manifest-path poc/v4-m00-evidence-store/Cargo.toml
~~~

Also verify root production workspace does not depend on the POC.

### Done

- all required POC cases pass;
- POC only touches temp directories;
- SQLITE_POC_REPORT.md has reproducible evidence;
- POC is not a production dependency.

---

## T4 — AppConfig v2→v3 Compatibility Contract and Fixtures

**Primary areas:** protocol config types/fixtures, isolated migration-contract tests; production schema must remain v2.  
**AC:** M00-AC08, M00-AC09.  
**Upstream:** A15.

### Implement

Create versioned fixtures, recommended:

~~~text
docs-codex/v4/milestones/M00-baseline-contract-gate/fixtures/config/
├── v2-representative.json
├── v2-non-defaults.json
├── v2-secret-reference.json
├── v2-malformed.json
├── v4-candidate-v3.json
└── README.md
~~~

### v2 representative fields

Use non-trivial values for:

- configRevision;
- workspace;
- tunnelId;
- proxy;
- tunnel client path/autoDetect;
- apiKeyRef SecretReference;
- SecurityMode;
- ui.theme/ui.language;
- read/glob/write/edit/bash toggles;
- Policy defaults/user rule;
- Bash autoDetect/path.

No raw API key may appear.

### POC validator/migrator

Implement a test-only or isolated validator that proves:

- valid current v2 deserializes using current protocol types;
- candidate v3 representation can preserve all current values;
- SecretReference remains opaque;
- host-only future settings are not placed in RuntimeConfigSnapshot;
- malformed known fields fail;
- unsupported schema fails non-destructively;
- migration can be atomic/idempotent by design;
- config schema change does not require protocol/v2.

This task must not set:

~~~text
CURRENT_SCHEMA_VERSION = 3
~~~

and must not make ConfigStore write v3 in production.

### Decision output

Add the v2→v3 migration constraints to ADR_DECISIONS.md input notes for T6, including anything M01/M03/M07 must decide later such as exact defaults.

### Required task-local verification

At minimum:

~~~text
cargo test -p harness-control-protocol
cargo test -p chatgpt-harness-ui <focused config/migration tests>
~~~

If a separate POC manifest is introduced, run its full tests as well.

Add an assertion/test that CURRENT_SCHEMA_VERSION remains 2 at M00 completion.

### Done

- compatibility fixture matrix passes;
- production schema remains v2;
- no secret material enters fixtures/logs;
- protocol v1 remains unchanged except any strictly additive test-only support explicitly approved.

---

## T5 — Controlled Agent/Model Evaluation Feasibility Spike

**Primary area:** isolated evaluation spike and fixtures.  
**AC:** M00-AC10, supports M00-AC14.  
**Upstream:** F12-AC01, F12-AC03, F12-AC04, A10.

### Implement

Create an isolated evaluation spike, recommended:

~~~text
evaluation/m00-poc/
├── README.md
├── cases/
├── fixtures/
├── runner/
└── reports/
~~~

Exact language/tooling may follow repository conventions, but the spike must call the real Harness Runtime MCP rather than reimplementing tool behavior. If the spike uses Rust under the repository root, it must use the same explicit Cargo workspace-isolation rule as T3.

### Adapter

Define a small adapter contract supporting:

- pinned model identity;
- system/task input;
- tool schema exposure;
- tool-call extraction;
- returning real MCP tool results to the model;
- stop reason/final response;
- max step/time/spend enforcement.

Implement at least one concrete controllable Agent/Model adapter that can actually run.

Do not use passive ChatGPT production traffic as the controlled adapter.

### Ground-truth smoke case

Create at least one case with:

- stable case ID/version;
- isolated Workspace fixture;
- known file/state;
- a task where an existing read-only tool is appropriate;
- allowed/expected tool set;
- argument constraints;
- expected final observable result/state;
- deterministic grader.

### Runner

The runner must:

1. start or attach to an isolated test Runtime using the production MCP server path;
2. expose the current real tool schema;
3. let the concrete model emit actual tool calls;
4. forward calls through MCP;
5. collect Runtime outcome/evidence;
6. continue the controlled agent loop within limits;
7. grade final task state;
8. emit a report with model/adapter/tool-schema/case/fixture identities.

### Scoring fields for the spike

Record separately:

- selected tool allowed/correct for the case;
- arguments schema-valid;
- MCP/Runtime call outcome;
- task final-state pass/fail;
- call count;
- termination reason.

Do not report a full V4 quality score yet; M04 owns dataset scale, final metrics and thresholds.

### Paid/networked execution rule

Implementation/unit tests may mock the provider boundary only to test mechanics.

M00 acceptance requires a real concrete adapter execution. Before making any paid/networked model request, obtain explicit user authorization as required by the Roadmap.

If credentials/access are unavailable:

- do not downgrade to mock-only PASS;
- mark the corresponding acceptance evidence BLOCKED.

### Outputs

Create:

~~~text
docs-codex/v4/milestones/M00-baseline-contract-gate/EVAL_FEASIBILITY_REPORT.md
~~~

Record:

- adapter/provider/model/version;
- runner version/commit;
- case hash;
- fixture hash;
- tool schema identity;
- execution limits;
- actual tool calls;
- independent final-state result;
- cost/usage information only if actually available;
- known nondeterminism;
- recommendation for M04.

### Required task-local verification

Provide local deterministic tests for:

- case parsing;
- fixture hashing;
- step/time limits;
- tool-call forwarding;
- schema validation;
- scorer positive/negative fixtures;
- cleanup.

The real model smoke execution itself belongs in the formal M00 Integration/E2E evidence as defined later, not silently inside an unauthorized development task.

### Done

- concrete adapter path exists;
- runner mechanics pass locally;
- real smoke run is executable and has a defined authorization path;
- no production Runtime Agent loop has been added;
- no secret is committed.

---

## T6 — ADR Decisions, POC Isolation Audit and Development Completion

**Primary areas:** M00 milestone docs, repository dependency checks, affected upstream docs only if an ADR must be revised.  
**AC:** M00-AC12, M00-AC13, M00-AC14; closes T1–T5.

### Implement

Create:

~~~text
docs-codex/v4/milestones/M00-baseline-contract-gate/ADR_DECISIONS.md
~~~

Cover at least A01, A04, A05, A06, A07, A08, A10, A15, A17.

For each ADR:

~~~text
ADR ID
planning-baseline decision
M00 evidence
verdict: ACCEPTED / REVISED / BLOCKED
implementation constraints
open risk
downstream owner
~~~

### Upstream revision rule

If any verdict is REVISED:

- record `UPSTREAM_DECISION_REQUIRED` with the reason, evidence and affected PRD/Architecture/Roadmap decision;
- stop the affected T6 acceptance path;
- do not edit V4 PRD/Architecture/Roadmap from this implementation task merely to make M00 pass;
- wait for a separate explicitly approved upstream revision, then re-freeze the affected M00 contract documents and re-run the impacted verification;
- do not leave SPEC/PLAN/TASKS contradicting the approved upstream baseline.

If BLOCKED, do not relabel it ACCEPTED to let M01 proceed.

### Isolation audit

Prove:

- root workspace production crates do not depend on M00 SQLite POC;
- production Runtime/Tauri do not invoke Eval spike;
- no real AppData SQLite file is created;
- CURRENT_SCHEMA_VERSION is still 2;
- no new user-facing tool is registered;
- no Policy/Approval/Workspace semantics changed;
- no raw secret was committed into fixture/report;
- temporary DB/Workspace/process resources are cleaned.

### Documentation consistency

Update cross-links among:

- SPEC.md
- PLAN.md
- TASKS.md
- INTEGRATION_TEST_PLAN.md
- E2E_TEST_PLAN.md
- MANUAL_ACCEPTANCE_CHECKLIST.md
- AS_IS_BASELINE.md
- SQLITE_POC_REPORT.md
- EVAL_FEASIBILITY_REPORT.md
- ADR_DECISIONS.md
- future ACCEPTANCE.md and EVIDENCE_INDEX.md references.

The three validation plans are already frozen before implementation. Do not rewrite their acceptance criteria to fit the implementation and do not mark them PASS merely because M00 development is complete.

### Required task-local verification

Run the minimum full affected regression needed to confirm the POCs/test-only changes have not altered production behavior.

Expected baseline:

~~~text
cargo check --workspace
cargo test -p harness-control-protocol
cargo test -p chatgpt-harness-runtime <affected suites>
cargo test -p chatgpt-harness-ui <affected suites>
cargo test --manifest-path poc/v4-m00-evidence-store/Cargo.toml
~~~

Run eval local tests with the exact command established in T5.

Also perform:

- dependency inspection;
- git diff --check;
- repository cleanliness/resource cleanup check.

Record exact commands and results in ADR_DECISIONS.md or a development evidence section.

### Done

M00 Development Complete means:

- T1–T6 are complete;
- task-local verification passes;
- all required design outputs exist;
- no production V4 feature was accidentally implemented/enabled;
- no unresolved REVISED ADR remains only in milestone docs;
- formal Integration/E2E/Manual test execution is still pending.

Do not mark M00 ACCEPTED/FROZEN in TASKS. That verdict requires the later validation documents and actual execution evidence.

---

## 2. Task → AC → Reserved Test Traceability

| Task | M00 AC | Reserved downstream tests |
| --- | --- | --- |
| T1 | AC01, AC02, AC11 support | M00-INT-01/02, M00-E2E-01, M00-MAN-01 |
| T2 | AC03, AC04, AC05 | M00-INT-03/04/05, M00-E2E-01, M00-MAN-02 |
| T3 | AC06, AC07 | M00-INT-06/07/08, M00-MAN-03 |
| T4 | AC08, AC09 | M00-INT-09/10, M00-MAN-03 |
| T5 | AC10 | M00-INT-11, M00-E2E-02, M00-MAN-04 |
| T6 | AC12, AC13, AC14 | M00-INT-12, M00-E2E-03, M00-MAN-05 |

---

## 3. M00 Development Handoff

After T6, the next action is not M01 implementation.

The required sequence is:

~~~text
Pre-development: SPEC / PLAN / TASKS / Integration / E2E / Manual plans frozen
→ T1–T6 M00 Development Complete
→ independent QA executes Integration
→ Local/controlled E2E + explicitly authorized external E2E
→ Manual Acceptance
→ ACCEPTANCE + EVIDENCE_INDEX
→ M00 ACCEPTED / FROZEN
→ only then unlock M01/M02/M03
~~~

This preserves the Roadmap gate that M00 is an executable architecture contract, not only a documentation milestone.
