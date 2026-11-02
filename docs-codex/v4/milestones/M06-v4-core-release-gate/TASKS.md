# M06 TASKS — V4 Core Release Gate

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: IMPLEMENTATION BLOCKED UNTIL M01–M05 ACCEPTED / FROZEN AND M06 VALIDATION DOCS ARE FROZEN  
> Scope: V4 Core release engineering and integrated acceptance; no new P1/P2 feature delivery  
> Candidate rule: formal M06 validation must use one frozen Core RC  
> External rule: do not run real External ChatGPT, paid/networked controlled Model, destructive installation/rollback or user-instance restart without explicit execution-time authorization.  
> Contract rule: do not change M02 performance thresholds, M04/M05 Eval thresholds, upstream Core AC semantics or frozen M01–M05 documents merely to achieve release.

---

# 1. Dependency

~~~text
M01 ACCEPTED / FROZEN
M02 ACCEPTED / FROZEN
M03 ACCEPTED / FROZEN
M04 ACCEPTED / FROZEN
M05 ACCEPTED / FROZEN
        ↓
M06 six docs frozen
        ↓
T1 Gate verifier / candidate identity
        ↓
T2 Packaging / upgrade / rollback
        ↓
T3 Integrated Core regression
        ↓
T4 F05 + F06 requalification
        ↓
T5 F12 + external-chain binding
        ↓
T6 Final RC / Development Complete
        ↓
Formal M06 Integration → E2E → Manual
        ↓
V4 Core — ACCEPTED
~~~

M05 Development Complete alone does not unlock M06 implementation.

---

# T1 — Core Gate Verifier, Candidate Identity and Matrix Contract

**AC:** M06-AC01, AC02, AC03, AC04, AC19, AC20  
**Primary areas:** ui/scripts and existing validation helpers.

## Implement

### 1. Candidate identity collector

Reuse existing distribution/build metadata.

Collect:

- Git SHA;
- source dirty state;
- source state hash;
- product version;
- Portable hash;
- installer hash;
- UI hash;
- Runtime hash;
- Tunnel hash;
- protocol schema hash;
- config schema/profile identity;
- evidence schema/checksum;
- Tool catalog/schema identity;
- M04/M05 Eval identities.

Reject missing or inconsistent values.

### 2. Exact Core AC set

Encode the exact 28 canonical Core AC IDs from SPEC.

Do not encode test results or PASS values in source.

### 3. Core matrix validator

Consume structured execution-owned input.

Require for every mandatory row:

- scope ID;
- owner milestone;
- evidence reference;
- relevant M06 validation state;
- final status.

Reject:

- missing row;
- duplicate row;
- unknown substitution;
- missing evidence;
- FAIL;
- BLOCKED;
- INCONCLUSIVE;
- NOT_VERIFIED.

### 4. Non-Core classification

Support:

- CORE_MANDATORY;
- OUT_OF_CORE_P1;
- DEFERRED_P2;
- SEPARATELY_ACCEPTED_OPTIONAL.

Non-Core rows must never satisfy a Core AC.

### 5. Impact classification

Add impact categories:

- HOST_UI;
- HOST_LIFECYCLE;
- RUNTIME;
- TUNNEL;
- PROTOCOL_CONFIG;
- EVIDENCE_HISTORY;
- TOOL_CATALOG;
- EVAL;
- RELEASE_ONLY.

Map each category to invalidated M06 gates.

### 6. Same-candidate/equivalence tests

Positive:

- same candidate;
- release-only commit change with identical product/eval identities.

Negative:

- mixed Runtime hash;
- mixed Tunnel hash;
- mixed Tool schema;
- changed M04 dataset/scorer;
- missing External ChatGPT final requirement;
- one blocked Core row;
- duplicate/missing Core ID.

## Done

Release verdict input is complete, fail-closed and candidate-consistent.

---

# T2 — Core RC Packaging, Install, Upgrade and Rollback

**AC:** M06-AC05, AC06  
**Depends on:** T1.

## 1. Portable

Use existing production Portable build/verify path.

Record:

- candidate build SHA;
- source state;
- package hash;
- UI/Runtime/Tunnel hashes.

## 2. NSIS installer

Use existing Tauri build and NSIS verification.

Record:

- installer hash;
- verified payload hashes.

Assert Portable and installer derive from the same clean candidate.

## 3. Shipping-content audit

Verify package does not unintentionally include:

- synthetic API keys/secrets;
- test temp DB/config;
- Eval disposable Workspace;
- stale binaries;
- unintended development-only artifacts.

## 4. v2→v3 upgrade fixtures

Add/extend representative fixtures for:

- Workspace;
- Tunnel config;
- SecretReference;
- Tool toggles;
- Policy rules;
- desktop prefs;
- History config;
- grep post-M05 state.

Verify accepted migration behavior.

## 5. Existing v3 upgrade

Use a representative pre-Core v3 config.

Verify:

- no unrelated field loss;
- no unexpected revision drift;
- current product reaches truthful desired/effective state.

## 6. M03 History compatibility

Use representative History fixture.

Verify:

- readable after upgrade;
- no old Pending Approval restoration;
- no SessionGrant restoration;
- no ExecutionPermit restoration;
- writer/read ownership unchanged;
- physical schema only changes through accepted M03 rules.

## 7. Rollback

Freeze previous accepted package and pre-upgrade config.

Test:

1. Quit M06 cleanly;
2. preserve current user data;
3. restore pre-upgrade config backup when older binary needs older shape;
4. launch previous accepted package;
5. preserve compatible History;
6. no plaintext secret reconstruction;
7. no orphan Runtime/Tunnel.

Do not silently delete History/config to make rollback pass.

## Done

Core RC packages, upgrade and rollback are ready for formal validation.

---

# T3 — Integrated Core Lifecycle, Tool, Evidence and V3 Security Regression

**AC:** M06-AC07, AC08, AC09, AC12, AC13, AC14, AC15, AC16  
**Depends on:** T1/T2.

## 1. Lifecycle integration

Use existing Tauri/Runtime test layout.

Cover:

- Start;
- authoritative Ready;
- close main window;
- Tray/background state;
- background Tool use;
- Show;
- Stop;
- Restart;
- Quit;
- zero owned process orphan.

Do not treat process existence as Ready.

## 2. Approval notification state machine

Test:

- new Pending -> notify once;
- repeated same Pending -> no duplicate;
- resolved/expired before click -> current state, not revived state;
- notification failure -> Approval still exists;
- safe notification body.

Packaged OS Toast observation remains formal E2E/Manual where needed.

## 3. Disconnect notification state machine

Test:

- healthy → degraded/disconnected;
- repeated same incident;
- recovery;
- independent later incident;
- manual Stop/Quit.

Verify incident dedup and no false recovery truth.

## 4. Real Runtime MCP authority chain

Execute via real MCP:

- read_file;
- grep literal;
- grep deterministic truncation;
- ASK → allow_once;
- ASK → deny;
- Policy DENY;
- disabled Tool;
- cancel/timeout where applicable.

No direct ToolExecutor test may substitute for these integrated cases.

## 5. Shared evidence / History

For the same calls verify:

- exact runtime_id + request_id;
- Activity;
- Approval terminal state;
- History;
- restart persistence;
- no fuzzy timestamp primary join;
- old authorization not restored;
- safe grep evidence;
- no secret/raw path/raw grep content leak.

## 6. History operations

Exercise representative:

- filters;
- pagination;
- retention;
- clear;
- running/offline read behavior;
- degraded/busy/corrupt handling where existing fixtures allow.

Storage failure must not mutate Policy truth.

## 7. V3 security regression

Run affected tests for:

- Workspace escape;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- Approval;
- SessionGrant scope/lifetime;
- write/edit atomicity;
- Bash classification;
- Bash timeout/output/cancel/concurrency;
- SecretStore;
- Runtime/Tunnel lifecycle;
- desired/effective state;
- config revision/restart debt;
- i18n/Appearance smoke.

## 8. Crash/recovery

Inject:

- Runtime unexpected exit;
- Tunnel failure;
- incomplete request;
- Runtime/App restart;
- final Quit.

Verify:

- current lifecycle truthful;
- Diagnostics sees current failure;
- History incomplete semantics correct;
- no stale Approval/Grant/Permit;
- no owned orphan.

## Done

P0 features and V3 security semantics work together on the Core candidate.

---

# T4 — F05 Startup Performance and F06 Diagnostics Requalification

**AC:** M06-AC10, AC11  
**Depends on:** T2/T3.

## 1. Load accepted M02 performance contract

Obtain from frozen M02 evidence/contract:

- B0 identity;
- accepted candidate identity;
- benchmark harness/version;
- cold/warm definitions;
- sample count;
- exact P50 threshold;
- exact P95 guards/targets;
- failure-rate rule;
- environment class.

Do not copy values into editable M06 runtime flags.

## 2. Determine F05 equivalence

Compare M06:

- Tauri/App startup-relevant binary;
- Runtime;
- Tunnel;
- config profile;
- benchmark harness;
- environment class.

Return:

- EQUIVALENT_REUSE_ALLOWED;
- FULL_M02_BENCHMARK_REQUIRED;
- BLOCKED_MISSING_ACCEPTED_M02.

## 3. Benchmark path

If rerun required, execute the existing frozen M02 benchmark exactly.

Do not:

- lower thresholds;
- discard slow valid samples post hoc;
- average P95 values;
- replace Ready with process-spawn time.

## 4. Final Ready sanity

Even when quantitative evidence is reused, verify M06 candidate's Ready remains authoritative and can reach usable MCP/Tunnel state.

## 5. Diagnostics normal state

Verify all required components:

- Config;
- Workspace;
- Runtime;
- MCP;
- Tunnel;
- control plane;
- proxy;
- credential;
- lifecycle.

## 6. Diagnostics fault state

Use safe fixture/fault injection for:

- bad Workspace;
- Runtime failure;
- MCP target failure;
- Tunnel/control-plane failure;
- stale state;
- proxy/credential failure or UNKNOWN.

## 7. Diagnostics side effects/privacy

Verify:

- no automatic Restart;
- no config write just from diagnose;
- no secret mutation;
- no file Tool execution;
- no Pending Approval creation;
- safe copied summary.

## Done

M06 can prove F05/F06 on the final candidate without threshold drift.

---

# T5 — F12 Requalification, Candidate Binding and External-facing Tunnel Readiness

**AC:** M06-AC12, AC13, AC17, AC18  
**Depends on:** T3/T4.

## 1. Load accepted formal M04/M05 evidence

Require actual accepted:

- M04 baseline result;
- M05 strict candidate result;
- compare_m05 result;
- provider-drift result if applicable;
- security violation count;
- raw metric denominators;
- identity hashes.

Reject source-synthesized baseline.

## 2. F12 equivalence checker

Compare final M06:

- Runtime hash;
- Tool catalog/schema hash;
- M04 dataset hash;
- system prompt hash;
- scorer hash;
- M05 candidate profile hash;
- adapter ID/version;
- requested/resolved Model.

Return:

~~~text
EQUIVALENT_REUSE_ALLOWED
REQUALIFICATION_REQUIRED
BLOCKED_MISSING_ACCEPTED_EVIDENCE
~~~

## 3. Requalification command

If required:

- use existing M04/M05 runner;
- use existing compare_m05;
- use existing provider-drift control if needed;
- do not add an M06 scorer;
- do not change sample/thresholds;
- do not call provider before explicit user authorization.

## 4. Eval gate checks

Final accepted input requires:

- valid baseline/candidate sample;
- compare_m05 PASS;
- zero security violations;
- paired-control drift within guard where applicable;
- passive telemetry excluded.

## 5. Real Tunnel automated probe

With final packaged candidate and QA-controlled external MCP client:

- initialize;
- tools/list;
- read_file;
- grep;
- safe invalid request;
- reconnect;
- exact Activity/History correlation.

Use real Tunnel, not direct loopback.

Do not label this External ChatGPT.

## 6. Manual External ChatGPT handoff

Prepare final Manual scenario:

- main window hidden;
- External ChatGPT read_file;
- External ChatGPT grep;
- ASK;
- DENY;
- Approval notification;
- disconnect/Diagnostics;
- History verification;
- final Quit.

## Done

F12 and external transport are candidate-bound and ready for formal E2E/Manual.

---

# T6 — Final Development Regression, Candidate Freeze and Handoff

**AC:** M06-AC02, AC03, AC04, AC05, AC19, AC20  
**Depends on:** T1–T5.

## 1. Full Rust checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
~~~

## 2. Full UI checks

~~~text
cd ui
npm run typecheck
npm test
npm run build
~~~

## 3. Layout checks

Run:

- ui/scripts/check-repository-layout.ps1;
- ui/scripts/check-milestone-layout.ps1.

## 4. Production builds

Run existing:

- Portable build;
- Portable verification;
- NSIS build;
- NSIS verification;
- staged distribution validation.

## 5. Gate verifier negatives

Prove failure for:

- one missing canonical Core AC;
- one BLOCKED Core row;
- mixed candidate binary hash;
- M02 failed threshold input;
- F12 INCONCLUSIVE input;
- missing real-ChatGPT final requirement;
- package payload mismatch.

## 6. Create fresh M06 candidate

Use existing milestone delivery flow with:

~~~text
M06-v4-core-release-gate
~~~

Do not overwrite previous artifacts.

## 7. Development Complete output

Report in conversation:

~~~text
M06 Development Complete:
candidate SHA:
product version:
source dirty/state:
Portable path/hash:
Installer path/hash:
UI hash:
Runtime hash:
Tunnel hash:
Config schema:
Evidence schema/checksum:
Tool catalog hash:
M01 eligibility:
M02 eligibility:
M03 eligibility:
M04 eligibility:
M05 eligibility:
F05 equivalence/rerun status:
F12 equivalence/rerun status:
Upgrade development checks:
Rollback development checks:
Integrated Core development regression:
External ChatGPT: NOT EXECUTED unless actually performed
Formal M06 Integration: NOT EXECUTED
Formal M06 E2E: NOT EXECUTED
Manual: NOT EXECUTED
~~~

Do not create an extra milestone report.

Do not mark V4 Core — ACCEPTED.

### Done

M06 code/release tooling is ready for independent formal validation on the frozen RC.

---

# 2. Task → AC → Reserved Validation Traceability

| Task | M06 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01/02/03/04/19/20 | M06-INT-01/02/16, M06-E2E-01/04, M06-MAN-01/05 |
| T2 | AC05/06 | M06-INT-03/04, M06-E2E-01, M06-MAN-01/05 |
| T3 | AC07/08/09/12/13/14/15/16 | M06-INT-05/06/09/10/11/12/13, M06-E2E-02/03, M06-MAN-02/03/04 |
| T4 | AC10/11 | M06-INT-07/08, M06-E2E-02, M06-MAN-03 |
| T5 | AC12/13/17/18 | M06-INT-09/10/14/15, M06-E2E-03/04, M06-MAN-04/05 |
| T6 | AC02/03/04/05/19/20 | M06-INT-01/02/03/16, M06-E2E-01/04, M06-MAN-01/05 |

---

# 3. Development Handoff

After T6:

~~~text
M06 Development Complete
→ independent M06 Integration
→ same-candidate packaged E2E
→ explicit authorization for any required F12 real-Model rerun
→ Human Windows + External ChatGPT Manual Acceptance
→ user sign-off
→ V4 Core — ACCEPTED
→ unlock P1 sequence M07/M08/M09/M10
→ later M11 Full Release Gate
~~~

M06 does not unlock a V4 Full claim by itself.
