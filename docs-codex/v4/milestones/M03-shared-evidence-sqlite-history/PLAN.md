# M03 PLAN — Shared Evidence and Durable SQLite History

> Input: SPEC.md  
> Status: DESIGN READY / IMPLEMENTATION UNLOCKED  
> Scope: Shared Evidence Contract + F07  
> Dependency: M00 ACCEPTED / FROZEN  
> Repository rule: no new top-level directory. Shared DAL lands under runtime/evidence-store as a formal product library.  
> Validation rule: Integration/E2E/Manual plans will be frozen before T1 implementation begins.

---

## 1. Delivery Strategy

M03 is ordered so identity/privacy are fixed before SQLite becomes a production dependency:

~~~text
T1 Shared Evidence Contract + identity + HistoryConfig semantics
        ↓
T2 Internal evidence-store library + schema/migration/query/locks
        ↓
T3 Runtime projector + writer queue + retention + health/recovery
        ↓
T4 Tauri read-only facade + online/offline maintenance
        ↓
T5 Activity/History UI + Settings + i18n
        ↓
T6 Cross-cutting regression + package + Development Complete
~~~

T1 and the validation documents define the contract. T2–T5 implement it; T6 closes development only.

---

## 2. Phase A — Shared Evidence Contract

### Identity

Add runtimeId to live ActivityEvent and preserve canonical AuditEvent.event_id for Audit-originated Activity.

Do not replace request identity with Activity sequence number.

Add explicit optional Host mapping through Runtime launch/initialize context:

- hostSessionId;
- supervisorGenerationId;
- evidence-store launch settings/path.

### Safe projector

Implement Runtime SafeEvidenceProjector converting AuditEvent + RuntimeEvidenceContext into EvidenceEventV1.

Keep conversion closed and typed.

Create durable-specific Bash projection that omits command preview text.

### History protocol DTO

Add protocol/src/v1/history.rs for:

- coverage enums;
- terminal/outcome/approval observational enums;
- query params/result;
- detail result;
- clear result;
- store status.

Keep protocol v1 additive.

### Config semantics

Existing schema stays v3.

Validate HistoryConfig ranges.

Change restart comparison so HistoryConfig changes advance configRevision and produce Runtime restart debt.

History settings remain outside RuntimeConfigSnapshot; RuntimeHostContext carries storage policy.

Exit:

- protocol/schema/TS tests pass;
- identity/redaction sentinels pass;
- config revision semantics pass.

---

## 3. Phase B — Evidence-store Library

Create formal nested workspace crate:

~~~text
runtime/evidence-store
~~~

Add it to root workspace.

### Responsibilities

- durable DTO;
- SQLite schema;
- migration/checksum;
- writer ownership lock;
- read-only query;
- retention helpers;
- offline maintenance;
- store status.

No Policy/Approval/Permit dependencies.

### Schema

Implement SPEC schema v1 exactly.

Create indexes before production use.

Do not create request_spans v1.

### Migration

Implement:

- new DB create;
- version/checksum verification;
- backup + transaction upgrade framework;
- integrity check;
- newer schema rejection;
- corrupt preservation;
- no auto-delete.

### Ownership

Implement writer lock on history.writer.lock.

Read-only query path never acquires write ownership.

Offline maintenance acquires the same exclusive lock.

### Query

Implement keyset query and history.get.

Cursor includes filter fingerprint.

Enforce limit default 50/max 200.

### Query performance

Add module-owned benchmark/ignored test or deterministic executable test under existing product/test layout; do not create a benchmark archive.

Validate 50k/250k fixture against SPEC targets.

Exit:

- schema/migration/locking/query tests pass;
- second writer rejected;
- read-only reader works under WAL.

---

## 4. Phase C — Runtime Writer Integration

### Runtime launch

Tauri derives AppData History path and passes RuntimeHostContext to Runtime initialization.

Direct Runtime without Host context does not write real-user History.

### Writer lifecycle

At Runtime initialize:

- if History disabled: create disabled evidence sink, no DB;
- if enabled: start EvidenceWriter;
- acquire writer lock;
- open/migrate DB;
- recover unclean prior sessions;
- insert current runtime_sessions row.

Evidence failure must not abort Policy/Tool execution.

### Audit fanout

Compose current ActivityStore AuditSink with durable EvidenceAuditSink.

Do not create a second AuditEvent producer.

SafeEvidenceProjector runs from canonical AuditEvent.

### Queue

Use bounded 1024 work-item queue.

Producer path uses try_send/non-blocking enqueue.

Dedicated blocking writer worker owns rusqlite connection.

Batch up to 64 or 50ms.

### Loss tracker

Track request key on dropped events when possible.

Update:

- drop count;
- degraded status;
- request coverage;
- global unknown-coverage start if request-specific mapping is lost.

### Transactions

Event + request summary + terminal approval update in one transaction.

Duplicate event_id is a no-op for all derived durable rows.

### Stop/crash

Normal Stop drains up to 2s and closes session.

Next writer startup repairs prior unclosed session and request summaries to INCOMPLETE without synthetic terminal event.

### Online retention

Run startup + 256-event/60s cadence.

Delete whole request families.

Age first, then capacity/hysteresis.

Exit:

- real Runtime integration tests produce SQLite rows;
- authorization behavior unchanged under writer failure.

---

## 5. Phase D — Tauri History Facade and Maintenance

### Coordinator

Add a Host HistoryStoreCoordinator or equivalent inside ui/src-tauri existing module tree.

Responsibilities:

- derive controlled DB/lock paths;
- open read-only facade;
- expose history_query/get/status;
- coordinate history_clear;
- perform offline compatible migration when Runtime is stopped and lock is owned;
- never become a Runtime authorization owner.

### Running-state query

Use read-only shared DAL even while Runtime is running.

No Tauri write.

### Online clear

When Runtime runs:

- call Runtime history.clear;
- Runtime writer owns barrier and transaction;
- current open request/session retained.

### Offline clear

When Runtime stopped:

- verify supervisor stopped;
- acquire writer lock;
- clear all evidence/session rows;
- checkpoint/incremental vacuum;
- release lock.

### Store status

Combine:

- Runtime evidence.status if running;
- read-only/offline DB status;
- physical footprint;
- config desired/effective state.

### Failure mapping

Expose typed user-safe codes:

- EVIDENCE_BUSY
- EVIDENCE_WRITER_OWNERSHIP
- EVIDENCE_IO
- EVIDENCE_FULL
- EVIDENCE_CORRUPT
- EVIDENCE_INCOMPATIBLE_SCHEMA
- EVIDENCE_MIGRATION_FAILED
- EVIDENCE_QUEUE_OVERFLOW
- EVIDENCE_CAPACITY_DEGRADED

No raw filesystem path in UI message.

Exit:

- Runtime stopped History browsing works;
- running read-only query works;
- maintenance ownership tests pass.

---

## 6. Phase E — UI

### Activity page

Keep current Live tab.

Add History tab.

### List

Controls:

- time range;
- Tool;
- outcome;
- Policy decision;
- Approval state;
- reset.

Use keyset pagination.

Do not fetch all rows.

### Coverage/health

Show:

- COMPLETE / INCOMPLETE / UNKNOWN per request;
- store health banner;
- drop count warning;
- incompatible/corrupt state;
- History disabled state.

### Detail

Request-level drawer/page using history_get.

Render only safe DTO.

No Approval action buttons.

### Settings

Add History section:

- enabled;
- retention days;
- max storage;
- storage size;
- effective status;
- Restart Required;
- Clear History.

Confirm clear.

Explain local storage and lack of Harness-provided at-rest encryption.

### i18n

en + zh-CN complete.

Exit:

- frontend tests cover filters/pagination/detail/clear/status/settings.

---

## 7. Phase F — Development Completion

### Full functional regression

Run focused and workspace tests:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

### M03-specific local real-runtime checks

Before Development Complete run real Runtime/Tauri component flows for:

- successful Tool evidence;
- Policy DENY;
- ASK → allow/deny/expire/cancel terminal paths;
- cancelled execution;
- Runtime stop/restart;
- crash recovery;
- query while Runtime stopped;
- online/offline clear;
- retention;
- writer lock conflict;
- queue overflow;
- DB busy/full/corrupt/migration failure;
- secret/path/Bash preview sentinel scans.

These are development/integration-preparation checks, not a substitute for later formal Integration/E2E.

### Package delivery

Run repository delivery/layout scripts required by AGENTS.

Fresh package only; do not overwrite an older M03 candidate.

### Development Complete output

Report in conversation:

- candidate commit;
- SQLite schema version/checksum;
- exact dependencies;
- exact test commands/counts;
- query performance results;
- store failure matrix;
- candidate package path/hash;
- remaining Integration/E2E/Manual as NOT EXECUTED.

Do not create a seventh milestone document.

---

## 8. Concurrency Plan

Races to cover:

- Runtime starts while Tauri offline maintenance is acquiring lock;
- Tauri reads while Runtime commits WAL batch;
- clear barrier arrives while request events are queued;
- clear runs while a request is open;
- retention and clear collide;
- Stop occurs during writer batch;
- crash occurs after event insert but before summary update;
- duplicate event is retried;
- config History settings change while old Runtime generation remains active.

Rules:

- one writer lock controls write/maintenance ownership;
- event + summary in one transaction;
- Runtime operation mutex remains lifecycle owner;
- History config changes affect next Runtime generation only;
- read-only Tauri never upgrades itself while Runtime owns writer;
- active open requests are excluded from retention/online clear.

---

## 9. Performance / Resource Plan

Frozen values:

~~~text
queue capacity: 1024
batch max: 64
batch wait: 50ms
stop drain: 2s
default retention: 30d
default cap: 256MiB
size cleanup target: 90% cap
history.query default/max: 50/200
history.query P95: <=500ms on 50k requests / 250k events
history.get P95: <=250ms on same fixture
~~~

ExecutionPipeline cannot synchronously wait on database fsync.

---

## 10. Security Review Plan

Before T6:

- dependency graph proves evidence-store has no authorization modules;
- DB sentinel scan checks secret/raw env/raw output/physical path;
- Bash preview sentinel absent;
- UI copy does not include forbidden fields;
- History detail has no Approval action;
- clear/retention filesystem scope remains only AppData History directory;
- DB errors do not change Policy results in tests.

---

## 11. Rollback

M03 production config remains schema v3.

If M03 code is rolled back to a binary that does not understand DB schema v1:

- old binary must ignore/preserve the History files;
- it must not delete them.

Within M03:

- incompatible/newer DB is preserved;
- failed migration preserves original;
- disabling History stops new writes after restart but preserves data.

No downgrade migration is required.

---

## 12. Development Completion Gate

Before formal Integration:

- T1–T6 complete;
- all M03 task-local tests PASS;
- schema v1/checksum frozen;
- writer/read-only/maintenance ownership PASS;
- query/filter/pagination PASS;
- query performance PASS;
- retention/capacity/clear PASS;
- crash/incomplete PASS;
- degradation/privacy matrix PASS;
- full workspace/frontend regression PASS;
- fresh package delivered;
- no external ChatGPT E2E falsely marked PASS.

M03 Development Complete is not M03 ACCEPTED / FROZEN.
