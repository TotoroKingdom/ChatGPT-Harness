# M08 SPEC — Tool Metrics and MCP Tool Request Trace

> 布局与验证生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。启动阶段只创建 SPEC / PLAN / TASKS；后续补齐 Integration / E2E / Manual 三份验证计划，最终严格保持六文件布局。  
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M03 ACCEPTED / FROZEN AND M08 SIX DOCS ARE FROZEN  
> Milestone: M08 — Tool Metrics and MCP Tool Request Trace  
> Scope: P1 F08 + F14 only  
> Roadmap dependency: M03 Shared Evidence / SQLite History  
> Canonical ACs: F08-AC01 / F08-AC02 / F08-AC03 / F14-AC01 / F14-AC02 / F14-AC03 plus SEC-AC01/02/03 preservation  
> Architecture: A04–A09, A16, A17  
> Current main observed before this document: 3e400732dd2c14f668d2c1e1829efdc8488429a9  
> Current code fact: M03 Evidence Store physical schema is v1. It persists request_summary, evidence_events, approval_history and evidence_health; request_spans is explicitly absent/rejected. EvidenceEvent durationMs on terminal events currently represents whole request wall time from RequestContext.started_at, not executor duration.  
> Current M07 state supplied by user: Development Complete. M07 is not an M08 Roadmap dependency.

---

# 1. Objective

M08 turns the existing M03 safe evidence into two user-facing observability capabilities:

1. Tool Metrics — truthful, queryable counts/rates/durations derived from retained canonical evidence.
2. MCP Tool Request Trace — one request-local timeline showing the Runtime-observable stages of one MCP Tool request.

M08 must answer practical questions such as:

- How many Tool calls were received in this period?
- How many actually reached the executor?
- How many completed successfully vs failed, denied, cancelled or timed out?
- What is the average and P95 executor duration?
- How long did approvals wait?
- What is the full request wall time?
- For one request, where did it fail?
- Did the executor actually start?
- Was time spent in approval wait or execution?
- Which Runtime-observable stage was slow?

All answers must come from the same M03 canonical evidence identity. M08 cannot build a parallel log/analytics truth.

---

# 2. Canonical Product Contract

## F08-AC01

Metrics provide at least calls, success/failure, success rate, average execution duration, P95 execution duration and approval wait, queryable by Tool and time range.

## F08-AC02

Execution duration, approval wait and request wall time are separate. DENY, cancel, timeout and executor failure are not silently merged into one classification/denominator.

## F08-AC03

Sample size, INCOMPLETE/UNKNOWN coverage and P95 algorithm are transparent. Insufficient samples never appear as a stable statistical conclusion.

## F14-AC01

Each observable MCP Tool request correlates request received, Tool/capability resolution, Workspace, Policy, Approval when applicable, Execution when applicable, and terminal/incomplete state.

## F14-AC02

Trace correlates with Activity / Approval / History through stable request identity and explains failure stage, slow stage and whether executor started.

## F14-AC03

Trace does not fabricate ChatGPT hidden reasoning, unissued Tool choices, complete Agent Loop, provider spans or raw payloads.

## Shared Evidence preservation

### SEC-AC01

The same real Tool request must use stable exact identity across Activity, Approval, History, Metrics and Trace. Fuzzy time matching is not an acceptable primary join.

### SEC-AC02

Durable evidence remains typed, bounded and sanitized. Metrics/Trace do not create a second Policy or authorization truth.

### SEC-AC03

History/Trace evidence cannot restore Pending Approval, SessionGrant, ExecutionPermit or any other live authorization state.

All three Shared Evidence criteria remain mandatory. Metrics/Trace are observational projections only.

---

# 3. Current Baseline and Non-negotiable Facts

Current M03 data model already provides:

- runtime_id + request_id canonical request identity;
- event_id idempotency;
- typed EvidenceStage and HistoryOutcome;
- terminal/incomplete/unknown coverage;
- approval terminal evidence;
- request_summary.execution_started;
- terminal request durationMs;
- SQLite Runtime-only writer;
- Tauri read-only query path;
- retention/clear/degradation semantics.

Current terminal EvidenceEvent durationMs is whole-request elapsed time.

Therefore M08 MUST NOT relabel existing durationMs as executor duration.

Current physical schema v1 explicitly rejects request_spans. F14 durable trace therefore requires a versioned v1→v2 evidence-store migration.

---

# 4. Scope

M08 includes:

1. evidence-store physical schema v2 migration;
2. request-local trace identity and span persistence;
3. bounded Runtime trace projection from canonical AuditEvent;
4. exact Tool Metrics classification and formulas;
5. bounded Metrics query by time range and Tool;
6. Request Trace query by runtime_id + request_id;
7. migration/legacy/degraded coverage semantics;
8. Tauri read-only Metrics/Trace commands;
9. Activity/History UI integration for Metrics and Trace;
10. en/zh-CN localization;
11. retention/clear/corrupt/busy/full/drop behavior for spans;
12. query performance benchmark;
13. M03 privacy/authorization regression;
14. packaged Windows and real External ChatGPT acceptance.

---

# 5. Non-goals

M08 does not implement:

- F09 Token estimates;
- provider billed token usage;
- ChatGPT hidden reasoning;
- model Tool-selection candidates that were never emitted;
- complete Agent Loop Trace;
- OpenTelemetry exporter;
- W3C traceparent propagation;
- distributed trace across ChatGPT/provider infrastructure;
- cloud telemetry;
- remote analytics;
- new Policy/Approval authority;
- execution blocking when telemetry fails;
- a second SQLite database;
- materialized metrics that can drift from canonical request evidence;
- raw argv/env/stdout/stderr/file-body persistence;
- unbounded query/export;
- F12 Agent Tool-selection quality scoring;
- M10 MCP conformance.

---

# 6. Ownership and Data Flow

Frozen flow:

~~~text
Runtime MCP tools/call
→ ExecutionPipeline / Workspace / Policy / Approval / Executor
→ canonical AuditEvent
→ SafeEvidenceProjector
   → M03 History event
   → M08 TraceProjector span mutation
→ one Runtime-owned Evidence Store writer queue
→ one SQLite database
→ Tauri read-only evidence-store query facade
→ Activity / Metrics / Request Trace UI
~~~

Rules:

- AuditEvent remains the canonical semantic source.
- TraceProjector may add timing/shape projection but cannot re-run Policy or infer authorization.
- Metrics are queried from persisted canonical request summaries/events/spans.
- React never recalculates success denominator or P95 from raw rows.
- Tauri does not become a running-state writer.

---

# 7. Identity Contract

Primary request identity remains:

~~~text
(runtime_id, request_id)
~~~

M08 adds a local trace_id for user-facing trace grouping.

Rules:

- trace_id is generated once per request inside Runtime;
- trace_id is local Harness identity only;
- trace_id is not an upstream ChatGPT trace ID;
- trace_id is not a W3C traceparent;
- trace_id never replaces runtime_id + request_id as the canonical cross-surface key;
- Activity / Approval / History / Metrics / Trace cross-link by exact runtime_id + request_id;
- no timestamp-nearest/fuzzy correlation.

Span identity:

~~~text
span_id = local unique ID
parent_span_id = request root span or null
~~~

---

# 8. Evidence Store Physical Schema v2

AppConfig remains schema v3.

Control Protocol remains v1 with additive DTOs.

EvidenceEvent logical schema remains compatible with existing M03 History semantics.

The SQLite physical schema moves:

~~~text
SCHEMA_VERSION: 1 → 2
~~~

## 8.1 v2 migration additions

Existing v1 tables are preserved.

Add to request_summary:

~~~text
trace_id TEXT NULL
~~~

Add table:

~~~text
request_spans(
  span_id TEXT PRIMARY KEY,
  trace_id TEXT NOT NULL,
  parent_span_id TEXT NULL,
  runtime_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  span_sequence INTEGER NOT NULL,
  span_name TEXT NOT NULL,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT NULL,
  started_offset_ms INTEGER NOT NULL,
  duration_ms INTEGER NULL,
  status TEXT NOT NULL,
  failure_category TEXT NULL,
  coverage TEXT NOT NULL,

  UNIQUE(runtime_id, request_id, span_sequence),

  FOREIGN KEY(runtime_id, request_id)
    REFERENCES request_summary(runtime_id, request_id)
    ON DELETE CASCADE
)
~~~

Required indexes:

~~~text
(runtime_id, request_id, span_sequence)
(trace_id, span_sequence)
(span_name, duration_ms)
~~~

No metrics aggregate table is added in M08.

Metrics query request_summary + request_spans directly so retention/clear cannot leave stale aggregate truth.

## 8.2 v1 historical rows

Migration MUST NOT invent trace_id/spans for old requests.

For v1-origin requests:

- existing History remains readable;
- counts that can be proved from request_summary/events may still contribute;
- execution/approval/span durations that were never captured remain unavailable;
- Trace reports LEGACY_UNTRACED / NOT_AVAILABLE rather than reconstructing a fake timeline;
- coverage counters expose legacy/untraced samples.

## 8.3 Migration ownership

Normal automatic v1→v2 migration is owned by the Runtime writer path after exclusive writer ownership is acquired.

Tauri read-only History access must not silently mutate a v1 database merely to render UI.

Before migration, M08-capable Tauri may read compatible v1 History but Metrics/Trace report migration required/unavailable.

## 8.4 Migration safety

Before modifying v1:

- validate v1 checksum/shape;
- create a consistent sibling backup;
- run v2 migration transactionally;
- write v2 migration checksum;
- verify final shape/checksum;
- preserve original on failure.

No drop/recreate fallback.

Migration failure:

- does not change Policy truth;
- does not block Tool execution solely because observability is unavailable;
- produces evidence-store degraded/incompatible status;
- keeps old DB/backup for recovery.

---

# 9. Rollback Contract

A pre-M08 binary cannot safely assume it understands physical schema v2.

Rollback therefore is data-aware:

1. stop Runtime/App cleanly;
2. preserve current v2 DB without deleting it;
3. restore the verified pre-migration v1 backup for the older accepted binary;
4. start the older binary against v1;
5. never copy v2-only spans into v1 as guessed events;
6. never delete user History to make rollback pass.

M08 PLAN/TASKS must test this path with disposable data.

---

# 10. Runtime Trace Timing Contract

M08 adds safe timing projection to the existing request context/AuditEvent path.

Timing requirements:

- all duration arithmetic is Runtime-process monotonic;
- UTC timestamps are presentation/correlation timestamps, not duration subtraction authority;
- no cross-process Instant arithmetic;
- timing fields are scalar/bounded;
- no raw arguments/results are needed for timing.

The existing terminal request durationMs retains its M03 meaning: request wall elapsed time.

New stage timings are persisted only in request_spans.

---

# 11. Trace Span Model

Maximum spans per request:

~~~text
MAX_SPANS_PER_REQUEST = 16
~~~

Maximum active trace states:

~~~text
MAX_ACTIVE_TRACE_REQUESTS = 1024
~~~

If trace state capacity is exhausted:

- Tool execution continues under normal authority;
- trace coverage becomes UNKNOWN/DEGRADED;
- no extra ALLOW/DENY decision is created;
- drop/degradation is observable.

## 11.1 Required root

Every traceable request starts one root span:

~~~text
request_wall
~~~

Start: Runtime accepts the well-formed Tool request and assigns request_id.

End: terminal request outcome.

If Runtime crashes before terminal, root remains INCOMPLETE with no fabricated end/duration.

## 11.2 Required stage nodes/spans

At least:

- request_received;
- tool_resolution;
- workspace_validation;
- policy_evaluation;
- approval_wait when applicable;
- execution when applicable;
- terminal.

Point-only stages may have null duration if the current canonical pipeline cannot safely measure a duration. The UI must show “timing unavailable”, not zero milliseconds.

## 11.3 Tool resolution boundary

M08 must make Tool/capability resolution observable for well-formed tools/call requests that receive a Harness request_id.

A syntactically malformed MCP envelope rejected before a Harness request_id exists belongs to M10 protocol conformance and is outside F14 request trace.

Unknown/unavailable Tool resolution that receives request_id must terminate the trace truthfully before execution.

## 11.4 Workspace / Policy

Workspace and Policy spans/nodes come from the existing Runtime pipeline.

Policy DENY:

- Trace contains Policy result;
- executorStarted=false;
- no execution span is fabricated;
- request terminates Denied.

## 11.5 Approval

Approval wait starts at ApprovalRequired.

It ends at:

- ApprovalResolved allow;
- ApprovalResolved deny;
- ApprovalExpired;
- ApprovalCancelled.

Approval wait is separate from executor duration.

ASK deny/expired/cancelled does not create an execution span.

SessionGrant hit does not invent approval wait.

## 11.6 Execution

Execution span starts only when executor actually starts.

Execution span ends with:

- success;
- executor failure;
- cancellation;
- timeout.

Pre-execution failure cannot create execution duration.

---

# 12. Trace Result Contract

Trace query key:

~~~text
runtime_id + request_id
~~~

Trace result contains at minimum:

- runtimeId;
- requestId;
- traceId when available;
- toolName;
- request outcome;
- policy decision/reason;
- approval state/decision when applicable;
- executorStarted;
- coverage;
- root request wall span;
- ordered bounded spans/nodes;
- failureStage when safely determinable;
- slowestMeasuredSpan when at least one complete duration exists;
- store status / degradation notice.

Ordering is:

~~~text
span_sequence ASC
~~~

not client-side timestamp guessing.

Trace status never claims a missing stage occurred.

---

# 13. Metrics Time-Range Contract

Metrics query supports:

- fromUtc inclusive;
- toUtc exclusive;
- optional exact Tool filter.

Preset UI ranges:

- 24h;
- 7d;
- 30d.

Default:

~~~text
24h
~~~

Maximum query range:

~~~text
30 days
~~~

Requests are assigned to the range by their observed RequestReceived/first request start timestamp.

If RequestReceived is missing because coverage is incomplete, the request contributes to coverageUnknown/legacy counts but is not silently counted as a known received call.

No “all history” unbounded Metrics query.

---

# 14. Metrics Classification Contract

Metrics must expose the following independent counts.

~~~text
receivedCalls
executionAttempts
successfulCalls
failedCalls
deniedCalls
cancelledCalls
timedOutCalls
preExecutionFailedCalls
approvalExpiredCalls
approvalCancelledCalls
incompleteCalls
unknownCalls
legacyUntracedCalls
~~~

Definitions:

- receivedCalls: unique requests with observed RequestReceived in the filter range.
- executionAttempts: unique requests with ExecutionStarted.
- successfulCalls: executor started and terminated ExecutionCompleted success.
- failedCalls: executor started and terminated as ordinary executor/system Tool failure; excludes cancelled/timed out.
- deniedCalls: terminal Denied before successful executor completion, including Policy DENY or Approval deny.
- cancelledCalls: executor-started request terminally cancelled.
- timedOutCalls: executor-started request terminally timed out.
- preExecutionFailedCalls: failed before ExecutionStarted, excluding explicit denied/approval expired/cancelled.
- approvalExpiredCalls and approvalCancelledCalls: separate approval terminal categories.
- incompleteCalls: known request with incomplete/open evidence at query time.
- unknownCalls: coverage UNKNOWN or insufficient to classify safely.
- legacyUntracedCalls: pre-v2 request lacking M08 trace spans.

One request must not be double-counted into multiple mutually exclusive terminal categories.

---

# 15. Success Rate Contract

Authoritative M08 success rate is:

~~~text
executionSuccessRate =
  successfulCalls
  /
  (successfulCalls + failedCalls)
~~~

Denominator name:

~~~text
completedExecutionDenominator
~~~

The denominator intentionally excludes:

- DENY;
- cancel;
- timeout;
- approval expired/cancelled;
- pre-execution failure;
- incomplete;
- unknown.

Those categories remain separately visible next to the success rate.

UI must display numerator + denominator + sample status, not only a percentage.

---

# 16. Duration Metrics Contract

## 16.1 Execution duration

Source: complete execution span.

Samples include:

- successfulCalls;
- failedCalls.

Samples exclude:

- deny;
- cancel;
- timeout;
- pre-execution failure;
- incomplete/unknown;
- legacy request without execution span.

## 16.2 Approval wait

Source: complete approval_wait span.

Samples may end in:

- allow;
- deny;
- expired;
- cancelled.

Disposition is separate from duration.

## 16.3 Request wall time

Source: complete request_wall root span.

Samples include any safely classified terminal request outcome.

Incomplete/unknown root spans do not contribute duration.

---

# 17. Aggregate Statistics and P95

For each duration family return:

~~~text
sampleCount
averageMs
p95Ms
sampleStatus
~~~

sampleStatus:

- NO_DATA;
- INSUFFICIENT;
- SUFFICIENT.

Average for sampleCount > 0:

~~~text
averageMs = sum(valid duration_ms) / sampleCount
~~~

React must not recompute it from raw samples.

P95 algorithm is nearest-rank:

~~~text
sort valid duration_ms ascending
rank = ceil(0.95 * N)
p95 = value[rank - 1]
~~~

Never average daily/hourly P95 values.

Frozen gate:

~~~text
MIN_P95_SAMPLES = 20
~~~

Behavior:

- N = 0 → p95Ms = null, NO_DATA;
- 1 <= N < 20 → p95Ms = null, INSUFFICIENT;
- N >= 20 → compute p95Ms, SUFFICIENT.

Counts and averages may still be shown for low samples, but UI must explicitly show sample count and “insufficient for P95”.

Success-rate sample label uses completedExecutionDenominator:

- 0 → NO_DATA;
- 1–19 → INSUFFICIENT;
- >=20 → SUFFICIENT.

The exact ratio may be displayed for N>0, but low sample status remains visible.

---

# 18. Coverage and Data-quality Contract

Metrics return separate evidence-quality fields:

~~~text
completeRequestCount
incompleteRequestCount
unknownRequestCount
legacyUntracedRequestCount
durationEligibleCount per duration family
storeStatus
lastCommitAtUtc
dropCount
coverageUnknownSinceUtc
~~~

Sample sufficiency and evidence coverage are separate.

Examples:

- 10 complete samples → coverage can be complete while P95 is insufficient.
- 100 requests with dropped evidence → sample may be large while coverage is partial/unknown.
- v1 legacy records → counts may be provable while Trace/duration coverage is not.

UI must not collapse these into one green health indicator.

---

# 19. Query Bounds and Performance Budget

Frozen limits:

~~~text
MAX_METRICS_RANGE_DAYS = 30
MAX_TRACE_SPANS = 16
MAX_TRACE_EVENTS_EXPOSED = 64
MAX_METRICS_TOOL_FILTER_BYTES = 128
METRICS_QUERY_TIMEOUT = 2 seconds
TRACE_QUERY_TIMEOUT = 2 seconds
~~~

Metrics returns aggregate DTOs only, never raw duration sample arrays.

Controlled performance fixture:

~~~text
50,000 request_summary rows
250,000 evidence_events rows
200,000 request_spans rows
20 warm Metrics queries across 24h/7d/30d + Tool filters
~~~

Performance gate:

~~~text
P95 query duration <= 750 ms
0 timeouts
~~~

Benchmark uses one documented Windows test environment and read-only connection.

This is a query-performance budget, not an F05 startup threshold.

---

# 20. Tauri / Protocol API

Control Protocol remains v1 with additive DTOs.

Add typed DTO families equivalent to:

~~~text
MetricsQueryParams
MetricsQueryResult
MetricsCounts
DurationMetric
MetricsCoverage

TraceGetParams
TraceGetResult
RequestTraceSpan
TraceAvailability
TraceCoverage
~~~

Tauri commands:

~~~text
metrics_query(params)
trace_get({ runtimeId, requestId })
~~~

They use the shared evidence-store DAL and read-only connection.

They do not call PolicyEngine and do not execute Tools.

---

# 21. Metrics Query Failure Semantics

Invalid range/tool:

- INVALID_REQUEST;
- no partial silent widening.

Evidence store states:

- Disabled/missing → Metrics/Trace explicitly unavailable.
- v1 migration required → History readable; M08 Metrics/Trace report migration required/legacy unavailable.
- Busy → bounded existing busy_timeout then EVIDENCE_BUSY.
- Corrupt → preserve DB and return EVIDENCE_CORRUPT.
- Incompatible → EVIDENCE_INCOMPATIBLE_SCHEMA.
- Queue overflow/drop → partial aggregates may be returned only with coverage warning/dropCount.

No store failure changes Runtime authorization/execution result.

---

# 22. Retention and Clear

request_spans belongs to the same request family.

Retention/clear must delete atomically via request_summary cascade:

~~~text
request_summary
→ evidence_events
→ approval_history
→ request_spans
~~~

No orphan spans.

After clear/retention:

- Metrics immediately reflects retained data only;
- Trace for deleted request returns Not Found;
- live authorization is unchanged;
- no aggregate cache keeps deleted metrics.

---

# 23. Trace Privacy Contract

Allowed span fields:

- IDs;
- safe stage name;
- relative monotonic offset;
- duration;
- status;
- coverage;
- safe failure category.

Trace may display existing safe Tool/Policy/Approval metadata already allowed by M03.

Forbidden:

- API keys;
- raw secret values;
- raw env;
- raw argv;
- raw bash command;
- raw stdout/stderr;
- unbounded Tool result;
- raw grep query;
- grep match preview in durable Trace;
- unrestricted file body;
- unrestricted physical Workspace path;
- hidden model reasoning.

Trace is diagnostic metadata, not a payload dump.

---

# 24. UI Contract

Reuse the existing Activity/History observability surface rather than creating a disconnected analytics application.

## 24.1 Metrics UI

Activity/History gains a Metrics view/section.

Filters:

- 24h / 7d / 30d;
- Tool or All.

At minimum show:

- received calls;
- execution attempts;
- successful;
- failed;
- denied;
- cancelled;
- timed out;
- incomplete/unknown;
- execution success rate with numerator/denominator;
- average execution duration;
- P95 execution duration;
- approval wait;
- request wall time;
- sample count/status;
- evidence coverage/store warning.

Do not hide non-success categories behind one percentage.

## 24.2 Trace UI

From a History request detail, user can open Request Trace for the exact runtime_id + request_id.

Trace shows:

- Tool;
- request identity;
- outcome;
- Policy;
- Approval when applicable;
- executor started yes/no;
- ordered timeline;
- duration per measurable span;
- incomplete/unknown markers;
- failure stage;
- slowest measured span.

For DENY, explicitly show executor not started.

For legacy v1 request, show “Trace not available for legacy evidence”; do not fabricate nodes.

## 24.3 Localization

Full en + zh-CN coverage.

---

# 25. M03 History Compatibility

Existing History behavior remains valid after v2:

- old request list/detail still works;
- filters/pagination remain;
- terminal Approval remains observational;
- Tauri reader remains read-only while Runtime runs;
- retention/clear semantics remain;
- v2 spans never restore Pending/Grant/Permit;
- v1 legacy DB remains readable before migration;
- migration failure does not erase History.

M08 must rerun M03 integration/security regressions.

---

# 26. Metrics Are Not F12 Evaluation

Production Metrics only know what Runtime observed.

They cannot answer:

- which Tool model should have selected;
- Tool choices model considered but did not emit;
- hidden model reasoning;
- final Agent task completion unless separately ground-truthed by F12.

M08 UI must not label Tool call success rate as “Agent accuracy” or “Tool selection accuracy”.

---

# 27. Crash / Incomplete Trace Semantics

Runtime crash or hard termination may leave:

- open root span;
- open approval span;
- open execution span;
- missing terminal event.

On next read:

- coverage = INCOMPLETE or UNKNOWN;
- duration for unclosed span = null;
- no guessed end time;
- no guessed success/failure;
- executorStarted only true if ExecutionStarted was observed;
- Metrics exclude unclosed span from duration sample;
- request contributes to incomplete/unknown counts.

A later App/Runtime restart never completes the old trace by guessing.

---

# 28. Request Trace Stage Truth Table

| Scenario | Approval span | Execution span | Terminal truth |
| --- | --- | --- | --- |
| ALLOW success | none | complete | success |
| ALLOW executor fail | none | complete | failed |
| Policy DENY | none | absent | denied |
| ASK allow success | complete | complete | success |
| ASK deny | complete | absent | denied |
| ASK expired | complete | absent | approval_expired |
| ASK cancelled | complete | absent | approval_cancelled |
| Execution cancelled | optional/none | complete cancelled | cancelled |
| Execution timeout | optional/none | complete timed_out | timed_out |
| Workspace/pre-execution fail | none | absent | failed before executor |
| Runtime crash during execution | optional/none | incomplete | incomplete/unknown |

UI and Metrics must preserve these distinctions.

---

# 29. Security Invariants

1. AuditEvent remains canonical semantics.
2. Runtime remains authorization/execution authority.
3. Metrics/Trace never issue ALLOW/ASK/DENY.
4. History/Trace cannot create Pending Approval/SessionGrant/ExecutionPermit.
5. Runtime remains normal-running SQLite writer.
6. Tauri Metrics/Trace queries are read-only.
7. telemetry loss never changes Tool execution.
8. raw sensitive payloads are not persisted for observability.
9. exact request identity is mandatory.
10. no fuzzy time correlation.
11. legacy data is not upgraded by inventing facts.
12. passive Metrics do not masquerade as F12.

---

# 30. M08 Derived Acceptance Criteria

| M08 AC | Requirement |
| --- | --- |
| M08-AC01 | Evidence Store migrates transactionally from physical schema v1 to v2, preserves v1 History, adds trace_id/request_spans, backs up before migration, and never drop/recreates on failure. |
| M08-AC02 | AppConfig remains v3, Control Protocol remains v1 additive, and M08 creates no second evidence DB or authorization truth. |
| M08-AC03 | Metrics/Trace derive only from the canonical Runtime AuditEvent/Safe Evidence path and correlate by exact runtime_id + request_id. |
| M08-AC04 | Metrics supports bounded 24h/7d/30d and exact Tool filtering with received/execution/terminal classifications defined by this SPEC. |
| M08-AC05 | successfulCalls, failedCalls, denied, cancelled, timedOut, preExecutionFailed, approval terminal, incomplete and unknown categories are mutually truthful and not silently conflated. |
| M08-AC06 | executionSuccessRate uses successful / (successful + failed) completed-execution denominator and exposes numerator, denominator and sample status; DENY/cancel/timeout are separate. |
| M08-AC07 | execution duration, approval wait and request wall time come from distinct monotonic request-local spans; legacy terminal durationMs is never relabeled executor duration. |
| M08-AC08 | Average/P95 use valid duration samples, nearest-rank P95, MIN_P95_SAMPLES=20, and transparent NO_DATA/INSUFFICIENT/SUFFICIENT state. |
| M08-AC09 | Metrics exposes evidence completeness, unknown/drop/legacy counts separately from statistical sample sufficiency. |
| M08-AC10 | Metrics/Trace queries obey 30-day/span/output/time bounds and the frozen 50k-request query P95 <=750ms performance budget. |
| M08-AC11 | Each traceable request has one local trace_id and one root request_wall span without replacing the canonical runtime_id + request_id identity. |
| M08-AC12 | Request Trace covers received, Tool/capability resolution, Workspace, Policy, Approval if applicable, Execution if applicable, and terminal/incomplete truth without inventing absent stages. |
| M08-AC13 | DENY/ASK deny/expired/cancelled explicitly show executor not started; session-grant and no-approval paths do not invent approval wait. |
| M08-AC14 | Trace can report failure stage, slowest measured span, executorStarted and ordered stage timing when safely observable. |
| M08-AC15 | Crash/drop/open spans remain INCOMPLETE/UNKNOWN with null unobserved duration/end state; restart never guesses terminal truth. |
| M08-AC16 | Trace persists only bounded sanitized metadata and never raw secret/argv/env/stdout/stderr/file body/grep query/match preview/physical path/hidden reasoning. |
| M08-AC17 | Retention and clear delete spans with the request family and leave no stale metrics aggregate or orphan trace. |
| M08-AC18 | Disabled/busy/full/corrupt/incompatible/migration/drop states are explicitly surfaced; observability failure never alters Runtime Policy/execution. |
| M08-AC19 | UI provides localized Tool/time Metrics with numerator/denominator/sample/coverage transparency and no misleading “Agent accuracy” wording. |
| M08-AC20 | History request detail exposes exact request-local Trace timeline; legacy v1 requests show unavailable rather than reconstructed trace. |
| M08-AC21 | M03 History query/pagination/offline-read/writer-ownership/privacy/non-authorization behavior remains compatible after v2 migration and rollback path is tested. |
| M08-AC22 | Packaged candidate proves real Tool requests produce consistent History/Metrics/Trace facts, including ALLOW/ASK/DENY/failure/incomplete cases, without M03/V3 security regression. |

---

# 31. Canonical → Derived → Task → Validation Traceability

| Canonical | Derived | Task owner | Reserved Integration | Reserved E2E | Reserved Manual |
| --- | --- | --- | --- | --- | --- |
| SEC-AC01/02/03 | AC01/02/03/16/18/21 | T1/T2/T5 | M08-INT-01/02/03/15 | M08-E2E-01/03 | M08-MAN-01/04 |
| F08-AC01 | AC04/05/06/07/19 | T3/T4 | M08-INT-07/08/09/12 | M08-E2E-02 | M08-MAN-02 |
| F08-AC02 | AC05/06/07/13 | T2/T3 | M08-INT-06/07/08 | M08-E2E-02/03 | M08-MAN-02/03 |
| F08-AC03 | AC08/09/10/18 | T3/T5 | M08-INT-09/10/14/15 | M08-E2E-02/04 | M08-MAN-02/04 |
| F14-AC01 | AC11/12/13/15 | T2 | M08-INT-04/05/06 | M08-E2E-03 | M08-MAN-03 |
| F14-AC02 | AC03/11/14/20/22 | T2/T4/T6 | M08-INT-05/11/12/18 | M08-E2E-01/03 | M08-MAN-03/05 |
| F14-AC03 | AC15/16/19/20 | T2/T4/T5 | M08-INT-13/16/17 | M08-E2E-03/04 | M08-MAN-03/04 |

Reserved suites:

- Integration: M08-INT-01 through M08-INT-18.
- E2E: M08-E2E-01 through M08-E2E-04.
- Manual: M08-MAN-01 through M08-MAN-05.

---

# 32. Development Entry Gate

Formal M08 implementation may begin only when:

1. M03 is ACCEPTED / FROZEN;
2. all six M08 documents are frozen;
3. M03 schema-v1 checksum/fixtures and accepted migration/backup semantics are available;
4. canonical runtime_id + request_id correlation is stable;
5. no unresolved M03 evidence privacy/authorization blocker exists.

M07 Development Complete is not an M08 dependency. Roadmap dependency remains M03.

---

# 33. Development Exit Gate

M08 Development Complete means:

- T1–T6 implemented;
- physical schema v2/migration/rollback development tests PASS;
- TraceProjector/span lifecycle tests PASS;
- Metrics formula/P95/sample/coverage tests PASS;
- read-only Tauri Metrics/Trace queries PASS;
- UI/i18n development tests PASS;
- query performance fixture meets frozen budget;
- affected M03/V3 regression PASS;
- fresh Portable candidate produced;
- formal Integration/E2E/Manual remain NOT EXECUTED unless actually run.

Development Complete is not M08 ACCEPTED / FROZEN.

---

# 34. Final Exit Gate

M08 may become ACCEPTED / FROZEN only when:

- F08-AC01/02/03 PASS;
- F14-AC01/02/03 PASS;
- SEC-AC01/02/03 remain PASS;
- M08-AC01 through M08-AC22 PASS;
- M08 Integration PASS;
- M08 E2E PASS;
- M08 Manual Acceptance PASS;
- schema v1→v2 migration and rollback PASS;
- M03 History remains readable and non-authoritative;
- Metrics formulas match hand-computable fixtures;
- P95 nearest-rank/sample gate PASS;
- real requests show consistent History/Metrics/Trace facts;
- DENY/ASK/failure/crash semantics are truthful;
- legacy requests are not fabricated into Trace;
- privacy sentinel leakage = 0;
- query performance gate PASS;
- no current M03/V3 security blocker remains.

Successful wording:

~~~text
M08 — ACCEPTED / FROZEN
~~~

M08 does not deliver F09 Token estimates and does not by itself produce V4 Full acceptance.
