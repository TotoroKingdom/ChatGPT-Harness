# M08 PLAN — Tool Metrics and MCP Tool Request Trace

> Input: SPEC.md  
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M03 ACCEPTED / FROZEN AND M08 VALIDATION DOCS ARE FROZEN  
> Scope: P1 F08 + F14 only  
> Primary owner: Runtime canonical evidence + runtime/evidence-store + Tauri read-only observability facade + existing Activity/History UI  
> Data rule: one canonical AuditEvent/Safe Evidence truth; no second telemetry database or authorization path  
> External rule: real External ChatGPT, destructive migration/rollback on non-disposable user data, and restarting the user's active instance require explicit execution-time authorization.

---

# 1. Delivery Strategy

M08 is a data-contract and observability milestone built on M03.

~~~text
M03 ACCEPTED / FROZEN
        ↓
M08 six docs frozen
        ↓
T1 Evidence Store v2 + protocol DTOs + migration/rollback
        ↓
T2 Runtime request-local Trace instrumentation/projector
        ↓
T3 Metrics query engine + formulas + P95 + performance
        ↓
T4 Tauri read-only facade + Activity/History UI
        ↓
T5 degradation/privacy/retention/M03 regression
        ↓
T6 full development regression + Portable handoff
        ↓
Formal Integration
        ↓
Automated packaged E2E
        ↓
Manual real-ChatGPT acceptance
        ↓
M08 — ACCEPTED / FROZEN
~~~

The plan deliberately separates evidence capture from analytics presentation. A chart or timeline is not evidence authority.

---

# 2. Implementation Principles

1. Extend M03; do not replace it.
2. Preserve runtime_id + request_id as the canonical cross-surface key.
3. Keep Runtime as normal-running SQLite writer.
4. Keep Tauri Metrics/Trace query paths read-only.
5. Preserve existing M03 terminal durationMs semantics as request wall elapsed time.
6. Add executor/approval timings only through request-local spans.
7. Compute Metrics in the evidence-store query layer, not React.
8. Use exact frozen denominator/category rules from SPEC.
9. Use nearest-rank P95 from raw valid durations.
10. Migrate v1→v2 transactionally with backup and tested rollback.
11. Never fabricate trace spans for legacy v1 requests.
12. Telemetry failure never becomes an execution/security gate.
13. Do not implement F09 Token estimation in M08.
14. Do not market passive Runtime telemetry as F12 Agent accuracy.

---

# 3. Expected Landing Zones

Use existing modules only:

~~~text
protocol/src/v1/
protocol/tests/

runtime/src/audit/
runtime/src/evidence/
runtime/src/execution/
runtime/evidence-store/src/
runtime/tests/

ui/src-tauri/src/history.rs
ui/src-tauri/src/commands/
ui/src-tauri/ existing tests

ui/src/pages/activity/
ui/src/app/router/
ui/src/components/
ui/src/services/harness/
ui/src/i18n/
~~~

Likely internal additions may include files such as:

~~~text
runtime/src/evidence/trace.rs
runtime/evidence-store/src/metrics.rs
runtime/evidence-store/src/trace.rs
~~~

Names may follow current module organization.

No new top-level directory.

No separate analytics service/database.

---

# 4. Phase A — Protocol DTOs and Evidence Store v2

## 4.1 Add additive protocol-v1 DTOs

Add closed typed DTOs for:

- MetricsQueryParams;
- MetricsQueryResult;
- MetricsCounts;
- DurationMetric;
- MetricsCoverage;
- MetricsSampleStatus;
- TraceGetParams;
- TraceGetResult;
- RequestTraceSpan;
- TraceAvailability / TraceCoverage;
- any bounded enum required for span name/status.

Run schema/TypeScript generation and contract tests.

Do not create protocol/v2.

## 4.2 Freeze physical schema v2

Change only the evidence-store physical schema version from 1 to 2.

Preserve all v1 tables and columns.

Add:

- request_summary.trace_id nullable;
- request_spans table;
- required indexes;
- v2 migration checksum.

## 4.3 v1→v2 migration

Implement:

1. validate v1 schema/checksum;
2. obtain writer ownership;
3. create consistent sibling backup;
4. transactionally alter request_summary/add request_spans/indexes;
5. add schema_migrations v2 row/checksum;
6. verify final v2 shape/checksum;
7. preserve old DB/backup on failure.

Do not synthesize trace rows for old requests.

## 4.4 Compatible read semantics

A v2-capable reader must distinguish:

- v2 healthy;
- v1 readable but M08 trace unavailable/migration required;
- future incompatible schema;
- corrupt schema.

Existing History list/detail must remain available for valid v1 data where the current read-only compatibility layer can safely support it.

## 4.5 Rollback fixture

Create test-owned v1 fixture → migrate to v2 → preserve v2 → restore verified v1 backup → open with v1-compatible reader.

No destructive downgrade SQL.

Exit: physical migration and recovery are safe before trace instrumentation starts.

---

# 5. Phase B — Runtime Request-local Trace Instrumentation

## 5.1 Internal request trace state

Extend RequestContext or adjacent Runtime-owned state with bounded internal trace timing state:

- trace_id;
- root start Instant;
- stage sequence;
- optional active approval span;
- optional active execution span;
- bounded span count/coverage state.

This internal state is not serialized as raw runtime internals.

## 5.2 Timing source

Use Runtime monotonic Instant for durations/offsets.

Use UTC only for display start/end timestamps.

Never subtract UTC to produce authoritative duration.

## 5.3 Tool resolution observation

Ensure a well-formed tools/call request receiving a Harness request_id produces a truthful tool_resolution node/span before or during normal registry/capability resolution.

Do not extend F14 to malformed MCP envelopes without request_id.

## 5.4 Canonical event projection

TraceProjector consumes the same canonical pipeline/AuditEvent semantics.

Map:

- RequestReceived;
- WorkspaceValidated;
- PolicyEvaluated;
- ApprovalRequired/Resolved/Expired/Cancelled;
- SessionGrantMatched;
- ExecutionStarted;
- ExecutionCompleted/Failed;
- RequestDenied.

Do not re-run WorkspaceGuard or PolicyEngine.

## 5.5 Root span

Create request_wall at request start.

Close only on observed terminal request outcome.

Crash/open requests remain incomplete.

## 5.6 Approval span

Open on ApprovalRequired.

Close on resolve/deny/expire/cancel.

SessionGrant does not produce fake approval wait.

## 5.7 Execution span

Open only when ExecutionStarted is observed.

Close on success/failure/cancel/timeout.

No execution span on DENY/pre-execution failure.

## 5.8 Capacity handling

Enforce:

- MAX_SPANS_PER_REQUEST = 16;
- MAX_ACTIVE_TRACE_REQUESTS = 1024.

When exceeded:

- degrade trace coverage;
- increment/record observable telemetry degradation;
- continue Tool execution under normal Policy.

Exit: Runtime produces truthful bounded request-local span mutations without becoming a new authority.

---

# 6. Phase C — Evidence Writer and Span Persistence

## 6.1 Writer integration

Extend the existing Runtime evidence writer queue/batching transaction so event + request summary + trace/span mutation remain one coherent request family.

Do not create a second writer thread with independent ownership.

## 6.2 Idempotency

Span persistence must tolerate event retry/idempotent writer behavior.

Use stable per-request span sequence/identity semantics so a repeated event cannot create duplicate logical spans.

## 6.3 Open/incomplete spans

If terminal closure is absent:

- persist/open state safely when possible;
- reader returns null duration/end;
- never infer a terminal time during later startup.

## 6.4 Request summary trace_id

Set once for v2-native requests.

Do not retrofit random trace_id to legacy requests during migration.

## 6.5 Retention/clear

Ensure request_summary cascade removes request_spans.

Add tests proving no orphan spans and no metrics cache remains.

Exit: Trace data has the same durability and deletion semantics as M03 evidence.

---

# 7. Phase D — Metrics Query Engine

## 7.1 Query API

Implement in runtime/evidence-store using read-only connection semantics:

~~~text
metrics_query(fromUtc, toUtc, tool?)
~~~

Validate:

- from < to;
- <= 30 days;
- tool <= 128 bytes;
- exact Tool match only.

No silent range widening.

## 7.2 Classification

Implement one deterministic classifier per request according to SPEC:

- received;
- execution attempt;
- success;
- ordinary failure;
- denied;
- cancelled;
- timed out;
- pre-execution failure;
- approval expired;
- approval cancelled;
- incomplete;
- unknown;
- legacy untraced.

Mutually exclusive terminal categories must be tested with golden fixtures.

## 7.3 Success denominator

Compute:

~~~text
completedExecutionDenominator = successfulCalls + failedCalls
executionSuccessRate = successfulCalls / completedExecutionDenominator
~~~

Return numerator, denominator and sample status.

Do not include deny/cancel/timeout.

## 7.4 Duration families

Query valid complete spans separately:

- execution;
- approval_wait;
- request_wall.

Apply eligibility rules exactly.

## 7.5 Average/P95

Implement in Rust/DAL:

- average;
- nearest-rank P95;
- MIN_P95_SAMPLES=20;
- NO_DATA / INSUFFICIENT / SUFFICIENT.

Do not send raw samples to React.

## 7.6 Coverage

Return:

- complete/incomplete/unknown;
- legacy untraced;
- duration sample counts;
- store status/drop count/coverageUnknownSince.

Do not conflate statistical sufficiency and evidence completeness.

## 7.7 Performance indexes/query plan

Use existing indexes plus v2 span indexes.

Avoid N+1 per-request queries for aggregate Metrics.

Review EXPLAIN QUERY PLAN in tests/benchmark.

Exit: F08 formulas live in one typed query layer and are hand-verifiable.

---

# 8. Phase E — Query Performance Gate

Build a deterministic disposable SQLite fixture:

~~~text
50,000 request summaries
250,000 evidence events
200,000 request spans
~~~

Run 20 warm queries spanning:

- 24h all tools;
- 7d all tools;
- 30d all tools;
- representative exact Tool filters.

Measure query wall duration in the benchmark harness.

Frozen gate:

~~~text
P95 <= 750 ms
0 query timeouts
~~~

Use nearest-rank P95 for the benchmark itself.

Document the Windows test environment in execution evidence.

Do not tune by changing product semantics or excluding slow valid query variants after results are seen.

---

# 9. Phase F — Tauri Read-only Metrics/Trace Facade

Add Tauri commands:

~~~text
metrics_query
trace_get
~~~

Reuse the same app_data evidence DB resolution and shared evidence-store DAL used by History.

Rules:

- query_only connection while Runtime is running;
- no migration from a UI read;
- no PolicyEngine calls;
- no Tool execution;
- bounded typed errors;
- store status preserved.

Trace_get:

- exact runtime_id + request_id;
- no time fallback;
- return legacy unavailable if no v2 trace;
- max 16 spans and bounded supplemental events.

Exit: React receives already classified safe DTOs.

---

# 10. Phase G — Activity / History UI

## 10.1 Navigation

Reuse existing Activity/History surface.

Do not create a new application-level analytics subsystem.

An Activity subview/tab/section may host Metrics.

History request detail gains a Request Trace action/section.

## 10.2 Metrics presentation

Filters:

- 24h;
- 7d;
- 30d;
- exact Tool / All.

Show all core counts and categories.

Show execution success as numerator/denominator + percent.

Show sample status adjacent to average/P95.

Show store/coverage warnings prominently.

Do not use a success-only green dashboard that hides DENY/cancel/unknown.

## 10.3 Trace presentation

Render ordered span_sequence, not client timestamp sorting.

Show:

- request key;
- trace_id as local Harness trace;
- tool;
- outcome;
- Policy;
- Approval;
- executorStarted;
- spans/timing;
- failure stage;
- slowest measured span;
- coverage.

Missing timing = “not available”, never 0 ms.

Legacy v1 = explicit unavailable.

## 10.4 Privacy

Frontend receives no raw payload needed for charts/timeline.

Add DOM privacy sentinel tests.

## 10.5 Localization

Add en/zh-CN labels for formulas, categories, sample states, coverage and trace stages.

Exit: UI explains rather than obscures evidence limitations.

---

# 11. Phase H — Degradation, Migration and M03 Regression

## 11.1 Store state matrix

Cover:

- Disabled;
- missing;
- v1 migration required;
- Healthy v2;
- Busy;
- Full/capacity degraded;
- Queue overflow/drop;
- Corrupt;
- incompatible future schema;
- migration failed.

Metrics/Trace must be truthful under each.

## 11.2 Crash/incomplete

Create request then terminate Runtime at controlled stages:

- before execution;
- waiting approval;
- during execution.

Verify no guessed terminal/duration.

## 11.3 Retention/clear

Verify spans disappear with request family and Metrics recomputes retained truth immediately.

## 11.4 M03 regression

Rerun:

- History list/detail;
- filters;
- keyset pagination;
- Runtime writer ownership;
- Tauri offline/read-only;
- retention;
- clear;
- corruption/migration behavior;
- no authorization restoration;
- privacy sentinels.

## 11.5 V3 security smoke

Ensure no change to Workspace/Policy/Approval/SessionGrant/Tool execution semantics.

Exit: M08 is observability-only from an authority perspective.

---

# 12. External / E2E Handoff

Automated E2E later must generate real requests covering:

- ALLOW success;
- ASK allow;
- DENY;
- executor failure;
- timeout/cancel where safely automatable;
- incomplete/crash.

It then compares History, Metrics and Trace for exact same request identities.

Actual External ChatGPT remains Manual Acceptance per project validation boundary.

M08 does not need grep as a prerequisite; read/write or another controlled Tool can provide the formal M08 samples. M11 later covers full-candidate cross-feature grep Metrics/Trace.

---

# 13. Rollback Strategy

If M08 product code is rolled back:

- stop current Runtime/App;
- preserve v2 DB;
- restore the verified v1 backup for the older accepted M03-era binary;
- use old binary against v1;
- keep v2 copy for future recovery;
- do not attempt destructive SQL downgrade;
- do not copy trace spans into v1;
- do not delete secrets/config.

AppConfig remains v3 and requires no M08 config rollback.

---

# 14. Development Verification Commands

Minimum:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Also run:

- protocol schema/TS generation verification;
- evidence-store migration tests;
- M08 metrics performance benchmark;
- M03 regression suite;
- repository/milestone layout checks;
- fresh Portable build/delivery.

Development tests use disposable DB/Workspace/AppData.

---

# 15. Defect / Change Rules

If implementation reveals that a frozen M08 metric cannot be derived truthfully from current canonical events:

1. do not infer it in UI;
2. add the minimum canonical Runtime event/timing observation needed;
3. keep Policy/authorization semantics unchanged;
4. add negative tests for missing observation;
5. if product meaning would change, stop and request upstream SPEC/PRD revision.

Do not “solve” missing evidence by:

- timestamp nearest joins;
- raw log scraping;
- UI heuristics;
- reading hidden process internals;
- broad payload persistence.

---

# 16. Development Completion Gate

M08 Development Complete requires:

- T1–T6 complete;
- schema v2 migration/backup/rollback dev tests PASS;
- Trace span lifecycle/capacity/crash tests PASS;
- Metrics classification/formula/sample tests PASS;
- performance gate PASS;
- Tauri read-only commands PASS;
- UI/i18n/privacy tests PASS;
- M03 regression PASS;
- V3 security smoke PASS;
- fresh Portable candidate produced;
- formal Integration/E2E/Manual truthfully NOT EXECUTED unless actually run.

Development Complete is not M08 ACCEPTED / FROZEN.
