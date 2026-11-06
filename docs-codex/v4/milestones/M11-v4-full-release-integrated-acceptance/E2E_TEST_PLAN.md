# M11 Packaged E2E Test Plan — V4 Full Release Integrated Acceptance

> Status: PLAN ONLY / NOT EXECUTED
> Scope: packaged Portable and NSIS-installed candidate flows through real product boundaries.
> This file is a validation plan, not a PASS record and not a release decision.

## 1. Candidate and Environment

- Portable and NSIS must originate from the same clean, locked Full RC.
- Install, upgrade, rollback, uninstall and reinstall use disposable Windows paths and synthetic non-secret configuration.
- The same candidate identity is used for all cases except explicitly identified prior-version upgrade/rollback comparison binaries.
- Controlled MCP clients are not External ChatGPT. Their evidence remains separately labelled.
- Any real External ChatGPT step is reserved for Manual acceptance and requires execution-time authorization.

## 2. Case Matrix

| ID | Packaged flow | Required observations | Failure conditions |
| --- | --- | --- | --- |
| M11-E2E-01 | Install, identity and upgrade | Clean install, Auto Start OFF default, valid AppConfig, Runtime/Tunnel start, v2→v3 and existing-v3 preservation, evidence migration and reinstall. | Hash mismatch, lost field, plaintext secret, stale DB, accidental Tool exposure or orphan. |
| M11-E2E-02 | Desktop integrated lifecycle | Enable Auto Start, full Quit, reopen once, authoritative Ready, hide/Tray continuity, Stop, explicit Start, Quit. | Duplicate generation, false Ready, login-startup mechanism, same-process auto-restart or orphan. |
| M11-E2E-03 | Tool and evidence chain | Real packaged Runtime MCP read/grep, ASK allow/deny, Policy DENY, representative M09 ADMIT Tool if any, History/Metrics/Trace exact identity. | Security bypass, wrong catalog, fabricated stage/metric, raw payload leak or authorization restoration. |
| M11-E2E-04 | MCP and recovery | Raw/standalone controlled client initialize/list/call/error/cancel; safe Runtime/Tunnel disconnect and recovery; crash/restart state. | Protocol identity drift, unresolved work leak, stale Ready, fake terminal success or stale permit. |
| M11-E2E-05 | Full release preparation | Candidate release notes, package lifecycle residue scan, uninstall/rollback evidence handoff and external-chain preparation. | Premature acceptance wording, residue, package contamination or unbound evidence. |

## 3. Required Per-Case Record

Each execution record must contain:

- candidate ID, commit, branch and `source_dirty`;
- Portable, installer and installed payload hashes;
- disposable state root and cleanup result;
- exact client/binary identity;
- request correlation fields (`runtime_id`, `request_id`, and `trace_id` where available);
- observed outcome and bounded sanitized error projection;
- formal status `PASS`, `FAIL`, `BLOCKED`, `INCONCLUSIVE` or `NOT_VERIFIED`;
- blocker and evidence locator when not PASS.

## 4. Boundary Rules

- Portable smoke is not formal M11 E2E by itself.
- A standalone SDK client is not External ChatGPT.
- Internal Rust MCP tests do not prove installed Windows behavior.
- Historical evidence cannot be combined across incompatible package identities.
- Failed cleanup blocks the case even if the product action itself appeared successful.

## 5. Cleanup

Stop all Harness-owned processes before removing disposable state. Verify the external Workspace sentinel remains intact, no managed process remains, and reinstall can create a fresh isolated state. Never remove a path unless its resolved absolute location is inside the test root.
