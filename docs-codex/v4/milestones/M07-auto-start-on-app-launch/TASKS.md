# M07 TASKS — Auto Start Harness on App Launch

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: IMPLEMENTATION BLOCKED UNTIL M00/M01/M02 ACCEPTED / FROZEN AND M07 VALIDATION DOCS ARE FROZEN  
> Scope: P1 F02 only  
> Contract rule: reuse the existing schema-v3 host-only preference and standard Harness Start path. Do not add Windows login startup, schema v4, new Runtime lifecycle authority, retry loops, or relaxed eligibility/readiness.  
> External rule: do not run real External ChatGPT, modify the user's active credential/profile, or restart the user's active instance without explicit execution-time authorization.

---

# 1. Dependency

~~~text
M00 ACCEPTED / FROZEN
M01 ACCEPTED / FROZEN
M02 ACCEPTED / FROZEN
        ↓
M07 six docs frozen
        ↓
T1 Config contract
        ↓
T2 Shared Start path + AutoStartCoordinator
        ↓
T3 Settings toggle + i18n
        ↓
T4 Failure/concurrency/quit semantics
        ↓
T5 Packaged reopen + affected regression
        ↓
T6 Final development regression + delivery
        ↓
Integration → E2E → Manual
        ↓
M07 — ACCEPTED / FROZEN
~~~

M06 Development Complete is not the M07 dependency gate. Follow the frozen Roadmap.

---

# T1 — Freeze and Verify Existing F02 Config Contract

**AC:** M07-AC01, AC02  
**Primary areas:** protocol/src/v1/config.rs, ui/src-tauri/src/config/, generated protocol artifacts/tests.

## 1. Verify existing schema

Assert:

~~~text
CURRENT_SCHEMA_VERSION == 3
DesktopConfig.autoStartHarnessOnAppLaunch exists
DesktopConfig default == false
RuntimeConfigSnapshot excludes DesktopConfig
~~~

Do not add schema v4.

## 2. New-install default

Test ConfigStore::safe_defaults and first config creation:

- Auto Start false;
- no unrelated field drift.

## 3. v2→v3

Test migration:

- Auto Start becomes false;
- Workspace preserved;
- Tunnel preserved;
- SecretReference preserved;
- Tool/Policy fields preserve accepted migration semantics;
- configRevision semantics remain accepted.

## 4. Existing v3

Round-trip:

- explicit true remains true;
- explicit false remains false.

Do not normalize true back to false.

## 5. Focused desktop mutation

Test DesktopPreferencesPatch with only autoStartHarnessOnAppLaunch.

Assert:

- only this field changes;
- approvalNotifications preserved;
- connectionNotifications preserved;
- backgroundNoticeAcknowledged preserved;
- Runtime-affecting fields preserved;
- write is atomic.

## 6. No restart debt

Before/after Auto Start preference save:

- configRevision unchanged;
- RuntimeConfigSnapshot unchanged;
- no Restart Required;
- no lifecycle command invoked.

## Unit/contract verification

Run targeted protocol/config tests, then:

~~~text
cargo test -p harness-control-protocol
cargo test -p chatgpt-harness-ui-tauri config
~~~

Use actual package names if workspace naming differs.

### Done

The existing schema-v3 scaffold is proven suitable for F02 without migration expansion.

---

# T2 — Shared Host Start Request and AutoStartCoordinator

**AC:** M07-AC05, AC06, AC09, AC11  
**Depends on:** T1  
**Primary areas:** ui/src-tauri/src/commands/, harness.rs, desktop/, lib.rs.

## 1. Extract one shared standard Start helper

Refactor current harness_start Host logic into one internal function callable by:

- Tauri manual command;
- Tray Start;
- AutoStartCoordinator.

It must preserve:

- DesktopLifecycleCoordinator.start_allowed();
- HarnessController.start(...);
- existing operation mutex;
- existing BackendState/SecretStore;
- existing ControlError semantics.

Do not duplicate HarnessController.start.

## 2. Keep asynchronous Host behavior

Auto Start must not block App setup/UI event loop while waiting for Runtime/Tunnel readiness.

Use the same spawn_blocking/background lifecycle pattern as current manual Start.

## 3. Add AutoStartCoordinator

Prefer:

~~~text
ui/src-tauri/src/desktop/autostart.rs
~~~

Implement process-local once state.

Minimum observable internal outcomes:

- disabled;
- attempting;
- succeeded;
- failed;
- skipped/conflict.

State must store only safe bounded metadata.

## 4. App setup hook

After required Host managed state exists:

- read persisted config once;
- if no config -> disabled/no attempt;
- if invalid config -> no silent reset/start;
- if false -> no attempt;
- if true -> claim once and schedule shared standard Start.

Do not make React mount the trigger.

## 5. Standard path proof

Add tests/spies proving Auto Start does not directly call:

- RuntimeSupervisor.start;
- TunnelProcessManager.start.

It must reach those only through HarnessController.

## 6. Once guard

Repeated calls to coordinator trigger/evaluate in one process must not schedule another Start.

## 7. Quit guard

If app_quitting is already true, Auto Start must skip.

### Done

One process can issue at most one F02 Start request through the existing lifecycle authority.

---

# T3 — Settings Toggle and Localization

**AC:** M07-AC03, AC04  
**Depends on:** T1  
**Primary areas:** ui/src/pages/settings/, HarnessDataProvider/client, i18n resources, adjacent frontend tests.

## 1. Add General Settings row

Expose:

~~~text
Auto Start Harness on app launch
~~~

Use a checkbox/toggle consistent with current Settings UI.

## 2. Help text

Clearly communicate:

- starts Harness after app is opened;
- does not launch ChatGPT-Harness at Windows login;
- applies next App launch.

## 3. Persist through focused mutation

Use existing desktop preference client/provider path.

Do not submit a full AppConfig draft.

## 4. Save concurrency

While saving:

- disable only necessary control;
- do not overwrite another concurrent Settings mutation;
- on error refresh/revert to persisted truth.

## 5. No immediate lifecycle action

Add frontend tests:

- toggle ON does not call harnessStart;
- toggle OFF does not call harnessStop;
- neither calls harnessRestart;
- no Restart Required appears solely due this setting.

## 6. Localization

Add exact en and zh-CN strings.

No wording such as:

- Start with Windows;
- Launch at login;
- Windows startup.

## 7. Persisted refresh

Reload Settings/provider and verify saved true/false renders correctly.

### Done

User has an unambiguous, independent, persisted F02 control.

---

# T4 — Eligibility, Failure, Concurrency, Stop and Quit Semantics

**AC:** M07-AC07, AC08, AC09, AC10, AC11  
**Depends on:** T2/T3  
**Primary areas:** Tauri Host tests and existing lifecycle/diagnostics test modules.

## 1. Enabled happy path

With isolated valid config/secret and test Runtime/Tunnel:

- App setup evaluates enabled;
- one Auto Start request;
- StartupAttempt created;
- reaches Ready once;
- generation count = 1.

## 2. Disabled path

- no Start helper invocation;
- no Runtime/Tunnel launch;
- lifecycle stays Stopped.

## 3. Eligibility failures

Cover at least:

- invalid config;
- invalid/missing Workspace;
- missing Tunnel ID if required by current eligibility;
- missing API key/SecretReference;
- invalid Runtime distribution;
- invalid Tunnel distribution;
- app_quitting.

Expected:

- no bypass;
- no Ready;
- safe failure/diagnostic state;
- one attempt maximum when standard Start is actually invoked.

## 4. No retry

After failed Auto Start:

- wait/poll/status refresh;
- fix isolated config;
- verify no automatic second Start;
- manual Start may then succeed.

## 5. Auto vs manual race

Deterministically stage:

- Auto owns operation first;
- Manual owns operation first.

Assert:

- one Runtime generation max;
- one Tunnel generation max;
- duplicate receives skip/conflict/current-state result;
- no queued follow-up Auto Start.

## 6. Auto vs Tray race

Same one-generation assertions.

## 7. Auto then Stop

After Auto Ready:

- Stop;
- wait/refresh/hide/show;
- remain Stopped;
- no second automatic Start.

## 8. Restart

Explicit Restart still works and is not counted as Auto Start re-evaluation.

## 9. Quit race

Test:

- Quit before Auto claim;
- Quit during Auto Starting.

Assert:

- no Start after quit intent;
- existing shutdown/cancellation cleans processes;
- no orphan;
- failed Quit retry does not re-arm Auto Start.

## 10. Diagnostics

For representative Auto Start failure:

- existing Diagnostics identifies current cause;
- diagnostic run does not retry/start;
- previous Ready state does not mask current failure.

### Done

F02 remains fail-closed, single-shot and lifecycle-safe.

---

# T5 — Packaged Windows Reopen Harness and Cross-feature Regression

**AC:** M07-AC12, AC13, AC14  
**Depends on:** T2–T4  
**Primary areas:** existing E2E/test scripts, packaged candidate automation, affected M01/M02 regressions.

## 1. Add/extend packaged test harness

Support disposable profile/config/credential fixtures without touching the user's active profile.

## 2. Enabled full reopen

Automate:

1. save enabled=true in disposable profile;
2. fully quit App;
3. launch packaged App;
4. wait for one normal Auto Start;
5. reach authoritative Ready;
6. perform real external/loopback QA MCP probe according to test-plan boundary;
7. Close→Hide;
8. verify Harness still works;
9. Tray Show;
10. verify no second Auto Start;
11. Stop;
12. verify no same-process re-start;
13. Quit;
14. verify zero owned orphan.

## 3. Disabled full reopen

Automate:

- enabled=false;
- Quit;
- relaunch;
- observe Stopped/no managed startup;
- manual Start still succeeds.

## 4. Missing-secret packaged case

With isolated synthetic/missing credential state:

- enabled=true;
- relaunch;
- no Ready;
- Diagnostics identifies failure;
- no automatic retry.

## 5. Double-invocation/reopen behavior

Exercise normal supported user double-invocation/reopen path.

Assert F02 does not create duplicate managed Runtime/Tunnel.

If the product currently allows unsafe independent process ownership, report blocker instead of adding M07-only process hacks.

## 6. Windows login audit

Audit code/package behavior:

- no Run key;
- no Startup folder entry;
- no Task Scheduler registration;
- no Windows service;
- no login-launch plugin/config introduced.

## 7. M01 regression

Run:

- close/hide;
- Tray show/start/stop/quit;
- app_quitting;
- approval notification background behavior.

## 8. M02 regression

Run affected:

- StartEligibility;
- StartupPhaseRecorder;
- authoritative Ready;
- Diagnostics;
- current startup control-plane readiness behavior.

M07 introduces no new performance threshold and must not weaken M02.

## 9. V3/security smoke

Run:

- Workspace/security config;
- SecretStore boundary;
- Start/Stop/Restart;
- read/grep MCP smoke after Start.

### Done

Packaged candidate proves F02 in real App-process reopen scenarios without changing Windows startup semantics.

---

# T6 — Full Development Regression, Delivery and Handoff

**AC:** M07-AC01 through M07-AC14  
**Depends on:** T1–T5

## 1. Rust checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
~~~

## 2. Frontend checks

~~~text
cd ui
npm run typecheck
npm test
npm run build
~~~

## 3. Repository governance

Run:

- ui/scripts/check-repository-layout.ps1;
- ui/scripts/check-milestone-layout.ps1.

No new top-level directory.

## 4. Protocol/schema audit

Verify:

- schema version still 3;
- generated TS/JSON schema current;
- desktop field absent from RuntimeConfigSnapshot;
- no protocol/v2.

## 5. Security audit

Search/review for accidental:

- Windows startup registration;
- direct Runtime/Tunnel Start from AutoStartCoordinator;
- secret logging;
- retry loop;
- duplicate lifecycle mutex.

## 6. Fresh Portable candidate

Use existing delivery flow with milestone:

~~~text
M07-auto-start-on-app-launch
~~~

Do not overwrite prior candidates.

## 7. Development Complete output

Report:

~~~text
M07 Development Complete:
candidate SHA:
product version:
source dirty:
Portable path/hash:
UI hash:
Runtime hash:
Tunnel hash:
config schema:
Auto Start default:
Runtime config revision impact:
schema/protocol changes:
enabled dev scenario:
disabled dev scenario:
failure/no-retry scenario:
manual/Tray race:
Stop/no-restart:
Quit cleanup:
M01 regression:
M02 regression:
V3/security regression:
External ChatGPT: NOT EXECUTED
Formal Integration: NOT EXECUTED
Formal E2E: NOT EXECUTED
Manual: NOT EXECUTED
~~~

Do not mark M07 ACCEPTED / FROZEN during Development Complete.

### Done

M07 candidate is ready for independent formal validation.

---

# 2. Task → AC → Reserved Validation Traceability

| Task | M07 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01/02 | M07-INT-01/02, M07-E2E-01, M07-MAN-01 |
| T2 | AC05/06/09/11 | M07-INT-04/05/07/08, M07-E2E-02/03, M07-MAN-02/03 |
| T3 | AC03/04 | M07-INT-03, M07-E2E-01, M07-MAN-01 |
| T4 | AC07/08/09/10/11 | M07-INT-06/07/08/09/10, M07-E2E-02/03, M07-MAN-02/03/04 |
| T5 | AC12/13/14 | M07-INT-11/12, M07-E2E-03/04, M07-MAN-04/05 |
| T6 | AC01–AC14 regression closure | all formal suites after Development Complete |

---

# 3. Development Handoff

After T6:

~~~text
M07 Development Complete
→ independent M07 Integration
→ packaged automated E2E
→ real External ChatGPT Manual Acceptance
→ user sign-off
→ M07 — ACCEPTED / FROZEN
→ contributes to M11 V4 Full Gate
~~~

M07 does not by itself produce V4 — ACCEPTED / FROZEN.
