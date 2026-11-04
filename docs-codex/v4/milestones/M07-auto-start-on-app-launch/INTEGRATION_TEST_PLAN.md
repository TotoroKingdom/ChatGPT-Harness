# M07 Integration Test Plan — Auto Start Harness on App Launch

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M07 — Auto Start on App Launch  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M07 Development Complete  
> Scope: config persistence, Host orchestration, shared lifecycle authority, failure/concurrency semantics, affected M01/M02/V3 regression  
> External boundary: Integration MUST NOT require External ChatGPT, Windows login startup, or the user's active profile. Use isolated AppData/config/credential fixtures and test-owned Runtime/Tunnel processes.  
> Results rule: do not edit this frozen plan to record PASS/FAIL.

---

# 1. Objective

Integration proves that F02 is an additive Host trigger over the existing Harness lifecycle rather than a second startup system.

It must prove:

- schema v3 remains unchanged;
- the persisted Auto Start preference defaults safely and preserves explicit user choices;
- the setting is Host-only and creates no Runtime restart debt;
- Settings writes only the focused desktop preference;
- each App process evaluates/schedules Auto Start at most once;
- Manual/Tray/Auto Start converge on one standard lifecycle path;
- eligibility, SecretStore, security and authoritative Ready are not bypassed;
- failures do not retry automatically;
- Stop does not cause same-process re-start;
- Quit wins lifecycle races;
- Diagnostics/startup evidence remains truthful;
- no Windows login persistence mechanism is added;
- affected M01/M02/V3 behavior remains intact.

Integration PASS does not imply M07 ACCEPTED / FROZEN.

---

# 2. Entry Criteria

Formal M07 Integration may begin only when:

- M00 is ACCEPTED / FROZEN;
- M01 is ACCEPTED / FROZEN;
- M02 is ACCEPTED / FROZEN;
- all six M07 documents are frozen;
- T1–T6 are Development Complete;
- candidate SHA is frozen and source is clean;
- a fresh M07 Portable candidate exists;
- no unresolved F02 blocker is known.

If any Roadmap dependency is not ACCEPTED / FROZEN, formal M07 Integration is BLOCKED.

Preflight:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

---

# 3. Evidence Header

Record:

~~~text
Milestone: M07
Candidate commit:
Branch / dirty:
Product version:
Portable path/hash:
UI hash:
Runtime hash:
Tunnel hash:
Config schema version:
Protocol revision:
Workspace fixture:
Credential fixture type:
Windows/test environment:
QA owner:
Execution time:
~~~

Do not record secret values, raw environment, unrestricted physical paths or credential-bearing URLs.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE
- NOT_VERIFIED

---

# 4. Required Integration Cases

## M07-INT-01 — Schema-v3 default, migration and round-trip contract

**AC:** M07-AC01  
**Task:** T1

Verify against real ConfigStore/protocol serialization:

1. CURRENT_SCHEMA_VERSION remains 3;
2. DesktopConfig contains autoStartHarnessOnAppLaunch;
3. new/safe defaults are false;
4. v2→v3 migration produces false;
5. explicit existing-v3 true remains true;
6. explicit existing-v3 false remains false;
7. reset returns to false;
8. RuntimeConfigSnapshot does not contain desktop;
9. protocol/v2 is not introduced.

Negative controls:

- malformed/unsupported config must fail rather than be rewritten as enabled/disabled;
- partial desktop shape must not be silently accepted if current schema validation rejects it.

**PASS:** F02 uses the existing v3 host-only field without schema expansion or choice loss.

---

## M07-INT-02 — Focused persistence, configRevision and restart-debt isolation

**AC:** M07-AC02  
**Task:** T1

Use desktop_preferences_set / ConfigStore::set_desktop_preferences.

Verify changing only Auto Start:

- preserves approvalNotifications;
- preserves connectionNotifications;
- preserves backgroundNoticeAcknowledged;
- preserves Workspace/Tunnel/Security/Tools/Policies/Bash/History/UI;
- reads the latest persisted config rather than committing a stale full Settings draft;
- uses atomic persistence;
- does not increment Runtime-facing configRevision;
- does not alter RuntimeConfigSnapshot;
- does not create Restart Required;
- invokes no Start/Stop/Restart lifecycle command.

Race/focused-save negative control:

- an unrelated concurrent latest-value mutation must not be overwritten by an older Auto Start UI snapshot.

**PASS:** Auto Start persistence is independent, atomic and Host-only.

---

## M07-INT-03 — Settings toggle, localization and next-launch semantics

**AC:** M07-AC03, AC04  
**Task:** T3

Render the real Settings page/provider/client boundary.

Verify:

- persisted false renders unchecked;
- persisted true renders checked;
- toggle uses focused desktop-preference mutation;
- save-in-flight disables the control appropriately;
- success is visible;
- failure restores/refetches persisted truth;
- English and zh-CN text says App launch, not Windows login;
- toggle ON does not call harnessStart/harnessRestart;
- toggle OFF does not call harnessStop/harnessRestart;
- no Restart Required banner is created only by this setting;
- notification/language/theme/connection edits remain independent.

**PASS:** UI reflects persisted truth and cannot be mistaken for Windows login startup or immediate lifecycle action.

---

## M07-INT-04 — Per-process once guard and Host initialization ordering

**AC:** M07-AC05  
**Task:** T2

Exercise AutoStartCoordinator directly through the real Host composition/test seam.

Verify:

- evaluation occurs only after required managed Host state is available;
- React mount is not required;
- first disabled evaluation records disabled and schedules no Start;
- first enabled evaluation schedules one standard Start;
- repeated trigger/evaluate calls in the same App process schedule no second Start;
- window Show/Hide/navigation/status refresh cannot invoke it;
- coordinator state contains only bounded safe metadata;
- no background retry timer is registered.

**PASS:** one App process owns at most one F02 evaluation/scheduled request.

---

## M07-INT-05 — Shared standard Start path and lifecycle authority

**AC:** M07-AC06  
**Task:** T2

Verify Manual UI Start, Tray Start and Auto Start converge on the same internal Host start-request helper.

For Auto Start verify the call path still includes:

- DesktopLifecycleCoordinator.start_allowed;
- HarnessController.start;
- HarnessController operation mutex;
- StartEligibility;
- Config/Secret/Security validation;
- Runtime startup;
- MCP readiness;
- Tunnel startup/readiness;
- authoritative Harness Ready;
- StartupPhaseRecorder.

Negative controls:

- no direct RuntimeSupervisor.start path from AutoStartCoordinator;
- no direct TunnelProcessManager.start path;
- no second lifecycle mutex;
- no modified Ready predicate.

**PASS:** F02 is only a trigger for the accepted standard lifecycle.

---

## M07-INT-06 — Eligibility/security failure matrix

**AC:** M07-AC07  
**Task:** T4

Using isolated fixtures, enable Auto Start and test representative current StartEligibility failures:

- invalid config;
- missing/invalid Workspace;
- missing Tunnel ID where required;
- missing credential/SecretReference;
- invalid Runtime distribution;
- invalid Tunnel distribution;
- inconsistent security mode;
- App quitting.

Verify:

- no failing prerequisite is bypassed;
- no automatic config repair occurs;
- no credential is created;
- no Policy/Tool/security mode is changed;
- no false Ready;
- failure code/message is bounded and safe;
- Runtime/Tunnel are not left partially owned after failed startup.

**PASS:** Auto Start has exactly the same security prerequisites as Manual Start.

---

## M07-INT-07 — Auto Start vs Manual/Tray concurrency

**AC:** M07-AC09  
**Tasks:** T2/T4

Deterministically stage:

1. Auto Start claims/starts first, then Manual Start;
2. Manual Start claims first, then Auto Start;
3. Auto Start vs Tray Start;
4. near-simultaneous requests while lifecycle is Starting.

Verify:

- one operation mutex remains authoritative;
- at most one Runtime generation is launched;
- at most one Tunnel generation is launched;
- duplicate request gets existing lifecycle conflict/skip/current-state semantics;
- Auto Start does not queue a retry;
- final state is one truthful lifecycle state.

**PASS:** race conditions cannot create duplicate Harness generations.

---

## M07-INT-08 — Quit race and process cleanup

**AC:** M07-AC11  
**Task:** T4

Test:

### Quit before Auto Start claim

- set app_quitting;
- attempt/evaluate Auto Start.

Expected: skip; zero Runtime/Tunnel launch.

### Quit during Auto Starting

- hold startup at a deterministic test seam;
- begin explicit Quit;
- release/start cleanup.

Verify:

- existing shutdown/cancellation semantics own cleanup;
- no new Auto Start begins after quit intent;
- zero owned Runtime/Tunnel orphan at completion;
- failed Quit retry does not re-arm Auto Start.

**PASS:** explicit Quit always wins and no F02 process debt remains.

---

## M07-INT-09 — Failure terminal state, no retry and manual recovery

**AC:** M07-AC08  
**Task:** T4

Cause one genuine standard Auto Start failure after the request is scheduled.

Verify:

- exactly one automatic attempt;
- coordinator reaches one terminal failed state;
- StartupAttempt records truthful failed/incomplete phase state as appropriate;
- periodic status refresh does not retry;
- Diagnostics does not retry;
- fixing the test fixture in the same App process does not retry;
- explicit Manual Start after repair can succeed normally.

**PASS:** F02 failure is single-shot and recoverable only through explicit user action or a later App relaunch.

---

## M07-INT-10 — Stop/Restart semantics after successful Auto Start

**AC:** M07-AC10  
**Task:** T4

After successful Auto Start → Ready:

- execute Stop;
- wait through normal background/status intervals;
- hide/show window;
- refresh status;
- ensure Harness remains Stopped;
- verify no second Auto Start request;
- execute explicit Manual Start and explicit Restart to confirm existing semantics remain available;
- ensure Restart is not counted as F02 re-evaluation.

**PASS:** user lifecycle intent remains authoritative after F02 startup.

---

## M07-INT-11 — Reopen/activation model and Windows-login non-goal audit

**AC:** M07-AC12, AC13  
**Task:** T5

Use Host/package-oriented test seams where possible.

Verify the trigger model:

- clean new App process may evaluate once;
- Close→Hide does not evaluate again;
- Tray Show does not evaluate again;
- page navigation does not evaluate again;
- supported normal second activation/reopen behavior does not create a duplicate managed Harness generation.

Audit product/source/package for M07 additions:

- no registry Run-key write;
- no Startup-folder shortcut;
- no Task Scheduler registration;
- no Windows service;
- no login-start plugin/config;
- no new daemon executable.

**PASS:** F02 is app-launch Auto Start only.

---

## M07-INT-12 — M01/M02/V3 regression and product-boundary smoke

**AC:** M07-AC14  
**Tasks:** T5/T6

Run affected real-component regression:

### M01

- Close→Hide;
- Tray Show/Start/Stop/Quit;
- app_quitting;
- Approval notification background state.

### M02

- StartEligibility;
- StartupPhaseRecorder;
- authoritative Ready;
- current F05 readiness predicate;
- Diagnostics read-only behavior.

### V3/security

- SecretStore reference boundary;
- Workspace/security config;
- Runtime/Tunnel Start/Stop/Restart;
- no orphan process;
- read_file and grep MCP smoke after a successful Start.

Verify M07 did not change Runtime/Tool/Eval identity unexpectedly. If it did, stop and classify downstream requalification impact rather than silently accepting it.

**PASS:** F02 adds no regression to the accepted lifecycle/security contract.

---

# 5. Integration Exit Gate

M07 Integration is PASS only when:

- M07-INT-01 through M07-INT-12 are all PASS;
- M00/M01/M02 are ACCEPTED / FROZEN;
- schema remains v3;
- Auto Start default is false;
- focused save creates no Runtime restart debt;
- exactly-once process semantics are demonstrated;
- all start origins use one standard lifecycle authority;
- eligibility/security failures remain fail-closed;
- no automatic retry exists;
- concurrency produces at most one generation;
- Stop remains Stopped;
- Quit leaves zero owned orphan;
- no Windows login startup mechanism exists;
- affected M01/M02/V3 regression passes;
- no secret/privacy sentinel leaks.

Integration PASS unlocks M07 automated E2E. It does not mark M07 accepted.
