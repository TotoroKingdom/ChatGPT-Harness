# M01 TASKS — Tray, Background Lifecycle and Notifications

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: DRAFT / IMPLEMENTATION BLOCKED UNTIL M00 ACCEPTED / FROZEN  
> Scope: F01 + F03 + F04 only.  
> Rule: every Task includes implementation plus Unit/contract/code-level verification. Integration/E2E/Manual are separate and must not be marked PASS here.

---

## 1. Dependency

~~~text
M00 ACCEPTED / FROZEN
        ↓
T1 Config v3 + Tauri desktop dependency gate
        ↓
T2 Desktop lifecycle + Tray
        ├───────────────┐
        ↓               ↓
T3 Approval            T4 Disconnect/recovery
notifications          incidents
        └───────┬───────┘
                ↓
T5 React navigation + Settings + i18n
                ↓
T6 Packaging / regression / Development Complete
~~~

T3 and T4 may be developed in parallel after T2 if they do not edit the same coordinator file concurrently.

---

## T1 — AppConfig v3 Desktop Preferences and Tauri Capability Gate

**Primary files:** protocol/src/v1/config.rs, generated schema/TS fixtures, ui/src-tauri/src/config/*, ui/src-tauri/Cargo.toml, Cargo.lock, Tauri capabilities/config.  
**AC:** M01-AC05, M01-AC14; supports AC15.  
**Depends on:** M00 accepted A15/migration contract.

### Implement

#### Rebase first

Read M00 ADR_DECISIONS.md and final v3 migration fixture/contract.

If it differs from this M01 draft, update SPEC/PLAN/TASKS before coding.

#### Production schema v3

Set production CURRENT_SCHEMA_VERSION to 3 only after M00 gate.

Add DesktopConfig host-only preferences required by SPEC:

- approval_notifications = true
- connection_notifications = true
- background_notice_acknowledged = false
- auto_start_harness_on_app_launch = false if present in the M00-frozen V4 v3 contract; do not implement auto-start behavior

Use existing Rust naming/serde camelCase conventions.

#### v2→v3 migration

Create an explicit v2 source model/migration path.

Preserve:

- configRevision
- Workspace
- Tunnel ID/proxy/client/autoDetect
- SecretReference
- Security
- UI theme/language
- Tool toggles
- Policies/user rules
- Bash

If M00 frozen v3 also includes future host/history fields, follow its defaults exactly.

Validation before atomic write.

Migration does not use normal semantic save increment.

#### Host-only revision semantics

Update same_restart_semantics or equivalent so desktop preference changes do not increment Runtime configRevision.

RuntimeConfigSnapshot must not contain DesktopConfig.

Add focused mutation API/method for desktop preferences so unrelated Settings drafts cannot overwrite them.

#### Tauri dependencies

Enable Tauri tray-icon feature.

Add official Tauri v2 notification plugin.

Select/pin a stable v2 plugin version whose desktop click/action callback is compatible with the project Rust toolchain. Record:

- rustc version
- plugin version
- Tauri version
- Windows support assumptions

If a Rust toolchain upgrade is required, include it in this Task and run full workspace compatibility checks.

Initialize notification plugin without yet emitting product notifications.

### Required Unit / Contract Tests

At minimum:

- v2 representative fixture → v3.
- v2 non-default Tool/Policy/Bash/UI values preserved.
- SecretReference preserved, raw secret absent.
- configRevision preserved by migration.
- desktop defaults correct.
- v3 load/save idempotent.
- malformed known v2/v3 field fails.
- unsupported future schema unchanged.
- migration write failure preserves original bytes.
- desktop preference change does not increment Runtime configRevision.
- RuntimeConfigSnapshot serialization contains no desktop field.
- autoStart field remains false/inert if present.
- generated TS/schema contracts updated.
- tray/notification dependency compiles for target.

### Done

- v3 is production config.
- migration is safe and atomic.
- desktop prefs are Host-only.
- no F02 behavior exists.
- Tauri desktop APIs are ready for T2/T3.

---

## T2 — DesktopLifecycleCoordinator, Close→Hide, Tray and Explicit Quit

**Primary files:** new ui/src-tauri/src/desktop/*, ui/src-tauri/src/lib.rs, existing harness.rs integration, Tauri window/tray wiring.  
**AC:** M01-AC01, AC02, AC03, AC04; supports AC15/AC16.  
**Depends on:** T1.

### Implement

#### Coordinator

Create DesktopLifecycleCoordinator (or equivalent single owner).

State includes:

- app_quitting
- tray handle/menu item handles
- latest HarnessStatus projection
- pending-count projection placeholder
- notification coordinator hooks
- one-time education handshake state

Do not copy HarnessStatus into a second independent state machine; cached projection is display-only.

#### CloseRequested

Register main-window close interception.

Normal Close:

- prevent close;
- never call harness_stop;
- if background notice acknowledged → hide;
- if not acknowledged → keep window visible and emit first-close education intent.

Implement Host commands/events for:

- acknowledge + hide
- explicit Quit from education flow

If React unavailable, fail safe without stopping/quitting.

#### Show/focus helper

One helper handles:

- show
- unminimize
- focus
- optional navigation event

Use for Tray and notifications.

#### Tray

Create one application tray icon using existing app assets/default icon.

Menu:

- Show
- Status
- Pending approvals
- Start
- Stop
- Quit

Dynamic enablement must follow lifecycle/eligibility.

Status item uses HarnessStatus only.

Pending count accepts Known/Unavailable from T3.

#### Tray Start/Stop

Call standard HarnessController path and existing SecretStore/config eligibility.

Do not bypass operation mutex.

Handle lifecycle conflict visibly/logically without spawning duplicates.

#### Explicit Quit

Implement:

- set app_quitting atomically before lifecycle cleanup;
- disable/reject new Tray Start;
- call serialized Harness Stop;
- bounded cleanup/forced termination through existing owned managers if required;
- suppress intentional disconnect notifications through T4 integration hook;
- verify no owned process before app exit.

### Required Unit Tests

Cover:

- Close acknowledged → prevent close + hide; no Stop.
- first Close → education request, no quit.
- education Continue → ack persisted + hide.
- Tray Show idempotent.
- Start/Stop dispatches existing APIs.
- Start during Starting/Ready rejected/disabled without duplicate.
- Close racing Quit → Quit wins; no hide loop.
- Quit sets app_quitting before Stop.
- Quit Stopped state exits cleanly.
- Quit Running state stops owned processes.
- status projection maps all HarnessLifecycle values.
- no process PID heuristic produces Ready.

### Done

- desktop remains alive when window hides.
- Tray is functional without React for lifecycle actions.
- explicit Quit is distinct from Close/Stop.

---

## T3 — Approval Notifications, Pending Count and Notification Routing

**Primary files:** desktop/notifications.rs, desktop/routing.rs, Runtime Tauri activity sink integration, approval query adapter, tests.  
**AC:** M01-AC06, AC07, AC08, AC09, AC10; supports AC03/AC16.  
**Depends on:** T2.

### Implement

#### Host event fan-out

Extend existing TauriProcessEventSink/Activity delivery so one safe ActivityEvent can reach:

- existing React runtime:activity emitter;
- Desktop NotificationCoordinator.

Do not remove/change existing frontend event semantics.

#### Approval refresh trigger

On:

- ApprovalRequested
- ApprovalResolved
- ApprovalDenied
- ApprovalExpired
- ApprovalCancelled
- Runtime/session change

query authoritative approvals and update pending count.

Pending count states:

- Known(n)
- Unavailable

Tray must never show stale numeric count as current after Runtime loss.

#### Bootstrap baseline

On coordinator attach/reconnect:

- query current Pending approvals;
- set Tray count;
- seed notification dedupe;
- do not notify every pre-existing Pending item.

#### Identity

Use the M00-frozen Approval identity.

If current protocol lacks required distinct approval_id, add the minimum typed safe field only if M00 says it is required.

Never join by timestamp.

#### Dedupe

Bounded in-memory dedupe by Runtime/Approval identity.

Repeated Activity events and polling refresh cannot resend.

Runtime identity change resets/separates key space.

#### Notification formatter

Create a closed formatter.

Allowed output:

- app name
- approval required
- logical Tool name
- optional bounded M00-approved safe summary

Prohibit raw metadata dump.

Add synthetic secret/path/argv/body sentinel tests.

#### Delivery capability

Implement capability/permission handling:

- Granted → attempt send.
- Denied/Unavailable → record delivery-unavailable projection and fall back.
- delivery error → no repeated loop.
- Approval state unchanged in every branch.

#### Click routing

Register desktop click/action listener.

On click:

- parse only typed safe routing data;
- show/focus app;
- re-query approvals;
- emit typed navigation intent to request_id;
- do not call resolve.

If terminal → view terminal.

If Runtime/request unavailable → non-actionable stale/unavailable path.

#### Windows callback limitation

Document/test behavior supported by chosen plugin.

Do not promise Action Center click beyond plugin/OS guarantee.

### Required Unit Tests

At minimum:

- new Pending → one notification.
- duplicate event → no second notification.
- poll same Pending → no second notification.
- bootstrap existing Pending → count only, no burst.
- two distinct approvals → two notifications.
- runtime rollover same request string → identity remains distinct/safe.
- terminal before send → do not notify stale Pending.
- notification formatter sentinel privacy.
- delivery denied/failure leaves Pending unchanged.
- count transitions 0→1→0.
- Runtime lost → count Unavailable.
- click Pending → approvals navigation + live actions.
- click Approved/Denied/Expired/Cancelled → terminal, no actions.
- click Runtime gone → stale/unavailable, never Pending.
- click callback does not call approvals.resolve.

### Done

- F03 mechanics complete at code level.
- Tray fallback is always present.
- React visibility is not required to generate notifications.

---

## T4 — Disconnect / Recovery Incident State Machine

**Primary files:** desktop/incidents.rs, desktop/notifications.rs, HarnessController status observer integration, tests.  
**AC:** M01-AC11, AC12, AC13; supports AC16.  
**Depends on:** T2.

### Implement

#### Status observation

Allow Desktop coordinator to observe HarnessStatus changes without replacing the existing UI harness:status emission.

If HarnessController currently has a single status sink, refactor to an explicit multiplexer/observer list or compose one Host sink that fans out to UI + Desktop.

HarnessController remains lifecycle owner.

#### Incident state

Implement process-local incident state with:

- active_session_id
- had_ready
- open incident ID
- start lifecycle/component snapshot
- notification_sent
- recovery_sent
- intentional_stop_or_quit guard

#### Open rules

Open once when a session that had Ready loses Ready due to Degraded/Error/Runtime/Tunnel failure.

Do not open on initial startup failure before Ready unless an accepted upstream M00/M01 contract explicitly adds it.

Do not open on user Stop/Quit.

Controlled restart leaves Ready intentionally; suppress disconnect incident during the controlled transition unless restart terminates in a true Error episode after the transition.

#### Dedupe

Repeated degraded/error/stale polls in same incident do not re-notify.

Error message text changes alone do not create new incidents.

#### Recovery

Open incident + authoritative Ready:

- close incident;
- optionally notify once if connection notifications enabled.

Stop during incident:

- close silently;
- no fake recovery.

New Ready loss later:

- new incident; notify again.

#### Notification payload

Use generic bounded state text.

No raw control-plane errors/proxy/credential values.

Navigation opens Overview/current state.

### Required Unit Tests

Use table-driven transition tests:

- Stopped→Starting→Ready = no incident.
- Ready→Degraded = one incident/notification.
- Degraded repeated N times = still one.
- Degraded→Error = still same incident.
- Error/degraded→Ready = one recovery.
- Ready→Degraded after recovery = second incident.
- Ready→Stopping→Stopped user Stop = no incident.
- Ready→Stopping→Stopped explicit Quit = no incident.
- controlled restart Ready→Stopping→Starting→Ready = no incident.
- Runtime crash after Ready = incident.
- session rollover clears old incident keys.
- notification disabled still tracks incident but sends nothing.
- notification send failure does not change lifecycle/incident source truth.

### Done

- F04 code-level behavior matches incident semantics.
- no polling spam.
- notification success/failure has zero effect on HarnessStatus.

---

## T5 — React Host Navigation, First-Close UX, Settings and i18n

**Primary files:** ui/src/App.tsx, router, HarnessDataProvider/client services, Settings page, ApprovalsPage integration, i18n resources/tests.  
**AC:** M01-AC02, AC08, AC14; supports AC07/AC09/AC16.  
**Depends on:** T3 + T4.

### Implement

#### Host navigation event

Add one typed listener at app/router root.

Accept only known page/ID forms.

Approval notification intent:

- navigate to approvals with objectId/requestId;
- refresh approvals/live Runtime before actionability.

Connection intent:

- navigate to Overview.

Unknown route payload ignored/fails closed.

#### First-close education

Render the Host-triggered education dialog/surface.

Explain:

- window Close keeps app in Tray;
- Harness can continue running;
- Stop Harness and Quit are separate;
- Tray is the explicit reopen/quit surface.

Actions:

- Continue in background → focused host command persists ack and hides.
- Quit → explicit host Quit path.

Do not invoke Stop on Continue.

#### Settings

Add Desktop / Notifications controls.

Approval notifications:

- toggle focused config mutation.

Connection notifications:

- toggle focused config mutation.

No Auto Start UI.

Show fallback explanation for Windows/system notification blocking.

#### Stale Approval route

Ensure ApprovalsPage can render selected terminal item safely after refresh.

If unavailable due Runtime gone, show non-actionable status; no synthetic pending object.

#### i18n

Add all M01 English and Simplified Chinese strings.

Host-side Tray/notification labels should use an accepted localization strategy; if Host does not have full i18n runtime, freeze minimal safe bilingual/system-language behavior rather than hardcoding inconsistent copies.

### Required Frontend Tests

- Host route → approvals/objectId.
- Host route → overview.
- unknown route ignored.
- terminal approval deep-link disabled actions.
- unavailable approval no actions.
- first-close dialog content/actions.
- Continue calls ack/hide, not Stop.
- Quit calls explicit quit.
- two notification toggles save independently.
- focused mutation preserves unrelated config.
- no restartRequired from desktop preference.
- auto-start control absent.
- en/zh-CN resource coverage.

### Done

- user-facing M01 flow is complete.
- Host authority remains intact.
- deep link does not authorize anything.

---

## T6 — Windows Packaging, Cross-Cutting Regression and Development Completion

**Primary files:** package/capabilities/build scripts as required, desktop tests, milestone development evidence.  
**AC:** M01-AC04, AC15, AC16; closes all previous ACs.  
**Depends on:** T1–T5.

### Implement / Verify

#### Package wiring

Ensure:

- tray feature is enabled in release build;
- app icon is available to Tray;
- notification plugin/capabilities are included;
- notification action/click listener is registered early enough;
- NSIS package has required app identity behavior;
- Portable behavior is detected/truthfully degraded if Windows Toast is unavailable.

Do not add undocumented global permissions beyond what plugin requires.

#### Regression

Run:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
npm run portable:windows
npm run tauri:build
~~~

If package scripts require prerequisites, record exact commands/results.

#### Static/security review

Confirm:

- notification formatter cannot serialize whole ApprovalRequest.
- no raw SecretStore value enters notifications/logs.
- no Tray/React path calls approvals.resolve except existing user approval UI.
- no second lifecycle truth.
- no new daemon/process.
- DesktopConfig absent from RuntimeConfigSnapshot.
- current Workspace/Policy/Bash/SecretStore tests still pass.

#### Cleanup

Stop package smoke instances.

Verify no owned Runtime/Tunnel orphan.

Clean temporary profile/config test data.

#### Development report

Record:

- candidate commit;
- dependency/toolchain versions;
- exact test commands/counts;
- known package limitations;
- any Portable vs NSIS difference;
- remaining items reserved for Integration/E2E/Manual.

### Done

M01 Development Complete means:

- T1–T6 complete;
- all task-local/unit/contract tests pass;
- both Windows artifacts build as required;
- no external E2E has been falsely claimed;
- M01 is ready for INTEGRATION_TEST_PLAN / E2E_TEST_PLAN / MANUAL_ACCEPTANCE_CHECKLIST execution.

Do not mark M01 ACCEPTED/FROZEN here.

---

## 2. Task → AC → Reserved Validation Traceability

| Task | M01 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC05, AC14, AC15 support | M01-INT-06/07, M01-E2E-04, M01-MAN-03 |
| T2 | AC01–AC04 | M01-INT-01/02/03/05, M01-E2E-01/04, M01-MAN-01/02 |
| T3 | AC06–AC10 | M01-INT-04/08/09/10/11, M01-E2E-02, M01-MAN-02/04 |
| T4 | AC11–AC13 | M01-INT-12/13/14/15, M01-E2E-03, M01-MAN-05 |
| T5 | AC02, AC08, AC14, AC16 support | M01-INT-10, M01-E2E-02/03, M01-MAN-01/03/04/05 |
| T6 | AC04, AC15, AC16 | M01-INT-16, M01-E2E-01/02/03/04, all Manual |

---

## 3. Development Handoff

After T6:

~~~text
M01 Development Complete
→ execute frozen Integration plan
→ execute packaged Windows E2E including real ChatGPT + ASK + disconnect
→ Human Manual Acceptance
→ ACCEPTANCE / EVIDENCE_INDEX
→ M01 ACCEPTED / FROZEN
~~~

M01 does not unlock itself merely because the code compiles.
