# M07 PLAN — Auto Start Harness on App Launch

> Input: SPEC.md  
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M00/M01/M02 ACCEPTED / FROZEN AND M07 VALIDATION DOCS ARE FROZEN  
> Scope: P1 F02 only  
> Primary owner: Tauri Host desktop/lifecycle + existing Settings surface  
> Runtime rule: no new Runtime lifecycle authority, no new RuntimeConfigSnapshot field, no schema v4  
> External rule: real External ChatGPT, restarting the user's active instance, and destructive persistent-state testing require explicit execution-time authorization.

---

# 1. Delivery Strategy

M07 closes the F02 scaffold already present in schema v3.

~~~text
existing DesktopConfig bool
        ↓
T1 freeze/verify config contract
        ↓
T2 shared Host Start helper + AutoStartCoordinator
        ↓
T3 Settings toggle + i18n
        ↓
T4 concurrency/failure/quit semantics
        ↓
T5 packaged reopen + regression harness
        ↓
T6 full development regression + Portable handoff
        ↓
Formal Integration
        ↓
Automated E2E
        ↓
Manual External ChatGPT acceptance
        ↓
M07 — ACCEPTED / FROZEN
~~~

Do not build a second startup architecture.

---

# 2. Implementation Principles

1. Reuse the existing schema-v3 field rather than migrating again.
2. Keep F02 Host-only.
3. Refactor only enough to make Manual Start, Tray Start and Auto Start share one standard Host start-request function.
4. Preserve HarnessController as authoritative lifecycle owner.
5. Preserve the existing operation mutex.
6. Preserve M02 StartEligibility, StartupPhaseRecorder and Ready predicates.
7. Never auto-retry after an F02 failure.
8. Keep UI saves focused and conflict-safe.
9. Add no Windows login persistence.
10. Treat failures as real product state, not as a reason to alter config/security automatically.

---

# 3. Expected Landing Zones

Prefer existing modules:

~~~text
protocol/src/v1/config.rs
ui/src-tauri/src/config/
ui/src-tauri/src/commands/
ui/src-tauri/src/desktop/
ui/src-tauri/src/lib.rs
ui/src/pages/settings/
ui/src/app/providers/
ui/src/services/harness/
ui/src/i18n/
ui/src-tauri/ existing test modules
ui/src/ adjacent tests
~~~

Likely new internal file:

~~~text
ui/src-tauri/src/desktop/autostart.rs
~~~

No new top-level directory.

No standalone daemon/project.

---

# 4. Phase A — Config Contract and Compatibility

## 4.1 Verify existing field

Confirm current generated protocol/TS/JSON schema all expose:

~~~text
desktop.autoStartHarnessOnAppLaunch: boolean
~~~

## 4.2 Default

Verify DesktopConfig::default and ConfigStore::safe_defaults produce false.

## 4.3 Migration

Verify v2→v3 produces false without changing unrelated values.

Verify existing v3 true/false round-trips unchanged.

## 4.4 Focused save

Extend/retain tests proving desktop_preferences_set:

- reads latest config first;
- changes only requested desktop field;
- preserves notifications/background preference;
- preserves all Runtime-affecting config;
- saves atomically.

## 4.5 Revision semantics

Verify Auto Start-only save:

- does not increment configRevision;
- does not create Restart Required;
- does not change RuntimeConfigSnapshot.

Exit: schema/config behavior is already correct or minimally repaired without schema bump.

---

# 5. Phase B — Shared Standard Start Request

The current Tauri command contains standard Host checks around HarnessController.start.

Extract a reusable internal helper, conceptually:

~~~text
request_harness_start(app, origin)
~~~

or equivalent.

The helper owns the same path for:

- Manual UI Start;
- Tray Start;
- Auto Start.

It must preserve:

~~~text
DesktopLifecycleCoordinator.start_allowed
→ HarnessController.start
→ operation mutex
→ StartEligibility
→ StartupPhaseRecorder
→ authoritative Ready
~~~

Start origin may be internal/observational only.

Do not use origin to weaken eligibility or security.

Unit tests:

- manual path still works;
- lifecycle conflict still returns bounded error;
- app_quitting still blocks;
- helper does not directly start Runtime/Tunnel.

Exit: only one lifecycle start implementation remains.

---

# 6. Phase C — AutoStartCoordinator

## 6.1 Process-local once guard

Coordinator owns one evaluation/claim per App process.

Required states may be represented differently, but tests must distinguish:

- disabled;
- scheduled/attempting;
- succeeded;
- failed;
- skipped.

## 6.2 Setup ordering

During Tauri setup:

1. all managed Host states exist;
2. ConfigStore path is available;
3. SecretStore BackendState is managed;
4. Tray/desktop coordinator has valid lifecycle state;
5. read persisted Auto Start preference;
6. if true, schedule exactly one background standard Start request.

Do not block the Tauri UI thread while Harness Start waits for Runtime/Tunnel readiness.

## 6.3 Missing config

No config means no automatic Start.

Do not create an enabled value.

## 6.4 Invalid config

Do not silently rewrite or reset it.

Do not Start.

Current config/diagnostic error remains visible through accepted surfaces.

## 6.5 Enabled

Schedule one start.

If it succeeds, normal lifecycle reaches Ready.

If it fails, record one safe terminal coordinator outcome and stop retrying.

Exit: App launch triggers the existing lifecycle safely and only once.

---

# 7. Phase D — Settings UI

Place Auto Start in the existing General settings area unless current UI composition strongly requires another existing section.

Add:

- checkbox/toggle;
- label;
- explanatory help;
- saved feedback.

Wording explicitly says:

- open app → Start Harness;
- not Windows login startup;
- next App launch.

Use existing HarnessDataProvider/config mutation path for desktop_preferences_set.

Do not use full AppConfig draft.

## UI state rules

- persisted false → unchecked;
- persisted true → checked;
- in-flight save → disabled;
- save error → restore/refetch persisted truth;
- save success → no restart banner;
- toggle ON does not invoke harnessStart;
- toggle OFF does not invoke harnessStop.

Add en/zh-CN strings.

Exit: user can control F02 without misunderstanding its scope.

---

# 8. Phase E — Concurrency and Lifecycle Races

Build deterministic tests around shared lifecycle authority.

## 8.1 Auto vs Manual

Cases:

- Auto claims first, Manual arrives;
- Manual claims first, Auto arrives;
- Tray Start races Auto.

Expected:

- one generation maximum;
- no second queued Auto attempt;
- conflict/skip is bounded;
- one final authoritative lifecycle state.

## 8.2 Auto vs Quit

Cases:

- Quit before Auto claim;
- Quit while Auto start is in progress.

Expected:

- no new Start after app_quitting;
- in-flight processes cleaned via existing lifecycle;
- zero orphan at completion.

## 8.3 Auto then Stop

After Ready:

- user Stop;
- remain Stopped;
- wait/status refresh/window hide/show;
- no Auto re-trigger.

## 8.4 Failure then repair

Auto fails due controlled missing credential/config issue.

After repair in same process:

- no automatic retry;
- Manual Start may succeed.

Exit: no retry storms or duplicate lifecycle generation.

---

# 9. Phase F — Diagnostics / Startup Evidence

Because Auto Start uses HarnessController.start, verify it appears in existing M02 startup evidence.

Required:

- one StartupAttempt per actual Auto Start request;
- normal phases;
- failure status when eligibility/readiness fails;
- authoritative Ready only at normal final condition.

Diagnostics:

- missing credential/config/workspace/tunnel failure remains visible;
- diagnostic execution stays read-only;
- Auto Start state does not override Diagnostics truth.

No new startup performance formula.

---

# 10. Phase G — Packaged Windows Reopen Scenarios

Prepare automated E2E support for:

## Enabled

~~~text
save enabled
→ Quit
→ launch Portable/App
→ one Auto Start
→ Ready
→ real MCP probe succeeds
→ Close→Hide
→ probe still succeeds
→ Show
→ Stop
→ wait
→ remains Stopped
→ Quit
~~~

## Disabled

~~~text
save disabled
→ Quit
→ launch
→ Stopped
→ no Runtime/Tunnel launched by Auto Start
→ Manual Start succeeds
~~~

## Missing credential

~~~text
enabled
→ isolated config with missing test credential
→ launch
→ no Ready
→ one failed startup attempt
→ Diagnostics identifies credential/setup problem
→ no retry
~~~

Do not modify the user's real credential/profile for automated tests.

---

# 11. Regression Strategy

Run targeted regression after each phase, then full development gate.

Required affected suites:

- Config migration/contract;
- desktop preferences;
- Settings;
- Harness start/stop/restart;
- DesktopLifecycleCoordinator;
- Tray;
- M01 notification state;
- M02 startup recorder;
- M02 Diagnostics;
- SecretStore;
- protocol generation;
- Runtime/Tunnel ownership;
- existing read/grep smoke.

If a change touches Runtime/Tool catalog/Eval unexpectedly, stop and classify impact before continuing.

---

# 12. Rollback Strategy

M07 is designed for simple rollback.

Product code rollback removes:

- AutoStartCoordinator;
- Settings toggle exposure;
- shared helper refactor if necessary.

Persisted schema remains v3 and the already-existing boolean remains safe.

A user config with autoStartHarnessOnAppLaunch=true is parseable by the pre-M07 current code because the field already exists.

Therefore no destructive config downgrade should be needed.

Rollback must not delete config or secrets.

---

# 13. Development Verification Commands

Minimum:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Then repository/milestone layout checks and production Portable delivery scripts.

Development tests must not perform External ChatGPT or real-user destructive profile changes.

---

# 14. Defect Rules

If implementation reveals an upstream contradiction:

- do not edit frozen upstream docs merely to make tests pass;
- stop and report the exact conflict;
- only revise contracts with explicit user authorization.

Examples that require escalation:

- existing schema-v3 field is incompatible with real persisted configs;
- standard Start path cannot be called safely from setup without a lifecycle refactor larger than M07;
- current app allows unsafe duplicate process ownership that F02 exposes;
- Auto Start would require bypassing SecretStore/eligibility.

---

# 15. Development Completion Gate

M07 Development Complete requires:

- config contract tests PASS;
- shared start helper PASS;
- AutoStartCoordinator once/race tests PASS;
- Settings/i18n PASS;
- enabled/disabled/failure/Stop/Quit development scenarios PASS;
- no schema bump;
- no Runtime restart debt;
- no Windows login persistence;
- affected M01/M02/V3 regressions PASS;
- fresh Portable candidate generated;
- formal Integration/E2E/Manual remain truthfully NOT EXECUTED unless run.

Development Complete is not ACCEPTED / FROZEN.
