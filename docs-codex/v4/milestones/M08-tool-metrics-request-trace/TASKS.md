# M08 TASKS — Tool Metrics and MCP Tool Request Trace

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: IMPLEMENTATION BLOCKED UNTIL M03 ACCEPTED / FROZEN AND M08 VALIDATION DOCS ARE FROZEN  
> Scope: P1 F08 + F14 only  
> Contract rule: extend M03 canonical Safe Evidence and SQLite; do not create a second telemetry truth, second writer, new authorization path, F09 Token metrics, hidden-model trace, or post-hoc metric semantics.  
> External rule: do not run real External ChatGPT, destructive migration/rollback against user data, or restart the user's active instance without explicit execution-time authorization.

---

# 1. Dependency

~~~text
M03 ACCEPTED / FROZEN
        ↓
M08 six docs frozen
        ↓
T1 Evidence Store v2 + protocol DTOs
        ↓
T2 Request-local Trace capture/persistence
        ↓
T3 Metrics query engine + performance
        ↓
T4 Tauri facade + Metrics/Trace UI
        ↓
T5 Migration/degradation/privacy/M03 regression
        ↓
T6 Full development regression + delivery
        ↓
Integration → E2E → Manual
        ↓
M08 — ACCEPTED / FROZEN
~~~

M07 Development Complete is not an M08 dependency.

---

# T1 — Evidence Store v2, Additive Protocol DTOs, Migration and Rollback

**AC:** M08-AC01, AC02, AC17, AC21  
**Primary areas:** protocol/src/v1/, protocol/tests/, runtime/evidence-store/src/.

## 1. Add protocol-v1 DTOs

Define closed typed DTOs for:

- MetricsQueryParams;
- MetricsQueryResult;
- MetricsCounts;
- DurationMetric;
- MetricsCoverage;
- MetricsSampleStatus;
- TraceGetParams;
- TraceGetResult;
- RequestTraceSpan;
- TraceAvailability;
- Trace coverage/status enums as required.

Requirements:

- deny unknown fields where consistent with current protocol;
- bounded string/enums;
- no arbitrary JSON payload;
- regenerate TS/JSON schema;
- do not create protocol/v2.

## 2. Keep AppConfig unchanged

Assert:

~~~text
CURRENT_SCHEMA_VERSION == 3
~~~

No M08 AppConfig field is required.

## 3. Create physical Evidence Store schema v2

Preserve all v1 tables/columns.

Add:

~~~text
request_summary.trace_id TEXT NULL
request_spans(...)
~~~

with the exact fields/constraints/indexes from SPEC.

Update:

- SCHEMA_VERSION;
- deterministic schema checksum logic;
- schema-shape verification;
- migration fixtures.

## 4. Implement transactional v1→v2

Required sequence:

1. verify v1 checksum/shape;
2. acquire exclusive writer ownership;
3. make consistent sibling backup;
4. begin migration transaction;
5. alter request_summary;
6. create request_spans/indexes;
7. write migration row/checksum;
8. commit;
9. reopen/verify v2 shape.

Failure must:

- preserve original/backup;
- return typed migration/store error;
- never drop/recreate.

## 5. Legacy handling

Verify migration does NOT:

- assign fake trace_id to v1 rows;
- create fake request_spans;
- reinterpret existing duration_ms as executor duration.

Old History must remain readable.

## 6. Read compatibility

Add reader behavior for:

- valid v1 History;
- healthy v2;
- corrupt;
- future incompatible.

Tauri/read-only access must not auto-migrate v1.

## 7. Rollback fixture

Test:

~~~text
v1 fixture
→ migrate v2
→ preserve v2 DB
→ restore verified v1 backup
→ read with v1-era compatible reader
~~~

No destructive SQL downgrade.

## 8. Retention/clear schema linkage

Ensure request_spans uses request-family cascade.

### Verification

Run targeted:

~~~text
cargo test -p harness-evidence-store
cargo test -p harness-control-protocol
~~~

Use actual package names if workspace naming differs.

### Done

One safe versioned evidence database supports both legacy History and M08-native Trace.

---

# T2 — Runtime Request-local Trace Timing and Persistence

**AC:** M08-AC03, AC07, AC11, AC12, AC13, AC14, AC15, AC16  
**Depends on:** T1  
**Primary areas:** runtime/src/execution/, runtime/src/audit/, runtime/src/evidence/, runtime/evidence-store writer.

## 1. Add internal trace state

Extend RequestContext or adjacent request-owned state with:

- local trace_id;
- monotonic root start;
- bounded span sequence;
- optional active approval span;
- optional active execution span;
- trace coverage/capacity state.

Do not expose Instant through protocol.

## 2. Enforce limits

Constants:

~~~text
MAX_SPANS_PER_REQUEST = 16
MAX_ACTIVE_TRACE_REQUESTS = 1024
~~~

Capacity failure degrades trace only.

Tool execution/Policy must continue unchanged.

## 3. Generate root trace

At the earliest well-formed Tool request point with Harness request_id:

- generate one trace_id;
- open request_wall root;
- persist/associate trace_id with request_summary.

No second trace_id for retries of the same logical Runtime request.

## 4. Make Tool resolution observable

Add the minimum canonical observation needed for:

- Tool/capability resolution success;
- unknown/disabled/unavailable resolution failure.

Do not trace malformed MCP envelope before request_id.

Do not add Policy semantics to resolver.

## 5. Workspace / Policy nodes

Project from existing canonical pipeline stages.

Do not call guard/policy again.

## 6. Approval wait span

Open on ApprovalRequired.

Close on:

- allow;
- deny;
- expired;
- cancelled.

Do not open for SessionGrant hit.

## 7. Execution span

Open only on ExecutionStarted.

Close on:

- ExecutionCompleted;
- ordinary ExecutionFailed;
- cancel;
- timeout.

Pre-execution failure/deny gets no execution span.

## 8. Root close

Close root only on observed terminal request truth.

Preserve current EvidenceEvent terminal duration_ms semantics as wall elapsed.

Do not overwrite it with execution duration.

## 9. Crash/incomplete

At controlled writer/runtime termination:

- open spans remain incomplete;
- endedAt/duration remain null;
- no next-start repair guesses terminal time;
- executorStarted is true only if ExecutionStarted exists.

## 10. Span persistence

Use the existing Runtime evidence writer ownership/queue.

No second SQLite writer.

Ensure idempotency cannot duplicate logical span sequence.

## 11. Privacy tests

Assert serialized/persisted Trace contains no:

- raw secret;
- raw env;
- argv;
- raw bash command;
- stdout/stderr;
- file body;
- physical path;
- raw grep query;
- grep preview;
- hidden reasoning.

### Done

F14 trace facts are captured from the existing Runtime pipeline with monotonic, bounded, non-authoritative timing.

---

# T3 — Tool Metrics Query Engine, Statistical Semantics and Performance Gate

**AC:** M08-AC04, AC05, AC06, AC07, AC08, AC09, AC10, AC18  
**Depends on:** T1/T2  
**Primary areas:** runtime/evidence-store query modules and tests/bench harness.

## 1. Implement bounded query validation

Support:

- fromUtc inclusive;
- toUtc exclusive;
- exact Tool optional.

Enforce:

~~~text
MAX_METRICS_RANGE_DAYS = 30
MAX_METRICS_TOOL_FILTER_BYTES = 128
METRICS_QUERY_TIMEOUT = 2s
~~~

Reject invalid range; do not widen.

## 2. Implement request classifier

For each retained request classify the frozen independent counts:

- receivedCalls;
- executionAttempts;
- successfulCalls;
- failedCalls;
- deniedCalls;
- cancelledCalls;
- timedOutCalls;
- preExecutionFailedCalls;
- approvalExpiredCalls;
- approvalCancelledCalls;
- incompleteCalls;
- unknownCalls;
- legacyUntracedCalls.

Add golden tests for every Request Trace truth-table row.

Prevent mutually exclusive terminal double counting.

## 3. Implement success rate

Return:

~~~text
completedExecutionDenominator
successfulCalls
executionSuccessRate
sampleStatus
~~~

Formula exactly:

~~~text
successful / (successful + failed)
~~~

Exclude DENY/cancel/timeout/pre-exec/approval terminal/incomplete/unknown.

## 4. Implement duration families

Use request_spans only for:

- execution;
- approval_wait;
- request_wall.

Never use old v1 request_summary.duration_ms as execution duration.

## 5. Implement averages

Calculate in Rust/DAL using valid samples.

No raw duration arrays returned to UI.

## 6. Implement nearest-rank P95

Freeze:

~~~text
MIN_P95_SAMPLES = 20
rank = ceil(0.95 * N)
~~~

Expected:

- 0 -> NO_DATA / null;
- 1..19 -> INSUFFICIENT / null;
- >=20 -> SUFFICIENT / numeric P95.

Add exact hand-computable boundary tests for N=0,1,19,20,21 and non-sorted inputs.

## 7. Coverage output

Return:

- complete/incomplete/unknown request counts;
- legacy count;
- duration-eligible sample counts;
- evidence store state;
- last commit;
- drop count;
- coverageUnknownSince.

Coverage warning and sample status remain separate.

## 8. Store degradation

Test Metrics behavior for:

- disabled/missing;
- v1 legacy;
- busy;
- drop;
- full/degraded;
- corrupt;
- incompatible.

No query path changes Runtime Policy.

## 9. Performance fixture

Create disposable deterministic fixture:

~~~text
50k request_summary
250k evidence_events
200k request_spans
~~~

Run 20 warm query variants.

Gate:

~~~text
P95 <= 750ms
0 timeouts
~~~

Inspect SQLite query plan/index use.

### Done

F08 metrics are one deterministic server-side computation with frozen formulas and bounded performance.

---

# T4 — Tauri Read-only Facade and Activity/History Metrics/Trace UI

**AC:** M08-AC03, AC10, AC14, AC19, AC20  
**Depends on:** T1–T3  
**Primary areas:** ui/src-tauri/src/history.rs, commands, UI Activity/History, harness client/provider, i18n.

## 1. Add Tauri commands

Add:

~~~text
metrics_query(params)
trace_get(params)
~~~

Reuse the same evidence DB resolution and evidence-store DAL.

## 2. Read-only guarantee

While Runtime is active:

- open/query read-only;
- query_only true;
- no schema migration;
- no maintenance write;
- no Policy/Tool call.

Add command tests.

## 3. Error mapping

Map store/query failures into bounded existing/new ControlError codes without raw SQLite/physical path leakage.

## 4. Metrics UI

Add an Activity/History Metrics section/tab.

Filters:

- 24h default;
- 7d;
- 30d;
- Tool/All.

Render:

- received;
- attempts;
- success;
- failure;
- deny;
- cancel;
- timeout;
- incomplete/unknown;
- success numerator/denominator/percent/status;
- avg/P95 execution;
- approval wait;
- request wall;
- duration sample counts;
- coverage/store status.

Do not compute formulas in React.

## 5. Trace entry

From exact History request detail:

- call trace_get(runtimeId, requestId);
- never search by timestamp.

## 6. Trace timeline

Render ordered span_sequence with:

- stage;
- timing or unavailable;
- status/coverage;
- policy/approval context;
- executorStarted;
- failure stage;
- slowest measured span.

DENY must visibly show executor not started.

Legacy v1 request must visibly say Trace unavailable for legacy evidence.

## 7. UI truthfulness

Forbidden labels:

- Agent accuracy;
- Tool selection accuracy;
- ChatGPT reasoning;
- actual model trace;
- billed tokens.

## 8. Localization/privacy

Add en/zh-CN.

Run DOM sentinels against:

- secret;
- raw path;
- raw query;
- raw file body;
- raw argv/output.

### Done

Users can inspect Metrics/Trace without React becoming an analytics or authorization authority.

---

# T5 — Migration/Retention/Degradation/Crash and M03 Security Regression

**AC:** M08-AC01, AC15, AC16, AC17, AC18, AC21, AC22  
**Depends on:** T1–T4  
**Primary areas:** evidence-store migration/maintenance/query tests, Runtime integration, Tauri History regression.

## 1. Migration matrix

Test:

- clean v1 → v2;
- v1 with multiple Runtime sessions;
- v1 with terminal approvals;
- v1 with incomplete requests;
- migration interrupted/fails;
- bad checksum;
- corrupt shape;
- future schema.

Verify original/backup preservation.

## 2. Legacy UI/query behavior

Before Runtime migration:

- old History remains readable;
- Trace says legacy/unavailable;
- Metrics never treats legacy wall duration as execution span.

After migration:

- same old History rows remain;
- no invented spans.

## 3. Retention/clear

Verify request-family deletion removes:

- events;
- approvals;
- spans.

Metrics recomputes immediately.

No orphan span/stale aggregate.

## 4. Storage degradation

Inject existing M03 fixtures for:

- busy;
- full/capacity;
- queue overflow/drop;
- corrupt;
- incompatible.

Verify Metrics/Trace status/warnings are truthful.

Tool execution authority remains unchanged.

## 5. Crash traces

Exercise controlled crash while:

- before executor;
- awaiting approval;
- executing.

Verify INCOMPLETE/UNKNOWN and null unobserved duration.

Restart must not “finish” old trace.

## 6. M03 regression

Rerun formal affected tests for:

- exact runtime/request correlation;
- History query/filter/pagination;
- History get;
- Runtime writer ownership;
- Tauri read-only;
- offline compatibility/maintenance;
- retention;
- clear;
- migration/corruption;
- privacy;
- no Pending/Grant/Permit restoration.

## 7. V3 security regression

Representative:

- Workspace;
- Policy;
- Approval;
- SessionGrant;
- Tool execution;
- SecretStore.

Metrics/Trace must not generate or alter authorization.

## 8. Rollback

From migrated disposable v2:

- stop;
- preserve v2;
- restore pre-migration v1 backup;
- validate old-reader History;
- verify no user data deletion.

### Done

M08 survives storage/lifecycle failure without weakening M03 security or data truth.

---

# T6 — Full Development Regression, Package and Handoff

**AC:** M08-AC01 through M08-AC22  
**Depends on:** T1–T5

## 1. Rust checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
~~~

## 2. Protocol generation

Regenerate/verify protocol JSON Schema and TypeScript.

Assert:

- Control Protocol remains v1;
- AppConfig schema remains v3;
- Evidence Store physical schema is v2;
- no F09 token DTOs accidentally added.

## 3. UI checks

~~~text
cd ui
npm run typecheck
npm test
npm run build
~~~

## 4. Performance gate

Run M08 deterministic Metrics query benchmark.

Record:

- environment;
- fixture hash/counts;
- per-query timings;
- P95;
- timeout count.

Must satisfy frozen gate.

## 5. Layout/governance

Run:

- ui/scripts/check-repository-layout.ps1;
- ui/scripts/check-milestone-layout.ps1.

Do not add a top-level analytics/evidence directory.

## 6. Security/static audit

Search/review for:

- second SQLite writer/database;
- UI-calculated success/P95;
- timestamp fuzzy joins;
- raw payload persistence;
- “Agent accuracy” labels;
- F09 token fields;
- OpenTelemetry/provider-trace claims;
- Policy calls from Metrics/Trace.

## 7. Fresh Portable

Use existing delivery flow with milestone:

~~~text
M08-tool-metrics-request-trace
~~~

Do not overwrite prior candidates.

## 8. Development Complete output

Report in conversation:

~~~text
M08 Development Complete:
candidate SHA:
product version:
source dirty:
Portable path/hash:
UI hash:
Runtime hash:
Tunnel hash:
AppConfig schema:
Control protocol revision:
Evidence Store schema/checksum:
v1→v2 migration:
rollback dev test:
Metrics formulas:
P95/sample gate:
query performance P95:
Trace stage coverage:
legacy behavior:
crash/incomplete behavior:
privacy regression:
M03 regression:
V3/security regression:
External ChatGPT: NOT EXECUTED
Formal Integration: NOT EXECUTED
Formal E2E: NOT EXECUTED
Manual: NOT EXECUTED
~~~

Do not mark M08 ACCEPTED / FROZEN during Development Complete.

### Done

M08 candidate is ready for independent formal validation.

---

# 2. Task → AC → Reserved Validation Traceability

| Task | M08 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01/02/17/21 | M08-INT-01/02/03/15, M08-E2E-01/04, M08-MAN-01/04 |
| T2 | AC03/07/11/12/13/14/15/16 | M08-INT-04/05/06/13/16, M08-E2E-03, M08-MAN-03/04 |
| T3 | AC04/05/06/07/08/09/10/18 | M08-INT-07/08/09/10/14/15, M08-E2E-02/04, M08-MAN-02/04 |
| T4 | AC03/10/14/19/20 | M08-INT-11/12/17, M08-E2E-01/02/03, M08-MAN-02/03/05 |
| T5 | AC01/15/16/17/18/21/22 | M08-INT-01/13/15/16/18, M08-E2E-01/03/04, M08-MAN-01/04/05 |
| T6 | AC01–AC22 regression closure | all formal suites after Development Complete |

---

# 3. Development Handoff

After T6:

~~~text
M08 Development Complete
→ independent M08 Integration
→ packaged automated E2E
→ real External ChatGPT Manual Acceptance
→ user sign-off
→ M08 — ACCEPTED / FROZEN
→ contributes to M11 V4 Full Gate
~~~

M08 does not deliver F09 and does not by itself produce V4 — ACCEPTED / FROZEN.
