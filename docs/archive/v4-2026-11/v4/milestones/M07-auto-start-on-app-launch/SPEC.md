# M07 SPEC — Auto Start Harness on App Launch

> 布局与验证生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。启动阶段只创建 SPEC / PLAN / TASKS；后续补齐 Integration / E2E / Manual 三份验证计划，最终严格保持六文件布局。  
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M00/M01/M02 ACCEPTED / FROZEN AND M07 SIX DOCS ARE FROZEN  
> Milestone: M07 — Auto Start on App Launch  
> Scope: P1 F02 only  
> Roadmap dependencies: M00 + M01 + M02  
> Upstream canonical ACs: F02-AC01 / F02-AC02 / F02-AC03  
> Architecture: A01 / A02 / A11 / A15; Desktop Integration §6 Auto Start  
> Current main observed before this document: 886fd3f85fc519ad9ec9d295622a871e04896664  
> Current code fact: schema v3 already contains host-only desktop.autoStartHarnessOnAppLaunch, default false, and desktop_preferences_set can persist it without Runtime restart debt; the current Settings page does not expose the toggle and current App setup does not auto-start Harness from it.  
> Current dependency note: latest M02 evidence on main is still candidate-only rather than ACCEPTED / FROZEN. M07 documentation may proceed; formal implementation remains blocked until Roadmap dependencies are accepted.

---

# 1. Objective

F02 removes one repetitive user action:

> When the user explicitly opens ChatGPT-Harness, the application may automatically execute the normal Start Harness flow.

The feature is controlled by one independent persisted setting:

~~~text
Auto Start Harness on app launch
~~~

New installations keep it OFF by default.

When enabled, each real App process launch evaluates the preference once and, after Host initialization is ready, performs at most one automatic request to the existing standard Harness Start path.

M07 does not create a second lifecycle authority and does not change what “Ready” means.

---

# 2. Canonical PRD Contract

## F02-AC01

Auto Start on app launch is an independent persisted setting. New install default is OFF. Upgrade must not overwrite an existing explicit user choice.

## F02-AC02

When enabled, after App initialization the Host automatically attempts the standard Harness Start flow exactly once and does not bypass config / eligibility / secret / security checks.

## F02-AC03

Auto Start failure enters truthful Error / Diagnostics behavior, does not repeatedly start, does not report false Ready, and is not Windows login startup.

These three canonical ACs are mandatory and cannot be weakened by M07-derived behavior.

---

# 3. Current Code Baseline

At the observed main revision:

## 3.1 Already present

protocol/src/v1/config.rs already defines:

~~~text
AppConfig.schemaVersion = 3
AppConfig.desktop: DesktopConfig
DesktopConfig.autoStartHarnessOnAppLaunch: bool
Default = false
~~~

The field is host-owned and excluded from RuntimeConfigSnapshot.

ui/src-tauri/src/config/mod.rs already defines a focused DesktopPreferencesPatch with:

~~~text
autoStartHarnessOnAppLaunch?: bool
~~~

and ConfigStore::set_desktop_preferences(...).

same_restart_semantics(...) excludes the entire DesktopConfig, therefore a desktop-preference-only save does not advance Runtime-facing configRevision or create Runtime restart debt.

ui/src-tauri/src/commands/mod.rs already exposes desktop_preferences_set and the standard harness_start command.

HarnessController already owns:

- StartEligibility;
- authoritative lifecycle;
- Runtime/Tunnel startup;
- operation mutex;
- StartupPhaseRecorder;
- final Ready;
- startup failure handling.

DesktopLifecycleCoordinator::start_allowed() already rejects Start when Harness is Starting/Ready/Degraded/Stopping or App Quit is in progress.

## 3.2 Missing F02 product closure

Current Settings UI does not expose the Auto Start preference.

Current Tauri App setup does not evaluate this preference and does not issue an automatic Harness Start.

There is no M07-owned per-process Auto Start coordinator/state that guarantees one evaluation/attempt.

Therefore the configuration field is existing scaffolding, not evidence that F02 is implemented.

---

# 4. Scope

M07 includes:

1. freeze and verify existing schema-v3 Auto Start preference contract;
2. Settings UI toggle;
3. zh-CN / en localization;
4. Tauri Host Auto Start coordinator;
5. one process-local evaluation per App launch;
6. standard Harness Start reuse;
7. concurrency arbitration with manual/Tray Start;
8. Quit/Stop race handling;
9. truthful startup-attempt and Diagnostics failure path;
10. App reopen behavior;
11. disabled behavior;
12. missing/invalid config/credential/eligibility behavior;
13. packaged Windows validation;
14. real External ChatGPT final acceptance in Manual;
15. affected M01/M02/V3 regression.

---

# 5. Non-goals

M07 does not implement:

- Windows login startup;
- Start with Windows;
- registry Run keys;
- Startup folder shortcuts;
- Task Scheduler entries;
- Windows service installation;
- background daemon separate from Tauri;
- automatic retry loop;
- automatic restart after user Stop;
- automatic recovery policy beyond existing Harness behavior;
- new Runtime lifecycle protocol;
- new RuntimeConfigSnapshot field;
- schema v4;
- new readiness condition;
- new performance threshold;
- new secret storage;
- automatic Start immediately when the user toggles the setting ON;
- automatic Stop immediately when the user toggles the setting OFF;
- M08 Metrics/Trace;
- M09 file Tools;
- M10 MCP conformance.

M07 is only “App launched → optionally request the existing Start flow once”.

---

# 6. Persisted Configuration Contract

Canonical persisted path remains:

~~~text
desktop.autoStartHarnessOnAppLaunch
~~~

Type:

~~~text
boolean
~~~

## 6.1 New install

Default:

~~~text
false
~~~

A missing config file must not cause Harness to auto-start.

The ordinary config bootstrap may later create safe defaults, but Auto Start inspection itself must not manufacture an enabled preference.

## 6.2 v2 → v3 migration

A v2 config has no explicit F02 choice.

Migration produces:

~~~text
desktop.autoStartHarnessOnAppLaunch = false
~~~

All existing Workspace, Tunnel, SecretReference, Tool, Policy and other supported values remain governed by the accepted v2→v3 migration contract.

## 6.3 Existing v3

Explicit true must remain true across normal load/save/upgrade.

Explicit false must remain false.

M07 must not reinterpret an invalid/corrupt config as “false and continue normally”; invalid config remains an error/diagnostic condition.

## 6.4 Reset

Resetting local config returns to safe new-install semantics:

~~~text
autoStartHarnessOnAppLaunch = false
~~~

## 6.5 No schema bump

M07 MUST NOT increment:

~~~text
CURRENT_SCHEMA_VERSION = 3
~~~

The field is already part of schema v3.

---

# 7. Host-only / Restart Semantics

Auto Start is a Host-only desktop preference.

Changing it:

- persists atomically through desktop_preferences_set;
- does not alter RuntimeConfigSnapshot;
- does not increment Runtime-facing configRevision;
- does not create Restart Required;
- does not restart Runtime/Tunnel;
- does not stop an active Harness;
- does not start Harness immediately in the current App process.

The saved value controls the next real App process launch.

This follows A15: Host-only preferences do not create Runtime restart debt.

---

# 8. Settings UX Contract

Expose one independent toggle in Settings, preferably the existing General section rather than creating a new navigation surface.

Required label meaning:

~~~text
Auto Start Harness on app launch
~~~

Required help meaning:

~~~text
Automatically starts Harness after you open ChatGPT-Harness.
This does not start ChatGPT-Harness when Windows signs in.
The change takes effect the next time the app is launched.
~~~

Equivalent localized wording is allowed.

Required behavior:

- current persisted value is shown;
- toggle save uses focused desktop-preference mutation;
- save success/failure is visible;
- failed save reverts/refreshes to persisted truth;
- setting is disabled while its own mutation is in flight;
- no Runtime restart badge is created by this change;
- existing notification preference saves cannot overwrite Auto Start;
- Auto Start save cannot overwrite notification/language/theme/connection drafts.

The UI does not claim that enabling it has already started Harness.

---

# 9. App Launch Definition

For F02, “App launch” means:

> creation and initialization of a new ChatGPT-Harness Tauri application process.

It does NOT mean:

- showing an already-running hidden window;
- clicking Tray → Show;
- navigating between pages;
- re-rendering React;
- reopening the main window after Close→Hide;
- changing the setting;
- manually stopping Harness;
- a background status refresh.

Each App process may evaluate the Auto Start preference at most once.

A clean Quit followed by starting the application again creates a new App process and therefore a new allowed Auto Start evaluation.

---

# 10. Auto Start Coordinator

Add one Host-owned coordinator, preferably under the existing desktop module, for example:

~~~text
ui/src-tauri/src/desktop/autostart.rs
~~~

No new top-level directory.

Conceptual state:

~~~text
AutoStartCoordinator {
  evaluated: bool
  enabledAtLaunch: bool
  state:
    not_evaluated
    disabled
    pending
    attempting
    succeeded
    failed
    skipped
  safeFailureCode?: String
}
~~~

This state is observational Host state only. It is not authorization and does not create execution permits.

The exact internal representation may differ, but the following semantics are frozen.

---

# 11. Initialization Ordering

Auto Start may be scheduled only after required Tauri managed state is installed and the normal Host lifecycle can safely execute a Start.

At minimum the Host must have initialized:

- ConfigStore path/access;
- SecretStore BackendState;
- RuntimeSupervisor;
- TunnelProcessManager;
- HarnessController;
- DesktopLifecycleCoordinator;
- Tauri event loop/setup state needed by normal Start.

The coordinator reads the persisted preference once.

If disabled:

~~~text
no Start request
state = disabled
~~~

If enabled:

~~~text
schedule one background standard Start request
~~~

The Start must run off the UI/event-loop thread using the same blocking/background pattern as the existing command path.

React mounting is not a prerequisite for Auto Start.

---

# 12. Standard Start Reuse

M07 must not duplicate Harness startup logic.

Manual UI Start, Tray Start and Auto Start must converge on one internal Host start-request helper that preserves the existing behavior:

~~~text
DesktopLifecycleCoordinator.start_allowed()
→ HarnessController.start(...)
→ HarnessController operation mutex
→ StartEligibility
→ Config / Secret / Security validation
→ Runtime start
→ MCP readiness
→ Tunnel start/readiness
→ authoritative Harness Ready
→ StartupPhaseRecorder
~~~

Allowed refactor:

- extract a shared internal Host helper from the current harness_start command;
- have both Tauri command and Auto Start coordinator call it.

Forbidden:

- Auto Start calling RuntimeSupervisor.start directly;
- Auto Start calling TunnelProcessManager.start directly;
- bypassing HarnessController;
- bypassing StartEligibility;
- fabricating Ready;
- creating a parallel lifecycle mutex.

---

# 13. Exactly-once Process Semantics

“Exactly once” means:

- preference evaluated at most once per App process;
- if enabled, at most one Auto Start request scheduled per App process;
- no automatic retry loop after failure;
- hide/show/navigation cannot trigger another Auto Start;
- manual Stop after an Auto Start does not cause automatic re-Start in the same process;
- failed manual Start later in the same process does not reactivate Auto Start;
- clean Quit + new App process may Auto Start once again.

The coordinator must have an explicit once/claim guard, not rely on timing assumptions.

---

# 14. Concurrency Contract

Auto Start shares the same lifecycle authority as Manual Start and Tray Start.

## 14.1 Manual/Tray Start wins first

If Harness has already entered Starting/Ready/Degraded:

- Auto Start does not start a second generation;
- Auto Start records a safe skipped/conflict outcome;
- no retry is queued.

## 14.2 Auto Start wins first

If Auto Start owns the normal start operation:

- a concurrent Manual/Tray Start is rejected/merged by existing lifecycle semantics;
- no second Runtime/Tunnel generation launches;
- UI receives existing lifecycle conflict/current-state behavior.

## 14.3 Operation mutex

There remains one Harness operation mutex.

M07 must not add a second lifecycle lock that can deadlock with the existing operation mutex.

## 14.4 Duplicate outcome

A duplicate/race prevented by authoritative lifecycle state does not trigger an Auto Start retry.

---

# 15. Stop / Restart / Quit Semantics

## Stop

After successful Auto Start:

- user Stop works normally;
- Harness becomes Stopped;
- Auto Start does not re-run in the same App process.

## Restart

User Restart remains an explicit lifecycle action and is not another Auto Start evaluation.

## Quit

app_quitting wins races.

If Quit begins before Auto Start starts:

- Auto Start is skipped;
- no Runtime/Tunnel launch occurs after quit intent.

If Quit begins while Auto Start is already Starting:

- existing Stop/Quit lifecycle semantics own cancellation/cleanup;
- no orphan Runtime/Tunnel remains.

Failed Quit cleanup retains existing retry semantics and still must not create another Auto Start attempt.

---

# 16. Eligibility / Security Contract

Auto Start has no special eligibility.

It uses the same current checks as Manual Start, including as applicable:

- config present/valid;
- Workspace valid;
- Runtime distribution available;
- Tunnel ID/config valid;
- API key/SecretReference present;
- Tunnel distribution available;
- lifecycle allows Start;
- security mode fields valid;
- App is not quitting.

M07 cannot silently:

- substitute another Workspace;
- create credentials;
- disable Policy;
- enable developer mode;
- skip Tunnel readiness;
- skip MCP target readiness;
- change Tool configuration.

---

# 17. Secret Contract

The Auto Start preference contains no secret.

Auto Start:

- reads only the persisted boolean preference;
- uses existing SecretStore lookup through standard Start;
- does not copy API keys into config;
- does not put secret values in coordinator state/logs/UI;
- does not expose SecretReference internals beyond existing safe status.

Missing/failed credential remains a normal eligibility/start failure.

---

# 18. Failure Semantics

There is no Auto Start retry loop.

On a genuine standard Start failure:

- the one automatic attempt ends;
- existing StartupPhaseRecorder records the attempt/failure where applicable;
- existing Harness lifecycle/error semantics remain authoritative;
- Diagnostics reports current cause;
- UI must not report Ready;
- user may repair configuration and manually Start;
- coordinator does not automatically retry after the fix;
- a future full App relaunch may attempt again if preference remains enabled.

Safe typed error codes may be recorded. Raw secrets/argv/env/physical paths are not.

---

# 19. Diagnostics and Startup Evidence

M07 reuses existing M02 startup/diagnostic surfaces.

A real Auto Start attempt must create the same truthful StartupAttempt stages as a manual Start because it uses the same HarnessController.start path.

M07 does not create a second startup timing model and adds no new F05 threshold.

Diagnostics remains read-only by default.

If Auto Start fails before Ready:

- current cause is diagnosable;
- Ready is not fabricated;
- previous successful evidence does not override current failure.

---

# 20. Disabled Behavior

With preference false:

- App launch does not invoke Start;
- lifecycle remains Stopped unless an explicit action starts Harness;
- no Runtime/Tunnel process is launched by F02;
- no failed Auto Start attempt is fabricated;
- Manual Start and Tray Start remain unchanged.

This is a mandatory product state.

---

# 21. App Reopen / Hidden Window Behavior

Required enabled sequence:

~~~text
enabled=true
→ Quit App
→ launch new App process
→ one Auto Start
→ Ready
→ close main window
→ hidden in Tray
→ Harness remains running
→ Tray Show
→ no second Auto Start
→ Stop
→ no automatic re-Start
~~~

Required disabled sequence:

~~~text
enabled=false
→ Quit App
→ launch new App process
→ no Auto Start
→ Stopped
→ manual Start still works
~~~

F01 background behavior remains authoritative after Auto Start succeeds.

---

# 22. Double-launch / Duplicate Activation Boundary

M07 does not add multi-instance or Windows login infrastructure.

If the current platform can create a second application process from a second executable invocation, M07 must not introduce extra duplicate managed Runtime/Tunnel ownership.

The supported packaged validation path must exercise the product’s normal double-invocation/reopen behavior and prove that F02 does not create duplicate managed Harness generations.

If a true second process can independently own duplicate managed Runtime/Tunnel processes, report the lifecycle blocker rather than hide it with M07-specific heuristics.

---

# 23. UI / Localization

Add en and zh-CN strings for:

- title;
- help text;
- saved success;
- failure via existing error-presentation mapping where applicable.

Wording must distinguish:

~~~text
open ChatGPT-Harness app → optionally Start Harness
~~~

from:

~~~text
Windows sign-in → launch ChatGPT-Harness
~~~

Do not label the setting “Start with Windows”, “Launch at login”, or equivalent.

---

# 24. Protocol / Schema Boundary

M07 requires no Runtime wire-protocol change.

Expected:

~~~text
CURRENT_SCHEMA_VERSION = 3
RuntimeConfigSnapshot does not contain desktop
DesktopConfig.autoStartHarnessOnAppLaunch remains boolean
~~~

If generated TypeScript/JSON Schema artifacts are stale, regenerate them as contract maintenance only.

Do not create protocol/v2.

---

# 25. Security Invariants

M07 preserves:

1. Runtime as execution/security authority.
2. HarnessController as lifecycle owner under Tauri Host.
3. Auto Start cannot create ExecutionPermit/Approval/SessionGrant.
4. Auto Start only initiates standard lifecycle Start.
5. StartEligibility cannot be bypassed.
6. SecretStore boundary remains unchanged.
7. Ready predicate remains unchanged.
8. Workspace/Policy/Approval/Tool semantics remain unchanged.
9. History remains observational.
10. Quit wins over future Auto Start scheduling.
11. No unbounded retry/background loop.
12. No Windows persistence/startup mechanism is installed.

---

# 26. Affected Regression Scope

At minimum rerun affected behavior for:

- M01 Close→Hide;
- Tray Start/Stop/Show/Quit;
- app_quitting race;
- Approval notification background state;
- M02 StartEligibility;
- M02 StartupPhaseRecorder;
- authoritative Ready;
- M02 Diagnostics;
- config v2→v3;
- configRevision/restart-debt semantics;
- SecretStore presence checks;
- Runtime/Tunnel Start/Stop/Restart;
- no orphan process;
- read/grep MCP smoke after Auto Start.

M04/M05 paid Evaluation need not rerun unless implementation unexpectedly changes Runtime/Tool/Eval identities. Such a change is outside expected M07 scope and must be treated as an impact blocker/requalification question.

---

# 27. M07 Derived Acceptance Criteria

| M07 AC | Requirement |
| --- | --- |
| M07-AC01 | Existing schema-v3 desktop.autoStartHarnessOnAppLaunch remains the only persisted F02 preference; new/default/v2-migrated state is false, explicit v3 true/false is preserved, and no schema v4 is introduced. |
| M07-AC02 | Auto Start is Host-only: focused preference save is atomic, does not change RuntimeConfigSnapshot/configRevision, does not create Restart Required, and cannot overwrite unrelated Settings state. |
| M07-AC03 | Settings exposes a localized independent toggle with clear “app launch, not Windows login” wording and persisted truth. |
| M07-AC04 | Enabling/disabling affects only future App process launches; enabling does not immediately Start and disabling does not Stop current Harness. |
| M07-AC05 | Each App process evaluates the preference once and schedules at most one Auto Start request after required Host state is initialized; React/window show is not the trigger. |
| M07-AC06 | Auto Start reuses the same standard Harness Start path, start_allowed, operation mutex, StartEligibility, StartupPhaseRecorder, Runtime/Tunnel startup and authoritative Ready; no direct bypass exists. |
| M07-AC07 | Config/Workspace/Tunnel/Secret/security eligibility failures remain fail-closed and use existing safe Diagnostics/startup failure semantics; no workaround is introduced. |
| M07-AC08 | A genuine Auto Start failure performs no automatic retry, never reports false Ready, remains diagnosable, and permits later explicit Manual Start after repair. |
| M07-AC09 | Auto Start, UI Start and Tray Start races launch at most one managed Harness generation; duplicate attempts are rejected/skipped by shared lifecycle authority and never queue a second start. |
| M07-AC10 | User Stop/Restart retains existing semantics; after Auto Start then Stop, F02 does not re-start in the same App process. |
| M07-AC11 | App Quit wins races: no new Auto Start begins after quit intent, in-flight Start is cleaned by existing shutdown semantics, and no owned Runtime/Tunnel orphan remains. |
| M07-AC12 | Full Quit→reopen with enabled causes one automatic Start; Close→Hide/Tray Show/navigation do not retrigger; disabled reopen causes no automatic Start. |
| M07-AC13 | M07 creates no Windows login/startup registration, Task Scheduler/Run-key/Startup-folder behavior or independent daemon. |
| M07-AC14 | Packaged Windows candidate preserves M01/M02/V3 lifecycle/security behavior and, when enabled, reaches real authoritative Ready such that the normal external MCP/ChatGPT product chain can use Tools. |

---

# 28. Canonical → Derived → Task → Validation Traceability

| Canonical | Derived | Task owner | Reserved Integration | Reserved E2E | Reserved Manual |
| --- | --- | --- | --- | --- | --- |
| F02-AC01 | AC01/02/03/04 | T1/T3 | M07-INT-01/02/03 | M07-E2E-01 | M07-MAN-01 |
| F02-AC02 | AC05/06/07/09/11 | T2/T4 | M07-INT-04/05/06/07/08 | M07-E2E-02/03 | M07-MAN-02/03 |
| F02-AC03 | AC08/10/12/13/14 | T4/T5/T6 | M07-INT-09/10/11/12 | M07-E2E-03/04 | M07-MAN-03/04/05 |

Reserved suites:

- Integration: M07-INT-01 through M07-INT-12.
- E2E: M07-E2E-01 through M07-E2E-04.
- Manual: M07-MAN-01 through M07-MAN-05.

---

# 29. Development Entry Gate

Formal M07 implementation may begin only when:

1. M00 is ACCEPTED / FROZEN;
2. M01 is ACCEPTED / FROZEN;
3. M02 is ACCEPTED / FROZEN;
4. all six M07 docs are frozen;
5. existing F02 config scaffold is compatible with current main;
6. no unresolved lifecycle/config blocker prevents one standard Start path from being reused.

M06 Development Complete is neither an M07 prerequisite nor a substitute for M01/M02 acceptance. Roadmap dependencies remain M00/M01/M02.

---

# 30. Development Exit Gate

M07 Development Complete means:

- T1–T6 implemented;
- unit/contract tests PASS;
- Settings toggle works against isolated config;
- Host Auto Start coordinator works in test-owned App lifecycle;
- enabled/disabled/error/concurrency/quit semantics have development evidence;
- no schema bump/restart debt/Windows-login behavior introduced;
- affected M01/M02/V3 regression PASS;
- fresh Portable candidate produced;
- formal Integration/E2E/Manual remain NOT EXECUTED unless actually run.

Development Complete is not M07 ACCEPTED / FROZEN.

---

# 31. Final Exit Gate

M07 may become ACCEPTED / FROZEN only when:

- F02-AC01/02/03 PASS;
- M07-AC01 through M07-AC14 PASS;
- M07 Integration PASS;
- M07 E2E PASS;
- M07 Manual Acceptance PASS;
- real Windows App Quit→reopen is tested;
- enabled path Auto Starts exactly once;
- disabled path does not Auto Start;
- missing secret/invalid eligibility does not report Ready;
- manual/Tray/Auto races create no duplicate generation;
- user Stop does not trigger same-process Auto Start again;
- Quit leaves no owned orphan;
- no Windows login startup mechanism exists;
- real external product-chain Tool use succeeds after enabled Auto Start;
- no unresolved F01/F05/F06/V3 regression remains.

Successful milestone wording:

~~~text
M07 — ACCEPTED / FROZEN
~~~

M07 alone does not imply V4 Full acceptance; M11 remains the Full Release Gate.
