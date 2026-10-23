# M01 SPEC — Tray, Background Lifecycle and Notifications

> 布局与测试生命周期遵守 [AGENTS.md](../../../../AGENTS.md)，先复用现有模块及同类测试，不创建独立实验工程或新顶级目录。当前仅有 SPEC / PLAN / TASKS 三份草案，缺少 Integration / E2E / Manual 三份计划，未达到六文件冻结门槛；M00 未验收，禁止开始实现。报告更新既有证据记录，不逐次新建总结报告。

> Status: DRAFT / DESIGN READY — IMPLEMENTATION BLOCKED BY M00 ACCEPTANCE  
> Milestone: M01 — Tray, Background Lifecycle and Notifications  
> Scope: P0 F01 System Tray + F03 Approval Notifications + F04 Disconnect / Recovery Notifications  
> Depends on: M00 — As-Is Baseline and Architecture Contract Gate  
> Upstream: V4 PRD / Architecture / Roadmap + M00 implementation-frozen contract once accepted  
> Important: this document may be reviewed before M00 closes, but no implementation Task may start until M00 is ACCEPTED / FROZEN and any M00 ADR revisions have been rebased into M01.

---

## 1. Objective

M01 upgrades ChatGPT-Harness from a foreground-window application into a Windows desktop application that can remain useful while its main window is hidden.

The milestone delivers one coherent desktop-continuity system:

~~~text
authoritative HarnessController / Runtime Approval state
        ↓
Tauri DesktopLifecycleCoordinator
        ├─ Window Close → Hide / explicit Quit
        ├─ TrayController → Show / Start / Stop / state / pending count / Quit
        ├─ NotificationCoordinator
        │    ├─ Approval notifications
        │    └─ Disconnect / recovery incident notifications
        └─ NotificationRouter
             ↓
       Show / Focus main window
             ↓
       React route intent
             ↓
       refresh authoritative Runtime state
~~~

M01 must preserve the existing authority model:

- Tauri Host owns desktop lifecycle.
- HarnessController remains the only Harness lifecycle truth.
- Runtime remains the only Tool / Workspace / Policy / Approval / execution truth.
- Tunnel remains transport only.
- React displays state and navigation; it does not decide whether the Harness is Ready or whether an Approval is still Pending.
- Windows notifications and Tray are notification/control surfaces, not new state authorities.

---

## 2. Gate from M00

M01 implementation is blocked until M00 is ACCEPTED / FROZEN.

Before T1 implementation begins, rebase this SPEC against M00 outputs and confirm:

- A01/A02/A03 desktop ownership remains accepted.
- A08 identity semantics used by Approval notification correlation remain accepted.
- A15 v2→v3 config migration direction remains accepted.
- any M00 changes to Runtime/Host identity naming are reflected here.
- no M00 POC has become a production dependency that M01 accidentally relies on.

If M00 revises an upstream decision, update M01 SPEC/PLAN/TASKS before declaring M01 SPEC_READY.

---

## 3. Canonical Upstream Acceptance Criteria

### F01 — System Tray

- F01-AC01 — closing the main window hides to Tray while Harness remains live; Close is not Stop/Quit.
- F01-AC02 — Tray provides Show, Start, Stop, true connection state, Pending Approval count and Quit from authoritative lifecycle state.
- F01-AC03 — explicit Quit follows normal lifecycle cleanup and leaves no owned Runtime/Tunnel orphan.

### F03 — Approval Notifications

- F03-AC01 — every newly Pending Approval sends at most one corresponding Windows notification and Tray/Approval Center remains fallback.
- F03-AC02 — notification click focuses app and re-queries live approval state; resolved/expired requests never become Pending again.
- F03-AC03 — notification failure never loses Approval; notification content excludes secrets/raw unrestricted arguments/unprocessed file content.

### F04 — Disconnect / Recovery Notifications

- F04-AC01 — disconnect/degraded notification follows authoritative state transition and one incident episode is not spammed repeatedly.
- F04-AC02 — recovery may notify once; a later independent fault may open a new incident.
- F04-AC03 — notification success is not recovery; navigation opens the real current Overview/diagnostic state.

Relevant Architecture decisions:

- A01 — preserve React/Tauri/Runtime/Tunnel boundaries.
- A02 — Tray/notification/Close/Quit live in Tauri Host.
- A03 — use Tauri 2 TrayIconBuilder and official notification plugin; Windows click/fallback requires packaged validation.
- A15 — AppConfig V4 migration direction is v2→v3; Host-only prefs do not create Runtime restart debt.

---

## 4. Current Baseline to Preserve

Current code facts to re-check against M00:

- ui/src-tauri/src/lib.rs creates RuntimeSupervisor, TunnelProcessManager and HarnessController and emits harness:status.
- HarnessController serializes Start/Stop/Restart through its operation mutex.
- HarnessStatus already distinguishes Stopped / Starting / Ready / Degraded / Error / Stopping and component states.
- Runtime activity already reaches Tauri as safe ActivityEvent and includes ApprovalRequested / ApprovalResolved / ApprovalExpired / ApprovalCancelled event types.
- ApprovalRequest is Runtime-owned and includes request_id, runtime_id, generation_id, tool, sanitized_arguments, scope_summary, status and terminal decision.
- React ApprovalsPage already supports opening a specific request by object/request ID.
- current Tauri dependencies include dialog plugin but no notification plugin and no tray-icon feature.
- current AppConfig production schema is v2.

M01 must extend these boundaries, not duplicate them.

---

## 5. Scope

### 5.1 Included

M01 implements:

1. AppConfig v2→v3 production migration for V4 host-only desktop preferences.
2. DesktopConfig with M01-owned notification/background preferences and a dormant M07 auto-start field if required by the M00-frozen v3 contract.
3. tray-icon feature and a single application Tray icon/menu.
4. CloseRequested interception and Close→Hide behavior.
5. one-time first-close background-continuity education.
6. explicit Quit coordination and owned-process cleanup.
7. Tray Show/Focus, Start, Stop, current lifecycle state, Pending Approval count, Quit.
8. Host-level pending-approval observation independent of React visibility.
9. Approval Windows notification de-duplication.
10. privacy-bounded Approval notification copy.
11. notification click routing to the specific Approval.
12. terminal/stale re-query on click.
13. disconnect/degraded incident state machine.
14. one notification per incident plus optional once-per-recovery notification.
15. independent Approval and Connection notification preferences.
16. Windows notification capability/failure fallback to Tray and main Approval UI.
17. React host navigation event handling.
18. Settings/i18n copy required by M01.
19. packaged Windows validation hooks for later Integration/E2E/Manual plans.

### 5.2 Explicit non-goals

M01 does not implement:

- F02 automatic Harness Start on app launch; any v3 auto-start field remains false/inert until M07.
- F05 startup performance optimization.
- F06 one-click diagnostics implementation.
- F07 SQLite History.
- durable notification history.
- notification-driven remote approval without opening the application.
- approving directly from a Windows toast.
- restoring a Pending Approval from notification payload.
- a second Harness lifecycle state machine.
- a second Policy/Approval evaluator.
- Windows login/boot autostart.
- custom background daemon/service.
- model/tool Evaluation.
- grep or new Runtime Tools.
- full Action Center historical-click reliability guarantees that the OS/plugin cannot provide.

---

## 6. AppConfig v3 Desktop Contract

M01 is the first production V4 milestone that requires host-only persisted desktop preferences. It therefore consumes M00’s v2→v3 compatibility contract and performs the production schema bump to v3, unless M00 explicitly assigns that migration to another accepted contract.

Required M01 desktop semantics:

~~~text
AppConfig v3
├ existing v2 fields unchanged
└ desktop
   ├ approvalNotifications: true
   ├ connectionNotifications: true
   ├ backgroundNoticeAcknowledged: false
   └ autoStartHarnessOnAppLaunch: false   # persisted but behavior remains M07-only
~~~

If the M00-frozen candidate v3 fixture also includes future History fields, M01 migration must preserve/default them exactly as frozen but must not activate F07 behavior.

### 6.1 Host-only semantics

desktop fields:

- never enter RuntimeConfigSnapshot;
- never increment Runtime-affecting configRevision by themselves;
- never require Runtime restart;
- are owned by Tauri/UI presentation.
- cannot change Policy/Approval/Tool behavior.

### 6.2 Migration rules

v2→v3 must:

- preserve Workspace/Tunnel/Proxy/Security/Tools/Policy/Bash/UI/SecretReference;
- preserve configRevision;
- validate before atomic replacement;
- never copy raw SecretStore value into config;
- reject malformed known fields;
- leave unsupported/newer schema untouched;
- be idempotent on subsequent v3 loads.

M01 may add focused mutation helpers such as desktop_preferences_set so toggling notification settings cannot accidentally save an unrelated Settings draft.

### 6.3 First-close acknowledgement

backgroundNoticeAcknowledged records that the user has seen the one-time Close→Tray explanation.

It is presentation state only.

Resetting application config may reset this acknowledgement; that is acceptable if documented.

---

## 7. Desktop Lifecycle Contract

### 7.1 DesktopLifecycleCoordinator

Introduce one Tauri Host coordinator under:

~~~text
ui/src-tauri/src/desktop/
~~~

Recommended internal modules:

~~~text
desktop/
├ mod.rs
├ lifecycle.rs
├ tray.rs
├ notifications.rs
├ routing.rs
└ incidents.rs
~~~

Names may change during implementation if ownership remains identical.

The coordinator owns:

- app_quitting state;
- main-window visibility intent;
- first-close education flow;
- Tray handles/menu projection;
- notification delivery capability;
- Approval notification dedupe;
- connection incident state;
- Host→React navigation intents.

It does not own Harness Ready or Approval truth.

### 7.2 CloseRequested

When the main window receives a normal user CloseRequested:

If app_quitting = false:

- prevent actual process exit;
- do not stop Harness;
- do not stop Runtime/Tunnel;
- if backgroundNoticeAcknowledged = true, hide the main window immediately;
- if acknowledgement is false, keep the window alive and show the one-time background-continuity explanation;
- the explanation must make explicit:
  - Close hides to Tray;
  - Stop Harness is separate;
  - Quit is available from Tray;
  - Harness may continue serving ChatGPT while hidden.

After user acknowledges Continue in background:

- persist acknowledgement through a focused Host-only mutation;
- hide the window.

If the education UI cannot be rendered, fail safe by not quitting or stopping Harness. A fallback hide is acceptable only if the acknowledgement remains false so education can be shown later.

### 7.3 Show / Focus

Tray Show, tray-icon activation and notification routing use one idempotent Host helper:

~~~text
show_main_window()
→ unminimize if needed
→ show
→ set_focus
→ emit optional navigation intent
~~~

Repeated calls are safe.

### 7.4 Explicit Quit

Explicit Quit is allowed only from a deliberate user action such as Tray Quit or a future in-app Quit command.

Flow:

~~~text
set app_quitting = true
→ suppress new desktop Start actions
→ enter existing HarnessController serialized stop path
→ wait for bounded graceful cleanup
→ verify/force cleanup of owned Tunnel/Runtime through existing managers if necessary
→ destroy tray/notification resources
→ exit application
~~~

Requirements:

- no new lifecycle mutex separate from HarnessController operation serialization;
- no tray Start after app_quitting begins;
- no disconnect/recovery notifications caused by intentional Quit;
- no owned Runtime/Tunnel orphan remains.

OS kill/crash is not advertised as graceful Quit.

---

## 8. Tray Contract

### 8.1 Required menu

Tray menu contains, at minimum:

- Show ChatGPT-Harness
- Status: <authoritative lifecycle> — disabled informational item
- Pending approvals: <N or unavailable>
- Start Harness
- Stop Harness
- separator
- Quit

Pending approvals item may open Approvals.

### 8.2 Authoritative status

Tray lifecycle text is derived only from current HarnessStatus:

- Stopped
- Starting
- Ready
- Degraded
- Error
- Stopping

Do not infer Ready from:

- window visible/hidden;
- process PID exists;
- Tunnel process exists;
- last cached Ready after freshness loss.

### 8.3 Start / Stop

Tray Start and Stop call the same HarnessController APIs used by the main UI.

Start:

- checks standard start eligibility and SecretStore/config path;
- never bypasses setup/credential/security checks;
- is disabled or rejects cleanly during conflicting lifecycle transitions.

Stop:

- uses existing serialized stop;
- never means Quit;
- leaves desktop app and Tray alive.

### 8.4 Pending Approval count

The count must be refreshed from Runtime approval truth, not incremented/decremented only from notification events.

State:

~~~text
Known(N)
Unavailable
~~~

Rules:

- trigger a refresh on ApprovalRequested/Resolved/Denied/Expired/Cancelled;
- refresh on Runtime session/generation change;
- on Runtime unavailable, display unavailable rather than a stale numeric count;
- a stale cached count cannot be used to authorize or reject anything.

---

## 9. Approval Notification Contract

### 9.1 Source

Host consumes safe Runtime/Tauri Activity events even when the main React window is hidden.

On ApprovalRequested:

1. capture request/runtime identity from the event;
2. query the Runtime approval list/state;
3. locate the matching currently Pending request;
4. compute the M00-frozen approval identity;
5. apply dedupe;
6. if notifications are enabled and OS delivery is available, send one Windows notification;
7. refresh Tray pending count regardless of notification delivery result.

Do not notify solely because a polling loop repeatedly sees Pending.

### 9.2 Identity / de-duplication

Use the identity frozen by M00.

Preferred current mapping:

~~~text
(runtime_id, approval_id)
~~~

If the current wire only exposes request_id and M00 proves Approval is one-to-one with request, the implementation may use:

~~~text
(runtime_id, request_id)
~~~

and record that mapping in the contract/tests.

If M00 proves approval_id is distinct and required, add the minimum typed protocol field rather than guessing.

Dedupe memory must be bounded.

Existing Pending items discovered at Host subscription/bootstrap should establish baseline state without firing a burst of “new” notifications.

### 9.3 Privacy-bounded content

Notification title/body may contain only:

- application identity;
- “Approval required”;
- logical Tool name;
- optionally a short bounded safe scope summary approved by the M00 evidence allowlist.

Do not include:

- raw argv;
- raw Bash command;
- raw file content;
- unrestricted sanitizedArguments serialization;
- secret-like values;
- physical path;
- raw error details.

Default body should prefer less information:

~~~text
Approval required for <tool>. Open ChatGPT-Harness to review.
~~~

### 9.4 Delivery failure

Notification delivery result is observational.

If notification is:

- denied by OS permission;
- unavailable in the current package type;
- rejected by plugin;
- click callback unavailable;

then:

- Approval remains Pending in Runtime;
- Tray pending count remains authoritative;
- main Approvals page remains usable;
- failure never resolves/cancels/recreates the request;
- do not retry in a tight loop.

### 9.5 Click routing

On supported notification click:

1. Show/focus main window.
2. Re-query Runtime approval state.
3. Emit Host→React navigation intent:
   - page = approvals
   - objectId = request_id
4. Approvals page opens that item if it exists.
5. Actions are enabled only from the re-queried live state.

Outcomes:

- Pending → normal approval actions.
- Approved/Denied/Expired/Cancelled → show true terminal state, no actions.
- Runtime gone / request unavailable → show non-actionable unavailable/stale state; never fabricate Pending.

M01 has no durable History, so a Runtime-gone request cannot be reconstructed from notification payload. M03 may later improve historical display.

---

## 10. Windows Notification Technical Boundary

Implementation uses the official Tauri notification plugin and Tauri 2 Tray APIs.

Dependency gate in T1 must verify the project Rust toolchain and select a stable Tauri v2 notification plugin version compatible with it.

For notification-click routing, use a stable v2 plugin version that supports desktop action/click callbacks. Desktop callback behavior is platform-limited: Windows may not report a click after a toast has moved to Action Center. Therefore:

- live toast click deep-link is required where callback is delivered;
- Action Center historical click is not assumed reliable;
- Tray + Approvals page is permanent fallback;
- packaged Windows behavior must be tested, not inferred from dev mode.

Tauri Tray requires the tray-icon Cargo feature.

No M01 acceptance may rely only on browser/jsdom behavior for Tray/Toast.

---

## 11. Disconnect / Recovery Incident Contract

### 11.1 Source of truth

Connection notifications observe HarnessController status transitions.

They do not derive connection state from notification delivery or React state.

### 11.2 Incident episode

An incident is scoped to one active Harness session.

Open an incident when a session that has reached Ready later enters an important non-ready condition such as:

- Ready → Degraded;
- Ready/Degraded → Error;
- Runtime crash causing Harness loss of Ready;
- Tunnel stale/disconnected/failure causing Harness loss of Ready.

Do not open a disconnect incident for:

- initial Stopped;
- normal Starting before first Ready;
- user-requested Stop;
- explicit Quit;
- planned restart transition where the product intentionally leaves Ready and the restart is under direct lifecycle control, unless that restart fails and becomes a real error episode.

### 11.3 De-duplication

For one open incident:

- send at most one disconnect/failure notification;
- status polling or repeated error messages do not create duplicates;
- severity escalation may update internal incident evidence but does not spam another toast unless a later SPEC explicitly defines escalation notification.

Incident key includes at least active Harness session identity plus a local incident sequence/UUID.

### 11.4 Recovery

When an open incident returns to authoritative Ready:

- close the incident;
- if connection notifications are enabled, send at most one recovery notification;
- recovery notification delivery does not cause or prove Ready — Ready existed first.

If the user Stops Harness during an incident:

- close incident as user-stopped;
- do not send recovery.

A later independent loss of Ready after recovery creates a new incident and may notify again.

### 11.5 Navigation

Disconnect notification opens/focuses the app at current Overview.

If M02 Diagnostics is available in a later integrated candidate, routing may include Diagnostics. M01 itself must not depend on M02.

The UI must display the current HarnessStatus after navigation, not the notification’s historical cached status.

---

## 12. Settings and UI Contract

Add a Desktop / Notifications settings area with:

- Approval notifications toggle — default ON;
- Connection notifications toggle — default ON;
- clear explanation that Windows/system settings can still block delivery;
- no M07 auto-start toggle yet.

Toggling either setting:

- persists host-only config;
- does not change configRevision for Runtime;
- does not require Harness restart;
- takes effect for subsequent notification decisions.

### Host navigation event

React root registers one listener for typed Host navigation intents.

Supported M01 intents:

~~~text
{ page: "overview" }
{ page: "approvals", objectId: "<request-id>" }
~~~

The router remains presentation-only.

### i18n

All new visible strings require English and zh-CN resources:

- Tray labels/status text where localized by Host if applicable;
- first-close education;
- notification titles/bodies;
- settings labels/help;
- notification unavailable/fallback copy;
- stale/unavailable Approval routing copy if new.

Machine state remains typed; translation happens only at presentation boundary.

---

## 13. Derived Acceptance Criteria

| M01 AC | Requirement |
| --- | --- |
| M01-AC01 | Normal main-window Close never stops/quits Harness; after acknowledgement it hides the window and Runtime/Tunnel remain governed by existing lifecycle. |
| M01-AC02 | First Close before acknowledgement clearly explains background continuity and the separate Stop/Quit actions; acknowledgement is persisted host-only. |
| M01-AC03 | Tray contains Show, authoritative lifecycle state, Start, Stop, Pending Approval count/state and Quit; all actions call existing authoritative Host/Runtime paths. |
| M01-AC04 | Explicit Quit is serialized with existing lifecycle operations and leaves no owned Runtime/Tunnel orphan; intentional Stop/Quit does not generate disconnect spam. |
| M01-AC05 | AppConfig v2→v3 migration preserves all existing settings/SecretReference/configRevision; M01 desktop preferences are host-only and create no Runtime restart debt. |
| M01-AC06 | Newly observed Pending Approval produces at most one notification per M00-frozen approval identity; bootstrap/polling does not resend duplicates. |
| M01-AC07 | Approval notification content is bounded and contains no raw secret/argv/file body/physical path/unrestricted argument payload. |
| M01-AC08 | Approval notification click shows/focuses the app, re-queries Runtime truth and routes to the matching Approval; terminal/unavailable requests are never reactivated. |
| M01-AC09 | OS notification denied/unavailable/failure leaves Approval untouched and Tray/Approvals UI usable as fallback. |
| M01-AC10 | Tray pending count is refreshed from Runtime approvals and explicitly shows unavailable rather than stale numeric truth when Runtime cannot answer. |
| M01-AC11 | One Ready-loss incident sends at most one disconnect notification despite repeated polls/status changes. |
| M01-AC12 | Returning from an open incident to authoritative Ready sends at most one recovery notification; a later independent incident may notify again. |
| M01-AC13 | User Stop/Quit/controlled restart transitions do not masquerade as connection incidents; notification delivery never changes Harness lifecycle. |
| M01-AC14 | Approval and connection notification preferences are independently persisted, default ON, take effect without Runtime restart, and do not expose M07 auto-start behavior. |
| M01-AC15 | Packaged Windows validation proves Tray/background continuity and notification fallback; platform/plugin click limitations are surfaced rather than hidden. |
| M01-AC16 | M01 introduces no new Policy/Approval/Ready authority in React, Tray or Notification code and preserves current Workspace/Policy/Approval/SecretStore behavior. |

---

## 14. Traceability

Reserved validation IDs define the handoff for later M01 test documents.

| Upstream | M01 AC | Task | Integration | E2E | Manual |
| --- | --- | --- | --- | --- | --- |
| F01-AC01 | AC01, AC02 | T2, T5 | M01-INT-01/02 | M01-E2E-01 | M01-MAN-01 |
| F01-AC02 | AC03, AC10 | T2, T3 | M01-INT-03/04 | M01-E2E-01 | M01-MAN-02 |
| F01-AC03 | AC04 | T2, T6 | M01-INT-05 | M01-E2E-04 | M01-MAN-02 |
| A15 | AC05, AC14 | T1, T5 | M01-INT-06/07 | M01-E2E-04 | M01-MAN-03 |
| F03-AC01 | AC06, AC09, AC10 | T3 | M01-INT-08/09 | M01-E2E-02 | M01-MAN-04 |
| F03-AC02 | AC08 | T3, T5 | M01-INT-10 | M01-E2E-02 | M01-MAN-04 |
| F03-AC03 | AC07, AC09 | T3 | M01-INT-09/11 | M01-E2E-02 | M01-MAN-04 |
| F04-AC01 | AC11, AC13 | T4 | M01-INT-12/13 | M01-E2E-03 | M01-MAN-05 |
| F04-AC02 | AC12 | T4 | M01-INT-14 | M01-E2E-03 | M01-MAN-05 |
| F04-AC03 | AC13 | T4, T5 | M01-INT-15 | M01-E2E-03 | M01-MAN-05 |
| A02/A03 | AC15, AC16 | T2–T6 | M01-INT-16 | M01-E2E-01/02/03/04 | M01-MAN-01/02/04/05 |

---

## 15. Technical Constraints

1. Tauri Tray feature and official notification plugin are Host dependencies only.
2. Pin actual versions in Cargo.lock; do not float an unverified plugin behavior.
3. If desktop click callback support requires a Rust toolchain update, that update must be explicit in T1 and verified before feature implementation.
4. Notification payload/data must use typed bounded values and stable logical IDs only.
5. No notification event may directly call approvals.resolve.
6. Pending count is a query projection, not an event counter authority.
7. Tray menu updates must tolerate Runtime/Tunnel status updates from non-UI threads safely.
8. app_quitting must be process-local Host state, not persisted config.
9. All Quit cleanup uses existing process ownership managers.
10. Portable/NSIS differences must be reported truthfully. A package mode with unavailable Toast must still provide Tray/Approval fallback and cannot be called “notification PASS” unless the required M01 package acceptance target supports it.

---

## 16. Exit Gate

M01 may be marked ACCEPTED / FROZEN only when:

- M00 is already ACCEPTED / FROZEN;
- M01-AC01 through M01-AC16 pass;
- v2→v3 migration and Host-only preference semantics pass;
- Tray/background lifecycle works in a packaged Windows build;
- real ChatGPT→Tunnel→Runtime tool use continues while main window is hidden;
- real ASK produces the required Windows notification in the supported acceptance package and fallback works when notification is unavailable;
- notification click re-queries true Approval state;
- disconnect/recovery incidents do not spam;
- explicit Quit leaves no owned orphan;
- Integration/E2E/Manual acceptance plans have been executed;
- no new authority/security regression exists.

M01 cannot be accepted on browser-only tests, dev-window smoke tests, or notification mocks.
