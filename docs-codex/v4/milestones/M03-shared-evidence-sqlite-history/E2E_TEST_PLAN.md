# M03 E2E Test Plan — Packaged Durable History and Authority Separation

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M03 — Shared Evidence and Durable SQLite History  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: independent QA / Coding Agent after M03 Development Complete + Integration PASS  
> Scope: packaged Windows candidate, real Runtime/Tunnel boundaries, durable SQLite History and real external ChatGPT where the case requires it.  
> Safety: destructive DB/fault scenarios must use a disposable Windows profile/AppData boundary or another proven isolated production-equivalent profile. Never corrupt or clear the user's real History database.

---

## 1. Objective

E2E proves the production candidate across the real product chain:

~~~text
Real ChatGPT
→ Tunnel
→ Runtime MCP
→ ExecutionPipeline / Policy / Approval
→ canonical AuditEvent
→ SafeEvidenceProjector
→ Runtime SQLite Writer
→ persisted History
→ Tauri read-only facade
→ packaged React History UI
~~~

and proves the inverse boundary:

~~~text
SQLite History
-X→ Pending Approval
-X→ SessionGrant
-X→ ExecutionPermit
-X→ Policy authority
~~~

The four frozen cases are:

- M03-E2E-01 — real external Tool requests create correctly correlated durable History across restart;
- M03-E2E-02 — packaged History UI remains queryable offline and old approvals are non-actionable;
- M03-E2E-03 — real storage degradation/recovery, retention and clear obey ownership/coverage semantics;
- M03-E2E-04 — packaged privacy/authority/regression gate including real post-maintenance Tool availability.

---

## 2. Candidate Lock

Record:

~~~text
Candidate commit:
Package type:
Package path:
Package SHA-256:
App version:
Runtime version:
Tunnel version:
Evidence-store version:
SQLite version:
Schema version/checksum:
Windows build:
Disposable profile/AppData identity:
Controlled Workspace fixture:
Config revision:
History desired/effective config:
Tunnel identity:
Proxy route:
Tester:
Execution time:
~~~

Do not record secrets or unrestricted physical paths in permanent evidence.

All four cases must use the same candidate build unless a failed case is rerun after a code change, in which case the affected formal suite must be rerun against the new candidate.

---

## 3. M03-E2E-01 — Real ChatGPT success / DENY / ASK-terminal requests persist across Runtime/App restart

**AC:** M03-AC01, AC02, AC03, AC10, AC11, AC17  
**Tasks:** T1/T3/T4/T5/T6

### Preconditions

- packaged candidate uses an isolated acceptance profile;
- History is enabled with frozen/default or explicitly recorded config;
- controlled Workspace is available;
- real ChatGPT → Tunnel → Runtime MCP chain is authorized;
- existing Tools/Policy can safely produce success, DENY and ASK outcomes.

### Action

From real ChatGPT, create at least:

1. one successful non-destructive Tool request;
2. one Policy DENY request;
3. one ASK request that reaches a terminal state (allow once/deny/expiry/cancel according to the safe fixture).

Capture request/runtime/approval identities through supported product evidence.

Then:

4. query History while Runtime is still active;
5. explicitly stop Harness;
6. quit/reopen the packaged app;
7. keep Runtime stopped initially and query History again;
8. start Harness and query again.

### Assertions

- each real external request appears under the correct runtime_id + request_id;
- no time-based/manual fuzzy matching is needed;
- live Activity and History event IDs agree where the same AuditEvent is projected;
- success is not confused with DENY/ASK;
- terminal Approval evidence is associated with the correct request;
- persisted records survive Runtime stop, App quit/reopen and a new Runtime identity;
- old rows remain historical and do not change to the new runtime_id;
- coverage is truthful;
- no secret/raw path/command body is visible.

If real ChatGPT is unavailable, this case is BLOCKED; a local direct MCP substitute cannot PASS this exact external boundary.

**PASS:** real product requests become durable, correctly correlated evidence across restart.

---

## 4. M03-E2E-02 — Packaged History filtering/detail/offline read and old Approval non-reactivation

**AC:** M03-AC04, AC07, AC10, AC11, AC14, AC15, AC16  
**Tasks:** T2/T4/T5

Use the durable data created by E2E-01 plus additional controlled records if needed.

### Action

1. With Harness stopped, open Activity → History.
2. Filter by time, Tool, outcome, Policy decision and Approval state.
3. Navigate more than one page using the production cursor path.
4. Open request details for success, DENY and Approval-related records.
5. Try every available History action/navigation.
6. Start Harness and repeat representative reads while Runtime writer is active.

### Assertions

- compatible History is readable with Runtime stopped;
- browsing History does not start Runtime;
- filters return the expected records;
- pagination has no duplicates/gaps under stable data;
- request detail shows safe ordered timeline;
- COMPLETE / INCOMPLETE / UNKNOWN is visible where applicable;
- terminal Approval evidence is visible but non-actionable;
- there is no Approve/Deny/replay-authorize action in History;
- an old Pending-looking historical observation cannot be approved after restart;
- no SessionGrant/Permit is revived;
- running-state Tauri reads coexist with Runtime writer without a second writer;
- DB path/physical Workspace path is not exposed in UI.

**PASS:** packaged History is useful offline but cannot become live authorization.

---

## 5. M03-E2E-03 — Controlled storage degradation → truthful coverage → recovery / retention / clear

**AC:** M03-AC05–AC09, AC13–AC16  
**Tasks:** T2/T3/T4/T5/T6

Use only the isolated acceptance profile.

### Phase A — ownership/degradation

Trigger at least one real production-equivalent writer/maintenance fault from:

- second-writer lock contention;
- deterministic SQLite busy/IO/full fixture supported by the QA harness;
- incompatible/corrupt disposable DB.

Run/observe the packaged app.

Required:

- History/evidence status becomes the correct typed degraded/busy/corrupt/incompatible state;
- Runtime Policy/Tool behavior remains governed by the existing Runtime authority;
- no DB is silently deleted/recreated;
- loss/coverage warning is visible where applicable.

Restore the controlled condition.

### Phase B — retention

Populate enough disposable historical data to exercise the real retention algorithm with QA-configured bounded limits or accelerated fixture support that preserves the production algorithm.

Required:

- whole request families are deleted;
- active/open request is not deleted;
- no orphan child rows;
- capacity cleanup aims at 90% target after age cleanup;
- result is reflected in packaged History/status.

### Phase C — clear

With Runtime running:

- execute Clear History through the UI;
- verify online Runtime-writer barrier semantics;
- verify current open request/session is not mutated.

Then stop Runtime and execute offline clear.

Required:

- offline maintenance obtains exclusive ownership;
- schema remains usable;
- clear reports results;
- no Workspace file is touched.

**PASS:** production degradation/maintenance semantics are visible and authority-safe.

---

## 6. M03-E2E-04 — Packaged privacy, authority isolation and post-maintenance real Tool regression

**AC:** M03-AC03, AC11, AC14, AC18  
**Tasks:** T1/T4/T5/T6

### Preconditions

- same final packaged candidate;
- controlled Workspace contains privacy sentinels and safe read fixture;
- History database has undergone at least one restart and clear/maintenance cycle;
- real ChatGPT route is authorized.

### Action

1. Inspect History list/detail and Settings after prior E2E cases.
2. Copy any user-visible request IDs/evidence text supported by UI.
3. Verify forbidden sentinels are absent from UI and persisted DB through QA inspection.
4. Confirm no old History record can approve/replay-authorize a request.
5. Start Harness normally.
6. From real ChatGPT, perform a fresh non-destructive Tool call.
7. Confirm the new request succeeds under current live Runtime policy and creates new History independently.
8. Exercise affected Tray/notification/diagnostics flows sufficiently to detect M03 regressions.

### Assertions

- no API key/SecretStore value;
- no physical Workspace path;
- no raw env/stdout/stderr;
- no unrestricted file content;
- no durable Bash command preview text;
- no History-derived SessionGrant/Permit;
- clear/retention did not damage current Runtime security semantics;
- post-maintenance real ChatGPT Tool call succeeds when current policy permits;
- new durable evidence appears with the new Runtime/request identity;
- M01/M02/V3 affected functionality remains operational;
- no duplicate Runtime/Tunnel/writer process exists.

If real ChatGPT is unavailable, the external part remains BLOCKED; do not substitute local MCP and claim full E2E PASS.

**PASS:** M03 is durable and observable without leaking data or becoming authority.

---

## 7. E2E Verdict Rules

Use only:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Examples:

- real ChatGPT not authorized/available → external-required case BLOCKED;
- corrupt disposable DB auto-deleted and recreated → E2E-03 FAIL;
- old Approval can be acted upon from History → E2E-02/04 FAIL;
- History survives restart but request identity is joined by timestamp → E2E-01 FAIL;
- storage fault changes Policy DENY into ALLOW or vice versa → E2E-03/04 FAIL;
- failure fixture accidentally touched the real user DB → stop testing; affected evidence is invalid and the case FAIL/BLOCKED pending incident review.

---

## 8. E2E Exit Gate

M03 E2E is PASS only when:

- M03-E2E-01 through M03-E2E-04 all PASS;
- all use the same final candidate identity;
- external ChatGPT cases were actually executed where required;
- History survives stop/restart/reopen;
- offline read works;
- old authorization cannot be restored;
- storage degradation/retention/clear is demonstrated on isolated data;
- privacy sentinels are absent;
- temp profile/databases/backups/locks/processes are cleaned;
- no later code change invalidates the evidence.

E2E PASS does not mark M03 ACCEPTED / FROZEN.
