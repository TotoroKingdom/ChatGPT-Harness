# M00 PLAN — As-Is Baseline and Architecture Contract Gate

> Status: IMPLEMENTATION READY / VALIDATION FROZEN  
> Input: SPEC.md  
> Upstream: V4 PRD / Architecture / Roadmap, FROZEN FOR M00  
> Goal: turn architectural assumptions into executable evidence and a frozen implementation contract for M01+  
> Rule: M00 validates and isolates POCs; it does not prematurely implement M03 History or M04 full Evaluation.

---

## 1. Delivery Strategy

M00 starts only after SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md, E2E_TEST_PLAN.md and MANUAL_ACCEPTANCE_CHECKLIST.md are present and frozen. The six implementation phases then execute against those validation targets.

M00 is executed in six ordered phases:

~~~text
Phase A  Current-main baseline audit
   ↓
Phase B  Shared Evidence Contract proof
   ↓
Phase C  SQLite ownership/durability POC
   ↓
Phase D  AppConfig v2→v3 migration-contract POC
   ↓
Phase E  Controlled Agent Evaluation feasibility spike
   ↓
Phase F  ADR decision freeze and development completion
~~~

Phases C and D may proceed in parallel after Phase B only if they do not edit the same production modules. Phase E may prepare fixtures earlier, but its acceptance run must use the identity/evidence boundaries frozen by Phase B.

No phase may “solve” a POC failure by silently weakening the upstream Architecture. A failed assumption becomes a documented REVISED/BLOCKED decision.

---

## 2. Repository and Ownership Boundaries

### Production owners to inspect, not redesign

| Area | Current owner / landing zone | M00 rule |
| --- | --- | --- |
| lifecycle | ui/src-tauri/src/harness.rs, runtime supervisor, tunnel manager | audit ownership; do not add new lifecycle owner |
| Runtime authorization | runtime ExecutionPipeline / WorkspaceGuard / Policy / Approval / SessionGrant | read/verify only except test-only instrumentation |
| canonical audit | runtime/src/audit/* | test-only projection proof allowed |
| live Activity | runtime/src/activity/* + current control/UI projection | do not replace or persist in M00 |
| config | protocol/src/v1/config.rs + ui/src-tauri/src/config/* | current production schema remains v2 |
| protocol | protocol/src/v1/* | additive test fixtures only; no protocol/v2 |
| SQLite POC | isolated poc/v4-m00-evidence-store/ preferred | must not be production dependency |
| Eval feasibility | isolated evaluation/m00-poc/ or equivalent | must not become production Runtime Agent loop |
| M00 docs | docs-codex/v4/milestones/M00-baseline-contract-gate/ | authoritative milestone artifacts |

If implementation needs a different POC path, ADR_DECISIONS.md records the final path and isolation rationale.

---

## 3. Phase A — Current-Main Baseline Audit

### Purpose

Establish the exact source-of-truth state before any V4 implementation work.

### Work

Inspect and record:

- Cargo workspace members/package versions;
- Tauri Host lifecycle ownership and operation serialization;
- RuntimeSupervisor process generation/identity/control ownership;
- Tunnel lifecycle owner and health inputs;
- MCP transport and endpoint ownership;
- ExecutionPipeline ordering and ExecutionPermit construction boundary;
- WorkspaceGuard enforcement;
- PolicyEngine and rule precedence;
- ApprovalManager / SessionGrantStore ownership and expiry;
- AuditEvent generation and AuditSink fan-out;
- ActivityStore/event-bus projection;
- AppConfig v2 and existing v1→v2 migration;
- RuntimeConfigSnapshot and desired/effective boundaries;
- SecretReference/SecretStore boundary;
- current test suites and existing external evidence limitations.

### Output

Create AS_IS_BASELINE.md with:

- fact ID;
- source/test evidence path;
- current status;
- verification command where applicable;
- V4 dependency;
- known issue/gap.

### Exit

M00-AC01 and AC02 have enough evidence for later Integration/Manual plans. Any material mismatch with V4 Architecture is recorded immediately as an ADR issue.

---

## 4. Phase B — Shared Evidence Contract Proof

### Purpose

Prove A04/A08 and SEC-AC01/02/03 against the current Runtime event model without adding durable production storage.

### Design sequence

1. enumerate current AuditEvent/PipelineEvent stages;
2. define the M00 safe fixture schema matching SPEC §6;
3. map current runtime_id/request_id/event_id/approval IDs;
4. document missing host-session/supervisor-generation mappings;
5. create positive fixture(s) for success/deny/approval/failure;
6. create negative fixture(s) containing deliberately sensitive or malformed candidate values;
7. add test-only adapter/projection proof that only allowlisted typed facts cross into the fixture;
8. prove evidence objects expose no authorization constructor or replay capability.

### Production-change boundary

Preferred implementation is unit/test-only code adjacent to runtime audit tests plus versioned JSON fixtures in the M00 directory or a test fixture directory.

Do not:

- export private Runtime authorization types just for M00;
- add DB writes to AuditSink;
- change existing Activity output semantics;
- make new protocol APIs for the POC.

If an additive DTO is genuinely required to prove feasibility, it must be narrowly scoped and reviewed as an Architecture-contract change before use.

### Exit

M00-AC03/04/05 pass locally, and contract fixtures are stable enough for SQLite POC consumption.

---

## 5. Phase C — SQLite Ownership / Durability POC

### Purpose

Prove the storage topology before M03 designs production DDL and APIs.

### Isolation

Use a separate test/POC crate, preferably:

~~~text
poc/v4-m00-evidence-store/
~~~

The root Cargo workspace must not import it as a production dependency. It may be run with its own manifest. If it is a Rust crate under the repository root, either declare an empty `[workspace]` in the POC manifest or add the POC path to root `workspace.exclude`; nested POC execution must never become an implicit production workspace member.

### POC layers

#### C1 — schema bootstrap

Minimal test schema:

- schema_migrations;
- evidence_events;
- request_summary;
- evidence_health or equivalent POC health record.

This schema is disposable and not the frozen M03 production DDL.

#### C2 — writer/read boundary

Simulate:

- Runtime writer opens DB, enables required pragmas and owns writes;
- Tauri reader opens read-only;
- reader sees committed data;
- reader cannot migrate/delete/write;
- second writer/maintenance actor is rejected according to POC lock contract.

#### C3 — atomic request evidence

Persist an event and update request summary in one transaction.

Test rollback on forced error.

#### C4 — idempotency

Reinsert same event_id and prove deterministic dedupe/no double count.

#### C5 — crash/incomplete

Stop/abort before terminal event and reopen. The request must remain INCOMPLETE/UNKNOWN, never inferred as success.

#### C6 — migration failure

Exercise version/checksum mismatch and failed migration. Original DB remains diagnosable; no silent delete/recreate.

#### C7 — stopped-runtime maintenance

Release writer, acquire exclusive maintenance lock, perform bounded maintenance, then reopen writer and reader coherently.

### Exit

M00-AC06/07 pass, commands are reproducible, and no POC DB remains outside temp directories.

---

## 6. Phase D — AppConfig v2→v3 Migration Contract POC

### Purpose

Prove future compatibility without changing production CURRENT_SCHEMA_VERSION.

### Inputs

Use current AppConfig v2 structures and current ConfigStore migration behavior as the source truth.

### Fixture matrix

At minimum:

1. representative valid v2;
2. v2 with non-default Tool/Policy/Bash/UI values;
3. v2 with SecretReference;
4. malformed known field;
5. unsupported schema;
6. candidate v3 round-trip/idempotency representation.

### Contract checks

- every existing v2 user setting is preserved;
- raw secrets never appear;
- configRevision rule is explicit;
- host-only future fields remain out of RuntimeConfigSnapshot;
- history config cannot become authorization state;
- malformed present values fail rather than default silently;
- unsupported future schema is non-destructive;
- persistent config version remains independent of protocol version.

### Isolation

Prefer a POC/test crate or fixture validator that consumes harness-control-protocol types. Do not edit CURRENT_SCHEMA_VERSION to 3 in M00.

### Exit

M00-AC08/09 pass and the fixture contract is ready for the relevant downstream milestone SPECs.

---

## 7. Phase E — Controlled Agent Evaluation Feasibility

### Purpose

Remove the biggest F12 feasibility risk before M04 becomes a hard gate.

### E1 — Adapter interface

Define a provider-neutral conceptual adapter boundary:

~~~text
case + pinned model/adapter config
→ agent turn
→ tool-call request(s)
→ real MCP tool execution
→ tool result(s)
→ next agent turn
→ final answer / stop reason
~~~

The spike must have one concrete controllable adapter that actually runs.

### E2 — isolated Runtime

Use an isolated Workspace and test Runtime instance. Do not attach the spike to the user’s normal active Workspace.

Reuse existing Runtime/MCP startup patterns where practical instead of creating a second server implementation.

### E3 — versioned case

Create at least one deterministic task where ground truth is meaningful, for example a Workspace inspection task requiring an existing read-only tool.

Record:

- case version/hash;
- fixture hash;
- expected/allowed tool set;
- argument rules;
- final-state assertion;
- maximum calls/steps/time/spend.

### E4 — scorer

The scorer separately records:

- emitted tool choice;
- argument schema validity;
- Runtime call outcome;
- final task state.

It must not equate MCP success with task success.

### E5 — reproducibility

Pin model/adapter identity and execution settings. Record unavoidable nondeterminism and rerun policy.

### Authorization / blocker

Do not execute paid/networked calls without explicit user authorization at execution time. If no valid controllable adapter can be executed, record M00 BLOCKED rather than replacing it with mock-only success. If the evaluation spike is implemented as a Rust crate, apply the same explicit workspace-isolation rule as the SQLite POC.

### Exit

M00-AC10 has executable evidence and the M04 team has a concrete feasible adapter path.

---

## 8. Phase F — ADR Freeze / Handoff

### Work

Create ADR_DECISIONS.md.

For each relevant ADR record:

- ADR ID;
- upstream decision;
- evidence generated in M00;
- verdict: ACCEPTED / REVISED / BLOCKED;
- implementation constraints passed to downstream milestones;
- unresolved risk;
- owner/next milestone.

Also close:

- POC isolation review;
- no-production-dependency check;
- temp resource cleanup;
- local verification commands;
- evidence index locations;
- documentation cross-links.

If any ADR is REVISED:

1. record `UPSTREAM_DECISION_REQUIRED` with the exact evidence and affected upstream decision;
2. stop the affected T6 acceptance path; the Coding Agent must not self-edit PRD/Architecture/Roadmap to make M00 pass;
3. perform the upstream revision as a separate explicitly approved architecture/product change;
4. re-freeze the affected M00 contract documents and re-run impacted verification;
5. only then may the ADR move to ACCEPTED.

### Exit

All M00 local development work is complete and ready for execution of the already-frozen Integration/E2E/Manual plans.

---

## 9. Task Ownership and Order

| Task | Primary ownership | Depends on | Principal output |
| --- | --- | --- | --- |
| T1 | docs + existing source/test inspection | upstream baseline | AS_IS_BASELINE.md |
| T2 | runtime audit/test fixtures | T1 | Shared Evidence Contract proof |
| T3 | isolated SQLite POC | T2 | ownership/durability proof |
| T4 | protocol/config fixture POC | T1 | v2→v3 migration contract |
| T5 | isolated evaluation spike | T1, T2 | real controlled Agent feasibility |
| T6 | docs/architecture freeze | T2–T5 | ADR_DECISIONS.md + development-complete evidence |

T3 and T4 may run in parallel. T5 may be prepared in parallel but cannot be accepted before T2 identity/evidence semantics are stable.

---

## 10. Rollback and Failure Policy

M00 production changes should be minimal. Therefore rollback is primarily deletion/revert of isolated POC/test artifacts.

Rules:

- POC crates must be removable without affecting production build;
- CURRENT_SCHEMA_VERSION remains 2, so config rollback is not needed;
- no user DB is created or migrated;
- no user Workspace is mutated by storage/config POCs;
- eval fixtures use isolated temporary Workspace and cleanup;
- if a test-only Runtime adapter change affects production compilation, revert that slice before trying a different POC approach;
- never “fix” an ADR failure by weakening a test expectation after observing the result.

A failed architecture assumption is a design result, not a test nuisance.

---

## 11. Verification Strategy Against Frozen Validation Plans

The Integration/E2E/Manual plans are authored and frozen before T1. TASKS still include only code-level/unit/POC verification; formal Integration/E2E/Manual execution occurs after T1–T6 Development Complete.

Expected command families:

~~~text
cargo check --workspace
cargo test -p harness-control-protocol <affected filters>
cargo test -p chatgpt-harness-runtime <affected filters>
cargo test -p chatgpt-harness-ui <affected filters>

cargo test --manifest-path poc/v4-m00-evidence-store/Cargo.toml
<config-contract POC command if a separate manifest is used>
<controlled-eval local/unit command>
~~~

Exact commands created by implementation are recorded in TASKS completion/evidence.

Formal component Integration, real ChatGPT E2E and Manual acceptance are defined in the already-frozen M00 validation documents. Implementation must satisfy those documents rather than regenerate them after coding.

---

## 12. Development Completion Gate

Before M00 can enter Integration testing:

- T1–T6 are done;
- all task-local tests pass;
- no production V4 feature has accidentally been enabled;
- production CURRENT_SCHEMA_VERSION is still 2;
- POC dependencies do not leak into production Runtime/Tauri;
- AS_IS_BASELINE.md is internally consistent;
- ADR_DECISIONS.md has no unhandled REVISED decision;
- eval feasibility has a runnable concrete adapter path, even though formal external execution may still await explicit authorization in E2E;
- git diff/checks show only intended files;
- cleanup has removed temporary DBs, test processes and fixture mutations.

Only then is M00 ready for its Integration/E2E/Manual acceptance sequence.
