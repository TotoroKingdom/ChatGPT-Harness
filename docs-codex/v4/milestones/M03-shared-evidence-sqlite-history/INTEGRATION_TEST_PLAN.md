# M03 Integration Test Plan — Shared Evidence and Durable SQLite History

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M03 — Shared Evidence and Durable SQLite History  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M03 Development Complete  
> Scope: P0 Shared Evidence Contract + F07 SQLite History  
> Rule: Integration validates real Runtime/Tauri/evidence-store composition. It must not weaken the frozen identity, privacy, SQLite ownership, retention, queue or query-performance contracts to obtain PASS.  
> Storage: all generated databases, lock files, migration backups and Workspace fixtures use disposable test locations; do not use or mutate the user's real History database.

---

## 1. Objective

Integration proves that the M03 production implementation satisfies the frozen Shared Evidence and History contract before packaged E2E and human acceptance.

The suite must prove:

- canonical AuditEvent identity survives into live Activity and durable History;
- runtime_id + request_id is the only primary Tool-request correlation key;
- SafeEvidenceProjector persists only typed/bounded/sanitized evidence;
- the shared evidence-store library has no authorization authority;
- SQLite schema/migration/WAL/writer-lock semantics are correct;
- Runtime is the only normal-running writer and Tauri remains read-only while Runtime is active;
- event + request summary + terminal Approval updates are atomic and idempotent;
- bounded queue/storage failures degrade evidence rather than Policy/Tool truth;
- crash/restart produces INCOMPLETE instead of fabricated terminal results;
- filters, keyset pagination and query/get performance meet the frozen contract;
- retention and manual clear remove whole request families without reviving or mutating live authorization;
- corrupt/incompatible/busy/full/drop states remain observable;
- M01/M02/V3 behavior does not regress.

---

## 2. Entry Criteria

Before executing formal Integration:

- M03 T1–T6 are Development Complete;
- candidate commit is frozen and working tree is clean;
- all M03 task-local tests are PASS;
- schema v1/checksum is fixed;
- final candidate contains the same Shared Evidence/SQLite implementation to be used by later E2E;
- a disposable database/profile boundary is available;
- no case requires access to the real user History database.

Run preflight:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

If the candidate cannot isolate destructive DB fixtures from user data, cases requiring corruption/full/clear are BLOCKED, not run against real data.

---

## 3. Evidence Header

Record once:

~~~text
Milestone: M03
Candidate commit:
Branch / dirty state:
App version:
Runtime version:
Tunnel version:
Evidence-store crate version:
SQLite library/version:
Schema version:
Schema checksum:
Windows version:
Package/runtime build identity:
Disposable AppData/profile root:
Workspace fixture:
Config revision:
History desired config:
History effective config:
QA owner:
Execution time:
~~~

Never record:

- API Key / SecretStore values;
- physical production Workspace path;
- raw command/environment/stdout/stderr;
- real user's History database content.

Each case records exact command/runner, fixture identity, expected, actual, verdict and cleanup.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

---

## 4. Required Integration Cases

### M03-INT-01 — Canonical AuditEvent identity reaches live Activity and durable History

**AC:** M03-AC01  
**Tasks:** T1/T3

Generate real Runtime requests through the production ExecutionPipeline.

Verify for each AuditEvent-derived observation:

- live ActivityEvent.eventId equals the originating AuditEvent.event_id;
- live ActivityEvent.runtimeId equals current Runtime runtime_id;
- live ActivityEvent.requestId equals AuditEvent.request_id;
- durable evidence_events.event_id is the same canonical event_id;
- durable request key is runtime_id + request_id;
- Activity sequence is not used as durable request identity;
- no timestamp-proximity join is required.

Run at least success, Policy DENY and ASK flows.

**PASS:** one real Runtime request has one stable request identity and one stable canonical event identity across projections.

---

### M03-INT-02 — Runtime identity, Host mapping and retry boundaries

**AC:** M03-AC02  
**Task:** T1

Use two Runtime processes and controlled Host launch contexts.

Verify:

- two runtimes using the same textual request_id remain distinct because runtime_id differs;
- hostSessionId/supervisorGenerationId are stored only when explicitly supplied;
- missing Host mapping remains null/unknown rather than inferred by time;
- a retry/new Tool request receives a new request_id unless the existing execution path explicitly resumes the same request;
- approval_id attaches only to the request that produced it;
- Runtime restart never reuses the previous runtime_id.

**PASS:** Host/session/generation metadata is auxiliary and cannot replace the canonical request key.

---

### M03-INT-03 — HistoryConfig validation and generation-effective restart semantics

**AC:** M03-AC12  
**Task:** T1

Verify AppConfig v3 defaults remain:

- enabled = true;
- retentionDays = 30;
- maxStorageMiB = 256.

Boundary tests:

- retentionDays 1 and 3650 accepted;
- 0 and 3651 rejected;
- maxStorageMiB 32 and 4096 accepted;
- 31 and 4097 rejected.

Verify:

- HistoryConfig-only change advances Runtime-facing configRevision;
- running generation reports Restart Required;
- old generation retains old effective History policy until restart;
- UI/Desktop-only preference changes remain revision-neutral;
- DB path is not written into AppConfig or RuntimeConfigSnapshot.

**PASS:** M03 activates History policy without changing AppConfig schema version or desktop preference semantics.

---

### M03-INT-04 — Durable SafeEvidenceProjector privacy allowlist

**AC:** M03-AC03  
**Tasks:** T1/T3

Inject sentinel values into controlled requests/log/error fixtures:

- synthetic API key;
- SecretStore value;
- physical Workspace path;
- credential-bearing proxy URI;
- raw environment;
- raw stdout/stderr;
- large file contents;
- unrestricted error string;
- Bash command preview;
- arbitrary nested JSON metadata.

Inspect:

- EvidenceEventV1 DTO;
- persisted SQLite text/blob columns;
- History query/get DTOs;
- UI-facing serialized result.

Verify none of the forbidden sentinels survives.

Verify allowed typed evidence still exists, including logical Tool identity, decisions, safe result scalars, bounded rule IDs and coverage.

**PASS:** persistent History is a closed typed projection, not a log dump.

---

### M03-INT-05 — evidence-store dependency and authority boundary

**AC:** M03-AC04  
**Task:** T2

Audit the formal runtime/evidence-store crate/module dependency graph and compile boundaries.

Verify it does not import or construct:

- PolicyEngine;
- ApprovalManager;
- SessionGrantStore;
- ExecutionPermit;
- Tool executors;
- Tauri UI state.

Exercise its public APIs against fixtures to prove they only provide:

- DTO/schema;
- migration;
- query;
- retention;
- maintenance;
- store status/locking.

**PASS:** the shared store cannot become a second authorization engine.

---

### M03-INT-06 — SQLite schema v1, indexes and checksummed migration

**AC:** M03-AC05  
**Task:** T2

On disposable databases verify:

- a missing DB is created only through writer/maintenance ownership;
- schema_migrations, runtime_sessions, request_summary, evidence_events, approval_history and evidence_health exist;
- frozen PK/FK semantics exist;
- required query indexes exist;
- request_spans is not silently introduced in schema v1;
- schema version/checksum match the candidate constants;
- checksum mismatch is rejected;
- newer/incompatible schema is preserved and rejected;
- migration runs transactionally;
- migration backup is created before upgrade and preserved on failure;
- integrity verification occurs;
- failed migration does not auto-delete/recreate the source DB.

**PASS:** database lifecycle matches the frozen v1 contract.

---

### M03-INT-07 — WAL one-writer/read-only-reader/maintenance lock ownership

**AC:** M03-AC07, M03-AC15  
**Tasks:** T2/T4

Using real file locks and SQLite connections:

1. Start Runtime writer and acquire history.writer.lock.
2. Open Tauri/shared DAL read-only reader.
3. Query committed rows while writer continues commits.
4. Attempt a second writer.
5. Attempt offline maintenance while Runtime writer owns the lock.
6. Stop Runtime writer.
7. Acquire maintenance lock and perform allowed maintenance.

Verify:

- WAL is enabled on writer;
- read-only reader works concurrently;
- Tauri connection is query-only;
- second writer fails deterministically;
- maintenance cannot steal writer ownership;
- maintenance succeeds only after Runtime stops/releases lock;
- no forced lock break.

**PASS:** A05/A07 ownership is enforced by executable behavior.

---

### M03-INT-08 — Atomic event + summary + terminal Approval transaction and idempotency

**AC:** M03-AC06  
**Tasks:** T2/T3

For success, DENY and terminal Approval events verify a committed transaction updates all required durable projections together.

Inject failure after each logical mutation point.

Verify:

- partial batch transaction commits none of that event's required rows;
- retrying the same event_id inserts no duplicate event;
- duplicate event does not increment/mutate request summary twice;
- duplicate terminal Approval does not create a duplicate approval_history row;
- foreign-key integrity remains valid.

**PASS:** event persistence is atomic and event_id-idempotent.

---

### M03-INT-09 — Bounded non-blocking writer queue and loss coverage

**AC:** M03-AC08  
**Task:** T3

Force writer to stall and fill the queue beyond frozen capacity 1024.

Verify:

- ExecutionPipeline producer uses non-blocking enqueue;
- Tool/Policy result does not wait on SQLite fsync;
- overflow increments dropCount;
- lastDropAt is updated;
- store status enters degraded;
- affected request becomes INCOMPLETE when request identity is retained;
- if bounded loss mapping itself saturates, global UNKNOWN coverage marker is set;
- overflow never converts ALLOW/DENY/ASK or execution outcome.

Also verify batch max 64 and batch-wait max 50 ms behavior.

**PASS:** evidence loss is visible and bounded without becoming authorization behavior.

---

### M03-INT-10 — Graceful stop, bounded drain and unclean crash recovery

**AC:** M03-AC09  
**Task:** T3

Cover:

- normal stop with no pending durable work;
- normal stop with queued evidence;
- drain completing within 2 seconds;
- drain exceeding 2-second budget;
- hard Runtime termination with an open request;
- next Runtime startup/recovery.

Verify:

- normal stop closes runtime_sessions;
- remaining open requests are INCOMPLETE;
- drain timeout is evidence degradation, not Tool failure;
- stale prior session receives unclean_restart or equivalent typed end reason;
- no synthetic terminal success/failure event is inserted;
- new Runtime has a different runtime_id.

**PASS:** crash gaps remain gaps.

---

### M03-INT-11 — History filters, keyset pagination and cursor integrity

**AC:** M03-AC10  
**Tasks:** T2/T4/T5

Build a mixed fixture with multiple runtimes, tools, outcomes, decisions, approval states and timestamps.

Verify filters individually and in combinations:

- fromUtc/toUtc;
- tool;
- outcome;
- policyDecision;
- approvalState.

Pagination assertions:

- default 50;
- max 200;
- deterministic newest-first request ordering;
- no duplicate/missing row across adjacent pages under stable data;
- opaque cursor contains/validates filter fingerprint;
- changed filters invalidate previous cursor with typed INVALID_CURSOR;
- normal History browsing does not use OFFSET;
- React never receives the whole DB for client-side pagination.

**PASS:** History browsing is bounded and stable.

---

### M03-INT-12 — Frozen history.query/history.get performance SLA

**AC:** M03-AC10  
**Task:** T2/T6

Use the frozen representative fixture:

- 50,000 request_summary rows;
- 250,000 evidence_events rows;
- page size 50;
- steady-state DB within configured 256 MiB target.

After 2 uncounted warm-up calls, measure 30 calls for representative indexed history.query and history.get operations on the same candidate/environment.

Required:

- history.query P95 <= 500 ms;
- history.get P95 <= 250 ms.

Verify the implementation did not meet SLA by dropping required filters, detail events or coverage fields.

**PASS:** both frozen query targets pass.

---

### M03-INT-13 — History detail and offline read path

**AC:** M03-AC11, M03-AC15  
**Tasks:** T2/T4/T5

Persist representative completed/incomplete requests, stop Runtime, then query through the Tauri History facade.

Verify:

- app/runtime need not be started merely to read a compatible existing DB;
- list result contains safe request-level summary;
- history.get returns ordered safe event timeline;
- terminal approval_history is associated correctly;
- request/store coverage notice is present;
- missing DB returns empty/uninitialized state without creating DB;
- read-only browsing does not run migration/delete/checkpoint/vacuum.

**PASS:** compatible history remains useful while Runtime is stopped.

---

### M03-INT-14 — Historical Approval is non-actionable and cannot restore live authority

**AC:** M03-AC11, M03-AC14  
**Tasks:** T1/T4/T5

Persist:

- ApprovalRequired observation;
- AllowOnce terminal Approval;
- AllowForSession terminal Approval;
- Denied;
- Expired;
- Cancelled;
- crash after ApprovalRequired with no terminal event.

Restart Runtime/App.

Verify:

- History exposes observational states only;
- there is no durable actionable Pending object;
- history.get never returns a live ApprovalRequest;
- no History API can call approvals.resolve;
- no SessionGrant is reconstructed;
- no ExecutionPermit is reconstructed;
- old approval/request ID cannot authorize a new Tool request;
- crash-only request is INCOMPLETE, not Pending.

**PASS:** F07-AC02 / SEC-AC03 is enforced.

---

### M03-INT-15 — Online and offline Clear History ownership/barrier semantics

**AC:** M03-AC14  
**Tasks:** T3/T4

#### Runtime running

Queue old terminal history, keep one request open, then issue Clear History.

Verify:

- Tauri routes online clear to Runtime;
- Runtime inserts a writer barrier after already queued work;
- terminal/incomplete historical request families before the barrier are removed;
- current active runtime session/open request remains;
- new evidence after barrier may reappear;
- no request is cancelled/resolved;
- no Policy/Grant state changes.

#### Runtime stopped

Acquire maintenance lock and clear all evidence/session rows while preserving schema_migrations.

Verify returned counts/footprint.

**PASS:** clear is storage maintenance only.

---

### M03-INT-16 — Age retention, capacity retention and whole-request deletion

**AC:** M03-AC13  
**Task:** T3

Use small deterministic test limits while preserving production algorithm.

Verify:

- age cleanup removes eligible closed/incomplete request families past retention window;
- current open requests are excluded;
- FK cascade removes associated evidence/approval rows with no orphans;
- age cleanup runs before capacity cleanup;
- capacity cleanup removes oldest eligible whole request families;
- cleanup targets 90% of configured maxStorageMiB;
- main DB + WAL are counted by the production footprint rule;
- inability to reach cap yields EVIDENCE_CAPACITY_DEGRADED and visible loss/coverage state;
- cleanup never touches Workspace files.

Also verify production defaults remain 30d/256MiB.

**PASS:** retention is bounded, atomic at request-family level and observable.

---

### M03-INT-17 — Busy/full/corrupt/incompatible/migration/drop health projection

**AC:** M03-AC15, M03-AC16  
**Tasks:** T3/T4/T5

Inject controlled store failures:

- read busy;
- writer lock conflict;
- SQLite busy;
- disk/full or deterministic IO failure;
- corrupt DB;
- incompatible newer schema;
- migration failure/checksum mismatch;
- queue overflow;
- capacity degradation.

Verify typed status/error mapping:

- disabled;
- healthy;
- degraded;
- busy;
- incompatible;
- corrupt;
- unavailable.

UI/DTO must not call a corrupt/incompatible database “empty history”.

Verify Tool/Policy outcomes remain unchanged under each evidence fault.

**PASS:** storage truth and execution truth remain separated.

---

### M03-INT-18 — M01/M02/V3 regression and repository/package boundary

**AC:** M03-AC18  
**Task:** T6

Run affected regressions for:

- Tray/background Close/Show/Start/Stop/Quit;
- Approval/disconnect notifications;
- StartupPhaseRecorder/startup lifecycle;
- one-click diagnostics;
- AppConfig v3 migration/current fields;
- Workspace boundary;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- Approval/SessionGrant;
- controlled Bash;
- SecretStore;
- Harness Start/Stop/Restart.

Also verify:

- no new repository top-level directory;
- evidence-store lives under the frozen existing module tree;
- Control Protocol remains v1 additive;
- M02 frozen performance thresholds/evidence are not modified by M03.

**PASS:** M03 adds durable evidence without regressing existing product authority or repository layout.

---

## 5. Integration Exit Gate

M03 Integration is PASS only when:

- M03-INT-01 through M03-INT-18 are all PASS;
- all DB/fault fixtures used disposable paths;
- no case silently substituted a weaker mock for a required real Runtime/Tauri/SQLite boundary;
- query SLA uses the final candidate;
- privacy sentinels are absent;
- temp databases, backups, locks, Workspace fixtures and processes are cleaned;
- no later production change invalidates the candidate evidence.

Integration PASS does not equal M03 ACCEPTED / FROZEN.
