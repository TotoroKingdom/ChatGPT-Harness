# M11 TASKS — V4 Full Release Integrated Acceptance

> Inputs: SPEC.md, PLAN.md
> Executor: Codex / Coding Agent
> Status: IMPLEMENTATION BLOCKED UNTIL M06+M07+M08+M09+M10 ACCEPTED / FROZEN AND M11 VALIDATION DOCS ARE FROZEN
> Scope: final V4 Full Release Gate only
> Mandatory scope: Shared Evidence + F01–F08 + F10–F14
> Optional disposition: F09 DEFERRED / F15 DEFERRED
> Rule: do not modify frozen milestone contracts to make a failing gate pass.
> Rule: Development Complete is not V4 — ACCEPTED / FROZEN.

---

# 1. Dependency

~~~text
M06 + M07 + M08 + M09 + M10 ACCEPTED / FROZEN
        ↓
M11 six docs frozen
        ↓
T1 Full RC identity + matrix + equivalence gate
        ↓
T2 package/install/upgrade/rollback/uninstall
        ↓
T3 integrated Core + P1 regression
        ↓
T4 F05 + F12 + F13 final-candidate quality gates
        ↓
T5 release truth + full candidate packaging + external-chain preparation
        ↓
T6 full regression + Development Complete handoff
        ↓
Integration → E2E → Manual
        ↓
V4 — ACCEPTED / FROZEN
~~~

Do not infer upstream acceptance from code presence or Development Complete.

---

# T1 — Implement the Full RC Identity, 15-feature Matrix and Evidence Equivalence Gate

**AC:** M11-AC01 through AC05, AC23, AC24
**Primary areas:** existing M06 release-gate tooling, ui/scripts/, fixtures/v4/, existing test modules.
**Production feature change:** none.

## 1. Reuse/extend M06 candidate identity tooling

Do not create a second independent release identity model if M06 already provides one.

Extend it to capture the M11-required fields from SPEC, including:

- clean source identity;
- Portable/installer/installed payload hashes;
- AppConfig schema/revision/profile;
- desktop preferences hash;
- Control Protocol revision/schema hash;
- evidence-store schema/migration identity;
- Tool catalog/per-tool schema hashes;
- M09 decision-set hash;
- F05 identity;
- F12/Eval identities;
- Metrics/Trace contract identity;
- M10 MCP/conformance identity;
- F09/F15 disposition.

Add deterministic serialization and hash coverage tests.

## 2. Add upstream acceptance gate

Require explicit accepted/frozen evidence for:

~~~text
M06
M07
M08
M09
M10
~~~

Allowed states for entry:

~~~text
ACCEPTED / FROZEN
~~~

Rejected entry states include:

- Development Complete;
- Integration PASS only;
- E2E PASS only;
- BLOCKED;
- INCONCLUSIVE;
- NOT_VERIFIED;
- missing.

Fail closed.

## 3. Add 15-feature matrix structure

Generate/check exactly one row for each:

~~~text
F01 ... F15
~~~

and a separate Shared Evidence section.

Mandatory expected dispositions:

~~~text
F01–F08 = PASS
F09 = DEFERRED
F10–F14 = PASS
F15 = DEFERRED
~~~

No mandatory row may be DEFERRED.

No P2 row may be fabricated as PASS without a separately frozen optional delivery contract.

## 4. Canonical AC completeness

Assert all 43 mandatory canonical IDs are represented exactly once.

Assert optional F09/F15 canonical ACs are represented as DEFERRED scope, not PASS evidence.

## 5. Evidence equivalence classification

Implement deterministic classification:

~~~text
EXACTLY_EQUIVALENT
REQUIRES_TARGETED_RERUN
REQUIRES_FULL_RERUN
INVALID_BLOCKER
~~~

Inputs must be machine-readable identity fields.

No fuzzy timestamps or prose heuristics.

## 6. Candidate invalidation tests

Cover at least:

- UI hash change;
- Runtime hash change;
- Tunnel hash change;
- Tool schema change;
- evidence DB schema change;
- M09 decision-set change;
- MCP revision/dependency change;
- Eval dataset/scorer change;
- F09/F15 disposition drift.

Verify affected evidence invalidation is precise and fail-closed.

### Verification

Run targeted release-gate unit/contract tests.

### Done

M11 has one authoritative candidate/matrix/equivalence gate and cannot assemble incompatible historical evidence.

---

# T2 — Implement Full Windows Package Lifecycle Validation

**AC:** M11-AC06, AC07, AC09, AC13, AC22
**Depends on:** T1
**Primary areas:** ui/scripts/, installer test helpers, fixtures/v4/, disposable Windows state.
**Production feature change:** none unless a real packaging defect is found.

## 1. Build one clean candidate pair

From one source candidate:

- Portable;
- NSIS installer.

Record:

- package hashes;
- UI hash;
- Runtime hash;
- Tunnel hash;
- installed payload hashes.

Reject mismatch.

## 2. Clean install fixture

Use isolated/disposable Windows paths.

Verify:

- installer succeeds;
- product launches;
- Auto Start default OFF;
- AppConfig is valid;
- Runtime/Tunnel can start;
- no test secrets;
- no stale user fixture;
- no debug-only Tool.

## 3. v2→v3 config upgrade fixture

Prepare synthetic non-secret v2 config.

Verify preservation of accepted fields:

- Workspace;
- Tunnel;
- Proxy;
- SecretReference;
- Tool config;
- Policy;
- Bash;
- relevant desktop/history defaults.

No plaintext secret.

## 4. Existing v3 upgrade fixture

Include explicit:

~~~text
desktop.autoStartHarnessOnAppLaunch = true
desktop notification preferences
history settings
Tool settings
Policy settings
~~~

Verify explicit choices survive.

## 5. M09 Tool migration

Load accepted decision set.

After upgrade:

- every ADMIT Tool follows accepted install/upgrade default semantics;
- every REJECT/DEFER Tool remains absent;
- no missing-field accidental exposure.

## 6. Evidence DB migration

Exercise accepted M03/M08 schema path.

Verify:

- History preserved;
- Metrics/Trace semantics remain valid;
- no silent database reset;
- no restored live approval/grant/permit;
- migration failure path preserves recoverability.

## 7. Rollback

Use accepted M06/M08 rollback semantics.

Assert:

- no destructive blind schema downgrade;
- previous accepted app/config can be restored through the accepted path;
- external Workspace untouched;
- SecretReference remains non-plaintext.

## 8. Uninstall

Against disposable installed candidate:

- stop app/Harness;
- uninstall;
- verify no managed Runtime/Tunnel orphan;
- verify product binaries/integration removed according to installer contract;
- verify external Workspace untouched;
- verify user-data handling matches established installer behavior;
- verify reinstall succeeds.

## 9. Package contamination checks

Reject package if it contains:

- conformance source checkout;
- Eval provider credentials;
- test config;
- temp Workspace;
- debug-only admission Tool;
- raw result reports;
- secret/plaintext token.

### Done

M11 has repeatable package lifecycle validation without changing user-data policy.

---

# T3 — Implement the Final-candidate Core + P1 Integrated Regression Harness

**AC:** M11-AC08 through AC13, AC18 through AC20
**Depends on:** T1/T2
**Primary areas:** Runtime/Tauri integration tests, existing M06/M07/M08/M09 test infrastructure.

## 1. Integrated desktop lifecycle

Exercise:

~~~text
App launch
→ set Auto Start enabled
→ full Quit
→ reopen
→ exactly one Auto Start
→ authoritative Ready
→ close/hide
→ background continues
→ Tray Show
→ Stop
→ no same-process Auto Start
→ explicit Start
→ Quit
~~~

Assert:

- one managed generation;
- no Windows login startup mechanism;
- no false Ready;
- no orphan.

## 2. Notification/diagnostics integration

Exercise packaged/host-level where possible:

- Pending approval notification generation;
- dedup;
- stale/resolved navigation truth;
- notification failure fallback;
- disconnect incident;
- recovery notification;
- Diagnostics normal/fault/recovery.

Do not claim OS visual Manual acceptance here if automation cannot observe it.

## 3. Core Tool authority regression

Real Runtime MCP:

- read_file success;
- grep literal/regex success;
- ASK allow;
- ASK deny;
- Policy DENY;
- disabled/unavailable Tool;
- Workspace escape sentinel;
- invalid arguments;
- unknown Tool;
- cancellation;
- product timeout where applicable.

Assert exact Workspace/Policy/Approval/Execution authority.

## 4. Accepted M09 Tool catalog regression

Load decision-set-v1 or accepted successor identity exactly.

For each candidate:

### ADMIT

Verify:

- present in ToolRegistry/config/policy/MCP;
- schema hash exact;
- normal path;
- DENY/ASK as applicable;
- Workspace boundary;
- destructive/dual-path constraints;
- final filesystem state.

### REJECT / DEFER

Verify:

- absent from registry;
- absent from config whitelist/defaults;
- absent from MCP tools/list;
- no dispatch implementation reachable.

M11 does not change verdicts.

## 5. Evidence consistency

For a controlled set of real requests, cross-check exact:

~~~text
runtime_id
request_id
tool
policy decision
approval outcome
execution started
terminal/incomplete outcome
History
Metrics
Trace
~~~

No timestamp-nearest join.

## 6. Metrics truth

Verify:

- calls;
- executed success/failure;
- DENY;
- cancelled;
- timeout;
- executor failure;
- average;
- nearest-rank P95;
- sample size;
- UNKNOWN/INCOMPLETE;
- approval wait;
- execution duration;
- wall time.

Do not recompute alternative formulas in UI test code.

## 7. Trace truth

Verify:

- received;
- resolution;
- Workspace;
- Policy;
- Approval if applicable;
- execution if applicable;
- terminal/incomplete.

Verify absent stages remain absent/UNKNOWN rather than fabricated.

No hidden reasoning/provider span.

## 8. Crash/restart

Inject controlled Runtime/Tunnel failure.

Assert:

- no stale Ready;
- History persists;
- Metrics/Trace do not fabricate terminal success;
- old Pending/Grant/Permit does not restore;
- next generation gets fresh authority;
- no orphan.

## 9. Privacy/security sentinels

Include synthetic:

- secret-like values;
- physical paths;
- oversized raw payload;
- environment-like strings.

Assert they do not leak into durable evidence/external error surfaces beyond accepted safe projection.

### Done

One final candidate passes the integrated product/security/evidence behavior before quality gates.

---

# T4 — Bind or Re-run F05, F12 and F13 on the Final Candidate

**AC:** M11-AC14 through AC17, AC20
**Depends on:** T1–T3
**Primary areas:** accepted M02/M04/M05/M09/M10 runners and evidence identity tooling.

## 1. F05 equivalence decision

Compare final candidate against accepted M02/M06 performance identity.

If EXACTLY_EQUIVALENT:

- bind accepted evidence;
- record equivalence fields.

Otherwise:

- execute exact frozen M02 benchmark;
- do not change samples/threshold/readiness semantics.

Required outcome:

~~~text
PASS
~~~

Any FAIL/BLOCKED/INCONCLUSIVE blocks Development Complete unless the only blocker is explicit execution-time authorization not yet granted and the handoff reports it truthfully.

## 2. F12 final Tool-catalog identity

Assert exact match among:

- production Tool catalog;
- accepted M09 decision set;
- evaluated schemas;
- M05 five-tool control identities where unchanged.

Any mismatch requires requalification before release.

## 3. F12 equivalence/rerun

If equivalent to accepted M09 final cumulative regression:

- bind result.

Otherwise:

- run the exact accepted M09/M05 quality path;
- real networked/paid Model requires explicit user authorization;
- use exact requested/resolved Model rules;
- preserve sample/repetition/drift/threshold/security semantics.

Forbidden:

- scripted adapter as formal substitute;
- threshold lowering;
- cherry-pick;
- different model without frozen drift handling.

## 4. F13 identity comparison

Compare final candidate with accepted M10:

- Runtime MCP hash;
- rmcp version;
- MCP revision;
- transport;
- Tool catalog/schema;
- error mapping;
- cancellation;
- runner identity.

## 5. Official conformance rerun when required

If not exactly equivalent:

- run the exact accepted M10 official conformance command;
- preserve actual result artifacts outside milestone docs;
- require zero blocking/wire-schema failure;
- do not use expected-failure suppression.

## 6. Harness-specific MCP security

Run independently from official conformance:

- Workspace;
- Policy;
- Approval;
- disabled Tool;
- invalid args;
- unknown Tool;
- cancellation;
- safe errors.

Label as Harness-specific evidence.

## 7. Controlled client interoperability

Run accepted M10 independent raw/SDK client checks as affected by candidate changes.

Do not call them External ChatGPT.

### Done

The final RC has candidate-valid performance, Agent quality and protocol-conformance evidence without mixing evidence classes.

---

# T5 — Prepare the Full Release Candidate, Release Truth and External-chain Handoff

**AC:** M11-AC02 through AC07, AC18, AC21 through AC24
**Depends on:** T1–T4
**Primary areas:** delivery scripts, artifact metadata, release-note/changelog existing location if present.

## 1. Freeze final candidate

After product-impacting defects are closed:

- clean checkout;
- candidate commit;
- build Portable;
- build installer;
- calculate identity manifest;
- verify source_dirty=false.

Any later product change invalidates this candidate.

## 2. Final feature matrix dry-run

Populate execution-time matrix structurally.

Before formal validation:

- mandatory Integration/E2E/Manual rows remain NOT_EXECUTED;
- F09/F15 remain DEFERRED;
- no acceptance verdict is emitted.

The gate should refuse premature V4 ACCEPTED.

## 3. Release Notes draft

Prepare truthful draft containing:

- delivered P0/P1 scope;
- M09 ADMIT/REJECT/DEFER summary;
- exact MCP revision;
- F05/F12/F13 result fields;
- F09 DEFERRED;
- F15 DEFERRED;
- known limitations.

Before final Manual:

~~~text
Real External ChatGPT = NOT_EXECUTED
V4 Full Gate = BLOCKED
~~~

Do not write a seventh M11 milestone doc.

## 4. External ChatGPT handoff data

Prepare the exact final-candidate facts needed by Manual acceptance:

- package path/hash;
- expected Tool catalog;
- safe disposable Workspace setup;
- approved Tool/policy scenarios;
- request correlation fields;
- expected History/Metrics/Trace surfaces;
- cleanup procedure.

Do not execute External ChatGPT without explicit user authorization.

## 5. Windows Manual handoff

Prepare final candidate for actual user/QA observation of:

- installer;
- upgrade;
- Auto Start;
- Tray;
- notifications;
- approval navigation;
- History/Metrics/Trace;
- rollback/uninstall;
- no residue.

This is preparation, not a PASS claim.

### Done

The Full RC is frozen and ready for independent validation with no premature acceptance wording.

---

# T6 — Run Full Development Regression and Produce the M11 Development Complete Handoff

**AC:** M11-AC01 through AC24
**Depends on:** T1–T5

## 1. Rust repository checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
~~~

Run affected targeted suites first if needed, then the required workspace gate.

## 2. Frontend checks

~~~text
cd ui
npm run typecheck
npm test
npm run build
~~~

## 3. Protocol/config checks

Verify:

~~~text
AppConfig schema = 3
Control Protocol = v1
no unintended major wire bump
M10 MCP claim unchanged
~~~

unless an explicitly approved upstream change says otherwise.

## 4. Release-gate checks

Verify:

- M06–M10 accepted/frozen;
- 43 mandatory canonical AC identifiers in matrix;
- F01–F15 exactly once;
- Shared Evidence section;
- F09/F15 DEFERRED;
- no mandatory DEFERRED;
- no early V4 accepted verdict.

## 5. Final package lifecycle development run

Repeat against the final candidate:

- Portable smoke;
- installer smoke;
- clean install;
- upgrade fixture;
- DB migration fixture;
- rollback fixture;
- uninstall fixture;
- reinstall;
- no orphan.

## 6. Final integrated regression

Repeat final candidate:

- Auto Start lifecycle;
- Tray/notifications/Diagnostics;
- read/grep/ASK/DENY;
- accepted M09 ADMIT Tool representative cases;
- History/Metrics/Trace consistency;
- crash/restart;
- privacy/security sentinels.

## 7. Quality gates

Confirm final status:

~~~text
F05 = PASS/equivalent
F12 = PASS/equivalent
F13 official conformance = PASS/equivalent
Harness MCP/security = PASS
~~~

If a required paid Model run is authorization-blocked, report M11 Development BLOCKED rather than inventing PASS.

## 8. Layout/governance

Run:

- ui/scripts/check-repository-layout.ps1;
- ui/scripts/check-milestone-layout.ps1 after all six M11 docs exist.

No extra milestone files.

## 9. Fresh delivery artifact

Use existing delivery script with:

~~~text
M11-v4-full-release-integrated-acceptance
~~~

Do not overwrite prior candidate.

## 10. Development Complete handoff

Report in conversation:

~~~text
M11 Development Complete
candidate SHA:
source dirty:
product version:

Portable path/hash:
Installer hash:
installed UI/Runtime/Tunnel hashes:

AppConfig schema:
Control Protocol:
evidence DB schema/migration:
Tool catalog hash:
M09 decision-set hash:

M06 accepted identity:
M07 accepted identity:
M08 accepted identity:
M09 accepted identity:
M10 accepted identity:

F05 equivalence/rerun status:
F12 equivalence/rerun status:
F13 official conformance equivalence/rerun status:
Harness security status:

F09: DEFERRED
F15: DEFERRED

Development regression:
Package lifecycle development checks:
Security/privacy:
Owned orphan count:

Formal M11 Integration: NOT EXECUTED
Formal M11 E2E: NOT EXECUTED
Real External ChatGPT: NOT EXECUTED
Formal M11 Manual: NOT EXECUTED
V4 — ACCEPTED / FROZEN: NO
~~~

Do not mark final acceptance during T6.

### Done

One final Full RC is ready for M11 independent Integration/E2E/Manual validation.

---

# 2. Task to AC to Reserved Validation Traceability

| Task | M11 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01–05/23/24 | M11-INT-01/02/03, M11-E2E-01, M11-MAN-01 |
| T2 | AC06/07/09/13/22 | M11-INT-04/05/06/11/15, M11-E2E-01/02, M11-MAN-01/02/06 |
| T3 | AC08–13/18–20 | M11-INT-07/08/09/10/11/12/13/14/15, M11-E2E-02/03, M11-MAN-02/03/04 |
| T4 | AC14–17/20 | M11-INT-16/17/18, M11-E2E-03/04, M11-MAN-04 |
| T5 | AC02–07/18/21–24 | M11-INT-19/20, M11-E2E-05, M11-MAN-01/05/06 |
| T6 | AC01–AC24 | M11-INT-01 through M11-INT-20, M11-E2E-01 through M11-E2E-05, M11-MAN-01 through M11-MAN-06 |

---

# 3. Development Handoff

After T6:

~~~text
M11 Development Complete
→ independent M11 Integration
→ packaged M11 E2E
→ real External ChatGPT + Windows Manual Acceptance
→ final matrix review
→ final Release Notes
→ V4 — ACCEPTED / FROZEN
~~~

If any mandatory Full/P1 requirement fails, M11 remains FAIL/BLOCKED/INCONCLUSIVE even if the earlier M06 Core remains accepted.
