# M01 E2E Test Plan — Packaged Windows Desktop Continuity

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M01 — Tray, Background Lifecycle and Notifications  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: QA plus explicitly authorized user/external execution where required  
> Purpose: prove the real packaged Windows flows for background continuity, Approval notifications, disconnect/recovery notifications and explicit Quit.  
> Gate: M00 must be ACCEPTED / FROZEN, M01 Development Complete, and M01 Integration PASS before formal E2E execution.

---

## 1. Objective

M01 E2E validates the actual user-visible chain rather than test doubles.

Required real boundaries:

~~~text
real packaged Windows app
→ Tauri Host / Tray / Windows notification
→ HarnessController
→ Runtime
→ Tunnel
→ real ChatGPT where specified
~~~

The four E2E cases are:

- M01-E2E-01 — hidden-window background continuity with real ChatGPT Tool call;
- M01-E2E-02 — real ASK → Windows Approval notification → specific Approval route;
- M01-E2E-03 — real managed connection fault → de-duplicated disconnect/recovery notifications;
- M01-E2E-04 — production config v3 + explicit Quit + no owned orphan / no accidental F02 behavior.

Mocks, browser-only tests and direct local MCP calls cannot replace the external boundary named by a case.

---

## 2. Acceptance Package

Normative notification acceptance target:

- Windows NSIS installed package, unless implementation evidence proves an equivalent supported notification/click contract for Portable.

Portable is still validated for:

- Tray;
- Close→Hide;
- Show/Start/Stop/Quit;
- safe fallback when OS notification capability is unavailable.

A Portable build with unavailable Windows Toast/click must be reported as such; it cannot be used to claim full notification E2E PASS.

Record:

~~~text
Candidate commit:
Dirty state:
Package type:
Package path:
Package SHA-256:
App version:
Runtime SHA-256:
Tunnel binary/version:
Windows version:
Tauri version:
Notification plugin version:
Config schema:
Workspace:
Tunnel ID / test identity:
Security mode:
Relevant Tool/Policy:
Tester:
Date/time:
~~~

Never record the API key value.

---

## 3. External Authorization

M01-E2E-01 and M01-E2E-02 require real external ChatGPT interaction.

Any action that restarts/manipulates a user-managed Harness/Tunnel instance or causes network calls must follow the current user authorization boundary.

If authorization/environment is unavailable:

- affected case = BLOCKED;
- do not substitute a local SDK, mock ChatGPT or direct Runtime request and mark PASS.

---

## 4. Required E2E Cases

### M01-E2E-01 — Hidden main window still serves real ChatGPT Tool call

**AC:** M01-AC01, AC03, AC15, AC16  
**Tasks:** T2/T6

#### Preconditions

- packaged candidate installed/launched;
- Harness reaches authoritative Ready;
- controlled Workspace contains a known read-only target;
- an existing non-destructive Tool such as read_file or glob is available;
- first-close education has been completed for the acceptance profile.

#### Action

1. close the main window using the normal Windows close button;
2. verify the window disappears but the Tray remains;
3. while the main window is hidden, from real ChatGPT request an actual Harness Tool call against the controlled Workspace;
4. require real tool use, not a prose answer;
5. after success, use Tray Show to reopen the app.

#### Assertions

- Close did not stop Runtime/Tunnel.
- ChatGPT→Tunnel→Runtime call succeeds while the main window is hidden.
- result matches controlled Workspace truth.
- Tray status remains consistent with Harness lifecycle.
- reopening shows the same active managed session/current state.
- no duplicate Runtime/Tunnel process was spawned.
- hidden-window operation does not require React visibility to keep Harness alive.

**PASS:** background continuity is real end-to-end.

---

### M01-E2E-02 — Real ASK generates one safe Windows notification and routes to the correct live Approval

**AC:** M01-AC06, AC07, AC08, AC09, AC10, AC15, AC16  
**Tasks:** T3/T5/T6

#### Preconditions

- normative packaged Windows candidate;
- Approval notifications enabled;
- Windows app notification permission enabled;
- controlled harmless Tool configured to produce ASK;
- Harness Ready;
- no unrelated Pending Approval in the acceptance session.

#### Action

1. from real ChatGPT issue a request that actually triggers the controlled Tool;
2. Runtime reaches Pending Approval;
3. observe the Windows notification;
4. do not create another request;
5. click the live notification while the callback is available;
6. inspect the opened Approval page/request;
7. resolve the request only through the normal Approval UI if needed for cleanup.

#### Assertions

- exactly one notification is produced for the new Pending identity.
- notification identifies the app and logical Tool without raw secret, raw command/argv, physical path or file body.
- Tray pending count becomes 1 from Runtime truth.
- notification click shows/focuses app.
- the specific matching request is selected.
- current Runtime state is re-queried before actions are enabled.
- click itself does not approve/deny.
- after normal resolution, count returns to 0.
- duplicate poll/event does not create another toast.

#### Fallback subcase

Temporarily disable/block Windows notifications for the app, then trigger a second controlled ASK.

Required:

- no lost Approval;
- Tray pending count still shows the request;
- Approvals UI can still open and resolve it;
- notification failure does not alter Policy/Approval truth.

Restore OS notification settings after the test.

#### Platform limitation

Historical Action Center click after a toast has moved out of its live state is not assumed reliable unless the chosen plugin/Windows combination proves it. The required E2E deep-link is the supported live notification callback. Terminal/stale click semantics are still covered by Integration.

**PASS:** real ASK notification and fallback both work without becoming an authorization source.

---

### M01-E2E-03 — Real managed connection fault produces one disconnect episode and one recovery episode

**AC:** M01-AC11, AC12, AC13, AC15, AC16  
**Tasks:** T4/T5/T6

#### Preconditions

- packaged candidate;
- connection notifications enabled;
- Harness has reached Ready;
- acceptance instance is controlled and may be faulted/recovered safely.

#### Fault action

Trigger one real managed Tunnel/connection fault using the accepted QA procedure for the candidate, for example terminating only the candidate-owned tunnel-client process or otherwise forcing its managed connection to lose Ready.

Do not kill unrelated system/ChatGPT/proxy processes.

#### Assertions during fault

- Harness leaves Ready authoritatively.
- one disconnect/degraded notification is produced for the incident.
- repeated health polls/status updates do not create repeated notifications.
- clicking the live notification shows/focuses the app at Overview.
- Overview shows the current lifecycle state, not a stale cached message.

#### Recovery

Allow the existing managed recovery path to return the same product to authoritative Ready.

Verify:

- at most one recovery notification;
- notification delivery did not cause the recovery;
- repeated Ready updates do not duplicate recovery.

#### New incident

After Ready is stable, trigger a second independent equivalent fault.

Verify:

- a new disconnect notification may appear because it is a new incident.

#### Intentional lifecycle control

Separately use normal Tray Stop / Start or controlled Restart.

Verify those intentional transitions do not produce disconnect/recovery spam.

**PASS:** real connection episode semantics match the incident contract.

---

### M01-E2E-04 — Production v3 config, explicit Quit and packaged cleanup integrity

**AC:** M01-AC04, AC05, AC14, AC15, AC16  
**Tasks:** T1/T2/T5/T6

#### Preconditions

Use a disposable acceptance profile initialized from a representative valid v2 config or an approved migration fixture path that exercises the real production ConfigStore migration.

#### Assertions before Quit

- application starts successfully after v2→v3 migration;
- existing Workspace/Tunnel/Tool/Policy/UI settings remain usable;
- Approval and connection notification preferences exist with expected defaults;
- changing those preferences does not request a Runtime restart;
- M01 exposes no Auto Start Harness behavior or Auto Start toggle;
- normal existing Harness Start works.

#### Explicit Quit

With Harness Ready:

1. choose Tray Quit;
2. observe application exit;
3. inspect candidate-owned processes.

Required:

- application exits;
- Runtime/Tunnel owned by the candidate are gone;
- no orphan remains;
- no disconnect/recovery notification is emitted merely because Quit was intentional.

Restart the desktop application manually.

Verify:

- notification preferences persist;
- application does not automatically start Harness because F02 belongs to M07.

**PASS:** config migration, Host-only preference persistence and explicit cleanup all hold at production boundary.

---

## 5. E2E Verdict Rules

Use:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Do not use SKIP as success.

Examples:

- real ChatGPT unavailable → E2E-01/02 BLOCKED.
- Windows notification cannot be delivered in the normative NSIS package → E2E-02 FAIL/BLOCKED according to root cause; fallback success alone does not satisfy the required notification path.
- fault was created by normal user Stop → not a valid E2E-03 fault.
- Quit exits UI but leaves candidate Runtime alive → E2E-04 FAIL.

---

## 6. E2E Exit Gate

M01 E2E is PASS only when:

- M01-E2E-01 PASS;
- M01-E2E-02 PASS;
- M01-E2E-03 PASS;
- M01-E2E-04 PASS;
- all cases bind to the exact candidate/package identity;
- cleanup succeeds;
- no production change invalidates the results.

Only after E2E PASS may final Manual Acceptance be signed.

E2E PASS does not itself mark M01 ACCEPTED / FROZEN.
