# M06 Integration Test Plan — V4 Core Release Gate

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M06 — V4 Core Release Gate  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M06 Development Complete  
> Scope: same-candidate Core release integration, package identity, upgrade/rollback, cross-module authority/evidence, F05/F12 gate mechanics  
> Network rule: Integration MUST NOT call paid/networked Model providers and MUST NOT require External ChatGPT. F12 real-Model requalification belongs to E2E-04 only when final-candidate identity requires it.  
> Safety: all config, History DB, Workspace, installer and process tests use disposable/test-owned locations. Do not mutate the user's active Harness instance or active Workspace.  
> Results rule: do not edit this frozen plan to record PASS/FAIL. Execution results belong in the validation conversation and candidate delivery evidence.

---

# 1. Objective

Integration proves that the M06 release gate itself is trustworthy before final packaged E2E and human acceptance.

It must prove that:

- M01–M05 acceptance is fail-closed as an entry dependency;
- one candidate identity binds source, package, binaries, schemas, Tool catalog and Eval identities;
- Core evidence cannot be assembled from incompatible candidates;
- all 28 canonical Core AC IDs are present and mandatory;
- Portable and NSIS payloads are built from the same clean source state;
- v2→v3 upgrade and tested rollback preserve user data/security semantics;
- Tray/background/notification state machines still use authoritative Host/Runtime state;
- M02 performance thresholds are imported unchanged;
- Diagnostics remains truthful and read-only;
- Shared Evidence and History preserve exact identity and non-authorization semantics;
- read/grep/ASK/DENY flow through the real Runtime authority path;
- V3 Workspace/Policy/Approval/SessionGrant/Bash/SecretStore/lifecycle regressions remain safe;
- crash/recovery leaves no stale authorization or owned orphan;
- M04/M05 Eval identities and comparator rules are not rewritten by M06;
- real Tunnel external-facing automation is distinguishable from External ChatGPT;
- final gate/release-note generation is Core-only and fail-closed.

Integration PASS does not equal V4 Core acceptance.

---

# 2. Entry Criteria

Formal M06 Integration may begin only when:

- M01 is ACCEPTED / FROZEN;
- M02 is ACCEPTED / FROZEN;
- M03 is ACCEPTED / FROZEN;
- M04 is ACCEPTED / FROZEN;
- M05 is ACCEPTED / FROZEN;
- all six M06 documents are frozen;
- M06 T1–T6 are Development Complete;
- candidate SHA is frozen and working tree is clean;
- fresh Portable and NSIS candidates exist from that SHA;
- accepted M02 performance contract/evidence identity is available;
- accepted M04 baseline and M05 formal regression identity/result are available;
- no unresolved Core blocker is known.

Preflight:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

If an upstream milestone is not ACCEPTED / FROZEN, formal M06 Integration is BLOCKED.

---

# 3. Evidence Header

Record once:

~~~text
Milestone: M06
Candidate ID:
Candidate commit:
Branch:
Source dirty:
Source-state SHA-256:
Product version:
Windows environment class:

Portable SHA-256:
Portable UI SHA-256:
Portable Runtime SHA-256:
Portable Tunnel SHA-256:

Installer SHA-256:
Installed UI SHA-256:
Installed Runtime SHA-256:
Installed Tunnel SHA-256:

Protocol revision/schema hash:
Config schema/effective revision/profile hash:
Evidence schema/checksum:
Tool catalog hash:
Per-tool schema hashes:

Accepted M02 evidence identity:
Accepted M04 baseline identity:
Accepted M05 formal regression identity:
M04 dataset/system-prompt/scorer hashes:
M05 candidate profile/tool-schema hash:
Requested/resolved Model:
Adapter identity:
QA owner:
Execution time:
~~~

Never record provider keys, SecretStore values, unrestricted physical paths, raw environment, raw file bodies or hidden model reasoning.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE
- NOT_VERIFIED

---

# 4. Required Integration Cases

## M06-INT-01 — Dependency gate and complete candidate identity

**AC:** M06-AC01, AC02, AC03  
**Tasks:** T1/T6

Verify the real gate tooling against:

1. all M01–M05 ACCEPTED / FROZEN;
2. each single upstream milestone changed to Development Complete only;
3. each single upstream verdict changed to FAIL/BLOCKED/INCONCLUSIVE;
4. clean candidate;
5. dirty candidate;
6. missing identity field;
7. binary/schema identity mismatch.

Required behavior:

- only the fully accepted dependency set passes entry eligibility;
- Development Complete never substitutes for ACCEPTED / FROZEN;
- candidate collector binds all SPEC §6 identities;
- source_dirty=true blocks release candidacy;
- product-impacting identity changes invalidate affected evidence;
- release-only equivalent changes are accepted only when all product/Eval equivalence keys remain unchanged.

**PASS:** M06 cannot begin formal release validation with incomplete dependencies or ambiguous candidate identity.

---

## M06-INT-02 — 28-row Core matrix completeness and same-candidate fail-closed logic

**AC:** M06-AC03, AC04, AC20  
**Task:** T1

Exercise the Core matrix validator with golden and negative fixtures.

Verify:

- exactly 28 canonical upstream Core AC IDs are required;
- each ID appears exactly once;
- mandatory V3/release rows are additionally required by the final matrix;
- every mandatory row has owner, evidence reference and final status;
- missing, duplicate or substituted AC IDs fail;
- PASS from a mismatched candidate fails equivalence validation;
- FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED blocks the Core gate;
- OUT_OF_CORE_P1 and DEFERRED_P2 never satisfy a Core row;
- unknown status fails closed;
- no validator path writes PASS into frozen milestone docs.

**PASS:** incomplete or mixed evidence cannot yield a release-eligible matrix.

---

## M06-INT-03 — Portable/NSIS same-source identity and shipping-content integrity

**AC:** M06-AC05  
**Tasks:** T2/T6

Using the final candidate build outputs, verify:

- source is clean;
- distribution build_sha equals candidate SHA;
- Portable manifest/hash set is complete;
- NSIS verification succeeds;
- UI/Runtime/Tunnel hashes in Portable and installer correspond to the same candidate;
- staged binaries do not come from an older milestone artifact;
- package does not contain synthetic secrets, disposable config/DB, Eval workspace or unintended test-only binary;
- package verification detects one-byte tampering;
- candidate delivery does not overwrite prior artifacts.

**PASS:** both distribution forms are one reproducible Core candidate.

---

## M06-INT-04 — v2→v3 upgrade, existing-v3 upgrade and rollback contract

**AC:** M06-AC06  
**Task:** T2

Use isolated fixtures and test-owned AppData.

### Upgrade

Verify v2 and pre-Core v3 inputs preserve:

- Workspace;
- Tunnel configuration;
- SecretReference;
- Tool toggles;
- Policy defaults/rules;
- desktop preferences;
- History config;
- grep M05 additive migration semantics;
- configRevision rules.

Verify:

- no plaintext secret migration;
- invalid config is not silently reset;
- History remains readable under accepted M03 semantics;
- old Pending Approval/SessionGrant/ExecutionPermit does not revive.

### Rollback

From upgraded isolated state:

- cleanly stop/quit current candidate;
- preserve current user data;
- restore pre-upgrade config backup where the older accepted release requires it;
- launch previous accepted release fixture/package;
- keep compatible History;
- surface incompatible data rather than deleting it;
- leave zero owned Runtime/Tunnel orphan.

**PASS:** upgrade and tested rollback are safe, explicit and non-destructive.

---

## M06-INT-05 — F01 background continuity and Tray authority

**AC:** M06-AC07  
**Task:** T3

Use real Host/Supervisor integration, not a React-only mock.

Verify:

- Start reaches authoritative Ready;
- close-window intent hides rather than stops/quits by default;
- Runtime/MCP/Tunnel state remains active while window hidden;
- background request can still reach Runtime;
- Tray Show restores window;
- Tray Start/Stop use normal lifecycle;
- Tray state and Pending Approval count come from authoritative state;
- Stop differs from Quit;
- explicit Quit runs normal shutdown;
- final managed Runtime/Tunnel process count is zero.

Negative control: process-exists without authoritative readiness must not show Ready.

**PASS:** window visibility never becomes lifecycle authority.

---

## M06-INT-06 — F03/F04 notification state machines and privacy

**AC:** M06-AC08, AC09  
**Task:** T3

Exercise the Host notification coordinator with real lifecycle/approval state boundaries.

### Approval

Verify:

- first new Pending emits at most one notification intent;
- repeated poll/observation does not duplicate;
- resolved/expired/cancelled state is queried on navigation;
- stale notification cannot revive Pending;
- notification failure does not lose Approval;
- notification content is sanitized.

### Disconnect

Verify:

- healthy→degraded/disconnected creates one incident;
- repeated same incident does not spam;
- recovery can emit at most one recovery notification;
- later independent incident may notify;
- manual Stop/Quit is not misclassified as an unexpected disconnect;
- notification success is never treated as proof of connection recovery.

OS-level visual Toast delivery/click is reserved for packaged E2E/Manual.

**PASS:** notification logic is truthful, deduplicated and non-authoritative.

---

## M06-INT-07 — Frozen M02 performance-contract import and equivalence decision

**AC:** M06-AC10  
**Task:** T4

Load the accepted M02 contract/evidence identity without copying thresholds into mutable M06 settings.

Verify:

- baseline identity is required;
- cold/warm definitions are unchanged;
- sample-count rules are unchanged;
- P50/P95/failure-rate rules are unchanged;
- A11 monotonic timing remains required;
- A16 raw-sample P95 semantics remain required;
- exact startup-relevant identity match returns EQUIVALENT_REUSE_ALLOWED;
- App/Tauri, Runtime, Tunnel, config-profile, harness or environment mismatch returns FULL_M02_BENCHMARK_REQUIRED;
- missing accepted M02 evidence returns BLOCKED_MISSING_ACCEPTED_M02;
- a threshold-failing result cannot be reclassified PASS by M06.

Use synthetic/golden result files only here; actual final-candidate benchmark runs in E2E when required.

**PASS:** M06 can only reuse or rerun the original F05 gate, never redefine it.

---

## M06-INT-08 — F06 Diagnostics truthfulness, fault matrix, side effects and privacy

**AC:** M06-AC11  
**Task:** T4

Run the real Diagnostics composition against isolated normal/fault states.

Required checks:

- Config;
- Workspace;
- Runtime;
- MCP;
- Tunnel;
- control plane;
- proxy;
- credential;
- lifecycle.

Faults include:

- inaccessible Workspace;
- Runtime unavailable;
- MCP target unavailable;
- Tunnel/control-plane failure;
- stale observation;
- proxy/credential FAIL or UNKNOWN where safely determinable.

Verify:

- only PASS/WARNING/FAIL/UNKNOWN are used;
- UNKNOWN is never presented as PASS;
- remediation points to valid user action;
- running diagnostics does not Restart Harness;
- no config/secret/file mutation;
- no new Pending Approval;
- recovery rerun reflects current state;
- copied summary contains no key, raw credential, unrestricted physical path, credential-bearing proxy URL, raw environment or unbounded stdout/stderr.

**PASS:** Diagnostics is useful, current, safe and read-only by default.

---

## M06-INT-09 — Shared Evidence exact identity, privacy and authorization isolation

**AC:** M06-AC12  
**Tasks:** T3/T5

Use real isolated Runtime MCP calls for read_file, grep, ASK-terminal and DENY.

Verify for each request:

- exact runtime_id + request_id is present;
- Activity and durable evidence use the same identity;
- AuditEvent remains the canonical evidence source;
- no timestamp-nearest primary join is used;
- durable projection remains typed/bounded/sanitized;
- grep raw query/filter/match preview is absent;
- SecretStore values/raw environment/raw physical path are absent;
- terminal historical Approval is observational only;
- no History row can create Pending Approval, SessionGrant or ExecutionPermit.

**PASS:** release roll-up does not weaken the Shared Evidence security contract.

---

## M06-INT-10 — F07 History ownership, persistence, query, maintenance and degradation

**AC:** M06-AC13  
**Task:** T3

Using isolated M03-compatible History:

Verify:

- Runtime is the sole normal-running writer;
- Tauri normal query path is read-only;
- terminal records persist across Runtime/App restart;
- filters by time/tool/outcome/policy/approval work;
- keyset pagination has no duplicate/gap under unchanged data;
- running clear routes through accepted Runtime semantics;
- offline maintenance requires stopped Runtime + exclusive ownership;
- retention removes whole request families and protects open requests;
- busy/full/corrupt/incompatible/drop states are visible;
- storage degradation does not alter Policy truth;
- corrupt/incompatible data is preserved, not silently reset;
- old Pending/Grant/Permit is never restored.

**PASS:** History remains durable and observational on the Core candidate.

---

## M06-INT-11 — Same-candidate read/grep/ASK/DENY authority chain

**AC:** M06-AC14  
**Task:** T3

Use the real Runtime MCP endpoint and final candidate Tool catalog.

Required calls:

1. read_file ALLOW;
2. grep literal ALLOW;
3. grep deterministic truncation;
4. ASK → allow_once;
5. ASK → deny;
6. Policy DENY;
7. disabled Tool;
8. invalid grep request.

Verify:

- all pass through Workspace/Tool/Policy/Approval/ExecutionPipeline;
- DENY/disabled/ASK-deny do not execute;
- allow_once resumes the exact request;
- grep does not spawn hidden shell search;
- successful final results match fixture truth;
- Activity/History identity matches exact requests;
- no authority comes from History or release tooling.

**PASS:** final Core Tool flow is one real authority path.

---

## M06-INT-12 — V3 security/lifecycle regression matrix

**AC:** M06-AC15  
**Task:** T3

Run affected production regressions for:

- Workspace traversal/escape;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- current Approval;
- SessionGrant scope/lifetime;
- write/edit atomicity;
- Bash classification;
- Bash timeout/output/cancel/concurrency;
- SecretStore references;
- Start/Stop/Restart;
- desired/effective state;
- config revision/restart debt;
- i18n/Appearance smoke;
- secret/raw-path leakage.

Required negative controls:

- Workspace outside sentinel remains unread;
- stale SessionGrant does not apply across invalid boundary;
- DENY cannot execute through another release path;
- secret fixture sentinel never enters evidence/log output.

**PASS:** M06 does not regress proven V3 security semantics.

---

## M06-INT-13 — Runtime/Tunnel crash, incomplete request, restart and orphan cleanup

**AC:** M06-AC16  
**Task:** T3

Inject controlled failures:

- Runtime unexpected exit;
- Tunnel unexpected exit/disconnect;
- active/incomplete Tool request;
- restart after failure;
- final Quit.

Verify:

- lifecycle becomes truthful Error/Stopped/Degraded rather than Ready;
- Diagnostics reflects current failure;
- incomplete request is represented as INCOMPLETE/UNKNOWN where accepted;
- no synthetic successful terminal event is invented;
- restart uses new Runtime identity as designed;
- old Pending Approval/SessionGrant/ExecutionPermit is not restored;
- Tunnel/Runtime duplicate process does not remain;
- final owned process count is zero.

**PASS:** crash/recovery cannot create stale authority or hidden process debt.

---

## M06-INT-14 — M04/M05 F12 equivalence and requalification gate mechanics

**AC:** M06-AC17  
**Task:** T5

No real provider call in Integration.

Load actual accepted M04/M05 machine-readable evidence and verify the equivalence checker requires:

- Runtime hash;
- Tool catalog/schema hash;
- M04 dataset hash;
- system prompt hash;
- scorer hash/version;
- M05 candidate profile hash;
- adapter identity/version;
- requested/resolved Model;
- formal validity result.

Expected outcomes:

- exact match -> EQUIVALENT_REUSE_ALLOWED;
- relevant identity mismatch -> REQUALIFICATION_REQUIRED;
- missing/non-formal/invalid accepted evidence -> BLOCKED_MISSING_ACCEPTED_EVIDENCE.

Verify final gate also requires:

- compare_m05 PASS;
- zero security violations;
- paired-control drift within guard when applicable;
- no M06 threshold override;
- no passive telemetry substitution.

**PASS:** M06 can bind or require F12 requalification without changing M04/M05 methodology.

---

## M06-INT-15 — Real-Tunnel external-client probe and evidence labeling

**AC:** M06-AC18  
**Task:** T5

Use a QA-controlled external MCP client through the real Tunnel against a disposable candidate instance.

Automate:

- initialize;
- tools/list;
- read_file;
- grep;
- safe invalid request;
- reconnect/retry;
- exact Activity/History correlation.

Verify:

- traffic actually traverses Tunnel;
- arguments/results are not mutated unexpectedly;
- reconnect does not duplicate authorization/execution;
- request identity remains exact;
- this evidence is labeled external MCP client, never External ChatGPT;
- absence of real ChatGPT evidence remains a final Manual blocker rather than being silently satisfied here.

**PASS:** the production external-facing path works and evidence provenance is truthful.

---

## M06-INT-16 — Final Core gate output, Release Notes scope and fail-closed verdict

**AC:** M06-AC19, AC20  
**Tasks:** T1/T6

Feed the gate verifier complete synthetic execution summaries.

Verify a release-eligible output requires:

- one candidate identity;
- 28 Core canonical rows PASS;
- mandatory V3/release rows PASS;
- package/upgrade/rollback PASS;
- F05 PASS under original M02 contract;
- F12 PASS/equivalent under original M04/M05 contract;
- E2E PASS;
- Manual PASS;
- real External ChatGPT evidence reference present;
- Windows notification/Tray manual observation present.

Release Notes must explicitly state:

- exact version/SHA/hashes;
- Core features delivered/verified;
- P1 OUT_OF_CORE / NOT_CORE_ACCEPTED;
- P2 DEFERRED or separately accepted;
- upgrade/rollback;
- performance/Eval/external status;
- known issues only if non-Core/non-security.

Negative controls must reject wording that claims V4 Full or V4 — ACCEPTED / FROZEN.

**PASS:** the only successful M06 product verdict is V4 Core — ACCEPTED.

---

# 5. Integration Exit Gate

M06 Integration is PASS only when:

- M06-INT-01 through M06-INT-16 are all PASS;
- all upstream dependency records are ACCEPTED / FROZEN;
- one final candidate identity is used;
- no real paid/network Model was required or called;
- real Runtime/MCP/Tauri/History/Policy/Approval boundaries were exercised where specified;
- real Tunnel external-client probe passed;
- package/upgrade/rollback integration evidence is complete;
- M02 and F12 equivalence decisions are explicit;
- no security/privacy sentinel leaked;
- no owned process/test resource remains;
- no mandatory blocker is unresolved.

Integration PASS unlocks automated M06 E2E. It does not mark V4 Core accepted.
