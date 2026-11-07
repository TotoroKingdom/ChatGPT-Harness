# M06 E2E Test Plan — V4 Core Release Gate

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M06 — V4 Core Release Gate  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: independent QA / Coding Agent after M06 Integration PASS  
> Scope: same-candidate packaged Core release validation  
> Automation boundary: all M06 E2E cases are executed autonomously by the Coding/QA Agent on the test machine. They must not require the user to click through External ChatGPT, Harness UI, notifications or installer dialogs. OS/UI automation may be used where required.  
> External ChatGPT boundary: real External ChatGPT is NOT an automated E2E dependency and belongs only to M06-MAN-05. A QA-controlled external MCP client must never be labeled as ChatGPT.  
> Authorization boundary: networked paid Model requalification, destructive install/rollback on non-disposable state, or restarting the user's active instance requires explicit execution-time authorization. Once authorized, the E2E execution itself remains agent-driven.  
> Results rule: do not edit this frozen plan to record PASS/FAIL.

---

# 1. Objective

E2E proves the final M06 Core Release Candidate through production or near-production boundaries.

The suite must prove:

- Portable and installer represent the same candidate;
- install/upgrade/rollback works on disposable Windows state;
- packaged Tray/background/notification/Diagnostics behavior is coherent;
- the original M02 F05 gate still passes on the final candidate when rerun is required;
- real Tunnel carries final-candidate Tool calls without bypass;
- read/grep/ASK/DENY result and History evidence correlate exactly;
- crash/restart preserves History but not live authorization;
- M04/M05 F12 evidence is equivalent to the final candidate or is formally requalified without changing thresholds;
- the Core matrix remains fail-closed.

Automated E2E PASS still leaves the real External ChatGPT user chain for Manual Acceptance.

---

# 2. Entry Criteria

Formal M06 E2E may begin only when:

- M01–M05 are ACCEPTED / FROZEN;
- M06 Development Complete;
- M06 Integration PASS;
- one final candidate identity is frozen;
- source is clean;
- fresh Portable and NSIS installer exist from that candidate;
- accepted M02 evidence/contract is available;
- accepted M04/M05 formal Eval evidence is available;
- isolated Windows AppData/config/Workspace/History locations are prepared;
- later product code changes have not invalidated the candidate.

For any E2E step that would make a paid/networked Model request, explicit user authorization must have been obtained before execution.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE
- NOT_VERIFIED

---

# 3. Candidate Header

Record:

~~~text
Candidate ID:
Candidate commit:
Product version:
Source-state SHA-256:
Portable SHA-256:
Installer SHA-256:
UI SHA-256:
Runtime SHA-256:
Tunnel SHA-256:
Protocol/schema hash:
Config schema/profile hash:
Evidence schema/checksum:
Tool catalog/schema hash:
M02 accepted evidence identity:
M04 accepted baseline identity:
M05 accepted formal regression identity:
F05 equivalence status:
F12 equivalence status:
Windows version/build:
QA owner:
Execution time:
~~~

The same candidate identity must appear in all four cases.

---

# 4. Required E2E Cases

## M06-E2E-01 — Same-candidate Portable + installer + upgrade + rollback

**AC:** M06-AC01–AC06, AC19–AC20  
**Integration dependencies:** M06-INT-01–04, M06-INT-16

Use disposable Windows state.

### A. Candidate integrity

Verify before launch:

- Portable build SHA matches candidate;
- installer build SHA matches candidate;
- UI/Runtime/Tunnel payload hashes match expected candidate hashes;
- package verification passes;
- source is clean.

### B. Portable launch

Launch the Portable candidate with isolated config/AppData.

Verify:

- App starts;
- Runtime/Tunnel staged resources are the candidate versions;
- basic Start/Stop succeeds;
- no test secret or temp fixture was bundled.

### C. Installer path

Install the NSIS candidate using automation into a disposable target/profile.

Verify:

- install succeeds without manual dialog intervention;
- installed payload hashes equal verified candidate payload hashes;
- App launches and can Start/Stop;
- uninstall/reinstall does not silently destroy data outside documented product semantics.

### D. Upgrade

Starting from representative pre-Core state:

- v2 config;
- pre-Core v3 config;
- SecretReference;
- Workspace/Tool/Policy state;
- M03 History DB;

upgrade to the M06 candidate.

Verify:

- config ends in accepted v3 shape;
- user settings preserved;
- grep upgrade semantics preserved;
- History remains readable;
- no old Pending/Grant/Permit restored;
- no plaintext secret appears.

### E. Rollback

Automate the frozen rollback sequence to the designated previous accepted package.

Verify:

- current candidate quits cleanly;
- pre-upgrade config backup is restored where required;
- previous accepted package launches;
- compatible History is preserved;
- incompatibility is surfaced rather than erased;
- final owned Runtime/Tunnel count is zero.

**PASS:** Portable, installer, upgrade and rollback all refer to one candidate and preserve user/security state.

---

## M06-E2E-02 — Packaged Windows lifecycle, Tray, notifications, F05 and Diagnostics

**AC:** M06-AC07–AC11  
**Integration dependencies:** M06-INT-05–08

Run the final packaged Windows candidate under automation.

### A. Lifecycle / Tray

Automate:

1. launch App;
2. Start Harness;
3. wait for authoritative Ready;
4. close/hide main window;
5. verify Harness remains alive;
6. invoke Tray Show;
7. Stop;
8. Start again;
9. final Quit.

Verify:

- hidden window does not stop Runtime/Tunnel;
- Tray state/Pending count follows authoritative state;
- no duplicate Runtime/Tunnel is created;
- final Quit leaves zero managed orphan.

### B. Approval notification OS integration

Using a controlled real Pending Approval, verify through OS/UI automation where supported:

- exactly one Approval notification appears for the new Pending;
- repeated observation does not create duplicates;
- notification navigation/focus targets current Approval state;
- resolving/expiring before navigation does not revive Pending;
- notification body contains no secret/raw unrestricted argument.

If the test environment cannot observe required OS notification behavior autonomously, this case is BLOCKED; do not replace it with a unit mock.

### C. Disconnect/recovery notification

Create a controlled Tunnel/control-plane incident.

Verify:

- one disconnect/degraded notification for the incident;
- repeated same failure does not spam;
- recovery can notify once;
- a new incident after recovery can notify again;
- manual Stop/Quit is not presented as unexpected disconnect.

### D. F05 final-candidate performance

Use the equivalence decision from M06-INT-07.

If EQUIVALENT_REUSE_ALLOWED:

- verify candidate identities actually match;
- execute the final Ready usability sanity required by M06;
- bind accepted M02 result to this candidate.

If FULL_M02_BENCHMARK_REQUIRED:

- run the exact frozen M02 benchmark automatically;
- preserve original cold/warm definitions, samples, P50/P95/failure-rate math and thresholds;
- require every frozen M02 PASS condition.

Any frozen M02 threshold failure fails this E2E case.

### E. F06 Diagnostics

Automate normal state plus representative fault/recovery.

Verify:

- required checks appear;
- FAIL/UNKNOWN are truthful;
- no automatic repair/restart;
- recovery updates current result;
- copied summary is sanitized.

**PASS:** packaged Windows desktop behavior, performance and Diagnostics all pass on the same RC.

---

## M06-E2E-03 — Real Tunnel Core Tool/authority/evidence/crash chain

**AC:** M06-AC12–AC16, AC18  
**Integration dependencies:** M06-INT-09–13, M06-INT-15

Use the final packaged candidate with:

- disposable Workspace;
- disposable History;
- real Tunnel;
- QA-controlled external MCP client.

Do NOT use External ChatGPT.

### A. Hidden-window request

Start Harness, reach authoritative Ready, hide the main window, then through the real Tunnel:

- initialize;
- tools/list;
- call read_file;
- call grep literal;
- call grep with deterministic truncation.

Verify Tool results against fixture ground truth.

### B. Authority

Through the same real Tunnel/Runtime:

- Policy ASK → allow_once;
- Policy ASK → deny;
- Policy DENY;
- disabled Tool;
- safe invalid grep request.

The test harness may resolve approvals through supported automation/control interfaces, but must use the real Pending Approval/Runtime state and exact request identity.

Verify:

- DENY/disabled/ASK-deny do not execute;
- allow_once resumes the exact pending request;
- no History row authorizes execution;
- no shell grep bypass exists.

### C. Evidence

For real Tunnel requests verify:

- exact runtime_id + request_id in Activity;
- same exact key in History;
- terminal Approval state where applicable;
- grep raw query/filter/match preview absent from durable History;
- SecretStore/outside path sentinels absent;
- reconnect does not duplicate execution.

### D. Crash/restart

Create controlled Runtime or Tunnel failure during/after a request.

Verify:

- lifecycle/Diagnostics reflects current failure;
- incomplete evidence is not rewritten as success;
- restart produces correct new Runtime identity;
- persisted History remains;
- old Pending/Grant/Permit does not revive;
- a new post-restart request follows current Policy;
- final Quit leaves no owned orphan.

### E. V3 adjacent regression

Through production boundaries, execute representative read/write/edit/Bash or the accepted safe regression subset required by the candidate's current config.

Verify Workspace, Approval, SecretStore and lifecycle boundaries remain intact.

**PASS:** the packaged external-facing product chain preserves Core authority, evidence and recovery without External ChatGPT being needed for automation.

---

## M06-E2E-04 — F12 final-candidate qualification + Core matrix/release gate

**AC:** M06-AC17, AC19, AC20  
**Integration dependencies:** M06-INT-14, M06-INT-16

### A. F12 identity decision

Compare final candidate to accepted M05 formal regression equivalence key.

If EQUIVALENT_REUSE_ALLOWED:

- verify every equivalence field matches exactly;
- bind the accepted formal M04/M05 result to the M06 candidate;
- do not rerun paid Model merely for ceremony.

If REQUALIFICATION_REQUIRED:

- obtain explicit user authorization before provider calls;
- automatically run the already-frozen formal M04/M05 controlled evaluation path;
- use unchanged dataset, prompt, scorer, repetitions, validity rules, compare_m05 thresholds and provider-drift rules;
- do not introduce an M06 scorer;
- record actual resolved Model/adapter identity;
- require zero security violations.

If authorization is not available, E2E-04 is BLOCKED, not PASS.

If paired control exceeds provider-drift guard, E2E-04 is INCONCLUSIVE.

### B. Core matrix assembly

Using real completed M06 Integration/E2E evidence plus upstream accepted evidence, generate the execution-time Core Product Acceptance Matrix.

Before Manual Acceptance the External ChatGPT/manual rows may remain NOT_VERIFIED, so the release gate itself must remain blocked.

Verify:

- all 28 canonical Core rows present;
- no P1/P2 row satisfies a Core row;
- package, upgrade, rollback, F05 and F12 evidence bind to the same candidate;
- no old candidate PASS is silently mixed in.

### C. Release Notes dry-run

Generate a candidate release-note output.

Verify it says:

- exact version/SHA/hashes;
- V4 Core scope only;
- P1 OUT_OF_CORE / NOT_CORE_ACCEPTED;
- P2 DEFERRED or separately accepted;
- actual F05/F12 status;
- External ChatGPT still pending before Manual if not yet executed.

It must not say V4 — ACCEPTED / FROZEN.

### D. Fail-closed final gate

Feed the gate current pre-Manual evidence.

Expected:

~~~text
V4 Core Gate = BLOCKED
reason includes missing Manual / real External ChatGPT acceptance
~~~

This is the correct automated E2E outcome before the user performs Manual Acceptance; the E2E case itself may PASS because the gate correctly refuses release.

**PASS:** F12 is valid for the final RC and the release gate truthfully remains blocked until mandatory Manual evidence is supplied.

---

# 5. E2E Verdict Rules

Examples:

- a user must click through an automated E2E step -> FAIL design / case not autonomously executable;
- internal MCP client labeled as ChatGPT -> FAIL;
- direct loopback substituted for real Tunnel in E2E-03 -> FAIL;
- required OS notification behavior cannot be automatically observed -> BLOCKED;
- final candidate requires F12 rerun but provider authorization unavailable -> BLOCKED;
- paired-control drift outside guard -> INCONCLUSIVE;
- frozen M02 threshold miss -> FAIL;
- any security violation -> FAIL;
- package hash mismatch -> FAIL;
- old Pending/Grant/Permit restored -> FAIL;
- same request joined by approximate timestamp rather than exact identity -> FAIL;
- Release Notes claim Full V4 -> FAIL;
- final gate reports ACCEPTED before Manual/External ChatGPT -> FAIL.

---

# 6. E2E Exit Gate

M06 automated E2E is PASS only when:

- M06-E2E-01 through M06-E2E-04 are all PASS;
- all four cases target the same final candidate identity;
- Portable/installer/upgrade/rollback evidence is valid;
- packaged Windows lifecycle/notification behavior is automated and valid;
- F05 final-candidate gate is PASS under frozen M02 rules;
- real Tunnel Core Tool/authority/evidence chain passes;
- crash/restart leaves no stale authorization/orphan;
- F12 is equivalently bound or formally requalified and PASS;
- security violations are zero;
- no secrets/privacy sentinels leak;
- pre-Manual Core gate correctly remains BLOCKED on missing human/External ChatGPT evidence.

Automated E2E PASS does not itself produce V4 Core — ACCEPTED.

Only Manual Acceptance may close the final External ChatGPT and human Windows acceptance rows.
