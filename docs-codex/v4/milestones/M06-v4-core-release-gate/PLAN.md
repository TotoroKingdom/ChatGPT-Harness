# M06 PLAN — V4 Core Release Gate

> Input: SPEC.md  
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M01–M05 ACCEPTED / FROZEN AND M06 VALIDATION DOCS ARE FROZEN  
> Scope: Core release engineering, integrated regression and acceptance; no new P1/P2 product scope  
> Candidate rule: formal M06 validation uses one frozen Core RC  
> External rule: real External ChatGPT, paid/networked Model requalification, destructive install/rollback actions and user-instance restarts require explicit execution-time authorization.

---

# 1. Delivery Strategy

M06 is a release gate, not a feature milestone.

~~~text
M01–M05 ACCEPTED / FROZEN
        ↓
M06 six docs frozen
        ↓
T1 Core gate verifier / candidate identity / matrix
        ↓
T2 RC packaging / install / upgrade / rollback
        ↓
T3 Cross-module Core integration + security
        ↓
T4 F05 performance + F06 diagnostics requalification
        ↓
T5 F12 + evidence + external-chain candidate binding
        ↓
T6 Final development regression + Core RC freeze
        ↓
Formal M06 Integration
        ↓
Same-candidate packaged E2E
        ↓
Human Windows + External ChatGPT Manual Acceptance
        ↓
V4 Core — ACCEPTED
~~~

No task may lower upstream thresholds or change Core semantics to make the release pass.

---

# 2. Implementation Principle

Prefer verification, orchestration and defect closure over new architecture.

Allowed M06 code work:

- release-gate scripts;
- candidate identity/verification helpers;
- missing cross-module automated tests;
- package integrity verification;
- upgrade/rollback fixtures;
- minimal fixes for discovered Core blockers.

Not allowed for convenience:

- second Runtime authority;
- direct Tool execution bypass;
- second Policy/Approval path;
- second History writer;
- new Eval methodology;
- relaxed M02/F12 gates;
- new P1/P2 feature.

If a Core defect requires product code change, fix minimally and invalidate/rerun affected candidate evidence.

---

# 3. Landing Zones

Use existing trees:

~~~text
ui/scripts/
runtime/tests/
ui/src-tauri/ existing modules/tests
ui/src/ existing tests
protocol/tests/
fixtures/ existing config/history fixture locations
~~~

No new top-level directory.

No standalone release-gate application.

The M06 milestone directory ultimately contains exactly six documents.

---

# 4. Phase A — Candidate Identity and Core Gate Verifier

## 4.1 Identity collector

Reuse current build/distribution metadata to collect:

- SHA / dirty state / source-state hash;
- version;
- Portable/installer hashes;
- UI/Runtime/Tunnel hashes;
- protocol schema hash;
- config schema/profile identity;
- evidence schema/checksum;
- Tool catalog/schema identity;
- M04/M05 Eval identity.

Fail on missing or inconsistent identity.

## 4.2 Core matrix validator

Encode the exact 28 canonical Core IDs from SPEC.

The validator must:

- require every Core ID exactly once;
- reject duplicate/missing IDs;
- require evidence reference;
- require final status;
- reject mandatory non-PASS;
- keep P1/P2 separate from Core;
- never write results into frozen planning docs.

## 4.3 Candidate invalidation

Define impact classes:

- HOST_UI;
- HOST_LIFECYCLE;
- RUNTIME;
- TUNNEL;
- PROTOCOL_CONFIG;
- EVIDENCE_HISTORY;
- TOOL_CATALOG;
- EVAL;
- RELEASE_ONLY.

Map each class to required reruns.

Golden/negative tests cover mixed hashes, missing Core rows, BLOCKED rows and release-only changes.

Exit: the gate is fail-closed before final product validation begins.

---

# 5. Phase B — Package, Install, Upgrade and Rollback

## 5.1 Same-SHA packages

From one clean candidate:

- build Portable;
- verify Portable;
- build NSIS;
- verify NSIS;
- compare staged payload hashes.

## 5.2 Shipping-content audit

Reject accidental shipping of:

- synthetic secrets;
- temporary DB/config;
- Eval test Workspace;
- stale binaries from another candidate;
- unintended development-only artifacts.

## 5.3 Upgrade fixtures

Use representative:

- v2 config;
- pre-Core v3 config;
- SecretReference;
- Workspace;
- Tool toggles;
- Policy rules;
- desktop preferences;
- History config;
- grep upgrade state.

Verify accepted v2→v3 behavior without data loss.

## 5.4 History compatibility

Use representative M03 History data.

Verify:

- readable after upgrade;
- writer ownership remains correct;
- no old authorization restored;
- no unexpected physical schema drift.

## 5.5 Rollback

Freeze the previous accepted package identity.

Test:

- clean Quit;
- preserve user data;
- restore pre-upgrade config backup where older binary requires it;
- launch previous accepted package;
- preserve compatible History;
- no plaintext secret conversion;
- no orphan process.

Exit: release packages and recovery path are testable.

---

# 6. Phase C — Integrated Core Scenarios

Avoid simply rerunning every old unit test. Add cross-component coverage for final-candidate behavior.

## 6.1 Lifecycle

Cover:

- Start;
- authoritative Ready;
- close/hide;
- Tray state;
- background Tool call;
- reopen;
- Stop;
- Restart;
- Quit;
- zero owned orphan.

## 6.2 Approval notification

Exercise:

- first Pending -> one notification intent;
- repeated observation -> no duplicate;
- resolved before click -> current state queried;
- notification failure -> Approval remains;
- safe notification content.

Final OS Toast observation remains packaged validation/manual where automation is insufficient.

## 6.3 Disconnect notification

Exercise:

- healthy -> degraded/disconnected;
- repeated same incident;
- recovery;
- later independent incident;
- manual Stop/Quit.

No notification result may be used as connection truth.

## 6.4 Tool authority

Real Runtime MCP:

- read_file success;
- grep literal;
- grep truncation;
- ASK allow_once;
- ASK deny;
- DENY;
- Tool disabled;
- cancel/timeout where applicable.

No direct ToolExecutor shortcut.

## 6.5 Evidence

For the same requests verify:

- exact runtime_id + request_id;
- Activity;
- terminal Approval;
- History;
- restart persistence;
- no stale authorization;
- grep privacy-safe evidence.

## 6.6 V3 regression

Cover:

- Workspace escape;
- Tool toggle;
- Policy;
- Approval;
- SessionGrant;
- write/edit atomicity;
- Bash;
- SecretStore;
- desired/effective state;
- config revision/restart debt;
- i18n/Appearance smoke.

## 6.7 Crash/recovery

Inject Runtime/Tunnel failure, incomplete request, restart and final Quit.

Exit: Core behaviors work together on one candidate.

---

# 7. Phase D — F05 Performance and F06 Diagnostics

## 7.1 Import M02 contract

Read the accepted M02:

- baseline identity;
- candidate identity;
- benchmark harness;
- cold/warm definition;
- sample count;
- P50/P95/failure thresholds;
- environment class.

Do not change them.

## 7.2 Equivalence decision

Compare final M06 startup-relevant hashes.

If equivalent, M06 may reference accepted M02 quantitative evidence plus final-candidate readiness sanity required by validation.

If not equivalent, run the full frozen M02 benchmark.

## 7.3 No post-hoc filtering

Current benchmark scripts must preserve:

- original sample selection;
- original P50/P95 math;
- original failure semantics.

## 7.4 Diagnostics matrix

Normal checks:

- Config;
- Workspace;
- Runtime;
- MCP;
- Tunnel;
- control plane;
- proxy;
- credential;
- lifecycle.

Fault fixtures:

- bad Workspace;
- Runtime unavailable;
- MCP target unavailable;
- Tunnel/control-plane failure;
- stale state;
- proxy/credential failure or UNKNOWN where safely testable.

Verify read-only behavior and sanitized summary.

Exit: F05/F06 can be formally requalified without contract drift.

---

# 8. Phase E — F12 and External-chain Binding

## 8.1 Read accepted Eval evidence

Require actual accepted M04/M05 formal results, not reconstructed constants.

Inputs:

- M04 baseline identity/result;
- M05 strict candidate identity/result;
- compare_m05;
- provider drift if used;
- security count;
- raw denominators.

## 8.2 F12 equivalence checker

Compare final M06:

- Runtime hash;
- Tool schema/catalog;
- M04 dataset/prompt/scorer;
- M05 profile;
- adapter/model identity.

Output one of:

~~~text
EQUIVALENT_REUSE_ALLOWED
REQUALIFICATION_REQUIRED
BLOCKED_MISSING_ACCEPTED_EVIDENCE
~~~

## 8.3 Formal rerun path

If required, use the already-frozen M04/M05 Runner.

Do not add an M06 scorer.

Do not call the provider until explicit user authorization.

## 8.4 Eval gate safety

Final gate input requires:

- valid sample;
- compare_m05 PASS;
- zero security violations;
- drift guard PASS when applicable;
- no passive telemetry substitution.

## 8.5 External-facing Tunnel

Use final packaged Tunnel with QA-controlled external MCP client:

- initialize;
- tools/list;
- read_file;
- grep;
- invalid request;
- reconnect;
- Activity/History correlation.

Label it external MCP evidence, not ChatGPT.

## 8.6 Real ChatGPT handoff

Prepare Manual steps for same candidate:

- hidden window;
- real ChatGPT read/grep;
- ASK;
- DENY;
- approval notification;
- disconnect/Diagnostics;
- History.

Exit: F12 and external chain are ready without faking evidence.

---

# 9. Phase F — Final Development Regression and RC Freeze

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

Also run existing:

- repository layout check;
- milestone layout check;
- Portable verifier;
- NSIS verifier;
- Core gate verifier golden/negative fixtures.

Create a fresh M06 candidate through existing delivery flow.

Do not overwrite earlier artifacts.

Development Complete response records:

~~~text
candidate SHA
product version
source dirty/state hash
Portable path/hash
Installer path/hash
UI/Runtime/Tunnel hashes
Config schema
Evidence schema/checksum
Tool catalog hash
M01–M05 eligibility
F05 equivalence/rerun status
F12 equivalence/rerun status
Upgrade development result
Rollback development result
Integrated Core development result
External ChatGPT status
Formal Integration status
Formal E2E status
Manual status
~~~

Do not claim V4 Core accepted during development.

---

# 10. Defect Closure Rules

When validation finds a Core blocker:

1. classify affected component;
2. make the smallest safe fix;
3. create a new candidate;
4. rerun affected unit/contract tests;
5. rerun affected M06 gates;
6. reuse unaffected evidence only when equivalence keys remain valid;
7. do not edit frozen requirements to make the fix pass.

Security/authorization/data-loss fixes invalidate all directly affected evidence.

---

# 11. Release Notes and Matrix Ownership

The final Core Product Acceptance Matrix and Release Notes are acceptance outputs.

They are assembled from:

- upstream accepted evidence;
- M06 Integration;
- M06 packaged E2E;
- M06 Manual Acceptance.

They are not implementation authority and do not create authorization.

Do not add a seventh file to the M06 milestone directory.

---

# 12. Failure / Rollback Plan

If M06 fails:

- do not publish a V4 Core acceptance claim;
- keep the previous accepted package as rollback target;
- leave P1/P2 roadmap unchanged;
- fix the owning Core defect;
- build a new candidate;
- rerun affected gates.

Do not reduce Core scope or thresholds.

---

# 13. Development Completion Gate

Before formal M06 Integration:

- T1–T6 complete;
- gate verifier golden/negative tests PASS;
- candidate identity reproducible;
- Portable + installer verification PASS;
- upgrade/rollback fixtures PASS;
- integrated local Core regression PASS;
- F05/F12 equivalence or rerun requirement is known;
- no known development-level Core blocker remains;
- external/paid/manual items remain honestly NOT EXECUTED if not run.

Development Complete is not V4 Core acceptance.
