# M11 SPEC — V4 Full Release Integrated Acceptance

> 布局与验证生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。启动阶段只创建 SPEC / PLAN / TASKS；后续补齐 Integration / E2E / Manual 三份验证计划，最终严格保持六文件布局。
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M06+M07+M08+M09+M10 ACCEPTED / FROZEN AND M11 SIX DOCS ARE FROZEN
> Milestone: M11 — V4 Full Release Integrated Acceptance
> Scope: V4 Full Scope final integrated release gate; no new product feature scope
> Roadmap dependencies: M06 + M07 + M08 + M09 + M10 all ACCEPTED / FROZEN
> Upstream contract baseline: M00 implementation-frozen PRD / Architecture / Roadmap
> Current main observed before this document: 013a8b5acfc61a409664f4c28b18550ec3638226
> Release semantics: only M11 may conclude V4 — ACCEPTED / FROZEN
> Planned optional disposition for this M11 release: F09 = DEFERRED, F15 = DEFERRED
> Current optional milestone fact: O01/O02 are not present in the milestone tree at this document revision

---

# 1. Objective

M11 is the final V4 Full Scope release gate.

It does not define a new end-user feature.

One fixed Full Release Candidate must simultaneously prove:

- the accepted V4 Core remains valid on the final Full candidate;
- all P1 features are present and accepted;
- Shared Evidence remains authoritative, exact and safe;
- V3 security/lifecycle regressions remain intact;
- package/install/upgrade/rollback/uninstall flows are safe;
- F05 performance remains within the already-frozen contract;
- F12 controlled Agent Evaluation remains valid for the final Tool catalog;
- F13 official MCP conformance remains independently valid;
- real External ChatGPT interoperates with the same packaged candidate;
- P2 F09/F15 are truthfully DEFERRED for this release rather than falsely claimed.

M11 is the only milestone that may emit:

~~~text
V4 — ACCEPTED / FROZEN
~~~

A collection of historical milestone PASS results is not sufficient.

---

# 2. Full Scope Release Contract

The PRD defines:

## V4 Core

Already owned by M06:

- Shared Evidence Contract;
- F01 System Tray;
- F03 Approval Notifications;
- F04 Disconnect Notifications;
- F05 Startup Performance;
- F06 One-click Diagnostics;
- F07 SQLite History;
- F10 grep;
- F12 Controlled Agent Evaluation;
- affected V3 Workspace / Policy / Approval / Bash / SecretStore / lifecycle;
- real ChatGPT MCP E2E.

## V4 Full P1 additions

Mandatory in M11:

- F02 Auto Start;
- F08 Tool Metrics;
- F11 File Tool Admission / qualified expansion;
- F13 MCP Protocol Compatibility;
- F14 MCP Tool Request Trace.

## Optional P2 disposition

For this frozen M11 release plan:

~~~text
F09 Estimated Tokens = DEFERRED
F15 Policy Simulator = DEFERRED
~~~

DEFERRED means:

- not implemented as a V4 Full requirement;
- not counted as PASS;
- not silently represented as delivered;
- clearly called out in Release Notes;
- not a blocker for V4 Full.

If the user explicitly elects to include F09/F15 before M11 six-document freeze, M11 documents must be explicitly revised before implementation. Once frozen, Coding/QA must not expand scope opportunistically.

---

# 3. Canonical Full Mandatory Acceptance Set

M11 mandatory canonical AC count is 43.

## Shared Evidence — 3

- SEC-AC01
- SEC-AC02
- SEC-AC03

## F01–F08 — 24

- F01-AC01 / 02 / 03
- F02-AC01 / 02 / 03
- F03-AC01 / 02 / 03
- F04-AC01 / 02 / 03
- F05-AC01 / 02 / 03
- F06-AC01 / 02 / 03
- F07-AC01 / 02 / 03
- F08-AC01 / 02 / 03

## F10–F14 — 16

- F10-AC01 / 02 / 03
- F11-AC01 / 02 / 03
- F12-AC01 / 02 / 03 / 04
- F13-AC01 / 02 / 03
- F14-AC01 / 02 / 03

Total:

~~~text
3 + 24 + 3 + 3 + 4 + 3 + 3 = 43
~~~

No mandatory AC may be omitted, merged away or downgraded.

Optional ACs:

- F09-AC01/02/03 = DEFERRED
- F15-AC01/02/03 = DEFERRED

---

# 4. Fifteen-feature Traceability Matrix

Formal M11 execution produces a 15-feature matrix plus a separate Shared Evidence section.

Required rows:

| Feature | Priority | Upstream owner | M11 required disposition |
| --- | --- | --- | --- |
| F01 Tray | P0 | M01/M06 | PASS |
| F02 Auto Start | P1 | M07 | PASS |
| F03 Approval notification | P0 | M01/M06 | PASS |
| F04 Disconnect notification | P0 | M01/M06 | PASS |
| F05 Startup performance | P0 | M02/M06 | PASS |
| F06 Diagnostics | P0 | M02/M06 | PASS |
| F07 History | P0 | M03/M06 | PASS |
| F08 Metrics | P1 | M08 | PASS |
| F09 Estimated tokens | P2 | O01 | DEFERRED |
| F10 grep | P0 | M05/M06 | PASS |
| F11 File Tool admission | P1 | M09 | PASS |
| F12 Controlled Eval | P0 | M04/M05/M06/M09 | PASS |
| F13 MCP compatibility | P1 | M10 | PASS |
| F14 Request Trace | P1 | M08 | PASS |
| F15 Policy Simulator | P2 | O02 | DEFERRED |

The matrix must include:

- canonical AC coverage;
- upstream accepted milestone/evidence identity;
- final-candidate recheck or equivalence decision;
- Integration result;
- E2E result;
- Manual result;
- final disposition;
- evidence locator;
- blocker if not PASS/DEFERRED.

A DEFERRED optional row must not have fabricated implementation evidence.

---

# 5. Architecture Invariants

M11 preserves all accepted V4 architecture decisions, including:

- A01 React / Tauri / Runtime / Tunnel authority boundary;
- A02/A03 Host ownership of Tray/notification/window behavior;
- A04 AuditEvent as canonical execution evidence;
- A05 Runtime-only SQLite writer while running;
- A06 evidence store has no authorization authority;
- A07 bounded durable evidence and observable degradation;
- A08 runtime_id + request_id canonical cross-surface identity;
- A09 Metrics/Trace derive from canonical evidence;
- A10 controlled Eval remains separate from production passive telemetry;
- A11 Tauri monotonic startup timing;
- A12 grep remains a native WorkspaceRead Tool through normal pipeline;
- A13 accepted M09 ADMIT Tools preserve dual-path/destructive safety;
- A15 AppConfig schema v3 and additive Control Protocol v1;
- A16 P95 is computed from raw samples using frozen semantics;
- A17 History/Metrics/Trace remain observational, never authorization truth;
- A18 MCP conformance remains independent from Harness security and real ChatGPT E2E.

M11 must not introduce release-only shortcuts around these invariants.

---

# 6. Non-goals

M11 does not:

- invent new product features;
- change any M01–M10 frozen feature semantics;
- lower F05 thresholds;
- change F12 dataset/scorer/comparator thresholds;
- change M09 ADMIT/REJECT/DEFER decisions;
- silently add a M09 DEFER/REJECT Tool;
- silently remove an M09 ADMIT Tool;
- change the MCP revision accepted by M10;
- relabel controlled MCP clients as ChatGPT;
- add F09 or F15;
- introduce a new AppConfig schema;
- introduce a new Control Protocol major revision;
- add a second evidence/metrics/trace truth;
- treat historical milestone PASS as automatic final-candidate PASS;
- write formal execution results back into frozen planning documents;
- create a seventh milestone report file.

---

# 7. Development Dependency Gate

Formal M11 implementation/validation is blocked until:

~~~text
M06 — ACCEPTED / FROZEN
M07 — ACCEPTED / FROZEN
M08 — ACCEPTED / FROZEN
M09 — ACCEPTED / FROZEN
M10 — ACCEPTED / FROZEN
M11 six docs — FROZEN
~~~

M06 transitively proves the Core prerequisite chain.

M11 documentation may be prepared before those dependencies are accepted.

"Development Complete" on an upstream milestone is not sufficient.

---

# 8. Full Release Candidate Identity

Before formal M11 Integration, freeze one candidate.

Required identity fields include at least:

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
desktop_preferences_hash

evidence_store_physical_schema_version
evidence_schema_checksum
evidence_migration_identity

effective_tool_catalog_hash
per_tool_schema_hashes
m09_decision_set_hash

m02_performance_contract_identity
m04_dataset_hash
m04_prompt_hash
m04_scorer_hash
m05_regression_profile_hash
m09_eval_profile_or_decision_hash
model_adapter_id_version
requested_model
resolved_model

m08_metrics_contract_identity
m08_trace_contract_identity

m10_mcp_revision
m10_rmcp_version
m10_official_runner_commit
m10_official_runner_package
m10_runner_sdk_version
m10_conformance_result_identity

optional_f09_disposition
optional_f15_disposition
~~~

The candidate must be reproducible from a clean checkout.

---

# 9. Single-candidate Evidence Rule

Formal M11 Integration, E2E and Manual acceptance target the same final candidate except explicit upgrade/rollback comparison steps.

After candidate lock:

## Release-only non-bundled change

Previously collected product evidence may remain valid only if automated equivalence proves all relevant identities are unchanged.

## Product-impacting change

Any product-impacting change creates a new candidate identity.

Affected formal evidence must rerun.

No release decision may assemble "best historical results" from multiple incompatible binaries.

---

# 10. Upstream Acceptance Is Entry Evidence, Not Final-RC Proof

M06/M07/M08/M09/M10 acceptance proves their milestone contracts on their accepted candidates.

M11 must still decide whether each accepted evidence set is:

~~~text
EXACTLY_EQUIVALENT
REQUIRES_TARGETED_RERUN
REQUIRES_FULL_RERUN
INVALID / BLOCKED
~~~

Equivalence must be machine-verifiable from relevant hashes/config/fixtures.

A human statement that "nothing important changed" is insufficient.

---

# 11. Core Revalidation on Full Candidate

P1 work may change Host, Runtime, Tool catalog, evidence DB and MCP wire.

Therefore final M11 must revalidate the accepted Core on the Full candidate.

At minimum final candidate rechecks:

- authoritative Start→Ready;
- Tray background continuity;
- Approval notification;
- Disconnect/recovery notification;
- Diagnostics;
- History;
- read_file;
- grep;
- ASK allow/deny;
- Policy DENY;
- Workspace boundary;
- Bash security boundary;
- SecretStore/reference path;
- crash/restart;
- clean Quit/no orphan;
- real ChatGPT chain.

Quantitative F05/F12 evidence follows dedicated equivalence rules below.

---

# 12. F05 Performance Final-candidate Rule

M11 never modifies M02 thresholds.

Reuse of accepted M02/M06 performance evidence requires exact match on the accepted equivalence key.

At least:

- startup-relevant Tauri/App hash;
- Runtime hash;
- Tunnel hash;
- startup config profile;
- benchmark harness/version;
- environment class;
- frozen M02 threshold contract.

If any key differs, rerun the exact frozen M02 benchmark on the M11 candidate.

Failure cannot be reclassified as "acceptable for Full".

---

# 13. F12 Controlled Evaluation Final-candidate Rule

M11 never modifies M04/M05/M09 quality thresholds.

The final Tool catalog may differ from M06 due to M09.

Formal F12 evidence must correspond to the exact final catalog/schema.

Required final validity includes:

- accepted immutable M04 baseline;
- accepted M05 control/regression contract;
- accepted M09 final decision set;
- final ADMIT Tool catalog and schemas;
- exact dataset/prompt/scorer/profile identities;
- adapter/model identity;
- sample/repetition rules;
- drift handling;
- zero security violation.

If final candidate identity differs from accepted quality evidence, rerun the frozen applicable M09 cumulative regression / M05 control according to upstream rules.

Paid/network Model execution requires explicit execution-time authorization.

No authorization means BLOCKED, not threshold change.

---

# 14. F13 MCP Conformance Final-candidate Rule

M11 preserves exactly the protocol claim accepted by M10.

M11 does not upgrade it.

Formal conformance reuse requires exact match on:

- Runtime MCP implementation hash;
- rmcp version;
- MCP protocol revision;
- Streamable HTTP adapter behavior;
- Tool catalog/schema;
- official runner commit/package;
- relevant conformance fixture identity.

Any wire-impacting change after M10 requires M10's official conformance suite to rerun on the final M11 candidate.

MCP official conformance remains separate from:

- Harness-specific Workspace/Policy/Approval security;
- real ChatGPT→Tunnel E2E.

All three evidence classes remain mandatory.

---

# 15. F08/F14 Evidence-store Final-candidate Rule

M11 verifies accepted M08 semantics on the final candidate:

- one canonical evidence store;
- exact runtime_id + request_id;
- History/Metrics/Trace consistency;
- Metrics classification/denominators;
- execution vs approval vs wall-time separation;
- P95/sample/UNKNOWN semantics;
- request-local Trace stages;
- no fabricated hidden reasoning;
- no raw payload dump;
- History/Trace cannot restore live authorization.

If physical DB schema changes after accepted M08, M11 is blocked until the change has an explicit accepted migration contract.

---

# 16. F11 Final Admission Set Rule

M11 consumes the accepted M09 immutable decision set.

For each candidate:

~~~text
mkdir
list_directory
copy
move
rename
delete
~~~

M11 verifies exactly one accepted verdict:

~~~text
ADMIT
REJECT
DEFER
~~~

Rules:

- every ADMIT and delivered Tool must be in the final Tool catalog;
- every REJECT/DEFER Tool must be absent;
- final schema hash must equal the evaluated/accepted schema;
- destructive/dual-path behavior must retain accepted M09 semantics;
- final Tool catalog must still satisfy F12 quality regression;
- M11 cannot change verdicts.

If the accepted M09 decision set cannot be reproduced, Full Gate is BLOCKED.

---

# 17. F02 Auto Start Final-candidate Rule

Final package must preserve:

- default OFF for new install;
- persisted explicit user choice;
- one standard Start attempt per real App process launch when enabled;
- no bypass of StartEligibility/config/secret/security;
- no repeated automatic retry;
- no false Ready;
- no Windows login startup mechanism;
- close/hide does not retrigger;
- Quit→reopen retriggers exactly once when enabled;
- Stop does not auto-restart within same process;
- no duplicate Runtime/Tunnel generation.

Auto Start must integrate with F01/F05/F06 rather than creating a second lifecycle.

---

# 18. Full Integrated Runtime/Product Chain

The final candidate must prove a coherent product flow:

~~~text
Install / Upgrade
→ App launch
→ optional F02 Auto Start
→ authoritative Ready
→ Close window / Tray background
→ real ChatGPT / controlled MCP request
→ Tunnel
→ Runtime MCP
→ Tool resolution
→ Workspace
→ Policy
→ Approval when ASK
→ Execution
→ canonical evidence
→ History
→ Metrics
→ Request Trace
→ Diagnostics / recovery when faulted
→ clean Quit
~~~

The chain must include both positive and negative behavior.

No stage may be replaced by a mock in formal Full E2E where the product boundary is the subject.

---

# 19. Real External ChatGPT Final Chain

Manual final acceptance requires real External ChatGPT on the same final candidate.

Minimum integrated flow:

1. install/launch final candidate;
2. enable Auto Start if not already enabled;
3. full Quit;
4. reopen App and observe exactly one standard Auto Start;
5. hide main window;
6. ChatGPT discovers the final effective Tool catalog;
7. execute a safe read Tool;
8. execute grep;
9. if M09 has ADMIT Tools, execute at least one representative ADMIT Tool appropriate for safe disposable Workspace;
10. exercise ASK → approve;
11. exercise ASK → deny or Policy DENY;
12. observe History;
13. observe Metrics;
14. inspect Request Trace for exact same request;
15. induce/recover a safe disconnect and use Diagnostics;
16. restart Runtime/App and verify History persists but live approval/grants do not;
17. clean Quit and verify no owned orphan.

Real ChatGPT cannot be substituted by:

- internal Rust MCP client;
- official conformance client;
- standalone TypeScript SDK client;
- scripted Model adapter.

---

# 20. Package Matrix

Final Full RC requires both:

~~~text
Portable
NSIS installer / installed application
~~~

They must originate from the same clean source candidate.

Verify:

- App/Runtime/Tunnel payload hashes;
- no stale binary;
- no temporary fixture;
- no secret;
- no test config;
- no conformance runner bundled;
- no evaluation provider credential bundled;
- no debug-only admission Tool.

Package mismatch blocks release.

---

# 21. Install / Upgrade / Rollback / Uninstall

Formal M11 covers actual Windows product lifecycle.

## Clean install

Verify safe defaults including Auto Start OFF.

## Upgrade

At minimum cover accepted prior release/config states required by upstream contracts:

- config v2→v3 preservation;
- existing v3 explicit desktop preferences;
- Workspace/Tunnel/Proxy/SecretReference/Tools/Policy/Bash fields;
- M09 Tool migration/default semantics;
- M03/M08 evidence DB migration;
- History retention;
- no live authorization restoration.

## Rollback

Follow accepted M06/M08 rollback contracts.

No silent DB/config downgrade that destroys user data.

Where an older binary cannot consume a newer DB/schema, use the accepted backup/restore or compatibility behavior rather than inventing destructive downgrade logic.

## Uninstall

Verify:

- product-owned binaries/integration are removed according to installer contract;
- no managed process remains;
- external Workspace is not deleted;
- secrets/config/history handling matches the documented installer/user-data behavior;
- test-only state is removed;
- reinstall remains possible.

M11 does not invent a new data-deletion policy.

---

# 22. Crash / Recovery / Incomplete Evidence

Final candidate must safely handle:

- Runtime crash;
- Tunnel crash/disconnect;
- App close/hide;
- App Quit;
- restart during/after approval;
- evidence writer degradation where applicable.

Required:

- no stale Ready;
- no restored Pending Approval/Grant/Permit;
- no fake success;
- incomplete/unknown evidence remains truthful;
- History remains observational;
- Metrics do not misclassify incomplete as success;
- Trace shows observable incomplete stages only;
- no owned orphan.

---

# 23. Privacy and Security Gate

Release blocker if any final test exposes:

- plaintext secret;
- unrestricted raw args;
- raw file body beyond accepted Tool output;
- raw environment;
- raw shell command material beyond accepted safe evidence;
- physical path where safe logical path should be used;
- unbounded payload;
- stack trace/internal OS detail to external client.

Security regression must include:

- Workspace escape sentinel;
- reparse/symlink/junction controls;
- TOCTOU cases from affected Tools;
- Policy DENY;
- ASK approval exact identity;
- stale approval;
- SessionGrant isolation;
- Bash control;
- SecretStore;
- destructive M09 Tool boundaries;
- MCP invalid args/unknown Tool;
- evidence authorization isolation.

Any security violation blocks Full acceptance.

---

# 24. Optional Feature Disposition

This M11 release freezes:

~~~text
F09 = DEFERRED
F15 = DEFERRED
~~~

Release Notes must say both explicitly.

The final package must not accidentally expose half-implemented F09/F15 UI/API claims.

If latent internal scaffolding exists, it must not be presented as shipped feature capability.

---

# 25. Full Release Notes Contract

Final release notes must truthfully state:

- V4 Full accepted candidate identity;
- mandatory feature list;
- final M09 Tool decision/ADMIT summary;
- MCP protocol revision actually claimed;
- F05 performance gate result;
- F12 controlled Eval result;
- F13 official conformance result;
- real ChatGPT E2E result;
- supported install/upgrade/rollback/uninstall facts;
- F09 DEFERRED;
- F15 DEFERRED;
- known non-blocking limitations;
- no claim to hidden ChatGPT reasoning/provider telemetry;
- no claim to newer MCP revisions.

No "all 15 features delivered" wording is allowed while F09/F15 are deferred.

---

# 26. Automatic Release Blockers

M11 Full Gate is BLOCKED/FAIL if any of the following occurs:

- M06/M07/M08/M09/M10 not ACCEPTED / FROZEN;
- any mandatory canonical AC is not PASS;
- final candidate is dirty/unreproducible;
- Portable/installer payload mismatch;
- config/DB migration data loss;
- rollback unsafe;
- uninstall leaves owned process or deletes external Workspace;
- F05 final evidence invalid/fails;
- F12 final-catalog evidence invalid/fails/inconclusive;
- M09 decision set/catalog mismatch;
- F13 official conformance missing/failing;
- Harness-specific MCP security failure;
- real ChatGPT final chain missing/failing;
- Windows Tray/notification/Auto Start not observed on packaged candidate;
- History/Metrics/Trace facts disagree for same request;
- fuzzy identity is used as primary join;
- stale authorization restored;
- secret/privacy sentinel leaks;
- crash/restart fabricates success;
- optional F09/F15 is claimed delivered despite DEFERRED status;
- release output claims V4 accepted before Manual PASS.

---

# 27. Full Product Acceptance Matrix

Formal execution produces one matrix containing:

## Shared section

All 3 Shared Evidence ACs.

## Mandatory features

All 43 mandatory canonical ACs.

## Release rows

At least:

- one-candidate identity;
- clean source/reproducibility;
- Portable integrity;
- installer integrity;
- clean install;
- upgrade;
- DB migration;
- rollback;
- uninstall;
- F05 evidence;
- F12 evidence;
- F13 official conformance;
- controlled MCP client;
- real External ChatGPT;
- crash/recovery;
- security/privacy;
- no orphan;
- Release Notes truthfulness.

## Optional rows

- F09 = DEFERRED
- F15 = DEFERRED

No mandatory row may be DEFERRED.

---

# 28. M11 Derived Acceptance Criteria

| M11 AC | Requirement |
| --- | --- |
| M11-AC01 | M06, M07, M08, M09 and M10 are all ACCEPTED / FROZEN before formal M11 implementation/validation. |
| M11-AC02 | One clean reproducible Full RC freezes source, package, config/protocol/evidence/tool/Eval/MCP identities and all formal M11 evidence binds to it. |
| M11-AC03 | Final 15-feature matrix contains F01–F15 plus separate Shared Evidence coverage; all 43 mandatory canonical ACs are present and no mandatory row is downgraded. |
| M11-AC04 | F09 and F15 are explicitly DEFERRED for this release, absent from shipped feature claims, and listed as DEFERRED in Release Notes. |
| M11-AC05 | Historical milestone evidence is reused only through explicit equivalence keys; product-impacting changes invalidate affected evidence and prohibit mixed-candidate release assembly. |
| M11-AC06 | Portable and NSIS are built from the same clean source and installed payload hashes match the candidate with no secret/test/debug-only artifact. |
| M11-AC07 | Clean install, config v2→v3/existing-v3 upgrade, evidence DB migration, rollback and uninstall preserve user/security semantics without silent data loss or orphan. |
| M11-AC08 | Final candidate revalidates Core background/Tray/notifications/Diagnostics/History/read/grep/ASK/DENY/security/crash/Quit behavior after P1 integration. |
| M11-AC09 | F02 Auto Start final package preserves OFF default, explicit choice, exactly-once standard Start, no login-startup, no duplicate generation and truthful failure/Ready behavior. |
| M11-AC10 | F08 Metrics on final candidate has correct calls/outcome/denominator/average/P95/approval-wait/execution/wall-time/sample/UNKNOWN semantics from canonical evidence. |
| M11-AC11 | F14 Request Trace on final candidate uses exact request identity, observable Runtime stages and truthful incomplete/slow/failure/executor-start state without hidden-reasoning fabrication or raw payload dump. |
| M11-AC12 | History, Metrics and Trace agree for the same request and none can restore/authorize Pending Approval, SessionGrant or ExecutionPermit. |
| M11-AC13 | Accepted M09 decision set exactly matches final Tool catalog: every ADMIT Tool/schema is present, every REJECT/DEFER Tool absent, and destructive/dual-path security semantics remain intact. |
| M11-AC14 | Final Tool catalog passes the unchanged F12 controlled quality gates using accepted M04/M05/M09 contracts; invalid/drifted/insufficient evidence remains BLOCKED/INCONCLUSIVE. |
| M11-AC15 | Final candidate satisfies the exact frozen F05 performance contract or M11 fails; no threshold reinterpretation is allowed. |
| M11-AC16 | Final MCP implementation preserves the exact accepted M10 protocol claim and official conformance evidence is equivalent or rerun with zero blocking failure. |
| M11-AC17 | Official MCP conformance, Harness-specific MCP/security behavior and real ChatGPT→Tunnel E2E remain three independent PASS evidence classes. |
| M11-AC18 | Full integrated lifecycle from install/launch/Auto Start/Ready/hide/Tool/Approval/History/Metrics/Trace/Diagnostics/recovery/Quit works on the same candidate without bypass or orphan. |
| M11-AC19 | Crash/disconnect/restart/evidence-degradation paths remain truthful, preserve durable observational evidence and restore no stale live authorization. |
| M11-AC20 | Privacy/security regression has zero secret/raw-unbounded payload/unauthorized Workspace mutation and zero Workspace/Policy/Approval/Execution bypass. |
| M11-AC21 | Real External ChatGPT on the final packaged candidate sees the expected final Tool catalog and completes the integrated safe success/error/approval/history/metrics/trace/recovery chain. |
| M11-AC22 | Windows packaged Manual verifies Tray, notifications, Auto Start, install/upgrade/rollback/uninstall and no persistent test residue. |
| M11-AC23 | Final Release Notes truthfully distinguish mandatory delivered scope, M09 Tool decisions, F09/F15 DEFERRED, protocol/Eval/conformance claims and known limitations. |
| M11-AC24 | Gate fails closed: any mandatory FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED prevents Full acceptance; only successful final wording is V4 — ACCEPTED / FROZEN. |

---

# 29. Canonical / Derived / Validation Traceability

Reserved validation suites:

- Integration: M11-INT-01 through M11-INT-20
- E2E: M11-E2E-01 through M11-E2E-05
- Manual: M11-MAN-01 through M11-MAN-06

| Coverage | Derived ACs | Reserved Integration | Reserved E2E | Reserved Manual |
| --- | --- | --- | --- | --- |
| Dependency/candidate/matrix/optional | AC01–05 | INT-01/02/03 | E2E-01 | MAN-01 |
| Package/install/upgrade/rollback/uninstall | AC06/07/22 | INT-04/05/06 | E2E-01 | MAN-01/06 |
| Core final-RC revalidation | AC08/15/19/20 | INT-07/08/09/10 | E2E-02 | MAN-02/04 |
| F02 desktop lifecycle | AC09/18/22 | INT-11 | E2E-02 | MAN-02 |
| F08/F14/evidence consistency | AC10/11/12/18/19 | INT-12/13/14 | E2E-03 | MAN-03 |
| F11/F12 final catalog/quality | AC13/14/20 | INT-15/16 | E2E-03/04 | MAN-04 |
| F13 conformance/security separation | AC16/17 | INT-17/18 | E2E-04 | MAN-04 |
| Real ChatGPT | AC17/18/21 | INT-19 | E2E-05 | MAN-05 |
| Final verdict/release notes | AC23/24 | INT-20 | E2E-05 | MAN-01/06 |

The final validation documents will expand these IDs without modifying this frozen requirement meaning.

---

# 30. Development Entry Gate

Formal M11 implementation begins only when:

1. M06 is ACCEPTED / FROZEN;
2. M07 is ACCEPTED / FROZEN;
3. M08 is ACCEPTED / FROZEN;
4. M09 is ACCEPTED / FROZEN;
5. M10 is ACCEPTED / FROZEN;
6. all six M11 documents are frozen;
7. accepted upstream candidate/evidence identities are available;
8. M09 decision-set identity is available;
9. M10 official conformance identity/result is available;
10. no known Full blocker exists.

Documentation may be prepared earlier.

---

# 31. Development Exit Gate

M11 Development Complete means:

- release-gate identity/matrix tooling implemented;
- package/install/upgrade/rollback/uninstall validation harness ready;
- full-candidate equivalence/invalidation logic implemented;
- integrated Core+P1 development regression PASS;
- F05/F12/F13 equivalence/rerun requirement known and development-time checks performed where authorized;
- F09/F15 disposition remains DEFERRED and no shipped claim exists;
- fresh Full RC Portable + installer produced;
- formal Integration/E2E/Manual remain truthfully NOT EXECUTED unless actually run.

Development Complete is not V4 acceptance.

---

# 32. Final Exit Gate

M11 may conclude:

~~~text
V4 — ACCEPTED / FROZEN
~~~

only when:

- M11-AC01 through M11-AC24 PASS;
- all 43 mandatory canonical ACs PASS;
- Shared Evidence PASS;
- all mandatory F01–F08 and F10–F14 rows PASS;
- F09 = DEFERRED;
- F15 = DEFERRED;
- M11 Integration PASS;
- M11 E2E PASS;
- M11 Manual PASS;
- one final candidate identity binds all formal evidence;
- Portable + installer integrity PASS;
- install/upgrade/DB migration/rollback/uninstall PASS;
- F05 frozen performance gate PASS;
- F12 final-catalog controlled Eval PASS;
- F13 official conformance PASS/equivalent on final candidate;
- Harness-specific security PASS;
- real External ChatGPT final integrated chain PASS;
- packaged Windows Tray/notification/Auto Start observations PASS;
- History/Metrics/Trace exact identity consistency PASS;
- zero stale authorization restoration;
- zero security/privacy sentinel violation;
- zero owned orphan;
- Release Notes truthfully state F09/F15 DEFERRED;
- no mandatory FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED remains.

If P1 fails but the previously accepted M06 Core remains valid, the truthful release status may remain:

~~~text
V4 Core — ACCEPTED
V4 Full — BLOCKED / FAIL / INCONCLUSIVE
~~~

but M11 itself is not ACCEPTED / FROZEN.

Successful Full wording is exactly:

~~~text
V4 — ACCEPTED / FROZEN
~~~

Forbidden shortcuts include:

~~~text
V4 — ACCEPTED
V4 Full — ACCEPTED
all 15 features delivered
F09 PASS
F15 PASS
~~~

unless the corresponding frozen contracts and evidence actually support those claims.
