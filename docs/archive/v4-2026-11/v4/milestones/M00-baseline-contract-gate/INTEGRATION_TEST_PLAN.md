# M00 Integration Test Plan — As-Is Baseline and Architecture Contract Gate

> 存放规则遵守 [AGENTS.md](../../../../AGENTS.md)。本目录只保留六份正式文档，固定测试数据沿用 `fixtures/v4/m00/`。独立报告、独立 ADR、Evidence Index 与独立 POC 工程已按用户要求删除，不恢复；删除不改写已经实际执行的 Development / Integration / E2E-03 结果。未具备真实边界证据的验收项保持 BLOCKED，未执行项保持 NOT EXECUTED，不得记为 PASS。

> Status: Integration PASS (12/12); M00 ACCEPTED / FROZEN under explicit user-approved E2E-02 deferral — E2E-01 PASS; E2E-02 DEFERRED BY USER (NOT EXECUTED; not a M00 blocker); E2E-03 PASS; Manual Acceptance APPROVED BY USER
> Milestone: M00 — As-Is Baseline and Architecture Contract Gate  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent Coding/QA Agent after T1–T6 Development Complete  
> Scope: current-main ownership truth, Shared Evidence Contract, isolated SQLite POC, Config v2→v3 contract, controlled-eval runner integration, POC isolation.  
> This document defines the frozen validation contract. The recorded execution result is M00-INT-01 through M00-INT-12 = PASS (12/12); this local/controlled result does not prove the external E2E boundaries or Manual Acceptance.

---

## 1. Objective

Integration verifies that the M00 contracts work across real implementation boundaries rather than only inside unit helpers.

The required proof chain is:

~~~text
current source / manifests
→ authoritative ownership map
→ real Runtime AuditEvent / AuditSink
→ typed safe evidence projection
→ isolated SQLite ownership + failure semantics
→ current AppConfig v2 + future v3 contract fixtures
→ isolated real Runtime/MCP + evaluation runner mechanics
→ no production feature leakage
~~~

Integration must answer:

- Is the M00 As-Is baseline actually true for the candidate commit?
- Can current Runtime evidence be correlated and safely projected without creating a second authorization authority?
- Is the proposed SQLite one-writer/read-only/offline-maintenance architecture viable?
- Can a future v2→v3 config migration preserve current user state while keeping production schema at v2 during M00?
- Can the evaluation runner drive a real isolated Runtime/MCP and independently score task state?
- Are all POCs isolated from production Runtime/Tauri and removable?

Integration does not prove the external ChatGPT chain or a real networked Agent/Model run; those are E2E responsibilities.

This plan is frozen before T1 begins. Implementation may satisfy these cases with concrete code/fixtures/evidence, but may not weaken or delete a case after observing an inconvenient result without the M00 Change Control process.

---

## 2. Entry Criteria

Before Integration execution:

- T1–T6 are Development Complete.
- 基线事实与源码引用（在既有 SPEC 中维护） exists.
- 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） exists in development-state form.
- Shared Evidence Contract fixtures exist.
- SQLite 验证结论（对话交付，不自动新建报告） exists.
- Config contract fixtures/tests exist.
- 评测可行性结论（对话交付，不自动新建报告） exists at least with local runner/adapter preparation evidence.
- task-local Unit/POC tests are PASS.
- production CURRENT_SCHEMA_VERSION is still 2.
- no unresolved compile failure exists.

Minimum preflight:

~~~text
cargo check --workspace
git diff --check
~~~

If implementation prerequisites are missing (not the current result):

~~~text
M00 Integration = BLOCKED_BY_IMPLEMENTATION (not current; current result is PASS 12/12)
~~~

Do not weaken a case to avoid a BLOCKED verdict.

---

## 3. Test Environment and Evidence Header

Every execution record, whether retained in this plan, existing fixtures or dialogue, must record:

~~~text
Milestone: M00
Candidate commit:
Branch:
Dirty state:
Windows version:
Rust toolchain:
App package/version:
Runtime package/version:
Tunnel version/identity if used:
Protocol package/version:
CURRENT_SCHEMA_VERSION:
SQLite POC library/version:
Eval runner version:
Workspace fixture:
Config fixture:
Execution time:
QA owner:
~~~

Every case records:

- Case ID
- M00 AC
- exact command / runner
- fixture identity/hash
- actual result
- PASS / FAIL / BLOCKED / INCONCLUSIVE
- evidence location (existing source/test output, fixture or dialogue; no standalone report is required)
- cleanup result

A screenshot alone is not sufficient Integration evidence.

---

## 4. Integration Boundary

Integration may use:

- production Rust types and real RuntimeCore/ExecutionPipeline/AuditSink;
- real protocol/config serialization;
- temporary Workspace directories;
- temporary config directories;
- isolated POC SQLite databases under temp directories;
- a real local Runtime child process and loopback MCP endpoint;
- deterministic/local test adapter for runner mechanics;
- synthetic secret/path sentinels;
- source/dependency inspection.

Integration must not:

- use the user’s real AppData database as a POC target;
- migrate the real user config to v3;
- call a paid/networked model without the separately authorized E2E step;
- use passive ChatGPT traffic to claim controlled selection accuracy;
- persist M00 POC evidence through production Activity/History;
- modify current Policy/Approval semantics;
- add a test-only authorization bypass;
- call an internal helper while claiming the full component boundary was tested.

---

## 5. Required Integration Cases

### M00-INT-01 — Current source/manifests match the source-backed baseline

**AC:** M00-AC01  
**Task:** T1

Validate 基线事实与源码引用（在既有 SPEC 中维护） against the exact candidate commit.

At minimum confirm:

- root Cargo workspace production members;
- package versions;
- CURRENT_SCHEMA_VERSION = 2;
- Runtime MCP transport implementation;
- HarnessController lifecycle operation serialization;
- RuntimeSupervisor process/generation identity ownership;
- current AuditEvent/AuditSink and ActivityStore wiring;
- ConfigStore current migration path;
- ToolRegistry/ToolConfig/Policy default ownership;
- no claimed production V4 SQLite/grep/full Eval/Tray/notification feature without source evidence.

Method:

- source/manifests are inspected from the candidate;
- referenced focused tests are executed;
- every material baseline claim has an existing path or is explicitly NOT_VERIFIED / NOT_IMPLEMENTED.

**PASS:** no material baseline statement contradicts candidate source/test behavior.

**FAIL:** a claimed implemented/verified behavior is unsupported or contradicted and 基线事实与源码引用（在既有 SPEC 中维护） is not corrected.

---

### M00-INT-02 — Architecture ownership has one authority per responsibility

**AC:** M00-AC02  
**Task:** T1  
**Upstream:** A01

Exercise or inspect real component boundaries to confirm:

- Tauri Host owns desktop/Harness lifecycle orchestration;
- Runtime owns Tool availability, Workspace authorization, Policy, Approval, Session Grants and ExecutionPermit;
- Tunnel remains a bridge rather than a Policy/lifecycle authority;
- React/UI consumes state and does not construct authoritative Ready/Policy/Approval results;
- Audit/Activity/POC code cannot return an execution permission.

Include at least one real Runtime request showing authorization outcome originates from Runtime, not from the UI/test projection.

**PASS:** ownership matches Architecture and no second authority is introduced by M00.

---

### M00-INT-03 — Stable request identity and coverage semantics

**AC:** M00-AC03  
**Task:** T2  
**Upstream:** SEC-AC01, A04, A08

Generate real Runtime AuditEvent sequences for at least:

- successful request;
- Policy DENY;
- ASK/Approval lifecycle if available in the local integration surface;
- execution failure/cancel path.

Verify:

- event_id is unique per observation;
- request_id is stable inside one request;
- two near-simultaneous requests remain distinct;
- correlation never depends on timestamp proximity;
- runtime_id is either explicitly available/mapped or recorded as a known M03 gap rather than invented;
- COMPLETE / INCOMPLETE / UNKNOWN semantics are representable by the M00 fixture contract;
- missing terminal observation does not become fabricated success/failure.

**PASS:** identity/correlation is deterministic and missing mappings are explicit.

---

### M00-INT-04 — Real AuditEvent → typed safe evidence projection and redaction

**AC:** M00-AC04  
**Task:** T2  
**Upstream:** SEC-AC02, A04

Feed real current AuditEvent values through the M00 test projection/fixture validator.

Positive checks:

- tool identity/capability;
- logical Workspace result;
- Policy decision/reason;
- bounded matched rule IDs;
- approval ID/decision where applicable;
- typed lifecycle/outcome/failure;
- bounded scalar duration/count/result facts.

Negative/sentinel checks must attempt to introduce:

- physical path;
- raw file body;
- unrestricted command/argv;
- raw environment;
- raw stdout/stderr;
- secret sentinel;
- oversized rule/error strings;
- arbitrary nested JSON.

Search emitted fixture/evidence text for every sentinel.

**PASS:** only allowed typed/bounded facts survive and all prohibited sentinels are absent or explicitly redacted according to contract.

---

### M00-INT-05 — Evidence/history cannot restore or alter authorization

**AC:** M00-AC05  
**Task:** T2/T3  
**Upstream:** SEC-AC03, F07-AC02, A17

Use fixture and POC historical records containing:

- prior approval_id;
- prior approval decision;
- Session-Grant-like facts;
- successful historical execution.

Verify there is no production or POC path that converts them into:

- Pending Approval;
- Session Grant;
- ExecutionPermit;
- current Tool enabled state;
- current Policy decision.

Additionally run a real Runtime request after loading/reading historical POC data and prove the Runtime performs its normal current authorization path.

**PASS:** history is observational only.

---

### M00-INT-06 — SQLite one-writer + read-only reader + deterministic writer ownership

**AC:** M00-AC06  
**Task:** T3  
**Upstream:** A05, A06, A07

Against the isolated SQLite POC:

1. create DB in a fresh temp directory;
2. acquire writer ownership;
3. enable/verify the intended WAL/read configuration;
4. open a read-only Tauri-like connection;
5. write and commit event data from writer A;
6. prove reader observes committed data;
7. prove reader cannot write/migrate/clear;
8. attempt writer B/maintenance while writer A is active;
9. prove the explicit ownership contract rejects it deterministically.

**PASS:** topology is viable without relying only on incidental SQLITE_BUSY timing.

---

### M00-INT-07 — SQLite atomicity, idempotency, correlation and crash/incomplete behavior

**AC:** M00-AC07  
**Task:** T3

Use multiple request identities with deliberately close timestamps.

Verify:

- event + request-summary update is one transaction;
- injected pre-commit error exposes neither half;
- duplicate event_id does not double-count/logically duplicate;
- requests are queried by runtime_id + request_id rather than timestamp;
- received/policy/execution-started with no terminal event reopens as INCOMPLETE/UNKNOWN;
- abrupt writer stop never synthesizes terminal success/failure.

**PASS:** all persistence semantics match the M00 contract.

---

### M00-INT-08 — SQLite migration failure, offline maintenance and observational failure semantics

**AC:** M00-AC07, supports M00-AC05/M00-AC14  
**Task:** T3

Validate:

- explicit schema version/checksum;
- successful POC forward migration;
- checksum/version mismatch;
- injected migration failure;
- failed migration preserves the original DB for diagnosis;
- writer stopped → exclusive offline maintenance lock can be acquired;
- bounded maintenance succeeds;
- writer/reader reopen coherently afterward;
- simulated persistence failure does not alter an independent Runtime Policy/Permit result.

**PASS:** maintenance and failure semantics are safe, diagnosable and non-authoritative.

---

### M00-INT-09 — Future AppConfig v2→v3 contract preserves current user semantics

**AC:** M00-AC08  
**Task:** T4  
**Upstream:** A15

Using source-controlled fixtures, validate preservation of:

- configRevision;
- Workspace;
- Tunnel ID;
- proxy;
- tunnel-client selection;
- autoDetect;
- SecretReference provider/key;
- Security mode;
- UI theme/language;
- Tool enable/disable;
- Policy defaults and user rules;
- Bash configuration.

Also validate:

- malformed known fields fail;
- unsupported future schema is non-destructive;
- raw API key does not exist in fixture/evidence;
- host-only future fields do not enter RuntimeConfigSnapshot;
- history settings do not create authorization state.

**PASS:** all current semantics survive the candidate contract.

---

### M00-INT-10 — Production config remains schema v2 and control protocol remains v1 additive

**AC:** M00-AC09  
**Task:** T4

Confirm against production candidate:

~~~text
CURRENT_SCHEMA_VERSION = 2
~~~

Create/load/save a disposable real current config and verify production ConfigStore still writes schema v2.

Confirm:

- M00 does not create protocol/v2;
- current control protocol package/path remains v1;
- POC candidate-v3 fixture is not accepted as current production config;
- no migration/rewrite of a real user profile occurs;
- any actual wire incompatibility discovered is recorded BLOCKED/REVISED rather than hidden.

**PASS:** M00 proves the future migration contract without shipping it.

---

### M00-INT-11 — Eval runner mechanics integrate with an isolated real Runtime/MCP

**AC:** supports M00-AC10; final real-model proof is M00-E2E-02  
**Task:** T5  
**Upstream:** A10, F12-AC01/03/04

Use:

- isolated Workspace fixture;
- real production Runtime process;
- real loopback MCP tools/list + tools/call path;
- evaluation runner;
- deterministic/local adapter or replayable provider stub only for runner mechanics.

Verify the runner can:

- discover/use real Tool schema;
- validate emitted arguments;
- forward a tool call over real MCP;
- receive real Runtime result;
- enforce max step/time limits;
- compute fixture/case/tool-schema identities;
- independently grade expected final state;
- distinguish MCP success from task success using a negative scorer fixture;
- cleanup Runtime/Workspace.

This case must not be used to claim a real model completed the task.

**PASS:** runner plumbing is genuinely connected to Runtime/MCP and scorer semantics are correct.

---

### M00-INT-12 — POC isolation, no production feature leakage, cleanup and change-control consistency

**AC:** M00-AC12, M00-AC13, M00-AC14  
**Task:** T6

Inspect dependency graph/build outputs and execute cleanup checks.

Verify:

- production root workspace crates do not depend on SQLite POC;
- Runtime/Tauri do not invoke Eval spike;
- no real AppData SQLite DB was created by M00;
- CURRENT_SCHEMA_VERSION is 2;
- no new user-facing Tool is registered;
- no production History/Trace API is exposed;
- no Policy/Approval/Workspace behavior changed to support a POC;
- no raw credential exists in tracked fixtures/evidence records;
- all test Runtime/Tunnel/child processes are stopped;
- temp DB/config/Workspace resources are removed;
- 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） contains ACCEPTED/REVISED/BLOCKED for required ADRs;
- any REVISED ADR has already been reconciled through a separately approved upstream PRD/Architecture/Roadmap change; an implementation-agent self-edit is not sufficient evidence.

**PASS:** M00 remains a clean architecture-contract gate.

---

## 6. Required Case Matrix

| Case | Primary AC | Expected evidence |
| --- | --- | --- |
| M00-INT-01 | AC01 | source/test-backed baseline audit |
| M00-INT-02 | AC02 | ownership boundary proof |
| M00-INT-03 | AC03 | real AuditEvent identity sequences |
| M00-INT-04 | AC04 | projection + sentinel/redaction evidence |
| M00-INT-05 | AC05 | historical-data/non-authorization proof |
| M00-INT-06 | AC06 | SQLite writer/reader ownership run |
| M00-INT-07 | AC07 | transaction/idempotency/crash run |
| M00-INT-08 | AC07/AC14 | migration/maintenance/failure run |
| M00-INT-09 | AC08 | config fixture preservation matrix |
| M00-INT-10 | AC09 | production schema/protocol check |
| M00-INT-11 | AC10 support | real Runtime/MCP runner integration |
| M00-INT-12 | AC12–AC14 | dependency/isolation/cleanup audit |

### Recorded execution result

- M00-INT-01 through M00-INT-12: PASS (12/12).
- M00 Integration: PASS.
- The result is local/controlled Integration evidence. It does not prove real ChatGPT → Tunnel → Runtime MCP or real Agent/Model → Eval Runner → Runtime MCP execution; those remain E2E responsibilities.
- The completed Development and Integration results remain valid even though standalone reports, the Evidence Index and POC source trees were deleted as requested.

---

## 7. Failure and Blocker Rules

Use these verdicts:

- PASS — case executed and all assertions satisfied.
- FAIL — case executed and a required assertion failed.
- BLOCKED — required executable/test surface/environment is unavailable.
- INCONCLUSIVE — execution occurred but evidence cannot support the required conclusion.

Examples:

- SQLite POC compiles but second-writer ownership is not deterministic → FAIL.
- runtime_id mapping is unavailable but documented as an M03 contract gap and no false correlation is made → may PASS the M00 feasibility contract if the SPEC allows the gap; evidence must be explicit.
- Eval runner only has mocked MCP, no real Runtime process → M00-INT-11 FAIL/BLOCKED.
- Config POC requires setting CURRENT_SCHEMA_VERSION=3 in production → FAIL.
- A production crate imports the M00 POC → FAIL.

---

## 8. Integration Exit Criteria

M00 Integration is PASS only when:

- M00-INT-01 through M00-INT-12 are PASS;
- no required case is silently omitted;
- all evidence is bound to the same candidate commit or explicitly identifies a later evidence-only commit with unchanged production code;
- no unresolved production change exists after the executed test evidence;
- cleanup is PASS.

Integration PASS does not mean M00 is ACCEPTED.

Next:

~~~text
Integration PASS (12/12)
→ E2E-01/E2E-03 recorded PASS; E2E-02 DEFERRED BY USER / NOT EXECUTED, with no PASS claim
→ M00 closeout accepted under the explicit user-approved E2E-02 deferral exception
→ Manual Acceptance APPROVED BY USER
→ human sign-off in the existing manual checklist + evidence locations
~~~
