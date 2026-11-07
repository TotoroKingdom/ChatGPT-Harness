# M11 Manual Acceptance Checklist — V4 Full Release Integrated Acceptance

> Status: CHECKLIST ONLY / NOT EXECUTED
> Scope: human-observed Windows packaging, desktop surfaces and real External ChatGPT acceptance.
> No item is a PASS until a reviewer observes and records it against the locked final candidate.

## 1. Preconditions

- [ ] Candidate identity is locked and source/package hashes are recorded.
- [ ] M06, M07, M08, M09 and M10 are explicitly `ACCEPTED / FROZEN` with evidence identities.
- [ ] M11 Integration and packaged E2E prerequisites are PASS.
- [ ] Disposable Workspace contains no user data and has before/after sentinels.
- [ ] Credentials use the approved SecretStore/reference path and are not entered into evidence.
- [ ] Reviewer has explicit authorization for real External ChatGPT and any active-instance restart.

## 2. MAN-01 — Release Identity and Scope

- [ ] Portable and NSIS hashes match the same candidate.
- [ ] Release Notes list delivered mandatory scope and the exact M09 disposition set.
- [ ] F09 Estimated Tokens is explicitly `DEFERRED`.
- [ ] F15 Policy Simulator is explicitly `DEFERRED`.
- [ ] No document or UI claims `V4 — ACCEPTED / FROZEN` before this checklist is complete.

## 3. MAN-02 — Windows Desktop Lifecycle

- [ ] Clean install uses Auto Start OFF by default.
- [ ] Explicit Auto Start choice persists after Quit and reopen.
- [ ] Reopen triggers exactly one standard Start and one managed generation.
- [ ] Ready is shown only after authoritative Runtime/Tunnel readiness.
- [ ] Close/hide leaves Tray background continuity and does not retrigger Auto Start.
- [ ] Tray Show, Start, Stop and pending count reflect authoritative state.
- [ ] Stop does not auto-restart in the same App process.
- [ ] Full Quit leaves no owned Runtime/Tunnel process.

## 4. MAN-03 — Notifications, History, Metrics and Trace

- [ ] Approval notification is bounded, deduplicated and routes to the current request.
- [ ] Resolved/stale notification navigation does not authorize or resurrect a request.
- [ ] Disconnect and recovery notifications are truthful and deduplicated.
- [ ] History shows the observed request identity and outcome.
- [ ] Metrics and Trace show the same `runtime_id + request_id`.
- [ ] Trace contains only observable Runtime stages and no hidden model reasoning.
- [ ] Incomplete/unknown evidence remains visibly incomplete/unknown.

## 5. MAN-04 — Real Tool and Security Chain

- [ ] ChatGPT discovers the final effective Tool catalog.
- [ ] A safe `read_file` call succeeds in the disposable Workspace.
- [ ] `grep` succeeds with bounded output.
- [ ] A representative admitted M09 Tool succeeds only when the accepted decision set contains an ADMIT Tool.
- [ ] ASK → approve mutates only the intended disposable target.
- [ ] ASK → deny and Policy DENY do not mutate the Workspace.
- [ ] Workspace escape, invalid arguments, unknown Tool and disabled Tool are safely rejected.
- [ ] No secret, raw environment, unrestricted args, physical path or stack trace appears externally.

## 6. MAN-05 — Real External ChatGPT Integrated Chain

- [ ] The same final packaged candidate is installed and launched.
- [ ] Auto Start, Ready, hide/Tray continuity and clean Quit are observed.
- [ ] Real External ChatGPT uses the real Tunnel and discovers the expected catalog.
- [ ] Safe read, grep, safe error and ASK allow/deny complete as expected.
- [ ] History, Metrics and Request Trace correlate to the same request.
- [ ] A safe disconnect/recovery and Diagnostics flow is observed.
- [ ] After restart, History persists but Pending Approval/Grant/Permit does not.

## 7. MAN-06 — Upgrade, Rollback, Uninstall and Residue

- [ ] v2→v3 configuration preserves Workspace, Tunnel, Proxy, SecretReference, Tools, Policy and Bash semantics.
- [ ] Existing v3 desktop preferences and History settings survive upgrade.
- [ ] Evidence DB migration preserves History and does not restore live authorization.
- [ ] Accepted rollback path does not destructively downgrade or erase user data.
- [ ] Uninstall removes product-owned integration and managed processes.
- [ ] External Workspace remains untouched.
- [ ] User-data handling matches the established installer contract.
- [ ] Reinstall succeeds and no test residue remains.

## 8. Final Manual Verdict

Record one status for every mandatory item and attach the candidate-bound evidence locator. Any unchecked, failed, blocked or inconclusive mandatory item prevents M11 acceptance. The only successful final wording is:

```text
V4 — ACCEPTED / FROZEN
```
