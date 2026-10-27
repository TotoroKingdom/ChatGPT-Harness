# M03 SPEC — Shared Evidence and Durable SQLite History

> 布局与测试生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。本目录启动阶段只创建 SPEC / PLAN / TASKS；后续补齐三份验证/人工验收计划，最终严格保持六文件布局。不得新增独立报告、实验工程、仓库顶级目录或把运行日志写入本目录。  
> Status: SPEC_READY / IMPLEMENTATION UNLOCKED  
> Milestone: M03 — Shared Evidence and Durable SQLite History  
> Scope: P0 Shared Evidence Contract + F07 SQLite History  
> Roadmap dependency: M00 only  
> Upstream gate: M00 is ACCEPTED / FROZEN. M03 does not depend on M01 or M02 acceptance. M02 current performance verdict must not be altered or reinterpreted by M03.  
> Current main observed before this document commit: 8a3e05022f3df4372ff0dc3e91a0c176b0034d58  
> Important: M03 delivers persistent evidence/history, not F08 Metrics UI, F14 Trace UI, F12 Evaluation, F10 grep, or authorization restore.

---

## 1. Objective

M03 turns the current Runtime-owned AuditEvent stream into one durable, queryable, privacy-bounded evidence foundation shared by later History, Metrics, Trace and Evaluation work.

M03 must establish one coherent chain:

~~~text
ExecutionPipeline
   ↓
Runtime AuditEvent                      ← canonical execution evidence source
   ↓
SafeEvidenceProjector
   ├─ Live Activity projection
   ├─ Approval/notification observation
   └─ Durable EvidenceEvent
          ↓
   Runtime-only EvidenceWriter
          ↓
     SQLite / WAL
          ↑
   Tauri read-only History facade
          ↓
   React Activity / History UI
~~~

The durable store is observational only.

It must answer:

- what Tool request was observed;
- which Runtime produced it;
- which request identity it belongs to;
- which Workspace/Policy/Approval/execution stages were observed;
- whether the request is complete, incomplete or unknown;
- what terminal outcome was observed;
- what terminal Approval evidence exists;
- whether evidence storage itself is healthy or degraded.

It must never answer:

- whether an old request is still authorized;
- whether an old Approval can still be approved;
- whether a Session Grant should be restored;
- whether a stored Policy decision authorizes a new request;
- whether a historical Tool was or is currently enabled.

---

## 2. Canonical Upstream Acceptance Criteria

### Shared Evidence

- SEC-AC01 — the same real Tool request must use stable identity across Activity, Approval, History, Metrics and Trace; no fuzzy time-based primary joins.
- SEC-AC02 — durable evidence stores only typed, bounded, sanitized safe projections and must not create a second Policy/authorization truth.
- SEC-AC03 — durable evidence cannot restore Pending Approval, Session Grant, ExecutionPermit or equivalent live authority.

### F07 — SQLite History

- F07-AC01 — Activity, terminal Approval and evidence needed later by Metrics/Trace remain queryable across Runtime/App restart and support time, Tool, outcome, policy decision, approval-related state and pagination.
- F07-AC02 — History and live authorization remain strictly separated; old Pending/Grant/Permit never revive from SQLite.
- F07-AC03 — retention, capacity, manual clear, migration/full/busy/corrupt/drop degradation are observable and never alter Policy truth.

### Architecture decisions consumed by M03

- A04 — Runtime AuditEvent is canonical evidence source.
- A05 — Runtime is the only normal-running SQLite writer; Tauri reads through read-only connections.
- A06 — shared evidence-store library contains safe DTO, migration, query and maintenance only; no authorization.
- A07 — WAL, versioned migration, bounded writer queue and retention.
- A08 — canonical Tool request key is runtime_id + request_id.
- A17 — History/Trace are non-authorization components; telemetry failure does not block Tool execution unless a future PRD explicitly changes that product policy.

---

## 3. Current Code Facts to Preserve

Current main already provides:

- RuntimeCore owns a real runtime_id generated per Runtime process.
- InitializeResult exposes runtime_id to Tauri.
- AuditEvent already carries event_id, request_id, typed lifecycle, policy/approval/outcome/failure evidence and sanitized Tool evidence.
- ApprovalRequest already binds runtime_id, request_id and generation identity.
- ActivityEvent currently exposes request_id but not runtime_id.
- EventBus currently creates its own Activity event_id instead of preserving AuditEvent.event_id.
- Runtime already has bounded in-memory Activity/EventBus paths.
- AppConfig schema v3 already contains HistoryConfig:
  - enabled = true
  - retentionDays = 30
  - maxStorageMiB = 256
- HistoryConfig is currently intentionally inert for restart semantics because M01 only preserved future fields.
- no production SQLite History store exists yet.
- no production evidence-store crate/library exists yet.
- no history.query/history.get/history.clear product API exists yet.

M03 must extend these facts; it must not replace current Runtime authority.

---

# 4. Scope

## 4.1 Included

M03 implements:

1. production Shared Evidence Contract v1;
2. stable runtime_id + request_id request identity across live Activity / Approval / History;
3. preservation of canonical AuditEvent.event_id into live Activity and durable evidence;
4. explicit Host session / supervisor generation mapping when known;
5. Runtime SafeEvidenceProjector;
6. formal internal shared evidence-store Rust library inside the existing runtime module tree;
7. SQLite schema v1, checksummed migrations and ownership lock;
8. Runtime-only writer connection with WAL;
9. bounded non-blocking writer queue and batching;
10. atomic event + request-summary updates;
11. terminal Approval history;
12. crash/unclean-session recovery to INCOMPLETE;
13. evidence health/drop/degradation tracking;
14. Tauri read-only query facade;
15. time/Tool/outcome/policy/approval filtering;
16. bounded keyset pagination;
17. request detail query with safe event timeline;
18. online and offline clear semantics;
19. retention by age and storage capacity;
20. AppConfig HistoryConfig validation and generation-effective restart semantics;
21. History UI integrated into existing Activity area;
22. History Settings controls/status;
23. en / zh-CN UI strings;
24. migration/busy/full/corrupt/drop/privacy regression tests.

## 4.2 Explicit non-goals

M03 does not implement:

- F08 Tool Metrics UI or final metric formulas;
- F09 token estimates;
- F12 Agent Evaluation;
- F14 user-facing Trace / Span UI;
- OpenTelemetry export;
- F10 grep;
- replay/retry from history;
- “Approve from History”;
- restoration of old Pending Approval;
- restoration of SessionGrant or ExecutionPermit;
- cloud sync;
- SQLite at-rest encryption;
- arbitrary SQL/export UI;
- free-text full-database search;
- indefinite retention;
- a second Runtime event source.

---

# 5. Repository Landing Zone

Architecture suggested a new top-level evidence-store directory, but current root AGENTS.md prohibits new top-level directories without explicit user authorization.

M03 therefore freezes this implementation landing:

~~~text
runtime/
├ Cargo.toml
├ src/...
└ evidence-store/
   ├ Cargo.toml
   └ src/
      ├ lib.rs
      ├ dto.rs
      ├ schema.rs
      ├ migration.rs
      ├ writer.rs
      ├ query.rs
      ├ retention.rs
      ├ maintenance.rs
      └ status.rs
~~~

Root Cargo workspace adds runtime/evidence-store as a member.

Crate name:

~~~text
chatgpt-harness-evidence-store
~~~

Dependencies:

- protocol crate for shared typed enums/History wire DTO where appropriate;
- rusqlite for SQLite;
- fs2 or equivalent file-locking library for cross-process writer/maintenance ownership;
- serde / chrono as needed.

The evidence-store crate must not depend on:

- Runtime PolicyEngine;
- ApprovalManager;
- SessionGrantStore;
- ExecutionPermit;
- Tool executor modules;
- Tauri;
- React.

Runtime and Tauri may both depend on evidence-store.

This preserves A06 without changing the repository top-level layout.

---

# 6. Shared Evidence Contract v1

## 6.1 Canonical request identity

The only canonical Tool request key is:

~~~text
(runtime_id, request_id)
~~~

Rules:

- runtime_id identifies one Runtime process.
- request_id identifies one Tool request inside that Runtime.
- the same textual request_id in another Runtime is a different request.
- a retry is a new request_id unless the existing Runtime execution model explicitly resumes the same request.
- timestamps are never the primary join.
- approval_id is associated only through Runtime evidence for the same request.
- host_session_id and supervisor_generation_id are optional explicit mappings, not substitutes for runtime_id.

## 6.2 Runtime identity injection

AuditEvent remains the canonical event source and does not need to become a Host-owned object.

SafeEvidenceProjector is constructed with Runtime process context:

~~~text
RuntimeEvidenceContext
- runtime_id
- host_session_id?
- supervisor_generation_id?
- runtime_version
- config_revision
~~~

The projector combines this process context with each AuditEvent.

Tauri must not add runtime_id to persisted events after the fact.

## 6.3 Host mapping

M03 adds an additive optional Host launch context to Runtime initialize/control wiring.

Recommended shape:

~~~text
RuntimeHostContext
- hostSessionId?
- supervisorGenerationId?
- evidenceStore
    - enabled
    - databasePath
    - retentionDays
    - maxStorageMiB
~~~

databasePath is internal control-plane data.

It must never be:

- exposed as an MCP Tool argument;
- stored inside EvidenceEvent;
- copied into History UI;
- logged in unrestricted diagnostic output.

Direct Runtime tests without Host context default to evidence persistence disabled unless they explicitly supply a temporary evidence path.

## 6.4 Canonical event identity

For an AuditEvent-originated live Activity:

~~~text
ActivityEvent.eventId == AuditEvent.event_id
ActivityEvent.runtimeId == current Runtime runtime_id
ActivityEvent.requestId == AuditEvent.request_id
~~~

EventBus must not generate a second UUID for an AuditEvent-originated event.

RuntimeStarted/RuntimeStopped presentation-only Activity events that are not derived from AuditEvent may still use independently generated Activity event IDs.

## 6.5 Coverage

Request-level durable evidence coverage:

- COMPLETE
- INCOMPLETE
- UNKNOWN

COMPLETE:

- a known terminal stage was durably committed;
- no known request-specific loss affects that request.

INCOMPLETE:

- the request was observed;
- Runtime/session ended without a durable terminal event, or a known event loss affected that request.

UNKNOWN:

- store corruption/recovery/global loss means completeness cannot be established safely.

Missing evidence is never converted to success/failure by inference.

---

# 7. Durable EvidenceEvent Contract

Logical durable event:

~~~text
EvidenceEventV1
- schemaVersion = 1
- eventId
- runtimeId
- requestId
- hostSessionId?
- supervisorGenerationId?
- approvalId?
- toolName
- occurredAtUtc
- recordedAtUtc
- stage
- lifecycleState
- workspaceValidation
- policyDecision?
- policyReason?
- matchedRuleIds[]
- approvalDecision?
- outcome
- failureCategory?
- durationMs?
- sanitizedArguments?
- resultEvidence?
- coverage
~~~

## 7.1 Frozen stages

M03 recognizes:

- request_received
- workspace_validated
- policy_evaluated
- approval_required
- approval_resolved
- approval_expired
- approval_cancelled
- session_grant_matched
- execution_started
- execution_completed
- execution_failed
- request_denied

No stage may imply executor start unless ExecutionStarted was actually observed.

## 7.2 Result distinctions

M03 preserves these distinctions:

- received request != execution attempt;
- ASK != success;
- Pending/ApprovalRequired != execution;
- DENY != executor failure;
- cancellation != generic success/failure;
- executor success != Agent task completion;
- missing terminal evidence != known failure.

---

# 8. Durable Privacy Allowlist

M03 durable storage is stricter than an arbitrary live log.

Allowed:

- stable UUID/string identities;
- logical Tool name;
- typed capability/lifecycle/outcome/failure;
- logical Workspace label, never physical root;
- typed Workspace validation result;
- typed Policy decision/reason;
- sanitized bounded rule IDs;
- approval ID and terminal decision;
- safe bounded duration/count/size values;
- closed typed SanitizedArgumentsEvidence variants after durable revalidation;
- closed typed ExecutionResultEvidence variants;
- coverage state;
- approved hashes/fingerprints.

Forbidden:

- API keys;
- SecretStore values;
- raw environment;
- unrestricted command/argv;
- raw stdout/stderr;
- full file contents;
- physical filesystem paths;
- credential-bearing proxy URL;
- arbitrary JSON blobs;
- unrestricted OS/control-plane error text;
- hidden model reasoning;
- unverified upstream trace IDs.

## 8.1 Bash-specific durable rule

Current live Bash activity may contain an analyzer-sanitized command preview.

M03 durable History must not persist that preview text.

Durable Bash evidence may persist only typed facts such as:

- mode;
- capability;
- logical working-directory category;
- classification;
- risk flags;
- session-grant-hit;
- duration/result scalar evidence;
- optional hash/length if explicitly useful.

Raw/sanitized command preview text remains live-only in M03.

## 8.2 JSON columns

SQLite may store JSON text only for closed typed DTOs:

- matched rule IDs;
- durable sanitized argument evidence;
- result evidence.

No generic serde_json::Value or arbitrary metadata map may be persisted.

---

# 9. SQLite Database Location and Ownership

## 9.1 Path

Tauri derives:

~~~text
app_data_dir/history/history.sqlite3
~~~

Associated lock:

~~~text
app_data_dir/history/history.writer.lock
~~~

The user cannot configure this path in M03.

The path is never under Workspace.

## 9.2 Writer ownership

Runtime writer must acquire an exclusive OS file lock on history.writer.lock for the entire writer lifetime.

If lock acquisition fails:

- Runtime Tool execution may continue;
- evidence writer enters DEGRADED ownership state;
- no second SQLite writer is opened;
- evidence.status reports the failure;
- M03 acceptance is blocked until resolved.

## 9.3 Tauri read-only access

While Runtime is running:

- Tauri opens SQLite with mode=ro;
- PRAGMA query_only=ON;
- Tauri does not migrate, delete, checkpoint, vacuum or write.

If DB does not yet exist:

- History facade returns an empty/uninitialized state;
- it does not create a DB merely by browsing History.

## 9.4 Offline maintenance

Only when RuntimeSupervisor confirms Runtime stopped and Tauri obtains the same exclusive history.writer.lock may Tauri:

- migrate an older compatible schema;
- clear History;
- run checkpoint/vacuum/retention maintenance.

Lock failure means maintenance is BLOCKED/BUSY, not forced.

---

# 10. SQLite Schema v1

M03 freezes physical schema version 1.

The implementation may format SQL differently, but table/column/key semantics below are contractual.

## 10.1 schema_migrations

~~~sql
schema_migrations(
  version INTEGER PRIMARY KEY,
  checksum TEXT NOT NULL,
  applied_at_utc TEXT NOT NULL
)
~~~

Migration checksum is deterministic and verified on open.

## 10.2 runtime_sessions

~~~sql
runtime_sessions(
  runtime_id TEXT PRIMARY KEY,
  host_session_id TEXT NULL,
  supervisor_generation_id TEXT NULL,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT NULL,
  end_reason TEXT NULL,
  runtime_version TEXT NOT NULL,
  config_revision TEXT NOT NULL
)
~~~

## 10.3 request_summary

~~~sql
request_summary(
  runtime_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  first_seen_at_utc TEXT NOT NULL,
  last_seen_at_utc TEXT NOT NULL,
  terminal_state TEXT NOT NULL,
  outcome TEXT NULL,
  policy_decision TEXT NULL,
  approval_state TEXT NOT NULL,
  failure_category TEXT NULL,
  execution_started INTEGER NOT NULL,
  duration_ms INTEGER NULL,
  evidence_coverage TEXT NOT NULL,
  PRIMARY KEY(runtime_id, request_id),
  FOREIGN KEY(runtime_id)
    REFERENCES runtime_sessions(runtime_id)
    ON DELETE CASCADE
)
~~~

terminal_state values are typed and closed:

- open
- succeeded
- failed
- denied
- cancelled
- timed_out
- approval_expired
- approval_cancelled
- incomplete
- unknown

History UI must not interpret open as a live authorization object.

## 10.4 evidence_events

~~~sql
evidence_events(
  event_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  approval_id TEXT NULL,
  tool_name TEXT NOT NULL,
  occurred_at_utc TEXT NOT NULL,
  recorded_at_utc TEXT NOT NULL,
  stage TEXT NOT NULL,
  lifecycle_state TEXT NOT NULL,
  workspace_validation TEXT NOT NULL,
  policy_decision TEXT NULL,
  policy_reason TEXT NULL,
  matched_rule_ids_json TEXT NULL,
  approval_decision TEXT NULL,
  outcome TEXT NOT NULL,
  failure_category TEXT NULL,
  duration_ms INTEGER NULL,
  sanitized_arguments_json TEXT NULL,
  result_evidence_json TEXT NULL,
  coverage TEXT NOT NULL,
  FOREIGN KEY(runtime_id, request_id)
    REFERENCES request_summary(runtime_id, request_id)
    ON DELETE CASCADE
)
~~~

## 10.5 approval_history

Only terminal Approval evidence becomes an approval_history row.

~~~sql
approval_history(
  runtime_id TEXT NOT NULL,
  approval_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  requested_at_utc TEXT NULL,
  terminal_at_utc TEXT NOT NULL,
  terminal_state TEXT NOT NULL,
  decision TEXT NULL,
  PRIMARY KEY(runtime_id, approval_id),
  FOREIGN KEY(runtime_id, request_id)
    REFERENCES request_summary(runtime_id, request_id)
    ON DELETE CASCADE
)
~~~

There is no durable Pending object.

An ApprovalRequired event may exist in evidence_events, but it is historical observation only.

## 10.6 evidence_health

Single-row durable health metadata:

~~~sql
evidence_health(
  id INTEGER PRIMARY KEY CHECK(id = 1),
  schema_version INTEGER NOT NULL,
  last_commit_at_utc TEXT NULL,
  last_checkpoint_at_utc TEXT NULL,
  last_maintenance_at_utc TEXT NULL,
  last_clear_at_utc TEXT NULL,
  drop_count INTEGER NOT NULL,
  last_drop_at_utc TEXT NULL,
  last_error_code TEXT NULL,
  degraded_since_utc TEXT NULL,
  coverage_unknown_since_utc TEXT NULL
)
~~~

Writer queue depth and current writer state are live Runtime values and must not be inferred from stale DB rows.

## 10.7 request_spans deferral

Architecture lists request_spans as a logical future table.

M03 schema v1 intentionally does not implement F14 span semantics.

M03 persists every canonical request stage needed for later F14 derivation. M08 may add request_spans through schema migration once Trace semantics are frozen.

This is a physical-schema refinement, not removal of F14 evidence.

---

# 11. Required Indexes

At minimum:

~~~text
request_summary:
- (first_seen_at_utc DESC, runtime_id DESC, request_id DESC)
- (tool_name, first_seen_at_utc DESC, runtime_id DESC, request_id DESC)
- (outcome, first_seen_at_utc DESC, runtime_id DESC, request_id DESC)
- (policy_decision, first_seen_at_utc DESC, runtime_id DESC, request_id DESC)
- (approval_state, first_seen_at_utc DESC, runtime_id DESC, request_id DESC)

evidence_events:
- (runtime_id, request_id, occurred_at_utc ASC, event_id ASC)
- (occurred_at_utc DESC, event_id DESC)
- approval_id when non-null

approval_history:
- (terminal_at_utc DESC, runtime_id DESC, approval_id DESC)
~~~

Query implementation must use bounded indexed access and must not load the full database into React.

---

# 12. SQLite Runtime Pragmas

Writer connection:

~~~text
journal_mode = WAL
foreign_keys = ON
synchronous = NORMAL
busy_timeout = 1000ms
auto_vacuum = INCREMENTAL
wal_autocheckpoint = 1000 pages
~~~

Read-only Tauri connection:

~~~text
query_only = ON
foreign_keys = ON
busy_timeout = 250ms
~~~

If a platform/SQLite limitation prevents one exact pragma, implementation must document and prove an equivalent safe behavior before changing this SPEC.

---

# 13. Migration Contract

## 13.1 New database

For a missing database:

- Runtime writer with ownership lock may create schema v1.
- Tauri read-only browsing must not create it.

## 13.2 Existing older compatible database

Before migration:

1. acquire exclusive writer/maintenance lock;
2. checkpoint WAL when possible;
3. create a consistent SQLite backup;
4. verify source schema/checksum;
5. run migration in transaction;
6. run integrity check;
7. commit only on success.

On success, transient backup may be removed after integrity verification.

On failure:

- transaction rolls back;
- source DB is preserved;
- backup is preserved;
- no auto-delete/recreate;
- writer reports migration degraded/unavailable;
- Tool authorization/execution continues independently.

## 13.3 Newer/incompatible database

If DB schema is newer than this binary supports:

- open read/write is refused;
- no downgrade is attempted;
- file is preserved;
- History UI reports incompatible schema.

## 13.4 Corruption

Corrupt DB:

- preserve file;
- do not reset automatically;
- report CORRUPT;
- current Tool execution remains independent.

---

# 14. Writer Queue and Transaction Contract

## 14.1 Queue

Runtime uses one bounded non-blocking producer path into one dedicated SQLite writer worker.

Frozen queue limits:

~~~text
capacity = 1024 EvidenceEvent work items
batch max = 64 events
batch wait max = 50 ms
graceful stop drain budget = 2 seconds
~~~

ExecutionPipeline must not block waiting for SQLite fsync.

Producer uses non-blocking enqueue.

## 14.2 Queue overflow

If queue is full:

- Tool execution/Policy result is unchanged;
- drop_count increments;
- request-specific loss is recorded in a bounded in-memory loss tracker where possible;
- evidence health enters DEGRADED;
- query coverage for affected request becomes INCOMPLETE;
- if request identity itself cannot be retained because the loss tracker is saturated, coverage_unknown_since_utc is set.

No queue overflow may become Policy DENY or implicit ALLOW.

## 14.3 Atomic event commit

For each new event, one transaction atomically:

1. inserts event_id if not already present;
2. creates/updates request_summary;
3. updates terminal approval_history where applicable;
4. updates evidence_health commit metadata.

If event_id already exists:

- no duplicate event row;
- no second summary mutation;
- no duplicate approval row.

## 14.4 Batching

Multiple events may share one SQLite transaction, but each event retains idempotent semantics.

A failed batch transaction commits none of that batch.

---

# 15. Runtime Session and Crash Recovery

## 15.1 Session open

After Runtime identity and Host mapping are known, writer records runtime_sessions row before accepting durable request events.

## 15.2 Normal stop

Normal Runtime stop:

- stops accepting new durable enqueue after pipeline shutdown boundary;
- drains queue up to 2 seconds;
- marks current runtime_session ended with normal_stop;
- any still-open request without terminal evidence becomes INCOMPLETE;
- checkpoints WAL on a best-effort bounded path.

Failure to finish drain is reported as evidence degradation, not execution failure.

## 15.3 Unclean restart/crash

On next successful writer open:

- any runtime_sessions row with no ended_at_utc from a prior process is marked ended;
- end_reason = unclean_restart;
- nonterminal request_summary rows for that runtime become INCOMPLETE;
- no synthetic success/failure event is inserted.

---

# 16. History Query API

M03 adds shared typed protocol DTOs in protocol/src/v1/history.rs for Tauri/React.

These are product query DTOs; Tauri may satisfy them directly from read-only SQLite.

## 16.1 Query filters

HistoryQueryParams:

~~~text
fromUtc?
toUtc?
tool?
outcome?
policyDecision?
approvalState?
cursor?
limit?
~~~

Time filtering uses request_summary.first_seen_at_utc.

Default UI range: last 24 hours.

API may query all retained history when no range is supplied.

## 16.2 Approval-state filter

History approval values are observational:

- none
- required_observed
- allow_once
- allow_for_session
- denied
- expired
- cancelled
- incomplete
- unknown

There is intentionally no durable actionable pending value.

## 16.3 Pagination

Keyset pagination only.

Default:

~~~text
limit = 50
max limit = 200
~~~

Cursor is opaque to React and encodes:

- cursor schema version;
- first_seen_at_utc;
- runtime_id;
- request_id;
- filter fingerprint.

Using a cursor with different filters fails with typed INVALID_CURSOR.

No OFFSET pagination for normal History browsing.

## 16.4 History list result

Each item includes only request-level safe summary:

- runtimeId
- requestId
- first/last seen time
- tool
- terminalState
- outcome
- policyDecision
- approvalState
- failureCategory
- durationMs
- evidenceCoverage

## 16.5 History detail

HistoryGet(runtime_id, request_id) returns:

- request summary;
- ordered EvidenceEvent safe projections;
- terminal approval history if present;
- store/coverage notice relevant to the request.

It never returns a live ApprovalRequest object.

---

# 17. History Query Performance Contract

Representative query fixture:

~~~text
50,000 request_summary rows
250,000 evidence_events rows
database remains within configured 256 MiB steady-state target
page size = 50
~~~

After 2 uncounted warm-up queries, run 30 measured queries on the same machine/database.

Targets:

~~~text
history.query P95 <= 500 ms
history.get   P95 <= 250 ms
~~~

Measure Tauri/shared-DAL query execution excluding human/UI render time.

A query implementation that meets latency only by dropping filters or returning incomplete rows fails.

---

# 18. HistoryConfig Runtime Semantics

Existing v3 fields are confirmed:

~~~text
history.enabled = true
history.retentionDays = 30
history.maxStorageMiB = 256
~~~

M03 freezes these defaults.

Validation:

~~~text
retentionDays: 1..=3650
maxStorageMiB: 32..=4096
~~~

To stop collecting History, user sets enabled=false.

## 18.1 Generation-effective behavior

M03 changes HistoryConfig from inert/future preference to Runtime generation-effective storage policy.

Therefore:

- changing any HistoryConfig field advances Runtime-facing configRevision;
- changing HistoryConfig while Harness is running produces Restart Required;
- desktop/ui-only preferences remain revision-neutral;
- HistoryConfig itself remains non-authorization configuration.

HistoryConfig does not need to enter RuntimeConfigSnapshot.

Instead it is passed through RuntimeHostContext.evidenceStore with the Host-derived DB path.

## 18.2 Disabled behavior

When enabled=false after restart:

- Runtime does not open EvidenceWriter;
- no new durable Tool evidence is written;
- existing database remains readable;
- Clear remains available;
- disabling History does not delete existing data;
- live Activity/Approval behavior remains unchanged.

---

# 19. Retention and Capacity Contract

## 19.1 Defaults

~~~text
age limit = 30 days
storage limit = 256 MiB
~~~

Whichever bound requires cleanup first applies.

## 19.2 Eligible deletion unit

Retention deletes whole request families only:

- request_summary;
- all evidence_events;
- terminal approval_history;
- future child records via foreign-key cascade.

Never delete one stage from an otherwise retained request solely to save space.

Open requests in the current live Runtime are not retention candidates.

## 19.3 Age cleanup

Delete closed/incomplete request families whose last_seen_at_utc is older than the configured retention window.

## 19.4 Capacity cleanup

Steady-state footprint includes:

- main SQLite database;
- WAL file.

SHM and transient migration backup are not counted as retained evidence footprint.

When footprint exceeds maxStorageMiB:

1. age cleanup first;
2. delete oldest eligible whole request families in bounded batches;
3. target 90% of maxStorageMiB to provide hysteresis;
4. checkpoint WAL;
5. run incremental vacuum as appropriate.

If cleanup cannot get below the configured bound because of busy/error/active data:

- writer enters EVIDENCE_CAPACITY_DEGRADED;
- new durable events may be dropped rather than allowing unbounded growth;
- drop/coverage status is visible;
- Runtime Policy/execution remains unchanged.

A failed migration backup may temporarily exceed the configured History cap; data preservation wins and UI must report the retained backup/degraded state.

## 19.5 Maintenance cadence

Online writer runs retention:

- once after writer startup/recovery;
- after every 256 committed events; or
- after 60 seconds with committed activity,

whichever occurs first.

Retention must not run synchronously inside Tool execution.

---

# 20. Manual Clear Contract

M03 exposes one user action:

~~~text
Clear History
~~~

with explicit confirmation.

## 20.1 Runtime running

Tauri sends online clear to the Runtime writer.

Writer inserts a maintenance barrier after already queued events.

Clear deletes:

- all terminal/incomplete historical request families;
- ended runtime sessions no longer referenced.

It does not delete the current active runtime session or currently open request rows.

New/current evidence after the clear barrier may appear again immediately.

Clear does not:

- cancel requests;
- resolve Approval;
- revoke or create SessionGrant;
- alter Policy;
- stop/restart Runtime.

## 20.2 Runtime stopped

Tauri verifies Runtime stopped, acquires history.writer.lock exclusively and may clear all evidence/session rows.

Schema/migration metadata remains.

## 20.3 Clear result

Return:

- deleted request count;
- deleted event count;
- deleted approval count;
- resulting database footprint;
- completed_at_utc.

No raw deleted content is returned.

---

# 21. Evidence Health Contract

EvidenceStoreStatus states:

- disabled
- healthy
- degraded
- busy
- incompatible
- corrupt
- unavailable

Runtime live status additionally reports:

- queueDepth
- queueCapacity
- dropCount
- lastDropAt?
- lastCommitAt?
- lastErrorCode?
- effectiveHistoryConfig
- schemaVersion

Tauri offline status reports what can be verified from DB/lock/files.

UI must display a visible warning when:

- dropCount > 0 for current runtime;
- database is degraded/corrupt/incompatible;
- query coverage is INCOMPLETE or UNKNOWN;
- retention cannot enforce capacity.

SQLite existence alone is not HEALTHY proof.

---

# 22. UI Contract

M03 reuses the existing Activity page.

## 22.1 Activity tabs

~~~text
Activity
├ Live
└ History
~~~

Live keeps existing behavior.

History reads SQLite through Tauri facade.

## 22.2 History list

Required filters:

- time range;
- Tool;
- outcome;
- Policy decision;
- Approval-related state.

Required behavior:

- default last 24h;
- clear/reset filters;
- 50 items per page;
- keyset next-page/load-more;
- empty state;
- storage-degraded banner;
- COMPLETE / INCOMPLETE / UNKNOWN coverage badge.

No free-text database search in M03.

## 22.3 Request detail

Display:

- Tool;
- request/runtime IDs with safe copy;
- start/end time;
- outcome;
- policy decision/reason;
- approval terminal evidence;
- execution status;
- safe event timeline;
- duration;
- evidence coverage.

History detail has no Approve/Deny/retry-authorize button.

If the same request is still live, History remains observational; live actions continue in the existing Approvals page only.

## 22.4 Settings History section

Expose:

- Enable History;
- Retention days;
- Maximum storage MiB;
- current storage footprint;
- current/effective status;
- Restart Required when desired config differs from running generation;
- Clear History;
- local-storage/privacy explanation.

M03 must explicitly state that History is stored locally and Harness does not claim SQLite at-rest encryption.

## 22.5 i18n

All new visible text requires English and zh-CN.

---

# 23. Protocol / API Contract

M03 keeps Control Protocol v1 additive.

New shared DTO/module:

~~~text
protocol/src/v1/history.rs
~~~

Recommended Host-facing product operations:

- history_query
- history_get
- history_clear
- history_status

Runtime Control RPC additions:

- evidence.status
- history.clear

Tauri history_query/history_get read the shared store directly and remain usable when Runtime is stopped.

Runtime history.clear is used only for online clear.

No protocol/v2.

---

# 24. Failure Semantics

## SQLite busy

- read query returns typed BUSY/retryable state;
- no forced writer takeover;
- no Policy impact.

## disk full / IO failure

- writer becomes DEGRADED;
- queue/drop counters surface evidence loss;
- current Tool outcome remains authoritative;
- no synthetic History success.

## corruption

- preserve DB;
- mark CORRUPT;
- do not auto-reset.

## incompatible schema

- preserve DB;
- mark INCOMPATIBLE;
- no downgrade.

## writer lock conflict

- refuse second writer;
- mark DEGRADED/ownership error;
- Runtime may continue without durable evidence.

## queue overflow

- drop evidence item;
- increment drop count;
- mark affected request INCOMPLETE where identity retained;
- global coverage may become UNKNOWN if loss mapping is itself incomplete.

## Runtime crash

- next writer recovery marks open prior requests INCOMPLETE;
- old ApprovalRequired evidence remains historical only.

---

# 25. Security Invariants

M03 must prove:

- History cannot construct ApprovalRequest.
- History cannot call approvals.resolve.
- History cannot construct SessionGrant.
- History cannot construct ExecutionPermit.
- History does not feed PolicyEngine.
- History does not determine Tool enabled state.
- DB path is not an MCP Tool input.
- History queries cannot escape AppData to arbitrary paths.
- physical Workspace root is not persisted in evidence.
- raw API key/SecretStore values never enter DB.
- durable Bash command preview text is not stored.
- clear/retention never touches Workspace files.
- storage failure never converts DENY to ALLOW or ALLOW to DENY.

---

# 26. Derived Acceptance Criteria

| M03 AC | Requirement |
| --- | --- |
| M03-AC01 | AuditEvent-derived live Activity and durable evidence preserve the canonical event_id; live Activity includes runtime_id and correlates by runtime_id + request_id without time joins. |
| M03-AC02 | Host session/supervisor generation mapping is explicit and optional; canonical request identity remains runtime_id + request_id. |
| M03-AC03 | SafeEvidenceProjector persists only the closed typed durable allowlist; secret/raw env/raw output/physical path/Bash preview sentinels are absent. |
| M03-AC04 | Shared evidence-store library exists inside the existing runtime module tree and contains no authorization/executor dependency. |
| M03-AC05 | SQLite schema v1, checksum migration, writer lock, WAL and event_id idempotency work as frozen. |
| M03-AC06 | Event insertion plus request-summary/terminal-approval update is atomic; duplicate event_id cannot double-update summaries. |
| M03-AC07 | Runtime is the only normal-running writer; Tauri read path is query-only and cannot migrate/clear while Runtime owns the writer lock. |
| M03-AC08 | Queue is bounded/non-blocking to Tool execution; overflow/full/busy/IO failure changes evidence health/coverage but not Policy or execution truth. |
| M03-AC09 | Normal stop drains boundedly; crash/unclean restart marks nonterminal requests INCOMPLETE without inventing terminal outcomes. |
| M03-AC10 | History query supports frozen time/Tool/outcome/policy/approval filters and bounded keyset pagination; query/get meet §17 performance targets. |
| M03-AC11 | History detail returns request summary, safe canonical event timeline and terminal Approval evidence, never a live authorization object. |
| M03-AC12 | Existing HistoryConfig defaults remain enabled=true/30 days/256 MiB; values validate to frozen ranges and become generation-effective with restart debt. |
| M03-AC13 | Age/capacity retention deletes whole request families, preserves open current requests, uses 90% hysteresis and reports degraded coverage if capacity cannot be enforced. |
| M03-AC14 | Online/offline Clear History follows the ownership/barrier rules and never cancels, approves, revokes or otherwise mutates live authorization. |
| M03-AC15 | Tauri can query compatible History while Runtime is stopped; corrupt/incompatible DB is preserved and surfaced rather than silently rebuilt. |
| M03-AC16 | Activity/History UI provides filters, pagination, details, coverage/health and Settings controls without creating History authorization actions. |
| M03-AC17 | History remains usable across Runtime/App restart and real success/DENY/ASK-terminal evidence can be correlated to the original request identities. |
| M03-AC18 | M03 preserves M01/M02/V3 lifecycle, notification, diagnostics, Workspace, Policy, Approval, Bash and SecretStore behavior. |

---

# 27. Reserved Validation Traceability

The later validation documents must keep these IDs.

| Upstream | M03 AC | Task | Integration | E2E | Manual |
| --- | --- | --- | --- | --- | --- |
| SEC-AC01, A04, A08 | AC01, AC02 | T1 | M03-INT-01/02/03 | M03-E2E-01 | M03-MAN-01 |
| SEC-AC02 | AC03, AC04 | T1/T2 | M03-INT-04/05 | M03-E2E-01/02 | M03-MAN-01/04 |
| A05/A06/A07 | AC05–AC09 | T2/T3 | M03-INT-06/07/08/09/10 | M03-E2E-02/03 | M03-MAN-02/04 |
| F07-AC01 | AC10, AC11, AC15, AC17 | T2/T4/T5 | M03-INT-11/12/13 | M03-E2E-01/02 | M03-MAN-01/03 |
| F07-AC02, SEC-AC03, A17 | AC11, AC14 | T1/T4/T5 | M03-INT-14/15 | M03-E2E-02/04 | M03-MAN-03/05 |
| F07-AC03 | AC12–AC16 | T3/T4/T5 | M03-INT-16/17 | M03-E2E-03/04 | M03-MAN-02/04/05 |
| Regression | AC18 | T6 | M03-INT-18 | M03-E2E-04 | M03-MAN-05 |

Reserved suites:

- Integration: M03-INT-01 through M03-INT-18.
- E2E: M03-E2E-01 through M03-E2E-04.
- Manual: M03-MAN-01 through M03-MAN-05.

---

# 28. Development Entry Gate

M03 implementation may begin after its six contract documents are present and reviewed.

Roadmap dependency is M00, and M00 is already ACCEPTED / FROZEN.

M01/M02 acceptance is not a dependency for M03.

However M03 must:

- preserve their current code behavior;
- not modify M02 frozen performance thresholds;
- not claim M01/M02 acceptance;
- report any regression caused by M03 as an M03 blocker.

---

# 29. Exit Gate

M03 may be marked ACCEPTED / FROZEN only when:

- M03-AC01 through M03-AC18 pass;
- all six M03 documents exist and validation plans were actually executed;
- canonical identity works on real Tool/Approval flows;
- Runtime-only writer and Tauri read-only/offline maintenance ownership is proven;
- History survives Runtime/App restart;
- old Pending/Grant/Permit cannot be restored;
- retention and manual clear pass;
- migration/busy/full/corrupt/drop degradation is observable;
- privacy sentinels are absent from DB and copy/UI output;
- query performance targets pass;
- packaged Windows Integration/E2E/Manual acceptance pass;
- no unresolved security/authority regression remains.

SQLite file existence alone is not acceptance.
