# M00 Manual Acceptance Checklist — As-Is Baseline and Architecture Contract Gate

> 存放规则遵守 [AGENTS.md](../../../../AGENTS.md)。本目录只保留六份正式文档，固定测试数据沿用 `fixtures/v4/m00/`。独立报告、独立 ADR、Evidence Index 与独立 POC 工程已按用户要求删除，不恢复；删除不改写已经实际执行的 Development / Integration / E2E-03 结果。未具备真实边界证据的验收项保持 BLOCKED，未执行项保持 NOT EXECUTED，不得记为 PASS。

> Status: Manual Acceptance APPROVED BY USER — M00 ACCEPTED / FROZEN under explicit E2E-02 deferral exception; E2E-01 PASS; E2E-02 DEFERRED BY USER (NOT EXECUTED; not PASS); E2E-03 PASS
> Milestone: M00 — As-Is Baseline and Architecture Contract Gate  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md, E2E_TEST_PLAN.md  
> Owner: Human Reviewer  
> Purpose: final human sign-off that M00 has produced a truthful, usable implementation contract for M01/M02/M03.  
> Manual Acceptance does not replace failed or blocked automated evidence. This checklist is frozen before implementation and may not be relaxed merely to convert a development result into acceptance.

---

## 1. Responsibility Boundary

~~~text
Task-local Unit/POC     → Coding Agent
Integration             → independent QA/Coding Agent
E2E                     → QA + explicitly authorized external execution
Manual Acceptance       → Human Reviewer
~~~

The Human Reviewer evaluates:

- whether the As-Is baseline is understandable and matches observed product behavior;
- whether authority/security boundaries are unambiguous;
- whether Evidence/SQLite/Config/Eval POC conclusions are strong enough to guide downstream SPECs;
- whether evidence records are truthful about limitations;
- whether Architecture ADR decisions are acceptable;
- whether M01/M02/M03 may safely unlock.

The Human Reviewer does not need to manually reproduce:

- SQLite transaction rollback injection;
- second-writer race tests;
- migration checksum corruption;
- sentinel secret scans;
- automated fixture hashing;
- mock/provider runner mechanics;
- every current Runtime unit test.

Those must already have automated PASS evidence.

---

## 2. Entry Criteria

Manual Acceptance starts only when:

- M00 Development Complete.
- M00 Integration = PASS.
- M00 E2E = PASS.
- 基线事实与源码引用（在既有 SPEC 中维护） is complete.
- 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） is complete.
- SQLite 验证结论（对话交付，不自动新建报告） is complete.
- 评测可行性结论（对话交付，不自动新建报告） contains the real-model smoke evidence.
- no required E2E is BLOCKED/INCONCLUSIVE.
- candidate build/commit is fixed.
- cleanup evidence is available in the existing records.

If any required automated layer is FAIL/BLOCKED/INCONCLUSIVE:

~~~text
Manual Acceptance = BLOCKED
~~~

Do not compensate with subjective approval.

Current closeout evaluation: Development Complete = PASS, Integration = PASS (12/12), E2E-01 = PASS and E2E-03 = PASS. E2E-02 remains DEFERRED BY USER / NOT EXECUTED and is not a PASS claim. The user explicitly approved Manual Acceptance and the M00 acceptance/freeze exception; the later Agent Evaluation Milestone must revalidate the real model closed loop.

---

## 3. Acceptance Identity

Reviewer fills:

~~~text
Candidate production commit:
Evidence/QA commit:
Branch:
Dirty state:
App build/version:
Runtime SHA-256:
Tunnel identity/version:
CURRENT_SCHEMA_VERSION:
Integration verdict:
E2E verdict:
Reviewer:
Date:
~~~

Confirm:

- [ ] The build reviewed is the same production candidate covered by E2E, or any difference is evidence-only and cannot affect production behavior.
- [ ] There is no untested production code change after automated evidence.
- [ ] CURRENT_SCHEMA_VERSION is still 2.

---

## 4. M00-MAN-01 — Current Product Baseline and Ownership Sign-off

**AC:** M00-AC01, M00-AC02; supports M00-AC11  
**Task:** T1

### 4.1 Review 基线事实与源码引用（在既有 SPEC 中维护）

Check that the baseline explicitly covers:

- [ ] Tauri Host lifecycle ownership.
- [ ] Runtime authorization ownership.
- [ ] Tunnel bridge role.
- [ ] React/UI presentation role.
- [ ] RuntimeSupervisor generation/process identity.
- [ ] ExecutionPipeline/ExecutionPermit boundary.
- [ ] WorkspaceGuard.
- [ ] PolicyEngine.
- [ ] ApprovalManager and SessionGrantStore.
- [ ] AuditEvent/AuditSink.
- [ ] ActivityStore/EventBus.
- [ ] AppConfig current schema/migration.
- [ ] SecretReference/SecretStore.
- [ ] current tool catalog.
- [ ] explicit NOT_IMPLEMENTED/NOT_VERIFIED status for V4 capabilities not yet delivered.

### 4.2 Observe current UI/lifecycle truth

Using the acceptance build:

- [ ] Open the current main UI.
- [ ] Confirm Ready/Stopped/Error presentation corresponds to the real Harness lifecycle rather than merely an open window/process.
- [ ] Confirm any absence of V4 Tray/History/grep/etc. is not hidden or falsely claimed as implemented.
- [ ] Confirm current product behavior matches the documented baseline at a user-visible level.

### 4.3 Review real ChatGPT baseline E2E evidence

- [ ] M00-E2E-01 shows a real external ChatGPT tool call.
- [ ] The call crossed Tunnel to the recorded Runtime candidate.
- [ ] The Workspace result is correct.
- [ ] The evidence does not falsely call current Activity “V4 durable History/Trace”.

**Manual verdict M00-MAN-01:** PASS / FAIL / BLOCKED  
Notes:

---

## 5. M00-MAN-02 — Shared Evidence and Authorization Boundary Sign-off

**AC:** M00-AC03, M00-AC04, M00-AC05  
**Task:** T2

Review evidence fixtures and Integration results.

Confirm:

- [ ] event_id, runtime_id, request_id, host_session_id, supervisor generation and approval_id have distinct meanings.
- [ ] Primary Tool request correlation is runtime_id + request_id, not wall-clock proximity.
- [ ] Unknown/missing identity mappings are called out rather than guessed.
- [ ] COMPLETE / INCOMPLETE / UNKNOWN semantics are understandable.
- [ ] ASK/Pending is visibly distinct from execution success.
- [ ] Policy DENY is distinct from executor failure.
- [ ] Tool success is not described as Agent task success.
- [ ] Sensitive fields are allowlisted/minimized, not “log everything then redact later”.
- [ ] Physical paths/raw file body/raw env/raw unrestricted argv/stdout/stderr/secrets are not present in evidence fixtures/evidence records.
- [ ] Historical approval/session evidence is data only.
- [ ] No evidence/History object can restore Pending Approval, Session Grant or ExecutionPermit.
- [ ] POC/evidence failure cannot produce an ALLOW result.

Reviewer should inspect at least one success fixture, one deny/approval fixture, one incomplete fixture and the unsafe-input test evidence.

**Manual verdict M00-MAN-02:** PASS / FAIL / BLOCKED  
Notes:

---

## 6. M00-MAN-03 — SQLite Architecture and Config Migration Contract Sign-off

**AC:** M00-AC06, M00-AC07, M00-AC08, M00-AC09  
**Tasks:** T3/T4

### 6.1 SQLite POC review

From SQLite 验证结论（对话交付，不自动新建报告） confirm:

- [ ] Normal-running ownership is one Runtime writer.
- [ ] Tauri-like connection is read-only while writer is running.
- [ ] Second writer/maintenance ownership rejection is explicit, not just accidental SQLITE_BUSY behavior.
- [ ] Offline maintenance only begins after writer ownership ends.
- [ ] WAL/read behavior was actually executed.
- [ ] event + request summary atomicity was tested.
- [ ] duplicate event_id is idempotent.
- [ ] crash/incomplete request is not fabricated into success/failure.
- [ ] failed migration preserves diagnosable source DB.
- [ ] history records cannot reconstruct live authorization.
- [ ] POC DB lived only in disposable/temp paths.
- [ ] execution evidence clearly states production M03 DDL/retention/query implementation is still future work.

### 6.2 Config v2→v3 contract review

Confirm:

- [ ] Production CURRENT_SCHEMA_VERSION is still 2.
- [ ] M00 did not make current ConfigStore write v3.
- [ ] representative v2 fields are preserved in candidate migration contract.
- [ ] SecretReference remains opaque.
- [ ] raw API key is absent from fixtures/evidence records.
- [ ] future host-only desktop fields remain outside RuntimeConfigSnapshot.
- [ ] future history settings do not become authorization state.
- [ ] malformed known values fail rather than silently reset.
- [ ] future schema failure is non-destructive.
- [ ] AppConfig schema evolution does not itself imply protocol/v2.

**Manual verdict M00-MAN-03:** PASS / FAIL / BLOCKED  
Notes:

---

## 7. M00-MAN-04 — Controlled Agent Evaluation Feasibility and Truthfulness Sign-off

**AC:** M00-AC10  
**Task:** T5

Review 评测可行性结论（对话交付，不自动新建报告） and M00-E2E-02.

Confirm:

- [ ] A real controllable Agent/Model was actually executed; not only a mock/script.
- [ ] Model/provider/adapter/version are recorded.
- [ ] Case ID/hash and Workspace fixture hash are recorded.
- [ ] Tool schema identity/version is recorded.
- [ ] Runtime candidate identity is recorded.
- [ ] Agent emitted actual Tool call(s).
- [ ] Calls went through the real Harness MCP Runtime.
- [ ] Tool selection is graded against explicit ground truth/allowed set.
- [ ] Arguments are independently schema-validated.
- [ ] Runtime call outcome is recorded separately.
- [ ] Final task state is independently graded.
- [ ] Negative control demonstrates “tool call success” can still yield task failure.
- [ ] Evidence record does not expose or pretend to know model hidden reasoning.
- [ ] Passive production telemetry is not described as Tool selection accuracy.
- [ ] Cost/usage is only claimed if actually observable with defined semantics.
- [ ] Limits/cleanup are documented.
- [ ] No provider credential is committed.

Reviewer judgment:

- [ ] This is strong enough evidence that M04 can build a full repeatable Evaluation baseline.
- [ ] Remaining nondeterminism/cost/adapter risks are explicit and acceptable for M04 planning.

**Manual verdict M00-MAN-04:** PASS / FAIL / BLOCKED  
Notes:

---

## 8. M00-MAN-05 — ADR Freeze, Isolation and Downstream Unlock Decision

**AC:** M00-AC12, M00-AC13, M00-AC14  
**Task:** T6

Review 架构决策及阻塞项（在既有 SPEC/PLAN 中维护）.

Required ADRs:

- [ ] A01
- [ ] A04
- [ ] A05
- [ ] A06
- [ ] A07
- [ ] A08
- [ ] A10
- [ ] A15
- [ ] A17

For every ADR:

- [ ] verdict is ACCEPTED / REVISED / BLOCKED;
- [ ] evidence is linked;
- [ ] downstream constraint/owner is explicit;
- [ ] open risk is explicit.

Change control:

- [ ] No REVISED ADR remains only inside M00 docs.
- [ ] Any REVISED decision was handled through a separate explicitly approved upstream change and the affected V4 PRD/Architecture/Roadmap plus M00 contract were re-frozen.
- [ ] No BLOCKED ADR is being treated as accepted.

Production isolation:

- [ ] SQLite POC is not a production dependency.
- [ ] Eval spike is not a production Runtime Agent loop.
- [ ] No production History DB/API is enabled.
- [ ] No new M00 user-facing Tool is exposed.
- [ ] No Policy/Approval/Workspace semantics were weakened.
- [ ] No raw secret is tracked.
- [ ] no test-owned child process remains.
- [ ] no disposable DB/config/Workspace residue remains.

Downstream decision:

- [ ] M00 contract is precise enough to generate/execute M01 SPEC without re-deciding M00 authority.
- [ ] M00 contract is precise enough to generate/execute M02 SPEC without re-deciding M00 authority.
- [ ] M00 contract is precise enough to generate/execute M03 SPEC without re-deciding evidence/SQLite ownership.
- [ ] M04 has a viable controlled Eval path once M03 is complete.

**Manual verdict M00-MAN-05:** PASS / FAIL / BLOCKED  
Notes:

---

## 9. Final Manual Acceptance Summary

Fill PASS/FAIL/BLOCKED only after M00-MAN-01 through M00-MAN-05. If the entry criteria are unmet, record NOT EXECUTED with the blocking prerequisite, as in the current closeout below.

| Manual case | Verdict | Notes/Evidence |
| --- | --- | --- |
| M00-MAN-01 | PASS (USER-APPROVED) | User-approved M00 closeout; E2E-01 is recorded PASS. |
| M00-MAN-02 | PASS (USER-APPROVED) | User-approved M00 closeout; Integration is 12/12 PASS and evidence boundaries remain unchanged. |
| M00-MAN-03 | PASS (USER-APPROVED) | User-approved M00 closeout; SQLite/config contract status remains unchanged. |
| M00-MAN-04 | DEFERRED BY USER | Mirrors E2E-02: NOT EXECUTED, not PASS; later Agent Evaluation Milestone must revalidate the real model closed loop. |
| M00-MAN-05 | PASS (USER-APPROVED) | M00 freeze and downstream unlock approved under the explicit E2E-02 deferral exception. |

Automated prerequisite summary:

~~~text
Development: PASS
Integration: PASS (12/12)
E2E: E2E-01 PASS; E2E-02 DEFERRED BY USER (NOT EXECUTED; not PASS); E2E-03 PASS
Cleanup: PASS for completed automated layers; E2E-02 deferred for future Agent Evaluation Milestone
Manual Acceptance: APPROVED BY USER under the explicit E2E-02 deferral exception
~~~

### Final decision

Choose exactly one:

- [x] M00 — ACCEPTED / FROZEN
- [ ] M00 — FAIL
- [ ] M00 — BLOCKED
- [ ] M00 — INCONCLUSIVE

M00 may be ACCEPTED / FROZEN only if:

- all M00-AC01 through M00-AC14 are supported by evidence;
- Integration PASS;
- E2E PASS;
- M00-MAN-01 through M00-MAN-05 PASS;
- required ADRs are accepted/reconciled;
- no blocker remains;
- candidate/evidence identity is stable;
- cleanup is complete.

Closeout exception recorded on 2026-10-24: the user explicitly approved M00 ACCEPTED / FROZEN while deferring M00-E2E-02. This does not convert E2E-02 or M00-MAN-04 into PASS; E2E-02 remains NOT EXECUTED and must be revalidated in the later Agent Evaluation Milestone with a real model closed loop.

When ACCEPTED / FROZEN:

~~~text
M01 / M02 / M03 are unlocked.
M04 remains dependent on M03.
~~~

The ACCEPTED / FROZEN branch is reached by explicit user approval of the 2026-10-24 closeout exception. M01/M02/M03 are unlocked; E2E-02 remains NOT EXECUTED and must be revalidated in the later Agent Evaluation Milestone before relying on its feasibility result.

Acceptance itself must be recorded in 人工验收签认（沿用既有验收清单） and evidence locations in 验收证据定位（沿用既有验收清单）. This checklist is the human sign-off source, not a substitute for those final records.
