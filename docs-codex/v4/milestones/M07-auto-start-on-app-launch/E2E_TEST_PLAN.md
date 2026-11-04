# M07 E2E Test Plan — Auto Start Harness on App Launch

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M07 — Auto Start on App Launch  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: independent QA / Coding Agent after M07 Integration PASS  
> Scope: packaged Windows App process launch/relaunch, Auto Start success/failure/disabled behavior, lifecycle continuity, real Tunnel/MCP usability  
> Automation boundary: all M07 E2E cases must be executable autonomously by the Coding/QA Agent on the test machine. They must not require the user to click Settings, Tray, dialogs or ChatGPT. OS/UI automation may be used.  
> External ChatGPT boundary: real External ChatGPT is reserved for M07 Manual Acceptance. A QA-controlled MCP client or probe must never be labeled as ChatGPT.  
> Safety: use disposable/test-owned AppData, Workspace and credential fixtures. Do not modify the user's active profile or credential.  
> Results rule: do not edit this frozen plan to record PASS/FAIL.

---

# 1. Objective

E2E proves F02 on the actual packaged application boundary:

~~~text
new App process
→ persisted host-only preference
→ Tauri setup
→ one Auto Start request
→ standard Harness lifecycle
→ authoritative Ready or truthful failure
→ Runtime/MCP/Tunnel usable state
~~~

It must also prove the negative path:

~~~text
preference disabled
→ new App process
→ no automatic Start
~~~

and lifecycle containment:

~~~text
Auto Ready
→ Close/Hide/Show
→ no retrigger
→ Stop
→ remain Stopped
→ Quit
→ no orphan
~~~

Automated E2E does not substitute for the final real External ChatGPT user acceptance.

---

# 2. Entry Criteria

Formal M07 E2E may begin only when:

- M00/M01/M02 are ACCEPTED / FROZEN;
- M07 Development Complete;
- M07-INT-01 through M07-INT-12 PASS;
- one final candidate SHA is frozen and source is clean;
- fresh packaged Portable candidate exists;
- isolated AppData/config/Workspace/credential fixtures exist;
- later code changes have not invalidated the candidate.

If packaged Windows execution cannot be performed, the relevant case is BLOCKED rather than replaced with unit/UI mocks.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE
- NOT_VERIFIED

---

# 3. Candidate Header

Record once:

~~~text
Candidate commit:
Product version:
Source dirty:
Portable path/hash:
UI hash:
Runtime hash:
Tunnel hash:
Config schema version:
Protocol revision:
Windows version/build:
Test AppData root:
Test Workspace fixture identity:
Credential fixture type:
QA owner:
Execution time:
~~~

Never record raw credential values.

---

# 4. Required E2E Cases

## M07-E2E-01 — Packaged Settings persistence and disabled launch

**AC:** M07-AC01–AC04  
**Integration dependencies:** M07-INT-01/02/03

Use the final packaged App with disposable AppData.

### A. Default/new profile

Launch with no prior config and let the normal product create/read its safe state.

Verify:

- Auto Start setting is OFF by default;
- Harness does not automatically start;
- no Runtime/Tunnel is launched by F02;
- Settings displays OFF;
- product remains usable for explicit setup/start.

### B. Persist ON without immediate start

Using UI automation:

1. open Settings;
2. turn Auto Start ON;
3. verify save success;
4. verify current Harness does not automatically Start merely because toggle changed;
5. verify no Restart Required created;
6. fully Quit App.

### C. Persist across process launch

Relaunch only far enough to verify the persisted ON value is still present; this launch is allowed to proceed into E2E-02 Auto Start behavior.

### D. Persist OFF

After later stopping/quitting the enabled scenario, turn Auto Start OFF, verify:

- no immediate Stop is caused by the toggle itself;
- persisted OFF survives full Quit/relaunch;
- relaunch remains Stopped with no automatic Start;
- explicit Manual Start still works.

**PASS:** the packaged preference is persistent, default-off, next-launch-only and Host-only.

---

## M07-E2E-02 — Enabled full App relaunch → one authoritative Auto Start → usable Harness

**AC:** M07-AC05, AC06, AC12, AC14  
**Integration dependencies:** M07-INT-04/05/12

Prepare a valid isolated profile with Auto Start ON and safe test credentials/config.

Perform a real full process sequence:

1. ensure no ChatGPT-Harness App/managed Runtime/Tunnel from the test profile is running;
2. launch packaged App;
3. do not press Start;
4. observe exactly one automatic startup attempt;
5. wait for authoritative Harness Ready;
6. verify one managed Runtime generation;
7. verify one managed Tunnel generation;
8. verify the normal startup phase record exists;
9. through the real Tunnel or product-approved external MCP test boundary, initialize/list tools and perform one safe read_file;
10. verify result matches fixture;
11. perform one grep smoke if current Tool profile allows it.

Required assertions:

- Ready is not inferred from process existence;
- Auto Start traverses normal Runtime/MCP/Tunnel readiness;
- no second startup attempt occurs from React mount/status refresh;
- no secret appears in logs/evidence used by the test;
- product remains on the same candidate binaries.

**PASS:** opening the App with F02 enabled yields one real usable Harness without any manual Start action.

---

## M07-E2E-03 — Hidden-window continuity, no retrigger, Stop semantics and duplicate activation

**AC:** M07-AC09, AC10, AC11, AC12, AC13, AC14  
**Integration dependencies:** M07-INT-07/08/10/11/12

Start from E2E-02 Ready state or recreate the same valid enabled launch.

Automate:

1. close main window so it hides to Tray;
2. verify Harness remains Ready/usable;
3. perform a safe MCP Tool probe while hidden;
4. Tray Show the window;
5. verify no second Auto Start attempt/generation;
6. exercise supported normal second app invocation/activation behavior;
7. verify F02 creates no duplicate managed Runtime/Tunnel;
8. issue Stop;
9. wait through normal status/readiness observation intervals;
10. hide/show again;
11. verify Harness remains Stopped;
12. verify Auto Start does not re-run;
13. explicit Manual Start may be used to prove normal lifecycle still works;
14. Stop if required;
15. explicit Quit;
16. verify zero managed Runtime/Tunnel orphan.

Also audit the installed/portable candidate for absence of:

- Windows Run-key registration;
- Startup-folder entry;
- Task Scheduler registration;
- Windows service;
- F02-specific daemon.

If supported double activation exposes a pre-existing unsafe independent multi-process ownership problem, mark BLOCKED/FAIL as appropriate; do not hide it with an F02-only workaround.

**PASS:** F02 triggers on real process launch only and remains subordinate to Tray/Stop/Quit lifecycle intent.

---

## M07-E2E-04 — Packaged failure → Diagnostics → no retry → explicit recovery

**AC:** M07-AC07, AC08, AC11, AC14  
**Integration dependencies:** M07-INT-06/08/09/12

Use a disposable enabled profile with one controlled eligibility/start failure, preferably a missing test credential or other safely isolated prerequisite.

Sequence:

1. ensure Auto Start ON;
2. fully Quit;
3. launch packaged App;
4. do not click Start;
5. observe one Auto Start attempt/failure;
6. verify Harness does not report Ready;
7. run the real Diagnostics surface/command through automation;
8. verify the current cause is identified safely;
9. wait/status-refresh for a bounded observation period;
10. verify no automatic retry occurs;
11. repair the isolated prerequisite in the same App process;
12. verify F02 still does not auto-retry;
13. explicitly invoke Manual Start;
14. verify the standard lifecycle can now reach Ready;
15. perform one safe MCP probe;
16. Quit and verify zero orphan.

Required assertions:

- no secret is logged;
- no config/security setting is silently weakened;
- no previous successful state masks the current failure;
- Diagnostics remains observational/read-only;
- one automatic failure does not become a retry storm.

**PASS:** packaged F02 failure is truthful, single-shot and explicitly recoverable.

---

# 5. E2E Verdict Rules

Examples:

- default profile auto-starts unexpectedly -> FAIL;
- toggling ON starts immediately in same process -> FAIL;
- toggling OFF stops current Harness -> FAIL;
- Auto Start uses direct Runtime/Tunnel startup bypass -> FAIL;
- more than one automatic attempt per process -> FAIL;
- Auto vs activation creates duplicate managed generation -> FAIL;
- Stop is followed by same-process automatic re-start -> FAIL;
- Quit leaves owned orphan -> FAIL;
- missing credential still reports Ready -> FAIL;
- Diagnostics itself triggers retry/start -> FAIL;
- any Windows login/startup registration added by M07 -> FAIL;
- internal MCP probe labeled as External ChatGPT -> FAIL;
- packaged test replaced with mock-only test -> BLOCKED/FAIL depending on cause.

---

# 6. E2E Exit Gate

M07 automated E2E is PASS only when:

- M07-E2E-01 through M07-E2E-04 all PASS;
- all cases target the same final packaged candidate;
- default/off behavior is proven on real App relaunch;
- enabled behavior reaches authoritative Ready exactly once;
- a real product MCP/Tunnel probe succeeds after Auto Start;
- hide/show/activation do not retrigger;
- Stop remains Stopped;
- controlled failure is diagnosable and not retried;
- explicit recovery works;
- Quit leaves no orphan;
- no Windows login startup mechanism exists;
- no secret/privacy sentinel leaks;
- no unresolved M01/M02/V3 regression appears.

Automated E2E PASS does not mark M07 ACCEPTED / FROZEN.

Real External ChatGPT final user-chain verification remains Manual Acceptance.
