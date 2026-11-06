# M11 PLAN — V4 Full Release Integrated Acceptance

> Input: SPEC.md
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M06+M07+M08+M09+M10 ACCEPTED / FROZEN AND M11 VALIDATION DOCS ARE FROZEN
> Scope: final V4 Full Release Gate only
> Mandatory scope: Shared Evidence + F01–F08 + F10–F14
> Optional disposition: F09 DEFERRED / F15 DEFERRED
> Release verdict owner: M11 only
> Security rule: no release-only bypass around Runtime Workspace / Policy / Approval / Execution authority.
> Evidence rule: official MCP conformance, Harness security semantics, controlled Eval and real External ChatGPT remain separate evidence classes.
> External rule: real External ChatGPT, paid/networked Model runs, destructive install/uninstall/upgrade on non-disposable state and active instance restart require explicit execution-time authorization.

---

# 1. Delivery Strategy

M11 is an integrated release gate, not a feature milestone.

~~~text
M06/M07/M08/M09/M10 ACCEPTED / FROZEN
        ↓
M11 six docs frozen
        ↓
T1 Full RC identity + 15-feature matrix + equivalence gate
        ↓
T2 package/install/upgrade/rollback/uninstall harness
        ↓
T3 Core + P1 integrated lifecycle/security/evidence regression
        ↓
T4 F05 + F12 + F13 final-candidate quality requalification
        ↓
T5 full release packaging + release truth + external-chain preparation
        ↓
T6 final development regression + Full RC handoff
        ↓
Formal Integration
        ↓
Automated packaged E2E
        ↓
Real External ChatGPT + Windows Manual
        ↓
V4 — ACCEPTED / FROZEN
~~~

M11 does not re-open feature design.

---

# 2. Planning Principles

1. Reuse accepted milestone semantics exactly.
2. Never turn historical PASS into automatic final-candidate PASS.
3. Freeze one Full RC identity before formal validation.
4. Compare evidence identities mechanically.
5. Rerun only the exact frozen upstream gate when equivalence fails.
6. Never weaken F05/F12/M09/M10 thresholds or semantics.
7. Keep F09/F15 DEFERRED and absent from release claims.
8. Preserve AppConfig schema v3 unless upstream change control explicitly says otherwise.
9. Preserve Control Protocol v1 additive compatibility.
10. Keep one canonical evidence store and exact request identity.
11. No new top-level directory.
12. No new milestone report files.
13. Real External ChatGPT is Manual/final acceptance evidence, never simulated by an internal client.
14. M11 development can add release-only verification helpers, but must not add product shortcuts.
15. A defect fix creates a new candidate and invalidates affected evidence.

---

# 3. Expected Landing Zones

Prefer existing locations:

~~~text
ui/scripts/
runtime/tests/
ui/src-tauri/
protocol/tests/
fixtures/v4/
artifacts/
~~~

Do not create repository-level release-gate, m11-runner, full-release or reports directories.

Any release-only fixture belongs in the existing fixture hierarchy and must contain no user secrets or real production state.

---

# 4. Phase A — Build the Full Release Identity Gate

## 4.1 Extend existing candidate identity tooling

M11 must be able to freeze:

- source SHA/dirty state;
- source-state hash;
- product version;
- Portable and installer hashes;
- installed payload hashes;
- AppConfig schema/revision/profile;
- Control Protocol schema/hash;
- evidence DB schema/migration identity;
- final Tool catalog and schema hashes;
- M09 decision set;
- M02 performance contract identity;
- M04/M05/M09 Eval identities;
- M08 Metrics/Trace contract identity;
- M10 MCP/runner identity;
- F09/F15 disposition.

Prefer extending the M06 candidate/gate tooling rather than creating a parallel release identity implementation.

## 4.2 Fifteen-feature matrix generator/checker

Generate or validate one execution-time matrix:

~~~text
F01 ... F15
+ Shared Evidence section
~~~

Mandatory row expectations:

~~~text
F01–F08 = PASS
F09 = DEFERRED
F10–F14 = PASS
F15 = DEFERRED
~~~

The checker fails on:

- missing feature;
- duplicate feature;
- wrong priority;
- missing canonical AC;
- mandatory DEFERRED;
- optional PASS without actual optional evidence;
- inconsistent milestone owner;
- candidate/evidence mismatch.

## 4.3 Upstream acceptance checker

Read accepted milestone handoff/evidence truth for:

~~~text
M06
M07
M08
M09
M10
~~~

Fail closed if any is not ACCEPTED / FROZEN.

Do not infer acceptance from code existence or Development Complete.

## 4.4 Evidence equivalence engine

Classify each upstream evidence set:

~~~text
EXACTLY_EQUIVALENT
REQUIRES_TARGETED_RERUN
REQUIRES_FULL_RERUN
INVALID_BLOCKER
~~~

The decision must be based on versioned identity keys, not prose.

Exit: the final release gate knows exactly what historical evidence may be reused and what must rerun.

---

# 5. Phase B — Package and Lifecycle Harness

## 5.1 Build one clean Full RC

From clean source:

- build Portable;
- build NSIS;
- record all payload hashes;
- verify UI/Runtime/Tunnel binaries derive from the same source candidate.

## 5.2 Clean-install fixture

Automate in disposable Windows test state where feasible.

Verify:

- install succeeds;
- app launches;
- config safe defaults;
- Auto Start default OFF;
- no test secret;
- no stale evidence DB;
- Runtime/Tunnel can start normally.

## 5.3 Upgrade matrix

Prepare controlled fixture states for:

- v2 config requiring v2→v3 migration;
- existing v3 with explicit desktop preferences;
- accepted prior-release Workspace/Tunnel/Proxy/SecretReference;
- accepted Tool/Policy/Bash fields;
- M09 migration/default behavior;
- evidence DB old/current schema states required by M03/M08.

After upgrade assert:

- no lost user fields;
- no plaintext secret;
- no accidental ADMIT Tool enablement;
- no rejected/deferred Tool exposure;
- History remains available;
- Metrics/Trace migration semantics are truthful;
- no stale live authorization.

## 5.4 Rollback

Use the accepted M06/M08 rollback model.

If schema incompatibility exists:

- restore accepted backup/compatibility path;
- do not invent destructive DB downgrade;
- preserve user Workspace/config/secret references;
- verify previous accepted release launches.

## 5.5 Uninstall

Automate disposable install uninstall where safe.

Assert:

- owned processes stop;
- installed binaries/integration removed;
- external Workspace remains untouched;
- user-data handling matches current installer contract;
- test-only state removed;
- reinstall works.

Do not define a new user-data deletion policy.

Exit: one repeatable package lifecycle harness exists for formal validation.

---

# 6. Phase C — Integrated Core + P1 Regression

## 6.1 Desktop lifecycle

Cover one final candidate flow:

~~~text
launch
→ optionally enable Auto Start
→ full Quit
→ reopen
→ one Auto Start
→ authoritative Ready
→ hide window
→ Tray Show/Start/Stop/state/Pending count
→ Quit
~~~

Verify:

- no duplicate Harness generation;
- no login startup mechanism;
- no same-process auto-restart after Stop;
- no orphan.

## 6.2 Notifications and Diagnostics

Exercise:

- Approval notification;
- resolved/stale click behavior;
- disconnect incident notification;
- recovery notification;
- dedup;
- notification failure fallback;
- Diagnostics normal/fault/recovery;
- no secret/raw arg/path leak.

## 6.3 Core Tool authority

Real Runtime MCP:

- read_file ALLOW;
- grep ALLOW;
- ASK allow;
- ASK deny;
- Policy DENY;
- disabled/unavailable direct call;
- Workspace escape;
- invalid args;
- unknown Tool;
- cancellation;
- product timeout where applicable.

No hidden shell workaround.

## 6.4 Final M09 catalog

Load accepted decision set.

For each candidate:

- verify verdict;
- verify catalog presence/absence;
- verify schema hash;
- for each ADMIT Tool run representative normal and negative security case;
- verify no REJECT/DEFER dispatch path.

## 6.5 History / Metrics / Trace consistency

For the same exact requests, compare:

- Activity;
- Approval History;
- History summary;
- Metrics classification;
- Request Trace stages;
- execution-start truth;
- outcome;
- timing class.

No fuzzy timestamp join.

## 6.6 Crash/recovery

Inject controlled:

- Runtime crash;
- Tunnel disconnect;
- restart;
- evidence writer degraded state where existing fixtures support it.

Verify:

- no fake success;
- no stale Ready;
- no authorization restore;
- incomplete/unknown metrics/trace truthful;
- no orphan.

Exit: the final Full RC is one coherent product rather than independently passing feature islands.

---

# 7. Phase D — F05 Final Performance Gate

## 7.1 Evaluate equivalence

Compare final candidate to accepted M02/M06 performance identity.

If exact, reuse formal quantitative evidence.

If any relevant hash/config/environment differs, rerun the exact frozen M02 benchmark.

## 7.2 Rerun requirements

Use the accepted M02:

- benchmark harness;
- cold/warm definitions;
- sample size;
- stage timings;
- P50/P95;
- failure rate;
- threshold;
- readiness semantics.

No new threshold.

## 7.3 Final classification

Possible:

~~~text
PASS
FAIL
BLOCKED
INCONCLUSIVE
~~~

Do not collapse BLOCKED/INCONCLUSIVE into PASS.

Exit: F05 evidence corresponds to the M11 candidate.

---

# 8. Phase E — F12 Final Tool-catalog Quality Gate

## 8.1 Validate catalog identity

The final candidate must exactly match:

- M05 accepted control schemas where unchanged;
- M09 accepted decision set;
- actual production Tool schemas;
- Eval candidate schemas.

## 8.2 Determine requalification need

If accepted M09 cumulative regression is exactly equivalent to M11 final candidate, bind it.

Otherwise execute the already-frozen M09/M05 evaluation path.

Do not create an M11-specific easier scorer.

## 8.3 Paid/network authorization

If a real Model rerun is required, stop at the authorization boundary until the user explicitly permits it.

Without permission:

~~~text
M11 quality gate = BLOCKED
~~~

Do not change Model or substitute scripted results for formal evidence.

## 8.4 Security and drift

Preserve:

- resolved model identity;
- drift rules;
- sample validity;
- zero security violation;
- exact thresholds;
- no cherry-pick rerun.

Exit: F12 quality evidence corresponds to final Tool catalog.

---

# 9. Phase F — F13 Final Conformance Gate

## 9.1 Preserve M10 claim

Use exact accepted:

- MCP revision;
- rmcp version;
- official runner commit/package;
- standalone client version;
- result-classification semantics.

M11 does not upgrade protocol.

## 9.2 Equivalence key

Compare final:

- Runtime MCP hash;
- Tool catalog/schema;
- transport code;
- error mapping;
- cancellation;
- dependency lock.

## 9.3 Official rerun when needed

If any wire-impacting identity differs:

- run the accepted M10 official server command/suite;
- preserve raw checks/wire-schema results;
- require zero blocking failure.

Do not use expected-failure suppression.

## 9.4 Harness-specific security

Run separately.

Never describe it as official conformance.

Exit: final RC has both protocol and product-security evidence.

---

# 10. Phase G — Release Truth and Optional Disposition

## 10.1 Freeze P2 absence

Verify final package has no product claim that F09/F15 are delivered.

If UI/API scaffolding exists, it must not surface as a supported Full feature.

## 10.2 Release Notes source

Use the repository's established release-note/changelog location if one exists at implementation time.

If no established committed release-note location exists, prepare the final release content as an artifact/conversation output rather than inventing a seventh M11 doc.

## 10.3 Required release facts

State:

- candidate/source;
- mandatory delivered features;
- M09 decisions;
- F05 result;
- F12 result;
- F13 protocol/conformance;
- External ChatGPT result after Manual;
- install/upgrade/rollback/uninstall result;
- F09 DEFERRED;
- F15 DEFERRED;
- known limitations.

Exit: release language cannot outrun test truth.

---

# 11. Phase H — Development Candidate Handoff

## 11.1 Full repository checks

Run:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Plus all affected milestone-specific development tests.

## 11.2 Governance checks

Run:

- repository layout checker;
- milestone layout checker after six M11 docs exist;
- no new top-level dir;
- no extra milestone file;
- no generated secret/test residue committed.

## 11.3 Build fresh candidate

Use:

~~~text
ui/scripts/deliver-milestone.ps1 -Milestone M11-v4-full-release-integrated-acceptance
~~~

or the exact current accepted delivery-script syntax if it changed legitimately before implementation.

Do not overwrite old artifact candidates.

## 11.4 Development smoke

Portable/installed product smoke:

- launch;
- Start/Auto Start;
- Ready;
- tools/list;
- safe Tool;
- History/Metrics/Trace;
- Quit;
- no orphan.

This is not formal E2E.

Exit: one Full RC is ready for independent validation.

---

# 12. Defect Closure Rules

If formal or development testing finds a defect:

1. classify owner milestone/feature;
2. fix minimally in product code;
3. create new candidate identity;
4. rerun impacted unit/contract tests;
5. rerun impacted upstream gate using its frozen semantics;
6. rerun M11 affected formal tests.

Never repair a failing gate by editing frozen SPEC/PLAN/TASKS/test plans.

If a defect proves the frozen requirement itself unimplementable or contradictory, stop and escalate for explicit document change authorization.

---

# 13. Evidence Reuse Examples

## Example A — UI-only text change after F12

If Runtime/Tool schemas/Eval inputs unchanged:

- F12 evidence may remain equivalent;
- UI/manual tests rerun as affected.

## Example B — M09 Tool schema fix

Requires:

- Tool schema hash change;
- M09/F12 quality requalification;
- MCP tools/list/schema checks;
- F13 conformance rerun if wire affected;
- real ChatGPT final Tool list rerun.

## Example C — Tauri Auto Start fix

Requires:

- F02 lifecycle tests;
- F01/F05/F06 affected regression;
- packaged E2E/Manual;
- F05 quantitative rerun if startup path/timing identity changes.

## Example D — evidence DB migration fix

Requires:

- M03/M08 migration/history/metrics/trace tests;
- upgrade/rollback;
- crash/recovery;
- no authorization restore.

---

# 14. Rollback Strategy

M11 introduces no new product schema by design.

Release-gate helper changes are non-authoritative and removable.

Product defect rollback:

- revert defecting product change;
- restore previous accepted code/dependency pair;
- use accepted config/DB rollback procedure;
- rebuild candidate;
- repeat impacted gates.

Never use rollback as a reason to erase user Workspace or plaintext-export secrets.

---

# 15. Development Verification Minimum

Before Development Complete:

- upstream accepted status check;
- candidate identity check;
- 15-feature matrix structural check;
- optional F09/F15 DEFERRED check;
- clean install smoke;
- upgrade fixture smoke;
- rollback fixture smoke;
- uninstall fixture smoke;
- Core final-candidate regression;
- F02 regression;
- F08/F14 evidence consistency;
- M09 catalog/decision-set consistency;
- F05 equivalence or rerun;
- F12 equivalence or authorized rerun;
- F13 equivalence or official rerun;
- security/privacy sentinels;
- no orphan;
- fresh Portable + installer;
- release truth check.

---

# 16. Development Completion Gate

M11 Development Complete requires:

- T1–T6 complete;
- one clean Full RC exists;
- mandatory matrix structure complete;
- upstream acceptance verified;
- package lifecycle development checks pass;
- integrated Core+P1 development regression pass;
- F05/F12/F13 final-candidate evidence status known and valid or truthfully BLOCKED where execution-time authorization is missing;
- F09/F15 still DEFERRED;
- no known security/privacy/orphan blocker;
- Formal Integration/E2E/Manual remain NOT EXECUTED unless actually run.

Development Complete does not authorize:

~~~text
V4 — ACCEPTED / FROZEN
~~~

Only the full validation lifecycle can do that.
