# M02 E2E Test Plan — Packaged Startup Performance and Connection Diagnostics

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M02 — Startup Performance and One-click Diagnostics  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: independent QA / Coding Agent, with explicit user authorization for external ChatGPT and managed-instance disruption  
> Gate: M02 Development Complete + Integration PASS.  
> Rule: external ChatGPT, networked validation, managed-process disruption and Windows package execution require the authorization boundary defined by AGENTS/Roadmap; unavailable authorization/environment means BLOCKED, not simulated PASS.

---

## 1. Objective

E2E proves the production package behavior across real boundaries:

~~~text
Packaged Windows App
→ Tauri Host Start
→ Runtime
→ MCP
→ Tunnel
→ control plane / target readiness
→ authoritative Harness Ready
→ real ChatGPT Tool availability
~~~

and:

~~~text
Packaged Windows App
→ One-click Diagnostics
→ current Config / Runtime / MCP / Tunnel / Proxy / Credential / lifecycle evidence
→ user-visible safe diagnosis
~~~

Four cases are frozen:

- M02-E2E-01 — independent packaged cold/warm performance cohort and frozen-threshold comparison;
- M02-E2E-02 — healthy Ready diagnostics and startup detail;
- M02-E2E-03 — real controlled fault → accurate diagnostics → recovery;
- M02-E2E-04 — Ready correctness via real ChatGPT Tool call + diagnostics read-only/regression.

---

## 2. Candidate and Environment Lock

Record:

~~~text
Candidate commit:
B0 commit:
Product logic baseline:
Package type:
Package path:
Package SHA-256:
App / Runtime / Tunnel versions:
Windows build:
CPU:
Power mode:
Workspace fixture:
Config revision:
Tunnel identity:
Proxy source / route mode:
Network type:
Start time:
Tester:
~~~

Do not record secrets.

Performance E2E comparison is invalid if baseline/candidate environment-lock fields differ materially.

---

## 3. M02-E2E-01 — Independent packaged performance cohort meets frozen targets

**AC:** M02-AC01–AC05  
**Tasks:** T1/T2/T6

This case independently verifies the final production package, not merely the Development Complete summary.

### Procedure

Use the exact SPEC cohort definitions.

Candidate:

- 2 cold primer runs;
- 30 counted cold runs;
- 2 warm primer runs;
- 30 counted warm runs.

Every accepted Start is counted in failure denominator.

For each attempt collect the product StartupAttempt output, not stopwatch-only manual timing.

### Assertions

- each attempt binds to one Harness session;
- authoritative Ready is the terminal success condition;
- ≥29 Ready samples in each cohort;
- nearest-rank P50/P95 reproduced independently;
- failure rate reproduced independently;
- candidate satisfies:
  - cold P50 <= 90% B0 cold P50;
  - warm P50 <= 85% B0 warm P50;
  - each P95 <=105% corresponding B0 P95;
  - at least one P95 <=90% corresponding B0 P95;
  - candidate failure rate <= baseline and <=1/30;
  - baseline zero failure implies candidate zero;
  - material-bottleneck clause A or B passes;
- no accepted sample is removed because it is slow.

### Failure

If network/environment changes invalidate the comparison, result is INCONCLUSIVE/BLOCKED and the whole cohort must be rerun under locked conditions. Do not selectively replace individual slow samples.

**PASS:** final package independently satisfies every frozen F05 threshold.

---

## 4. M02-E2E-02 — Healthy packaged Ready diagnostics and startup detail

**AC:** M02-AC06–AC09, AC13  
**Tasks:** T3/T4/T5

### Preconditions

- production package;
- valid v3 config;
- controlled Workspace;
- credential configured;
- Tunnel/control plane available;
- Harness reaches authoritative Ready.

### Action

1. Open Overview.
2. Select Run connection diagnostics.
3. Inspect all returned checks.
4. Inspect latest startup detail.
5. Copy safe diagnostic summary.

### Assertions

- overall reflects healthy current state;
- required check IDs are present;
- current Runtime/MCP/Tunnel/control-plane/target facts are PASS where positively verified;
- stopped/unavailable-dependent semantics do not appear falsely;
- proxy route matches observed current route without credential disclosure;
- credential presence/current accepted state is correctly distinguished;
- startup attempt matches the just-completed Start;
- total/stage duration and slowest phase are visible;
- copy summary contains useful safe facts;
- copy summary excludes physical Workspace path, secret, raw proxy credentials, raw logs and unrestricted error body;
- Overview and Settings show equivalent diagnostic truth.

**PASS:** a healthy real system can explain why it is healthy without unsafe evidence.

---

## 5. M02-E2E-03 — Controlled real fault → accurate diagnosis → recovery

**AC:** M02-AC07–AC12  
**Tasks:** T3/T4/T5

Use a controlled candidate instance only.

Execute at least two fault classes, one local and one connection-side:

### Local fault

Choose one supported controlled fixture such as:

- temporarily point the test profile at an inaccessible disposable Workspace; or
- use an acceptance profile with a deliberately missing tunnel-client distribution.

Do not modify the real user profile.

Run diagnostics.

Required:

- expected blocking check FAIL;
- dependent live checks UNKNOWN rather than fake PASS;
- remediation points to the correct Settings area;
- running diagnostics does not fix the fault automatically.

Restore the test profile.

### Connection fault

With the same candidate reaching Ready, trigger one safe managed connection fault, for example candidate-owned Tunnel failure/stale condition using the accepted QA mechanism.

Run diagnostics while the fault is active.

Required:

- Harness current lifecycle is non-Ready truthfully;
- Tunnel/control-plane/target checks identify the actual failure/stale state;
- proxy/credential evidence remains truthful rather than guessed;
- no secret/raw network response is exposed;
- diagnostics itself does not restart the product.

Then restore the external condition and use the normal product recovery/start path explicitly.

Run diagnostics again.

Required:

- current health returns to PASS where actually recovered;
- prior fault result is not reused as current truth.

**PASS:** diagnosis follows real state before, during and after a failure.

---

## 6. M02-E2E-04 — Authoritative Ready serves real ChatGPT Tool call and diagnostics has no side effect

**AC:** M02-AC05, AC10, AC14, AC15  
**Tasks:** T2/T3/T4/T6

### Preconditions

- packaged candidate is Ready;
- controlled Workspace contains a known read-only fixture;
- existing non-destructive Tool such as read_file/glob is enabled;
- real ChatGPT→Tunnel route is authorized.

### Action

1. Capture current candidate process identities/config revision/Workspace fixture.
2. Run connection diagnostics once.
3. Verify no lifecycle/config mutation occurred.
4. From real ChatGPT, require an actual Tool call to the controlled Workspace.
5. Verify returned result against fixture truth.
6. Run diagnostics again.

### Assertions

- diagnostics did not Stop/Restart/Start Harness;
- config revision did not change due solely to diagnostics;
- no Approval/Policy/SessionGrant was created by diagnostics;
- real ChatGPT Tool call succeeds after product says Ready;
- no false Ready from mere process existence;
- after the Tool call, current connection diagnostics still reflects the real current lifecycle;
- M01 Tray/background/notification behavior remains operational;
- no extra Runtime/Tunnel instance appears.

If real ChatGPT is unavailable, this case is BLOCKED. Direct local MCP cannot substitute for the required external boundary.

**PASS:** Ready is real and diagnostics is observational only.

---

## 7. Verdict Rules

Use only:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Examples:

- candidate benchmark misses warm P50 target → E2E-01 FAIL.
- benchmark environment changed mid-run → E2E-01 INCONCLUSIVE.
- real ChatGPT unavailable → E2E-04 BLOCKED.
- diagnostic run silently restarts Tunnel → E2E-03/04 FAIL.
- direct local MCP works but external ChatGPT was not run → E2E-04 remains BLOCKED.

---

## 8. E2E Exit Gate

M02 E2E PASS requires:

- M02-E2E-01 PASS;
- M02-E2E-02 PASS;
- M02-E2E-03 PASS;
- M02-E2E-04 PASS;
- same production candidate identity;
- performance environment lock maintained;
- all temporary fault/profile/process changes cleaned;
- no later production change invalidates evidence.

E2E PASS does not itself mark M02 ACCEPTED / FROZEN.
