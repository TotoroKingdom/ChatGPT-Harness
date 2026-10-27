# M03 TASKS — Shared Evidence and Durable SQLite History

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: IMPLEMENTATION READY AFTER VALIDATION DOCUMENTS ARE FROZEN  
> Scope: Shared Evidence Contract + F07 only  
> Dependency: M00 ACCEPTED / FROZEN  
> M01/M02 acceptance is not a Roadmap dependency. Do not modify their acceptance status or frozen thresholds.  
> Layout: no new top-level directory. Formal shared DAL goes under runtime/evidence-store.

---

## 1. Dependency

~~~text
M00 ACCEPTED / FROZEN
        ↓
M03 six contract docs frozen
        ↓
T1 Evidence contract / identity / config semantics
        ↓
T2 evidence-store schema / migration / query / ownership
        ↓
T3 Runtime writer / queue / retention / recovery
        ↓
T4 Tauri read-only facade / maintenance
        ↓
T5 History UI / Settings / i18n
        ↓
T6 Regression / package / Development Complete
~~~

---

# T1 — Production Shared Evidence Contract and Identity

**AC:** M03-AC01, AC02, AC03, AC12  
**Primary areas:** protocol/src/v1, runtime/src/audit, runtime/src/events, ui/src-tauri runtime initialize/config wiring.

## Implement

### 1. History protocol DTO

Create protocol/src/v1/history.rs and export it through existing v1 module/generation flow.

Add typed enums/DTOs for:

- EvidenceCoverage;
- HistoryTerminalState;
- HistoryApprovalState;
- History request summary;
- History query/get;
- History clear result;
- EvidenceStoreStatus;
- safe error codes.

Keep protocol v1 additive.

### 2. Stable live identity

Add runtime_id to ActivityEvent.

For AuditEvent-originated Activity:

- ActivityEvent.event_id must equal canonical AuditEvent.event_id;
- request_id remains canonical AuditEvent.request_id;
- runtime_id comes from the current Runtime process context.

Update EventBus/ActivityEventInput accordingly.

Runtime-only presentation events may still generate independent event IDs.

### 3. Runtime Host context

Extend initialize/internal Runtime control contract additively with optional Host context:

- host_session_id;
- supervisor_generation_id;
- EvidenceStore launch config:
  - enabled;
  - database path;
  - retention days;
  - max storage MiB.

Tauri derives path from app_data_dir/history.

No frontend or Tool chooses DB path.

### 4. SafeEvidenceProjector

Implement one production projector from:

~~~text
AuditEvent + RuntimeEvidenceContext → EvidenceEventV1
~~~

Do not add a second Policy evaluation.

Map only typed fields.

### 5. Durable privacy projection

Implement durable sanitized Tool evidence.

Special rule:

- Bash command preview text is not durable.

Add sentinels for:

- API key;
- physical Workspace path;
- proxy credential;
- raw env;
- raw stdout/stderr;
- Bash command preview;
- arbitrary JSON metadata.

### 6. HistoryConfig activation

Keep CURRENT_SCHEMA_VERSION = 3.

Validate:

- retentionDays 1..=3650;
- maxStorageMiB 32..=4096.

Change config restart semantics:

- history changes increment Runtime-facing configRevision;
- history change creates Restart Required;
- ui/desktop-only changes remain revision-neutral.

Do not put DB path into AppConfig.

### Required tests

- canonical event_id preserved;
- runtime_id present;
- request key distinguishes same request string across two runtimes;
- no timestamp join;
- explicit host mapping only;
- retry/new request identity separated;
- projector all stage/outcome variants;
- durable privacy sentinels;
- Bash preview absent;
- v3 History defaults unchanged;
- history config revision/restart semantics;
- invalid ranges fail closed;
- protocol TS/schema round-trip.

### Done

Shared Evidence Contract is production-ready before SQLite integration.

---

# T2 — Internal Evidence-store Library, SQLite Schema and Query DAL

**AC:** M03-AC04, AC05, AC06, AC07, AC10, AC15  
**Depends on:** T1.

## Implement

### 1. Create formal nested crate

Create:

~~~text
runtime/evidence-store/
~~~

Add root workspace member.

Crate must not import Runtime authorization/executor modules.

Add dependencies required for SQLite + file locking.

### 2. Schema v1

Implement SPEC tables:

- schema_migrations;
- runtime_sessions;
- request_summary;
- evidence_events;
- approval_history;
- evidence_health.

Implement required indexes.

Do not create request_spans in schema v1.

### 3. SQLite initialization

Writer pragmas:

- WAL;
- foreign_keys;
- synchronous NORMAL;
- busy_timeout 1000ms;
- auto_vacuum INCREMENTAL;
- wal_autocheckpoint 1000.

Reader:

- mode=ro;
- query_only;
- foreign_keys;
- busy_timeout 250ms.

### 4. Migration engine

Implement:

- create v1;
- checksum verification;
- consistent pre-migration backup;
- transactional migration framework;
- integrity check;
- newer-schema rejection;
- checksum mismatch;
- corrupt preservation;
- migration failure preservation.

No delete-and-recreate recovery.

### 5. Ownership lock

Implement history.writer.lock.

Tests:

- writer owns exclusive lock;
- second writer deterministically fails;
- read-only reader works concurrently;
- offline maintenance fails while writer owns lock;
- offline maintenance succeeds after writer exits.

### 6. Event transaction

Implement transaction that:

- inserts event_id idempotently;
- creates/updates request summary;
- writes terminal Approval row;
- updates health commit metadata.

Duplicate event_id must not mutate summary twice.

### 7. Query DAL

Implement:

- HistoryQueryParams filters;
- keyset cursor;
- filter fingerprint validation;
- default 50/max 200;
- HistoryGet detail;
- safe typed mapping;
- missing DB empty/uninitialized behavior.

No OFFSET browsing.

### 8. Query performance fixture

Using existing test layout/system temp:

- 50,000 request summaries;
- 250,000 events;
- <=256MiB steady fixture.

After warmups run 30 query/get measurements.

Targets from SPEC.

Do not commit raw benchmark database.

### Required tests

- schema exact version;
- indexes exist;
- migration checksum;
- newer version;
- corrupt source;
- backup preservation;
- WAL reader/writer;
- second writer;
- event+summary rollback;
- duplicate event;
- approval terminal row;
- cursor deterministic ordering;
- cursor filter mismatch;
- limit clamp/reject semantics;
- filters individually/combinations;
- query/get performance.

### Done

Shared DAL is ready for Runtime/Tauri integration.

---

# T3 — Runtime Evidence Writer, Queue, Health, Retention and Recovery

**AC:** M03-AC05–AC09, AC13, AC17  
**Depends on:** T2.

## Implement

### 1. Runtime EvidenceAuditSink

Create an AuditSink adapter that:

- receives canonical AuditEvent;
- projects to EvidenceEventV1;
- non-blocking enqueues to writer;
- never returns authorization state.

Compose it with existing ActivityStore sink.

Keep live Activity behavior.

### 2. Writer startup

At Runtime initialize:

If History disabled:

- writer state disabled;
- no lock/DB create.

If enabled:

- acquire writer lock;
- open/create/migrate DB;
- recover unclean prior sessions;
- create current runtime_sessions row;
- start dedicated writer worker.

Evidence initialization failure must not fail Tool authorization/runtime start; it becomes evidence degradation.

### 3. Queue

Frozen:

- capacity 1024;
- batch 64;
- batch wait 50ms;
- try_send producer.

Use dedicated blocking worker.

No SQLite write on ExecutionPipeline thread.

### 4. Loss tracking

On enqueue failure:

- increment atomic drop count;
- record last drop time;
- retain affected request key in bounded loss tracker where possible;
- mark request INCOMPLETE through writer when capacity returns;
- if mapping overflows, set global coverage UNKNOWN marker.

Expose evidence.status.

### 5. Session lifecycle

Normal stop:

- stop producer at correct pipeline boundary;
- drain up to 2s;
- mark open request summaries INCOMPLETE;
- close runtime session;
- checkpoint best effort.

Unclean next open:

- mark stale open session unclean_restart;
- mark its open requests INCOMPLETE;
- no fake terminal event.

### 6. Online retention

Run:

- startup;
- every 256 committed events; or
- 60 seconds of active committed history.

Age cleanup first.

Then capacity cleanup to 90% target.

Never delete current open requests.

Use checkpoint/incremental vacuum.

If cap cannot be enforced:

- enter EVIDENCE_CAPACITY_DEGRADED;
- prevent unbounded new growth by dropping durable events as necessary;
- record loss/coverage.

### 7. Online clear RPC

Add Runtime control methods:

- evidence.status
- history.clear

history.clear is a queue barrier.

While running, clear only terminal/incomplete history and ended sessions; preserve current open session/request.

### Required tests

- success Tool writes complete evidence;
- DENY;
- ASK allow/deny/expire/cancel;
- execution cancel/failure;
- same event duplicate;
- queue overflow;
- disk IO failure injection;
- busy;
- writer ownership conflict;
- storage full/cap failure;
- normal stop drain;
- stop drain timeout;
- crash recovery;
- open request incomplete;
- retention age;
- retention size;
- active request protected;
- online clear barrier;
- Policy result unchanged on every evidence failure.

### Done

Runtime durable writer works without becoming an authorization dependency.

---

# T4 — Tauri Read-only History Facade and Offline Maintenance

**AC:** M03-AC07, AC10, AC14, AC15  
**Depends on:** T2/T3.

## Implement

### 1. HistoryStoreCoordinator

Add inside existing ui/src-tauri tree.

Own only Host storage coordination:

- controlled DB path;
- read-only facade;
- Runtime online clear routing;
- offline maintenance;
- store status.

No Policy logic.

### 2. Tauri commands

Add:

- history_query;
- history_get;
- history_status;
- history_clear.

history_query/get use read-only shared DAL even when Runtime runs.

### 3. App startup preparation

Before Runtime launch:

- derive history directory/path;
- if existing older compatible DB requires migration and Runtime is stopped, acquire writer lock and migrate;
- migration failure is user-visible/degraded but does not delete DB or block unrelated app UI.

### 4. Online clear

If Runtime active:

- call Runtime history.clear;
- never open Tauri write connection.

### 5. Offline clear

If Runtime stopped:

- verify supervisor state;
- acquire writer lock;
- clear all history/session rows;
- checkpoint/incremental vacuum;
- return typed counts.

### 6. Offline History

With Runtime stopped:

- list/query/detail existing compatible DB;
- no Runtime startup required.

### 7. Status

Combine:

- desired HistoryConfig;
- active Runtime effective history status when running;
- DB file footprint;
- schema compatibility;
- drop/degraded status;
- restart debt.

No raw DB path in frontend DTO.

### Required tests

- missing DB;
- running read-only query;
- stopped read-only query;
- Tauri cannot write while Runtime owns lock;
- online clear routes RPC only;
- offline clear ownership;
- race Runtime start vs maintenance;
- migration before Runtime start;
- migration failure preserve;
- corrupt/incompatible display state;
- no Workspace file touched.

### Done

History remains queryable offline and ownership is enforced.

---

# T5 — Activity/History UI, Filters, Detail, Settings and i18n

**AC:** M03-AC10–AC16  
**Depends on:** T4.

## Implement

### 1. Activity navigation

Keep existing Activity page.

Add tabs:

- Live;
- History.

Do not replace the existing live Activity feed.

### 2. History filters

Add:

- time range;
- Tool;
- outcome;
- Policy decision;
- Approval-related state;
- reset filters.

Default last 24h.

### 3. Pagination

Use keyset next cursor.

Default 50.

No client-side full-database pagination.

### 4. History list

Show:

- time;
- Tool;
- outcome;
- policy;
- approval state;
- duration;
- coverage.

Coverage badge:

- COMPLETE;
- INCOMPLETE;
- UNKNOWN.

### 5. Detail

Open request detail with:

- request/runtime IDs;
- safe event timeline;
- policy;
- terminal Approval evidence;
- execution outcome;
- duration;
- coverage/store warning.

No Approve/Deny button.

No “re-run authorized” button.

### 6. Store health

Show clear banners for:

- disabled;
- degraded;
- busy;
- incompatible;
- corrupt;
- unavailable;
- drop count / incomplete coverage.

Never call corrupt DB “empty history”.

### 7. Settings

Add History controls:

- enabled;
- retentionDays;
- maxStorageMiB;
- current footprint;
- desired/effective state;
- Restart Required;
- Clear History.

History changes use existing config save semantics and now create restart debt.

Clear uses explicit confirmation.

### 8. Privacy copy

Explain:

- stored locally;
- bounded/sanitized evidence;
- no Harness-provided SQLite at-rest encryption claim.

### 9. i18n

Add complete en/zh-CN strings.

### Frontend tests

- Live tab regression;
- History default load;
- all filters;
- cursor load-more;
- cursor reset on filter;
- empty state;
- degraded banner;
- coverage badges;
- detail safe fields;
- no approval actions;
- settings validation;
- restart-required state;
- clear confirmation/result;
- corrupt/incompatible state;
- language coverage.

### Done

F07 is usable without mixing historical evidence and live authority.

---

# T6 — Cross-cutting Regression, Packaging and Development Complete

**AC:** M03-AC17, AC18; closes all M03 ACs  
**Depends on:** T1–T5.

## 1. Protocol and dependency audit

Confirm:

- Control Protocol remains v1 additive;
- evidence-store crate has no Runtime auth/executor dependencies;
- top-level repository layout unchanged;
- History DB path is Host controlled.

## 2. Privacy database audit

Populate sentinel scenarios and inspect DB through query/API plus direct test-only SQL.

Sentinels:

- API key;
- Workspace physical path;
- proxy credential;
- raw env;
- raw stdout/stderr;
- Bash command preview;
- large file content;
- raw error path.

None may appear in durable DB fields.

## 3. Real local Runtime matrix

Use real Runtime subprocess/MCP where applicable:

- read_file success;
- write/edit success in disposable Workspace;
- Policy DENY;
- ASK allow;
- ASK deny;
- Approval expiry/cancel;
- execution failure/cancel;
- Runtime stop/restart;
- crash/unclean restart.

Verify request keys and durable rows.

## 4. Storage failure matrix

Run:

- second writer;
- queue overflow;
- read busy;
- write busy;
- disk/IO failure fixture;
- corrupt DB;
- incompatible schema;
- migration checksum mismatch;
- migration failure;
- retention over age;
- capacity over max;
- online clear;
- offline clear.

Every failure must preserve Policy/Tool semantics.

## 5. M01/M02/V3 regression

Affected suites:

- Tray/background lifecycle;
- approval/disconnect notifications;
- startup recorder;
- one-click diagnostics;
- AppConfig v3;
- Workspace;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- Approval/SessionGrant;
- Bash;
- SecretStore;
- Harness Start/Stop/Restart.

Do not modify M02 performance thresholds/evidence.

## 6. Query performance

Re-run frozen query/get fixture on final candidate.

Record P95.

## 7. Full checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Run repository milestone delivery/layout scripts.

## 8. Candidate delivery

Build fresh M03 Portable through required delivery script.

Record path/hash.

Do not overwrite older milestone artifacts.

## 9. Cleanup

Remove:

- temporary SQLite DBs;
- migration backups from tests;
- temp Workspace;
- lock files;
- test processes.

Do not delete real user History.

## 10. Development Complete response

Report in conversation:

- candidate SHA;
- evidence-store version/dependencies;
- DB schema version/checksum;
- exact tests/counts;
- query performance;
- privacy sentinel result;
- failure matrix;
- package path/hash;
- formal Integration/E2E/Manual = NOT EXECUTED.

Do not create a seventh M03 file.

### Done

M03 Development Complete requires all task-local/development checks PASS and no unresolved implementation blocker.

Do not mark M03 ACCEPTED / FROZEN.

---

## 2. Task → AC → Reserved Validation Traceability

| Task | M03 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01, AC02, AC03, AC12 | M03-INT-01/02/03/04, M03-E2E-01, M03-MAN-01 |
| T2 | AC04, AC05, AC06, AC07, AC10, AC15 | M03-INT-05/06/07/11, M03-E2E-02/03, M03-MAN-02/03 |
| T3 | AC05–AC09, AC13, AC17 | M03-INT-08/09/10/16, M03-E2E-01/03, M03-MAN-02/04 |
| T4 | AC07, AC10, AC14, AC15 | M03-INT-12/13/14/15, M03-E2E-02/04, M03-MAN-03/05 |
| T5 | AC10–AC16 | M03-INT-13/17, M03-E2E-02/04, M03-MAN-01/03/04 |
| T6 | AC17, AC18 | M03-INT-18, all M03 E2E, M03-MAN-05 |

---

## 3. Development Handoff

After T6:

~~~text
M03 Development Complete
→ M03 Integration
→ M03 packaged E2E with real ChatGPT success/DENY/ASK History
→ Human Manual Acceptance
→ user sign-off
→ M03 ACCEPTED / FROZEN
→ unlock M04 / M08 dependency gates
~~~

A working SQLite file is necessary but not sufficient for M03 acceptance.
