# M01 Integration Test Plan — Tray, Background Lifecycle and Notifications

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M01 — Tray, Background Lifecycle and Notifications  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M01 Development Complete  
> Scope: F01 System Tray, F03 Approval Notifications, F04 Disconnect / Recovery Notifications, AppConfig v3 desktop migration, packaged Tauri desktop integration  
> Gate: execution requires M00 ACCEPTED / FROZEN and M01 Development Complete. If M00 changes an upstream contract, this plan must be reconciled before execution.  
> Storage rule: this milestone directory remains exactly six planning/acceptance files; execution logs and temporary artifacts are not added here.

---

## 1. Objective

Integration verifies that the M01 implementation works across real Host / Runtime / UI boundaries without relying on browser-only mocks or a second source of lifecycle/approval truth.

The integration boundary is:

~~~text
AppConfig v2/v3 + ConfigStore
        ↓
Tauri Host / DesktopLifecycleCoordinator
        ├─ Close / Hide / Show / Quit
        ├─ Tray menu
        ├─ Notification coordinator
        └─ Host navigation routing
        ↓
HarnessController authoritative lifecycle
        ↓
Runtime authoritative approvals / activity
        ↓
React presentation and existing Approvals UI
~~~

Integration must prove:

- Close is not Stop or Quit.
- Tray Start/Stop uses the same HarnessController path as the main UI.
- Pending Approval count is queried from Runtime truth, not maintained as an event-only counter.
- Approval notification dedupe/failure/click routing cannot resolve or recreate an Approval.
- Disconnect notification dedupe is driven by Harness lifecycle transitions.
- AppConfig v2→v3 preserves existing data and desktop preferences remain Host-only.
- package-level Tauri wiring for Tray and notifications exists without creating a new daemon or authority.

Integration does not replace real ChatGPT → Tunnel E2E or human Windows notification observation.

---

## 2. Entry Criteria

Before Integration execution:

- M00 is ACCEPTED / FROZEN.
- M01 T1–T6 are Development Complete.
- production CURRENT_SCHEMA_VERSION is 3.
- task-local Rust and frontend tests are PASS.
- Portable and NSIS package builds required by TASKS succeed.
- exact candidate commit is fixed.
- no unresolved implementation blocker exists.
- disposable test config / Workspace uses a unique system temporary directory and does not overwrite the real user profile.

Minimum preflight:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

If M01 implementation prerequisites are missing:

~~~text
M01 Integration = BLOCKED_BY_IMPLEMENTATION
~~~

---

## 3. Evidence Header

Every Integration execution must record in the conversation or existing accepted evidence channel:

~~~text
Milestone: M01
Candidate commit:
Branch:
Dirty state:
Windows version:
Rust toolchain:
Tauri version:
Notification plugin version:
App version:
Runtime version/path:
Tunnel version/path:
CURRENT_SCHEMA_VERSION:
Package under test:
Temporary config root:
Temporary Workspace:
Execution time:
QA owner:
~~~

Each case records:

- Case ID
- M01 AC
- exact command / test entrypoint
- expected behavior
- actual behavior
- PASS / FAIL / BLOCKED / INCONCLUSIVE
- cleanup result

No screenshot alone is sufficient Integration evidence.

---

## 4. Required Integration Cases

### M01-INT-01 — Close → Hide preserves managed Harness lifecycle

**AC:** M01-AC01  
**Tasks:** T2

Using the real Tauri Host integration surface:

1. establish Harness Ready using disposable config;
2. trigger normal main-window CloseRequested with background acknowledgement already true;
3. verify the close event is prevented and the main window becomes hidden;
4. verify HarnessController remains Ready and no harness_stop path is invoked;
5. verify Runtime/Tunnel owned processes remain the same managed generation/instance;
6. show the window again and verify the same active Harness session remains authoritative.

**PASS:** normal Close hides only; it does not stop/quit/recreate the Harness.

---

### M01-INT-02 — First-close education is one-time Host presentation state

**AC:** M01-AC02  
**Tasks:** T2/T5

With a migrated/fresh v3 config where background notice acknowledgement is false:

- first Close requests the education UI instead of quitting;
- Continue-in-background persists only the acknowledgement and hides the window;
- Quit chooses the explicit Quit path;
- acknowledgement mutation does not increment Runtime-affecting configRevision;
- a later Close hides directly without repeating the education;
- reset/new disposable config may show the education again.

**PASS:** the one-time education changes only Host presentation state and cannot stop Harness implicitly.

---

### M01-INT-03 — Tray lifecycle projection and Show/Start/Stop use authoritative Host APIs

**AC:** M01-AC03  
**Tasks:** T2

Validate the Tray menu across Stopped / Starting / Ready / Degraded / Error / Stopping.

Assertions:

- Status label derives from HarnessStatus.
- Show uses the shared show/focus helper.
- Start invokes standard Harness start eligibility and serialized HarnessController start.
- Stop invokes standard serialized HarnessController stop.
- conflicting Start/Stop actions are disabled or rejected without spawning duplicate Runtime/Tunnel processes.
- process existence alone never renders Ready.
- Stop leaves the Tauri application and Tray alive.

**PASS:** Tray is a control/projection surface, not a parallel lifecycle implementation.

---

### M01-INT-04 — Tray Pending Approval count is a Runtime query projection

**AC:** M01-AC03, M01-AC10  
**Task:** T3

Exercise real Runtime Approval states:

- zero Pending → Known(0);
- one new Pending → Known(1);
- multiple Pending → exact count;
- resolve/deny/expire/cancel → count refreshes;
- Runtime unavailable/session rollover → Unavailable rather than stale numeric count;
- Runtime reconnect/bootstrap → query current Pending list and restore current count.

Deliberately duplicate lifecycle Activity events.

**PASS:** duplicate events cannot inflate/deflate count because count is re-queried from Runtime truth.

---

### M01-INT-05 — Explicit Quit is serialized and leaves no owned orphan

**AC:** M01-AC04  
**Tasks:** T2/T6

Cover:

- Quit while Stopped;
- Quit while Ready;
- Quit racing a normal Close;
- Quit while a lifecycle operation is already changing state.

Verify:

- app_quitting is set before new desktop Start can be accepted;
- existing HarnessController serialization is used;
- owned Tunnel/Runtime are stopped/cleaned;
- Tauri exits only after the bounded cleanup path completes;
- no owned child process remains;
- intentional Quit does not open a disconnect incident or send a recovery notification.

**PASS:** Quit is explicit, bounded and distinct from Close/Stop.

---

### M01-INT-06 — AppConfig v2→v3 migration preserves current product state

**AC:** M01-AC05  
**Task:** T1

Run source-controlled or module-owned migration tests using representative v2 configs.

Preserve:

- configRevision;
- Workspace;
- Tunnel ID;
- proxy;
- tunnel-client selection / autoDetect;
- SecretReference;
- Security mode;
- UI theme/language;
- Tool enable/disable;
- Policy defaults/user rules;
- Bash settings.

Verify:

- desktop defaults are added;
- migration validates before atomic write;
- malformed known field fails;
- unsupported future schema remains untouched;
- raw secret never appears in config;
- repeated v3 load is idempotent.

**PASS:** existing user semantics survive migration and original data is not silently reset.

---

### M01-INT-07 — Desktop preferences are Host-only and create no Runtime restart debt

**AC:** M01-AC05, M01-AC14  
**Tasks:** T1/T5

Toggle:

- approvalNotifications;
- connectionNotifications;
- background acknowledgement through its focused path.

Verify:

- changes persist;
- RuntimeConfigSnapshot does not contain DesktopConfig;
- Runtime-affecting configRevision does not change solely because of these preferences;
- no Restart Required state appears because of notification preference changes;
- active Runtime generation remains valid;
- autoStartHarnessOnAppLaunch, if present in v3 schema, remains inert and no M01 UI exposes it.

**PASS:** desktop preferences remain Host-only.

---

### M01-INT-08 — Approval notification bootstrap and de-duplication

**AC:** M01-AC06  
**Task:** T3

Use real Runtime Approval lifecycle plus a test notification sink.

Cover:

- Host attaches with one pre-existing Pending Approval → count updates, no notification burst;
- one new ApprovalRequested → exactly one notification decision;
- duplicate ActivityEvent → no second notification;
- polling/refresh of same Pending → no second notification;
- two distinct approvals → two notification decisions;
- Runtime identity/session rollover isolates dedupe key space;
- bounded dedupe storage does not grow without limit.

**PASS:** only newly observed Pending identities notify once.

---

### M01-INT-09 — Notification delivery failure never changes Approval truth or fallback

**AC:** M01-AC06, M01-AC09, M01-AC10  
**Task:** T3

Simulate/drive notification capability states:

- permission/capability granted;
- denied;
- unavailable;
- plugin send error;
- click callback unavailable.

For every failure branch verify:

- Pending Approval remains Pending in Runtime;
- no approvals.resolve command is issued;
- Tray count remains queryable/current;
- Approvals UI remains usable;
- failure is not retried in a tight loop;
- delivery failure does not alter Tool/Policy/Execution state.

**PASS:** notification is advisory only.

---

### M01-INT-10 — Approval click routing re-queries live state before actionability

**AC:** M01-AC08  
**Tasks:** T3/T5

Use the real Host navigation bridge and real Runtime approvals.

Cases:

- click while Pending;
- click after Approved;
- click after Denied;
- click after Expired;
- click after Cancelled;
- click after Runtime/request becomes unavailable.

Verify:

- main window is shown/focused;
- Runtime is re-queried;
- Host emits approvals navigation with request ID only after routing validation;
- Pending opens normal current actions;
- terminal request renders true terminal state with actions disabled;
- unavailable/stale request is non-actionable;
- notification payload never recreates Pending;
- click callback never resolves approval directly.

**PASS:** notification click navigates; Runtime truth decides actionability.

---

### M01-INT-11 — Approval notification content is bounded and privacy-safe

**AC:** M01-AC07, M01-AC09  
**Task:** T3

Feed synthetic Approval data containing sentinel values for:

- raw API key / secret;
- physical path;
- raw Bash command / argv;
- file content;
- oversized argument;
- raw error detail;
- unrestricted metadata JSON.

Verify the produced notification title/body/data contains only accepted bounded fields such as:

- application identity;
- Approval required;
- logical Tool name;
- optional bounded approved safe summary;
- stable routing identity.

Search emitted payload for every sentinel.

**PASS:** no prohibited sentinel survives and no whole ApprovalRequest/metadata blob is serialized into the OS notification.

---

### M01-INT-12 — Ready-loss incident opens once and de-duplicates repeated fault observations

**AC:** M01-AC11  
**Task:** T4

Table-drive HarnessStatus transitions:

- Stopped→Starting→Ready;
- Ready→Degraded;
- repeated Degraded;
- Degraded→Error;
- repeated Error / changing error message;
- Runtime crash after Ready.

Verify:

- no incident before the session has reached Ready;
- first real Ready loss opens one incident;
- one disconnect notification decision is produced;
- repeated poll/status/error updates stay inside the same incident.

**PASS:** one fault episode is not spammed.

---

### M01-INT-13 — Intentional Stop/Quit/restart transitions do not masquerade as disconnect incidents

**AC:** M01-AC11, M01-AC13  
**Task:** T4

Validate:

- Ready→Stopping→Stopped via user Stop;
- Ready→Stopping→Stopped via explicit Quit;
- controlled Restart Ready→Stopping→Starting→Ready;
- restart failure that ends in a true Error after the controlled transition.

**PASS:**

- expected Stop/Quit/restart transition emits no disconnect/recovery spam;
- a genuine failure after the controlled transition may open a real incident;
- notification send result never mutates HarnessStatus.

---

### M01-INT-14 — Recovery closes one incident and a later independent fault creates a new incident

**AC:** M01-AC12  
**Task:** T4

Sequence:

~~~text
Ready
→ Degraded
→ repeated Degraded/Error
→ Ready
→ Ready
→ Degraded
~~~

Verify:

- first fault = incident #1, one disconnect notification;
- return to authoritative Ready closes incident #1;
- at most one recovery notification;
- repeated Ready does not duplicate recovery;
- later Ready loss creates incident #2 and may notify again.

Also verify that Stop during an open incident closes it silently rather than producing fake recovery.

**PASS:** episode lifecycle is stable and repeatable.

---

### M01-INT-15 — Connection notification routing opens current Overview state, not cached fault truth

**AC:** M01-AC13  
**Tasks:** T4/T5

On a disconnect notification click:

- show/focus main window;
- navigate to Overview;
- render the latest HarnessStatus after navigation;
- if the connection has already recovered, display Ready rather than the old notification fault state;
- if still degraded/error, display current degraded/error state;
- do not claim notification delivery as recovery.

M01 itself must not require M02 Diagnostics.

**PASS:** navigation is current-state presentation only.

---

### M01-INT-16 — Packaged Tauri wiring and authority/security regression

**AC:** M01-AC15, M01-AC16  
**Tasks:** T2–T6

Use the actual packaged Windows candidate for component integration checks that do not require real ChatGPT.

Verify:

- Tray exists and uses packaged app identity/icon;
- notification plugin/capability initializes;
- Close→Hide leaves app alive;
- current Activity UI still receives events;
- existing Approval resolution path still works only from Approval UI;
- Workspace/Policy/Bash/SecretStore regressions pass;
- no new daemon/process was introduced;
- no Tray/React code constructs Ready or Approval authority;
- NSIS package behavior is recorded;
- Portable differences are recorded truthfully;
- if Portable cannot deliver Windows notification callback, fallback remains functional and the limitation is not called PASS for notification capability;
- no temporary test profile/process remains after cleanup.

**PASS:** the packaged desktop integration respects all M01 authority boundaries.

---

## 5. Required Case Matrix

| Case | Main acceptance focus |
| --- | --- |
| M01-INT-01 | Close→Hide and same Harness session |
| M01-INT-02 | first-close education / acknowledgement |
| M01-INT-03 | authoritative Tray lifecycle actions |
| M01-INT-04 | Runtime-backed Pending count |
| M01-INT-05 | explicit Quit / orphan cleanup |
| M01-INT-06 | v2→v3 migration |
| M01-INT-07 | Host-only desktop preferences |
| M01-INT-08 | Approval notification dedupe/bootstrap |
| M01-INT-09 | delivery failure fallback |
| M01-INT-10 | click live re-query/navigation |
| M01-INT-11 | notification privacy |
| M01-INT-12 | fault incident dedupe |
| M01-INT-13 | Stop/Quit/restart suppression |
| M01-INT-14 | recovery/new incident |
| M01-INT-15 | current-state Overview routing |
| M01-INT-16 | packaged integration/security regression |

---

## 6. Verdict Rules

Use exactly:

- PASS — executed and all required assertions satisfied.
- FAIL — executed and one or more required assertions failed.
- BLOCKED — required environment/component/package is unavailable.
- INCONCLUSIVE — execution occurred but evidence cannot support the required conclusion.

Examples:

- Tray unit mock passes but packaged Tauri candidate has no Tray → M01-INT-16 FAIL.
- notification plugin unavailable on the acceptance package and fallback works → relevant fallback assertion may PASS, but notification capability itself is BLOCKED/FAIL according to required target; do not silently call it full PASS.
- Runtime unavailable and Tray shows old numeric Pending count → M01-INT-04 FAIL.
- click opens the request but uses cached Pending payload instead of live query → M01-INT-10 FAIL.

---

## 7. Integration Exit Gate

M01 Integration is PASS only when:

- M01-INT-01 through M01-INT-16 are PASS;
- all results bind to the same production candidate, or evidence-only changes are proven unable to affect production behavior;
- cleanup is complete;
- no later production change invalidates executed evidence;
- no case was replaced by a weaker mock boundary.

Integration PASS does not mark M01 ACCEPTED / FROZEN.

Next:

~~~text
Integration PASS
→ M01 E2E
→ M01 Manual Acceptance
→ user sign-off
~~~
