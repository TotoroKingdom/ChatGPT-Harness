# M05 E2E Test Plan — grep Tool and Quality Regression

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M05 — grep Tool and Quality Regression  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: independent QA / Coding Agent after M05 Integration PASS  
> Scope: final/near-final packaged product, real Runtime/MCP/filesystem/evidence, controlled real-Model quality regression, and production external-facing transport chain  
> Human boundary: E2E cases are QA-executed. External ChatGPT UI/user operation is not required for automated E2E and is reserved for M05-MAN-05. The actual External ChatGPT → Tunnel → Harness user chain is therefore closed only by combining E2E-04 transport evidence with MAN-05 real-user evidence; E2E must never pretend an internal MCP client is ChatGPT.  
> Paid/networked Model rule: M05-E2E-03 may run only after explicit user authorization immediately before execution. Without authorization it is BLOCKED.  
> Execution results: do not edit this frozen plan to record results; report actual results in the execution conversation/delivery evidence.

---

# 1. Objective

E2E proves the M05 candidate as a near-final product rather than a collection of unit/integration modules.

It must prove:

- packaged grep is discoverable and callable through production MCP;
- real filesystem results/limits match the frozen contract;
- Windows Workspace/Policy/Approval security survives the packaged boundary;
- exact Activity/M03 History evidence survives real process boundaries;
- the real controlled Model can run the unchanged 72-run M04 suite with the new grep catalog under the frozen comparator;
- provider/model drift is handled by paired control when required;
- the supplemental grep-admission set runs separately;
- the packaged Tunnel/Runtime chain can carry grep requests through the same external-facing transport used by ChatGPT;
- final External ChatGPT UI behavior remains a Human Manual Acceptance item, not simulated evidence.

---

# 2. Entry Criteria

Formal E2E may begin only when:

- M03 is ACCEPTED / FROZEN;
- M04 is ACCEPTED / FROZEN;
- M05 Development Complete;
- M05 Integration PASS;
- candidate SHA is frozen and working tree is clean;
- fresh Portable candidate is built from that SHA;
- Runtime and package hashes are recorded;
- accepted M04 formal baseline identity/result is available;
- no later code change invalidates the package;
- test-owned Workspace/config/History locations are prepared.

For E2E-03 additionally:

- user has explicitly authorized paid/networked controlled-Model execution for this batch;
- provider credential is available through the approved secure execution-time path;
- spend/run limits from M04 remain in force.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

---

# 3. Candidate Header

Record:

~~~text
Candidate commit:
Portable path:
Portable SHA-256:
Runtime SHA-256:
Tunnel binary/version/hash:
grep Tool schema hash:
Windows version:
M03 evidence schema/checksum:
M04 accepted baseline ID/hash:
M04 requested/resolved model:
M05 candidate profile hash:
M05 supplemental dataset hash:
QA owner:
Execution time:
~~~

Do not record secrets or real-user physical Workspace paths.

---

# 4. Required E2E Cases

## M05-E2E-01 — Packaged grep black-box functionality + exact evidence

**AC:** M05-AC01–AC04, AC12  
**Integration dependencies:** M05-INT-01–06, M05-INT-13

Launch the fresh Portable candidate with a disposable Workspace and disposable History DB.

Use the candidate's real production Runtime MCP endpoint through its normal Host/Runtime startup path.

Run black-box Tool calls covering:

1. literal search across nested files;
2. regex search;
3. relative scope;
4. include/exclude with exclude winning;
5. UTF-8/BOM;
6. deterministic ordering;
7. maxResults truncation;
8. maxOutputBytes truncation;
9. zero result;
10. invalid regex.

Verify from outside the executor module:

- tools/list exposes the exact grep schema;
- business results match fixture ground truth;
- line numbers/preview/offsets are correct;
- deterministic rerun returns the same prefix;
- invalid regex is typed;
- no direct/internal executor API is used;
- exact runtime_id + request_id from call maps to live Activity;
- same key maps to M03 History;
- durable History omits raw query/filter/match preview;
- restarting the app/runtime preserves terminal grep History without restoring authorization.

**PASS:** the packaged Runtime exposes the real bounded grep behavior and exact durable evidence.

---

## M05-E2E-02 — Packaged Windows authority/security under DENY/ASK/reparse/TOCTOU

**AC:** M05-AC07–AC12  
**Integration dependencies:** M05-INT-09–13

On actual Windows packaged candidate, use disposable outside-sentinel fixtures.

Execute through real MCP:

### Authority

- grep disabled;
- Policy DENY;
- Policy ASK → allow_once;
- Policy ASK → deny.

Verify no scan starts before authority permits it.

### Workspace

- parent/absolute/sibling escape;
- directory junction/reparse to outside;
- candidate reparse to outside;
- test-controlled TOCTOU candidate swap to outside.

### Lifecycle

- timeout;
- cancellation if supported by production request path;
- Runtime stop during active search.

Verify:

- outside sentinel bytes are never returned/read into evidence;
- DENY/disabled/ASK-deny do not scan;
- allow_once uses the exact pending request;
- no stale History Approval can authorize a new request;
- timeout/cancel/stop never return successful partial matches;
- no orphan process/worker remains;
- Activity/History terminal state is truthful;
- no raw physical path/OS error leaks.

**PASS:** packaged Windows security behavior matches the frozen authority and Workspace contract.

---

## M05-E2E-03 — Real controlled-Model 72-run regression + paired control + grep-admission set

**AC:** M05-AC14–AC18  
**Integration dependencies:** M05-INT-15–17  
**Authorization:** explicit user approval required immediately before execution.

This is the only M05 E2E case that uses the real paid/networked ModelAdapter.

### A. Freeze identity

Record before the first run:

- accepted M04 baseline identity/result;
- dataset/system-prompt/scorer hashes;
- requested/resolved Model;
- adapter ID/version;
- M05 candidate Tool schema/profile hash;
- Runtime binary hash;
- candidate commit.

No scoring-relevant identity may drift within a batch.

### B. Model identity decision

If the accepted M04 resolved model/adapter identity is still exactly available:

- proceed directly to strict candidate.

If it differs or cannot be established:

1. run m05-paired-control-v1 on unchanged m04-core-v1;
2. 24 cases ×3 = 72;
3. grep not exposed;
4. exact M04 Tool catalog/schema;
5. compare against accepted M04 baseline with existing provider-drift guard.

If paired control exceeds any drift guard:

~~~text
E2E-03 = INCONCLUSIVE
~~~

Do not continue to a candidate PASS/FAIL claim caused by an unstable provider/model baseline.

### C. Strict candidate

Run:

~~~text
profile: m05-grep-candidate-v1
dataset: unchanged m04-core-v1
tools: read_file, glob, grep, write, edit
logical runs: 72
~~~

Require:

- total logical runs = 72;
- scoreable >=70;
- every case >=2/3 scoreable;
- every category >=11/12 scoreable;
- one stable candidate identity;
- security violations = 0;
- exact Runtime/M03 evidence for every forwarded Tool call.

Apply existing compare_m05 without threshold override.

Record raw numerators/denominators for:

- Tool Selection;
- Argument Validity;
- Runtime Outcome Match;
- Task Completion;
- Unnecessary Call Rate;
- category metrics;
- per-case 0/3..3/3 counts.

Poor Model choices remain scoreable failures; do not rerun until success and discard failures.

### D. Supplemental grep-admission set

Run separately:

~~~text
set: m05-grep-admission-v1
cases: 4
repetitions: 3
logical runs: 12
~~~

Require:

- scoreable >=11/12;
- each case >=2/3 scoreable;
- security violations = 0;
- neutral prompts unchanged;
- metrics kept separate from strict comparator.

Inspect whether the real Model actually discovers/selects grep for literal/regex/filter/truncation tasks.

No absolute supplemental accuracy threshold may be invented after seeing results.

### E. Privacy/cleanup

Verify:

- provider credential absent from outputs/History;
- no hidden reasoning captured;
- no real user Workspace touched;
- temporary Runtime/Workspace/DB cleaned.

**PASS:** strict candidate passes the frozen comparator with a valid model identity/control, supplemental set is valid, and security violations are zero.

---

## M05-E2E-04 — Final packaged external-facing Tunnel → Runtime grep chain

**AC:** M05-AC19, AC20  
**Integration dependency:** M05-INT-18  
**Important boundary:** this case proves the production external-facing transport path but does not claim the client is External ChatGPT.

Use the final Portable candidate and the real Tunnel/control-plane configuration intended for product use, with a disposable Workspace.

From a QA-controlled external MCP client through the real Tunnel endpoint:

1. initialize the MCP connection;
2. tools/list;
3. confirm grep schema;
4. call literal grep;
5. call regex grep;
6. call scope/include/exclude grep;
7. trigger deterministic truncation;
8. trigger one safe invalid request.

Verify:

- request traverses real Tunnel rather than loopback-only test shortcut;
- Runtime receives/executes through normal ExecutionPipeline;
- result returned through Tunnel matches local fixture truth;
- exact runtime_id + request_id is observable in Activity/History;
- no transport layer mutates grep arguments/results;
- reconnect/retry does not duplicate authorization or invent a second Tool execution;
- current read_file/glob representative calls still work through the same transport;
- no active user's production Workspace is used.

This automated case is intentionally not labeled External ChatGPT evidence.

Actual External ChatGPT tool selection/call and user-visible behavior are verified in M05-MAN-05.

**PASS:** final packaged external-facing transport can carry real grep Tool calls without bypass or corruption.

---

# 5. E2E Verdict Rules

Use only:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Examples:

- no explicit authorization for E2E-03 -> BLOCKED;
- accepted M04 baseline identity missing -> BLOCKED;
- only scripted adapter used in E2E-03 -> BLOCKED;
- paired control exceeds drift guard -> INCONCLUSIVE;
- strict candidate 69/72 scoreable -> INCONCLUSIVE;
- any candidate security violation -> FAIL;
- comparator threshold exceeded -> FAIL;
- outside sentinel observed -> FAIL;
- timestamp-nearest correlation used instead of exact IDs -> FAIL;
- real Tunnel replaced by loopback shortcut in E2E-04 -> FAIL;
- internal MCP client called External ChatGPT -> FAIL;
- supplemental metrics substituted into strict 72-run comparator -> FAIL;
- real External ChatGPT not yet manually tested -> does not fail automated E2E; it keeps overall M05 acceptance pending MAN-05.

---

# 6. E2E Exit Gate

M05 automated E2E is PASS only when:

- M05-E2E-01 through M05-E2E-04 all PASS;
- final/near-final packaged candidate is the same SHA;
- Windows security case executed on real Windows;
- strict controlled-Model 72-run regression was actually executed after authorization;
- model/provider identity is valid or paired-control drift is within guard;
- compare_m05 passes unchanged thresholds;
- supplemental 12-run set is valid and separately reported;
- security violations are zero;
- exact runtime_id + request_id correlation is proven;
- final real Tunnel path works;
- no secrets/privacy sentinels leak;
- all temp resources are cleaned;
- no later code change invalidates the candidate.

Automated E2E PASS still does not mark M05 ACCEPTED / FROZEN.

The final true External ChatGPT → Tunnel → Harness → Runtime user chain and human usability/security review remain M05 Manual Acceptance, especially M05-MAN-05.
