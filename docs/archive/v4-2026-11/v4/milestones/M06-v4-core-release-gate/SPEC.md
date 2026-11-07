# M06 SPEC — V4 Core Release Gate

> 布局与验证生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。本目录启动阶段只创建 SPEC / PLAN / TASKS；后续补齐 Integration / E2E / Manual 三份验证计划，最终严格保持六文件布局。不得为执行结果新增 milestone report、evidence index、截图目录或第七份文档。  
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M01–M05 ACCEPTED / FROZEN AND M06 SIX DOCS ARE FROZEN  
> Milestone: M06 — V4 Core Release Gate  
> Scope: V4 Core 综合验收；不新增 P1/P2 产品范围  
> Roadmap dependencies: M01 + M02 + M03 + M04 + M05 all ACCEPTED / FROZEN  
> Upstream contract baseline: M00 implementation-frozen PRD / Architecture / Roadmap  
> Current main observed before this document: a19113fa6d9c139451637233fc169949a364e75c  
> Current M05 state supplied by user: Development Complete. This permits M06 documentation work only; it does not satisfy the M06 implementation/acceptance dependency.  
> Release semantics: M06 may conclude only V4 Core — ACCEPTED. It must not claim V4 — ACCEPTED / FROZEN, which belongs to M11.

---

# 1. Objective

M06 is the release gate for the V4 P0/Core slice. It does not introduce a new end-user feature.

One fixed Core Release Candidate must simultaneously prove:

- Shared Evidence Contract;
- F01 System Tray;
- F03 Approval Notifications;
- F04 Disconnect Notifications;
- F05 Startup Performance;
- F06 One-click Diagnostics;
- F07 SQLite History;
- F10 grep Tool;
- F12 Controlled Agent Evaluation;
- affected V3 Workspace / Tool Toggle / Policy / Approval / SessionGrant / Bash / SecretStore / lifecycle behavior;
- real External ChatGPT product-chain acceptance.

A collection of individually successful M01–M05 historical candidates is not sufficient. The final Core verdict must bind to one M06 candidate identity.

---

# 2. Canonical Core Scope

M06 rolls up exactly these upstream IDs.

## Shared Evidence

- SEC-AC01
- SEC-AC02
- SEC-AC03

## F01

- F01-AC01
- F01-AC02
- F01-AC03

## F03

- F03-AC01
- F03-AC02
- F03-AC03

## F04

- F04-AC01
- F04-AC02
- F04-AC03

## F05

- F05-AC01
- F05-AC02
- F05-AC03

## F06

- F06-AC01
- F06-AC02
- F06-AC03

## F07

- F07-AC01
- F07-AC02
- F07-AC03

## F10

- F10-AC01
- F10-AC02
- F10-AC03

## F12

- F12-AC01
- F12-AC02
- F12-AC03
- F12-AC04

Canonical Core count is 28: 3 Shared Evidence + 21 from seven three-AC P0 features + 4 F12.

No canonical Core AC may be omitted, merged away or downgraded to advisory.

| Scope | Primary evidence owner |
| --- | --- |
| F01 / F03 / F04 | M01 |
| F05 / F06 | M02 |
| Shared Evidence / F07 | M03 |
| F12 baseline | M04 |
| F10 + F12 candidate regression | M05 |
| Core integrated release verdict | M06 |

M06 consumes upstream accepted evidence but also executes final-candidate integrated roll-up tests.

---

# 3. Architecture Invariants

M06 preserves:

- A01 four-layer React / Tauri / Runtime / Tunnel boundary;
- A02 Tauri ownership of Tray/notification/window lifecycle;
- A03 packaged Windows Tray/notification implementation;
- A04 Runtime AuditEvent as canonical evidence source;
- A05 Runtime-only SQLite writer during normal running;
- A06 evidence-store has no authorization ownership;
- A07 WAL/migration/bounded writer/retention failure remains observational;
- A08 runtime_id + request_id as canonical request identity;
- A10 controlled Eval remains separate from passive telemetry;
- A11 Tauri monotonic startup timing;
- A12 grep as WorkspaceRead through normal execution pipeline;
- A15 v2→v3 config migration and additive Control Protocol v1;
- A16 raw-sample P95 semantics;
- A17 History/Eval remain non-authoritative.

M06 must not create release-only shortcuts around these boundaries.

---

# 4. Non-goals

M06 does not:

- make F02 Auto Start a Core requirement;
- add F08 Metrics;
- add F09 Token estimation;
- add F11 file Tools;
- add F13 official MCP conformance;
- add F14 Request Trace;
- add F15 Policy Simulator;
- change M02 performance thresholds;
- change M04/M05 dataset, scorer, sample or regression thresholds;
- redefine Workspace / Policy / Approval / SessionGrant / Bash / SecretStore;
- create protocol/v2;
- create a second evidence store;
- add a top-level repository directory;
- treat partially existing P1/P2 code as Core-accepted;
- treat missing external evidence as PASS.

P1/P2 capabilities present in the binary remain OUT_OF_CORE / NOT_CORE_ACCEPTED unless accepted by their owning milestone.

---

# 5. Dependency Gate

Formal M06 implementation requires:

~~~text
M01 = ACCEPTED / FROZEN
M02 = ACCEPTED / FROZEN
M03 = ACCEPTED / FROZEN
M04 = ACCEPTED / FROZEN
M05 = ACCEPTED / FROZEN
M06 six documents = frozen
~~~

The gate is fail-closed.

Development Complete for an upstream milestone is not enough.

A prior FAIL/BLOCKED/INCONCLUSIVE must be resolved by its owning milestone or an explicitly approved upstream contract revision. M06 cannot reinterpret it.

---

# 6. Core Release Candidate Identity

Before formal M06 Integration, freeze one candidate.

Required fields:

~~~text
candidate_id
git_commit_sha
branch
source_dirty
source_state_sha256
product_version
windows_environment_class

portable_package_sha256
portable_ui_sha256
portable_runtime_sha256
portable_tunnel_sha256

installer_sha256
installed_ui_sha256
installed_runtime_sha256
installed_tunnel_sha256

control_protocol_revision
generated_protocol_schema_hash

config_schema_version
effective_config_revision
config_profile_hash

evidence_schema_version
evidence_schema_checksum

effective_tool_catalog_hash
per_tool_schema_hashes

accepted_m04_baseline_identity
m04_dataset_hash
m04_system_prompt_hash
m04_scorer_hash
m05_candidate_profile_hash
m05_tool_schema_hash
adapter_id_version
requested_model
resolved_model
~~~

The candidate must reproduce from a clean checkout.

---

# 7. Single-candidate Evidence Rule

Formal M06 Integration, E2E and Manual acceptance target the same final candidate except explicit upgrade/rollback comparison steps.

After candidate lock:

## Release-only change

A non-bundled docs/release-test script change may retain product evidence only if:

- App/Runtime/Tunnel hashes are unchanged;
- protocol/config/evidence/tool identities are unchanged;
- Eval dataset/scorer/profile identities are unchanged;
- gate tooling verifies equivalence.

## Product-impacting change

Any change to product UI/Tauri/Runtime/protocol/Tunnel/config/history/tool/eval/build content creates a new candidate and invalidates affected evidence.

No mixing of best results from different product builds.

---

# 8. Upstream Evidence Reuse

Upstream acceptance is a prerequisite, not automatic final-RC proof.

## F05 equivalence key

M02 quantitative evidence is reusable only if all match:

- startup-relevant Tauri/App binary hash;
- Runtime hash;
- Tunnel hash;
- startup config profile;
- benchmark harness/version;
- environment classification;
- frozen M02 threshold contract.

Otherwise rerun the frozen M02 benchmark on M06.

M06 never changes M02 thresholds.

## F12 equivalence key

M05 formal Eval evidence is reusable only if all match:

- Runtime hash;
- Tool catalog/schema hash;
- M04 dataset hash;
- system prompt hash;
- scorer hash/version;
- M05 candidate profile hash;
- adapter ID/version;
- requested/resolved Model;
- formal validity contract.

Otherwise formal M05 regression must rerun. Paid/networked execution requires user authorization.

## History/evidence equivalence key

Reuse requires matching Runtime/evidence-store identity, Tauri History read path, evidence schema/checksum and history config semantics.

## Notification/lifecycle equivalence key

Reuse requires matching relevant Tauri Host and packaged Windows identity.

Packaged notification behavior cannot be proven solely by unit tests.

---

# 9. Core Product Acceptance Matrix

Formal M06 evidence must contain one matrix with:

- all 28 canonical Core AC rows;
- mandatory V3 regression rows;
- package/upgrade/rollback rows;
- External ChatGPT row;
- final candidate identity.

Required columns:

~~~text
scope_id
owner_milestone
upstream_verdict
m06_required_recheck
candidate_equivalence_key
integration_status
e2e_status
manual_status
final_status
evidence_reference
blocker
~~~

Allowed final statuses:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE
- NOT_VERIFIED

Every mandatory Core row must end PASS.

The execution matrix is acceptance output, not a seventh milestone file.

---

# 10. Release-blocker Rules

Automatic Core blockers include:

- any M01–M05 milestone not ACCEPTED / FROZEN;
- any Core canonical AC not PASS;
- M02 frozen performance gate not met;
- F12 formal baseline/candidate invalid or failed;
- real External ChatGPT Core chain not executed;
- Workspace escape;
- Policy/Approval bypass;
- stale Approval/Grant/Permit restoration;
- secret leakage;
- History becoming authorization truth;
- owned orphan Runtime/Tunnel after Quit;
- candidate/package/hash mismatch;
- installer/Portable payload mismatch;
- config upgrade data loss;
- unapproved irreversible migration;
- unverified required Windows Tray/Toast behavior;
- Core crash/recovery failure;
- any mandatory M06 test FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED.

A non-Core cosmetic/P1/P2 issue may be listed as a known issue only if it does not invalidate Core behavior, security, privacy, data integrity or package integrity.

M06 has no waiver mechanism for Core ACs.

---

# 11. Package Contract

M06 validates both:

1. Portable package;
2. Windows NSIS installer.

Both come from the same clean source candidate.

Required:

- source_dirty=false;
- distribution build_sha matches candidate;
- staged Runtime/Tunnel hashes match;
- Portable verification PASS;
- NSIS verification PASS;
- installed payload matches candidate;
- no test secret/temp DB/temp config unintentionally ships.

Use existing ui/scripts paths; no new top-level release project.

---

# 12. Upgrade Contract

M06 validates:

- representative v2 config;
- representative pre-Core v3 config;
- SecretReference;
- Workspace;
- Tool/Policy settings;
- desktop prefs;
- History config;
- M05 grep upgrade semantics;
- existing M03-compatible History DB.

Required:

- v2→v3 migration through accepted code;
- user values preserved;
- SecretStore remains references, not raw values;
- host-only settings do not create incorrect Runtime restart debt;
- History remains readable or follows accepted M03 migration rules;
- old Pending/SessionGrant/ExecutionPermit is not restored;
- invalid config is not silently reset to defaults.

---

# 13. Rollback Contract

Rollback target is the designated previous accepted release/candidate.

Freeze process:

1. cleanly Quit M06;
2. preserve current user data;
3. restore pre-upgrade config backup if the older binary requires the older shape;
4. launch previous accepted package;
5. preserve compatible History rather than deleting it;
6. surface incompatibility instead of destructive auto-reset;
7. verify no owned orphan and no stale authorization.

Rollback must not require plaintext secret reconstruction.

---

# 14. Integrated Core Lifecycle

Final-candidate evidence must cover this continuous product story:

~~~text
launch
→ Start Harness
→ authoritative Ready
→ close main window
→ remain alive in Tray
→ Tool request while hidden
→ read/grep
→ ASK
→ approval notification
→ resolve current Approval
→ execute
→ DENY
→ disconnect/degraded incident
→ notification
→ Diagnostics
→ recover
→ query History
→ restart Runtime/App
→ History persists
→ old authorization does not
→ Quit
→ owned Runtime/Tunnel = 0
~~~

The final product claim must prove the story on one candidate, even if automation is split into several cases.

---

# 15. F01 / F03 / F04 Roll-up

Final candidate proves:

## F01

- Close hides rather than stops/quits by default;
- Runtime/MCP/Tunnel/Approval continue;
- Tray Show/Start/Stop/state/Pending count/Quit are truthful;
- Quit cleans managed processes.

## F03

- one new Pending sends at most one notification;
- click/focus resolves current live Approval state;
- resolved/expired state never revives;
- notification failure does not lose Approval;
- content stays sanitized.

## F04

- disconnect/degraded is incident-deduplicated;
- recovery can notify once;
- a later new incident can notify;
- notification is not connection truth;
- navigation points to current Overview/Diagnostics.

---

# 16. F05 / F06 Roll-up

## F05

M06 imports the exact accepted M02 baseline, sample method and thresholds.

Final candidate must:

- record Start→Ready and stages;
- distinguish cold/warm;
- report raw samples, P50/P95 and failure rate;
- use A11/A16 timing/statistics semantics;
- meet every frozen M02 threshold;
- retain authoritative readiness checks.

An external bottleneck explanation cannot convert a frozen threshold failure into PASS.

## F06

Diagnostics must:

- cover config, Workspace, Runtime, MCP, Tunnel, control plane, proxy, credential and lifecycle where safely determinable;
- use PASS/WARNING/FAIL/UNKNOWN truthfully;
- provide safe evidence/remediation;
- remain read-only by default;
- not restart/mutate config/secret/files without explicit action;
- reflect recovery rather than stale success;
- produce a sanitized summary.

---

# 17. Shared Evidence / F07 Roll-up

Final candidate proves:

- Runtime AuditEvent remains canonical;
- exact runtime_id + request_id, no fuzzy primary join;
- durable evidence typed/bounded/sanitized;
- History non-authoritative;
- terminal records survive Runtime/App restart;
- old Pending/Grant/Permit do not revive;
- Runtime-only writer semantics remain;
- offline Tauri reads remain controlled/read-only;
- query/filter/pagination/clear/retention work;
- busy/full/corrupt/drop/degraded states stay observable and do not mutate Policy truth;
- grep History evidence remains privacy-safe.

---

# 18. F10 Roll-up

Final candidate includes accepted M05 grep:

- literal/regex;
- relative scope;
- include/exclude;
- line numbers;
- bounded preview/results;
- deterministic truncation;
- frozen scan/output/time bounds;
- Windows reparse/junction/symlink/TOCTOU protection;
- normal Workspace/Policy/Approval/ExecutionPipeline path;
- no hidden shell grep;
- exact Activity/History identity;
- no raw grep query/match text in durable evidence.

---

# 19. F12 Roll-up

M06 does not invent a new Eval methodology.

It imports:

- accepted M04 formal baseline;
- accepted M05 strict candidate regression;
- frozen scorer/sample/randomness/thresholds;
- provider-drift control semantics.

Core acceptance requires exact F12 identity equivalence or a formal requalification on M06.

Required outcome:

- valid sample;
- zero security violations;
- compare_m05 PASS;
- required paired-control drift within guard;
- passive telemetry excluded.

No M06 threshold override.

---

# 20. V3 Security Regression

Minimum final-candidate regressions:

- Workspace root/escape;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- Approval;
- SessionGrant scope/lifetime;
- write/edit atomicity;
- Bash classification/timeout/output/cancel/concurrency;
- SecretStore reference boundary;
- Runtime/Tunnel Start/Stop/Restart;
- desired/effective state;
- config revision/restart debt;
- i18n/Appearance smoke;
- secret/raw path leakage.

Any security regression blocks Core.

---

# 21. Crash / Recovery

Cover:

- Runtime unexpected exit;
- Tunnel unexpected exit/disconnect;
- App restart with persisted History;
- incomplete request;
- stop/quit during active work where supported.

Required:

- truthful lifecycle Error/Stopped/Degraded;
- current Diagnostics;
- History INCOMPLETE/UNKNOWN where appropriate;
- no old Approval/Grant/Permit restoration;
- new Runtime identity after restart as designed;
- no owned orphan after final Quit.

---

# 22. External-facing and External ChatGPT

## Automated external-facing E2E

Use the real packaged Tunnel with a QA-controlled external MCP client where feasible.

Do not call that client ChatGPT.

## Mandatory real External ChatGPT evidence

Core PRD requires real ChatGPT MCP E2E.

Final M06 acceptance therefore requires a human-observed same-candidate chain:

~~~text
External ChatGPT
→ real Tunnel
→ final M06 Runtime MCP
→ read/grep
→ ASK/DENY
→ execution
→ Activity/History
~~~

At least one Tool request occurs while the main window is hidden and Harness stays active in Tray.

Manual Acceptance may satisfy this requirement.

Without it, V4 Core is BLOCKED.

An internal MCP client cannot substitute.

---

# 23. Windows Notification Manual Boundary

Automation may validate notification decisions/navigation intents, but Core acceptance also requires packaged Windows observation of:

- Approval notification;
- disconnect notification;
- recovery dedup;
- click/focus/navigation;
- hidden-window behavior.

If final-package OS behavior is not observed, the corresponding Core AC remains NOT_VERIFIED/BLOCKED.

---

# 24. Release Notes Contract

Final acceptance output includes:

- product version/candidate SHA;
- package hashes;
- Core features delivered/verified;
- non-blocking known issues;
- P1 explicitly OUT_OF_CORE / NOT_CORE_ACCEPTED;
- P2 DEFERRED or separately accepted;
- upgrade/rollback notes;
- config/evidence schema versions;
- security/privacy notes;
- M02 performance verdict;
- M04/M05 Eval verdict;
- real ChatGPT status;
- final verdict.

Do not claim V4 Full, official MCP conformance, F14 full Agent reasoning trace, or unaccepted P1/P2.

Actual Release Notes are final execution output, not a new planning document.

---

# 25. M06 Derived Acceptance Criteria

| M06 AC | Requirement |
| --- | --- |
| M06-AC01 | M01–M05 are all ACCEPTED / FROZEN and M00 upstream contracts remain authoritative before formal M06 implementation/validation. |
| M06-AC02 | One clean Core RC identity freezes commit, version, Portable/installer/App/Runtime/Tunnel hashes and protocol/config/history/tool/Eval identities. |
| M06-AC03 | Evidence obeys same-candidate/equivalence rules; product-impacting changes invalidate affected formal evidence. |
| M06-AC04 | Core Product Acceptance Matrix contains all 28 canonical Core ACs plus mandatory V3/release rows with no omission/downgrade. |
| M06-AC05 | Portable and NSIS are built/verified from one clean source; payload hashes match and no test/secret artifact ships. |
| M06-AC06 | v2→v3 upgrade preserves config/SecretReference/History semantics and rollback safely restores the previous accepted release without silent data loss. |
| M06-AC07 | F01 background continuity, truthful Tray controls/count/state and orphan-free Quit pass on final candidate. |
| M06-AC08 | F03 approval-notification dedup/current-state navigation/privacy passes on packaged Windows. |
| M06-AC09 | F04 disconnect/recovery incident dedup, navigation and truthful recovery passes on packaged Windows. |
| M06-AC10 | Final candidate satisfies exact frozen M02 F05 benchmark/threshold and authoritative Ready semantics; M06 cannot lower/reinterpret them. |
| M06-AC11 | F06 diagnostics cover required components, remain truthful/read-only by default, provide safe remediation/evidence and recover without sensitive leakage. |
| M06-AC12 | Shared Evidence uses AuditEvent + exact runtime_id/request_id, typed bounded sanitized projection and strict historical-authorization isolation. |
| M06-AC13 | F07 History persistence/query/filter/pagination/retention/clear/degradation passes with Runtime/Tauri ownership intact. |
| M06-AC14 | Same-candidate Tool chain covers read/grep, ASK allow/deny and DENY through real Runtime authority with exact Activity/History and correct final result. |
| M06-AC15 | Workspace, Tool toggles, Policy, Approval, SessionGrant, write/edit, Bash, SecretStore and lifecycle V3 regression all PASS with zero bypass/leak. |
| M06-AC16 | Runtime/Tunnel crash, cancel/stop and restart recovery are truthful and leave no stale authorization or owned orphan. |
| M06-AC17 | Accepted M04 baseline + M05 regression are identity-equivalent to final RC or formally requalified; compare_m05/drift/security gates remain unchanged and PASS. |
| M06-AC18 | Automated real-Tunnel external-facing E2E passes and Manual contains a real External ChatGPT→Tunnel→Runtime read/grep/ASK/DENY→History chain on the same candidate. |
| M06-AC19 | Final Core Release Notes/acceptance output binds candidate, matrix, upgrade/rollback, performance/Eval/external evidence and excludes P1/P2 from Core claim. |
| M06-AC20 | Gate fails closed: any mandatory FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED prevents release; only successful product verdict is V4 Core — ACCEPTED. |

---

# 26. Reserved Validation Traceability

| M06 AC | Integration | E2E | Manual |
| --- | --- | --- | --- |
| AC01–AC04 | M06-INT-01/02 | M06-E2E-01 | M06-MAN-01 |
| AC05–AC06 | M06-INT-03/04 | M06-E2E-01 | M06-MAN-01/05 |
| AC07–AC09 | M06-INT-05/06 | M06-E2E-02 | M06-MAN-02 |
| AC10–AC11 | M06-INT-07/08 | M06-E2E-02 | M06-MAN-03 |
| AC12–AC14 | M06-INT-09/10/11 | M06-E2E-03 | M06-MAN-03/04 |
| AC15–AC16 | M06-INT-12/13 | M06-E2E-03 | M06-MAN-04 |
| AC17 | M06-INT-14 | M06-E2E-04 | M06-MAN-04 |
| AC18 | M06-INT-15 | M06-E2E-03 | M06-MAN-05 |
| AC19–AC20 | M06-INT-16 | M06-E2E-04 | M06-MAN-01/05 |

Reserved suites:

- Integration: M06-INT-01 through M06-INT-16.
- E2E: M06-E2E-01 through M06-E2E-04.
- Manual: M06-MAN-01 through M06-MAN-05.

---

# 27. Development Entry Gate

M06 implementation may begin only when:

1. M01–M05 are formally ACCEPTED / FROZEN;
2. all six M06 documents are frozen;
3. accepted upstream evidence identities are available;
4. M02 frozen performance contract is available unchanged;
5. accepted M04 baseline and M05 formal regression identities/results are available;
6. no unresolved Core blocker exists.

Documentation may be prepared before these conditions; implementation may not.

---

# 28. Development Exit Gate

M06 Development Complete means:

- release-gate tooling/harness changes implemented;
- packaging/identity tooling implemented;
- local/non-external cross-component development tests pass;
- upgrade/rollback fixtures ready;
- F05/F12 equivalence or rerun requirement known;
- final Core RC can be built reproducibly;
- formal Integration/E2E/Manual remain truthfully NOT EXECUTED unless actually run.

Development Complete is not V4 Core acceptance.

---

# 29. Final Exit Gate

M06 may conclude V4 Core — ACCEPTED only when:

- M06-AC01 through M06-AC20 PASS;
- all 28 canonical Core rows PASS;
- M06 Integration PASS;
- M06 E2E PASS;
- M06 Manual PASS;
- M02 frozen performance gate PASS;
- F12 formal evidence equivalent/requalified and PASS;
- real External ChatGPT evidence exists on the final candidate;
- packaged Windows Tray/notification behavior is observed;
- Portable + installer integrity PASS;
- upgrade + rollback PASS;
- no mandatory FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED remains;
- Release Notes use Core-only scope.

Successful product wording:

~~~text
V4 Core — ACCEPTED
~~~

Forbidden at M06:

~~~text
V4 — ACCEPTED
V4 — ACCEPTED / FROZEN
V4 Full — ACCEPTED
~~~
