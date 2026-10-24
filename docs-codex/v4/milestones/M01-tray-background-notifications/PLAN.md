# M01 PLAN — Tray, Background Lifecycle and Notifications

> 布局与测试生命周期遵守 [AGENTS.md](../../../../AGENTS.md)，先复用现有模块及同类测试，不创建独立实验工程或新顶级目录。M01 六份里程碑文档现已齐全；三份验证/验收计划已在实现前冻结。M00 已按当前仓库正式文档 ACCEPTED / FROZEN；M01 已完成针对 M00 的契约 rebase。报告更新既有证据记录，不逐次新建总结报告。

> Input: SPEC.md  
> Status: SPEC_READY / IMPLEMENTATION UNLOCKED AFTER M00 ACCEPTED / FROZEN REBASE
> Goal: implement F01/F03/F04 as one Tauri-owned desktop continuity subsystem without duplicating Harness or Runtime authority.  
> Rule: M00 acceptance and the M01 contract rebase are complete; implementation follows the frozen T1–T6 order.

---

## 1. Strategy

Implementation order:

~~~text
T1 Config v3 + dependency/capability gate
        ↓
T2 DesktopLifecycleCoordinator + Tray + Close/Quit
        ├───────────────┐
        ↓               ↓
T3 Approval            T4 Connection incident
notifications          notifications
        └───────┬───────┘
                ↓
T5 React navigation + Settings + i18n
                ↓
T6 Packaging / regression / development completion
~~~

Principles:

- Host first, React second.
- Reuse HarnessController and Runtime approval APIs; do not add duplicate state.
- Events may trigger refresh, but query result is truth.
- Notification delivery failure is never a security/lifecycle transition.
- Package-level Windows behavior is part of the design, not an afterthought.
- Config migration happens once and host-only preferences do not create Runtime restart debt.

---

## 2. Phase 0 — M00 Rebase Gate

Before coding:

1. read M00 架构决策及阻塞项（在既有 SPEC/PLAN 中维护）;
2. confirm A02/A03/A15 accepted;
3. import the M00-frozen v3 migration fixture/identity rules;
4. update this milestone if M00 changed IDs or config ownership;
5. mark M01 SPEC_READY only after this reconciliation.

The current M00 documents record ACCEPTED / FROZEN. If an implementation check discovers a contradiction with that contract, stop the affected Task and reconcile the contract before continuing.

---

## 3. Phase A — Config v3 and Tauri Capability Gate

### Config

Update protocol/config and ConfigStore to production schema v3.

Add DesktopConfig host-only fields required by SPEC.

Migration:

~~~text
read v2 raw JSON
→ dedicated v2→v3 migrator
→ preserve existing fields
→ apply desktop defaults
→ validate
→ atomic persist
~~~

Do not route migration through normal save revision increment.

Ensure focused host-only mutations preserve all other config data.

### Dependencies

Update Tauri host dependencies:

- enable Tauri tray-icon feature;
- add official Tauri v2 notification plugin;
- select exact stable plugin version compatible with project Rust toolchain;
- if click-routing requires plugin/toolchain upgrade, record and test it explicitly.

Initialize plugin/coordinator in lib.rs.

Exit:

- v2/v3 config tests pass;
- CURRENT_SCHEMA_VERSION = 3;
- RuntimeConfigSnapshot unchanged by desktop prefs;
- notification/tray APIs compile in Windows target.

---

## 4. Phase B — Desktop Lifecycle and Tray

Create ui/src-tauri/src/desktop/*.

### Lifecycle state

DesktopLifecycleCoordinator owns:

- app_quitting atomic/mutex state;
- first-close education state derived from config;
- window show/hide helper;
- Tray controller;
- notification coordinator/router;
- incident/dedupe state.

It receives authoritative status callbacks but does not own HarnessStatus.

### Close interception

Register main window close handler.

Implement:

- normal Close → prevent close;
- first-close education path;
- acknowledged Close → hide;
- no Harness Stop call.

### Tray

Create one TrayIcon with app icon and menu.

Wire:

- Show → show/focus helper;
- Start → standard Harness start;
- Stop → standard Harness stop;
- Pending approvals → show/focus + approvals route;
- Quit → explicit quit coordinator.

Create tray projection update on:

- HarnessStatus change;
- approval count refresh;
- config notification preference change if reflected in menu/tooltip.

### Quit

Add bounded explicit Quit path.

Set app_quitting before stop.

Reuse HarnessController operation mutex via existing stop API and existing owned-process managers.

Verify shutdown result before app exit.

Exit:

- pure Host unit tests cover Close/Show/Start/Stop/Quit state transitions;
- no React dependency is required for Tray Start/Stop/Quit.

---

## 5. Phase C — Approval Notifications

### Event ingress

Extend the existing Tauri Runtime activity sink so safe ActivityEvent is delivered to:

- current React event emitter;
- Desktop NotificationCoordinator.

Do not make React the notification source.

### Refresh-on-event

On approval lifecycle events:

- refresh Runtime approvals;
- update Tray count.

On ApprovalRequested:

- find matching Pending request;
- compute accepted identity;
- dedupe;
- send only if preference enabled and notification capability is available.

On terminal event:

- refresh count;
- retain bounded dedupe key until Runtime/session reset so stale poll cannot re-notify.

### Bootstrap

When coordinator attaches to an already-running Runtime:

- load current Pending list;
- show correct count;
- seed dedupe baseline;
- do not send a notification burst for pre-existing Pending items.

### Privacy

Build notification copy from a dedicated typed formatter, not arbitrary metadata JSON.

Add sentinel tests.

### Routing

Register notification click/action callback.

Use NotificationRouter to:

- show/focus;
- re-query request;
- emit typed navigation intent;
- never resolve/approve.

Exit:

- duplicate event/poll tests pass;
- resolved/expired/stale click tests pass;
- delivery failure tests leave Runtime state unchanged.

---

## 6. Phase D — Connection Incident Notifications

Attach NotificationCoordinator to HarnessController status sink/multiplexer.

If current HarnessController supports only one status sink, refactor it into a bounded/multiplexed observer mechanism without changing lifecycle authority.

### Incident state

Maintain process-local:

~~~text
session_id
had_ready
open_incident?
last_closed_incident?
~~~

Transitions:

- session reaches Ready → had_ready=true.
- later Ready→Degraded/Error → open incident + one notification.
- repeated degraded/error updates → no new notification.
- open incident→Ready → close + optional one recovery notification.
- open incident→user Stop/Quit → close silently.
- next Ready loss after recovery → new incident.

Use lifecycle/component transitions, not error-string changes, as primary semantics.

Exit:

- transition table tests cover repeated poll, escalation, recovery, new incident, user Stop/Quit and session rollover.

---

## 7. Phase E — React Navigation, Settings and First-Close UX

### Host navigation listener

Add a single root-level listener that translates typed Host navigation intents into existing router calls.

Approval intent:

~~~text
navigate("approvals", { objectId: requestId })
~~~

Overview intent:

~~~text
navigate("overview")
~~~

On Approval navigation, force/trigger live approval refresh before enabling actions.

### First-close education

Implement user-visible explanation and Host decision command/event handshake.

Options:

- Continue running in Tray;
- Quit application.

After Continue:

- set backgroundNoticeAcknowledged=true through focused mutation;
- Host hides window.

The UI must not call Harness Stop just because window is closing.

### Settings

Add independent toggles:

- Approval notifications
- Connection notifications

No Auto Start toggle in M01.

Focused mutation semantics avoid overwriting unrelated Settings drafts.

### i18n

Add en / zh-CN resources and presentation mapping.

Exit:

- React tests cover navigation, terminal/stale request view, settings toggles and education flow.

---

## 8. Phase F — Packaging and Development Completion

### Package verification preparation

Update Windows packaging/config/capabilities required by:

- Tray;
- notification plugin;
- icon resources;
- notification permissions/callback registration.

Treat NSIS installed package as the normative Toast/deep-link acceptance target unless implementation proves equivalent Portable support.

Portable must at minimum:

- keep Tray/background lifecycle safe;
- expose notification-unavailable fallback truthfully if Toast is unsupported;
- never lose Approval.

### Regression

Run affected Rust/frontend/package tests.

Verify:

- existing Harness Start/Stop/Restart unchanged;
- existing Approval resolution unchanged;
- current Activity UI still receives events;
- SecretStore behavior unchanged;
- no window-close regression in setup/recovery dialogs;
- no duplicate runtime/tunnel processes from Tray actions.

### Development complete

Update the existing milestone evidence index with development results required for later validation. Keep plans in this directory; do not create per-run reports or a separate verification project. Reuse module-owned tests and system temporary directories with cleanup as required by AGENTS.md.

Do not mark M01 accepted.

---

## 9. Module Ownership

| Area | Primary owner |
| --- | --- |
| AppConfig v3 / migration | protocol/src/v1/config.rs + ui/src-tauri/src/config/* |
| desktop coordinator | ui/src-tauri/src/desktop/* |
| Tray creation/menu | ui/src-tauri/src/desktop/tray.rs |
| Close/Quit | desktop/lifecycle.rs + existing harness.rs |
| Approval notification ingress | existing runtime Tauri event sink + desktop/notifications.rs |
| connection incidents | desktop/incidents.rs + HarnessStatus observer |
| notification click routing | desktop/routing.rs |
| React host navigation | ui/src/app/router / app root |
| Settings toggles | existing Settings + HarnessDataProvider/client |
| i18n | existing en/zh-CN resources |
| package config | ui/src-tauri capabilities/config + Cargo/package scripts |

No other module should become lifecycle or authorization owner.

---

## 10. Concurrency Plan

Potential races:

- window Close while Tray Quit starts;
- Tray Start while UI Start is already in progress;
- Tray Stop while Runtime/Tunnel recovery event arrives;
- ApprovalRequested followed immediately by ApprovalResolved before notification formatting;
- click notification after request expires;
- Ready→Degraded→Ready transitions arriving while a notification is being delivered;
- config toggle changed while event is being processed.

Rules:

- lifecycle action serialization remains HarnessController operation mutex.
- app_quitting blocks new desktop lifecycle actions.
- notification path re-queries before actionability.
- incident mutation is protected by one Host lock/serialized coordinator path.
- notification send occurs after state decision; send success/failure cannot mutate source state.
- no coordinator lock is held across long OS notification calls or Harness Stop if avoidable.
- generation/session mismatch discards stale event.

---

## 11. Rollback

If M01 needs rollback:

- v3 config must remain readable or have a documented safe downgrade strategy only if explicitly implemented; do not silently write v2 over v3.
- disabling notification plugin must not break Harness.
- Tray subsystem can be removed without changing Runtime.
- Close interception removal returns foreground behavior but must be explicit.
- no persistent authorization state exists in M01, so notification data requires no migration rollback.

Migration failure always preserves original v2 bytes.

---

## 12. Development Verification

Task-local/unit checks should culminate in:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Add focused Windows/Tauri host tests for desktop modules.

Package smoke preparation:

~~~text
npm run portable:windows
npm run tauri:build
~~~

Exact package execution and real Windows Toast/real ChatGPT behavior belong to M01 E2E/Manual plans.

---

## 13. Development Completion Gate

Before Integration:

- T1–T6 complete.
- M00 remains ACCEPTED/FROZEN.
- AppConfig v3 migration tests pass.
- no Runtime restart debt for desktop prefs.
- Tray/Close/Quit unit/component tests pass.
- Approval dedupe/privacy/router tests pass.
- incident transition tests pass.
- React navigation/settings/i18n tests pass.
- production workspace regression passes.
- NSIS/Portable build succeeds as required.
- no real external ChatGPT test is falsely marked complete.
- no notification mock is counted as packaged acceptance.
