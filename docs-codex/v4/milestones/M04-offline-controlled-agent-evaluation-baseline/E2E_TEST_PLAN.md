# M04 E2E Test Plan — Real Controlled Model Baseline

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M04 — Offline Controlled Agent Evaluation Baseline  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: independent QA / Coding Agent after Development Complete + Integration PASS  
> Scope: actual controllable Model → M04 Runner → real isolated Runtime MCP → Workspace/Policy/Approval → M03 evidence → deterministic final-state scorer  
> External authorization: REQUIRED. These cases make paid/networked OpenAI API requests. Do not execute them until the user explicitly authorizes the formal M04 model run.  
> Important: real ChatGPT → Tunnel is not the F12 controlled Agent. M04 uses the frozen controllable Model adapter. External ChatGPT remains a separate product E2E boundary for other milestones.

---

## 1. Objective

E2E proves the exact F12 chain that M00 deferred:

~~~text
Versioned case + ground truth
→ real OpenAI Responses Model turn
→ emitted Harness Tool call
→ M04 controlled Agent loop
→ real production Runtime MCP
→ Workspace / Policy / Approval / ExecutionPipeline
→ M03 canonical evidence
→ deterministic final-state scorer
→ baseline aggregate
~~~

A mock Model, scripted call sequence, direct ToolExecutor call, passive production log or one successful tools/call cannot PASS M04 E2E.

---

## 2. Preconditions and Authorization

Before any E2E call:

- M03 is ACCEPTED / FROZEN;
- M04 Development Complete = PASS;
- M04 Integration M04-INT-01~16 = PASS;
- candidate commit is frozen/clean;
- m04_eval validate = PASS;
- m04_eval preflight = PASS and made zero provider calls;
- user has explicitly authorized paid/networked formal M04 evaluation;
- provider credential is available only through the approved execution-time secure source;
- spend limits from SPEC remain unchanged.

If authorization or provider access is absent:

~~~text
E2E verdict = BLOCKED
~~~

Do not substitute a mock and call it PASS.

---

## 3. Candidate / Batch Lock

Record:

~~~text
Candidate commit:
Runner binary SHA-256:
Runtime binary SHA-256:
Runner version:
Runtime version:
Evidence schema/checksum:
Dataset ID: m04-core-v1
Dataset hash:
System prompt hash:
Scorer version/hash:
Eval profile hash:
Tool schema hash:
Adapter: openai-responses-v1
Requested model: gpt-6.1-terra
Resolved model:
Provider API surface:
Formal repetitions: 3
Formal logical runs: 72
Run order seed: 0x4D3034
Per-run max turns: 8
Per-run max Tool calls: 12
Per-turn max output tokens: 2048
Per-run wall clock: 90s
Per-run spend cap: USD 0.25 when computable
Suite spend cap: USD 15.00 when computable
Windows build:
Tester:
Execution time:
~~~

All scoreable formal runs in the batch must share the locked identities required by SPEC.

---

# 4. Required E2E Cases

## M04-E2E-01 — Real Model smoke across the six frozen task categories

**AC:** M04-AC01–AC05, AC11, AC16  
**Tasks:** T1/T2/T3/T5  
**Upstream:** F12-AC01, SEC-AC02/03

Run one preselected representative case from each category using the real Model:

- exact_read;
- path_discovery;
- content_search;
- write_state;
- edit_state;
- enforcement_and_recovery.

This is a smoke subset only and does not replace the formal 72-run batch.

For every representative run verify:

- at least one actual provider Model turn occurs;
- Runner supplies only the real frozen Harness Tool catalog;
- built-in provider tools are not used;
- Runtime child is isolated;
- read/write/edit calls go through real MCP;
- expected write/edit ASK is resolved only through real Runtime Approval;
- expected DENY/no-mutation case remains non-mutating;
- final-state scorer independently checks the result;
- Runtime/Workspace/DB is cleaned after each run;
- provider credential never appears in output.

The enforcement case must demonstrate that a Runtime denial/error can be the expected correct behavior while final task completion still passes.

**PASS:** each task category is executable through the real Model/Runtime/scorer chain.

---

## M04-E2E-02 — Real Model multi-step calls with exact M03 request correlation

**AC:** M04-AC02, AC06–AC11, AC14, AC17  
**Tasks:** T2/T3/T4  
**Upstream:** F12-AC01/02/04, SEC-AC01

Choose at least:

- one path_discovery case requiring glob → read_file;
- one content_search case requiring bounded current baseline search;
- one write_state case with ASK → allow_once;
- one enforcement case with DENY/rejection.

For each emitted Tool call inspect the safe Runner result and M03 evidence.

Verify:

- Model-emitted tool name/order is preserved;
- arguments are scored against real tools/list schema;
- semantic argument constraints are independently scored;
- every forwarded call has exact runtime_id + request_id;
- exact identity matches M03 Activity/History;
- no timestamp-nearest join is used;
- expected Approval request_id matches the same in-flight invocation;
- Runtime outcome metric is distinct from final task metric;
- final scorer checks actual Workspace/response state;
- ordinary production M03 records outside the Eval run are excluded;
- no hidden reasoning/unemitted alternatives are claimed.

Negative observation is acceptable and scoreable: the real Model may make a wrong Tool choice. E2E proves honest scoring, not perfection.

**PASS:** real Model calls are traceably scored from emitted decision to final state.

---

## M04-E2E-03 — Full 24 × 3 = 72 real-model baseline and validity gate

**AC:** M04-AC03–AC09, AC12, AC13, AC15  
**Tasks:** T1/T3/T4  
**Upstream:** F12-AC02/03

Execute the complete formal batch:

~~~text
24 cases × 3 repetitions = 72 logical runs
~~~

Rules:

- use frozen deterministic local case-order seed 0x4D3034;
- do not skip a case because prior repetition failed;
- do not rerun a completed poor-quality run;
- only frozen infrastructure retry is allowed;
- no case/prompt/fixture/scorer edits after seeing results;
- no threshold edits;
- no Model substitution;
- no partial-batch merge across changed identities.

After completion verify baseline validity:

~~~text
scoreable runs >= 70
every case >= 2/3 scoreable
every category >= 11/12 scoreable
one dataset/scorer/prompt/profile/catalog/model/runtime identity
security violations = 0
~~~

Produce separate metrics with raw numerator/denominator:

- Tool Selection Accuracy;
- Argument Validity;
- Runtime Outcome Match Rate;
- Final Task Completion Rate;
- Unnecessary Call Rate.

Also record:

- category metrics;
- per-case 0/3..3/3;
- scoreable/infra/limit/contract status counts;
- termination reasons;
- Tool-call counts;
- provider usage/cost only if actually available/computable;
- M03 evidence coverage.

No arbitrary absolute quality target is required for M04 baseline acceptance. Poor but honestly measured model performance may still form a valid baseline if all validity/security requirements pass.

If sample/identity requirements fail, verdict is BLOCKED/INCONCLUSIVE, not a threshold rewrite.

**PASS:** a valid, reproducible, truthfully scored M04 baseline exists for M05 comparison.

---

## M04-E2E-04 — Formal-run immutability, production isolation and post-eval regression

**AC:** M04-AC12, AC14–AC18  
**Tasks:** T4/T6  
**Upstream:** F12-AC03/04, SEC-AC02/03

Using the same final candidate and the E2E-03 baseline result:

### A. Immutability guards

Attempt formal mode with controlled invalid changes one at a time:

- subset/skipped cases;
- changed dataset hash;
- changed scorer hash;
- changed Tool catalog;
- changed model identity;
- permissive comparator threshold input.

Formal runner/comparator must refuse or mark the comparison invalid/INCONCLUSIVE rather than quietly accept.

No paid rerun is required for every invalid-change negative control if the guard is exercised before provider contact.

### B. Passive telemetry boundary

Load ordinary non-Eval M03 History data.

Verify it cannot be merged into F12 baseline denominators as Tool selection/task completion.

### C. M05 comparator compatibility

Feed the real M04 baseline aggregate as B into deterministic pre-versioned candidate/control fixtures.

Verify the exact frozen M05 thresholds produce the same PASS/FAIL/INCONCLUSIVE behavior already proven in Integration.

This is a comparator replay, not a claim that M05 grep exists.

### D. Production isolation

After Eval finishes:

1. start the normal ChatGPT-Harness product candidate normally;
2. verify production Runtime does not start an Eval Agent loop;
3. verify desktop app does not require Model-provider credential;
4. run representative current Workspace/Policy/Approval/History behavior using existing product paths;
5. confirm no Eval temp Workspace/DB/Runtime remains;
6. confirm no grep Tool appears.

Where a real ChatGPT external Tool regression is already required by the surrounding release process, it remains separate evidence; M04 controlled Model E2E does not replace it.

**PASS:** M04 baseline is immutable, non-authoritative, isolated from production and ready to serve as M05 regression input.

---

# 5. E2E Verdict Rules

Use only:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Examples:

- no explicit user authorization → BLOCKED;
- frozen model unavailable → BLOCKED until approved contract revision/access;
- resolved model changes mid-batch → INCONCLUSIVE;
- only scripted adapter executed → BLOCKED;
- 69/72 scoreable → INCONCLUSIVE;
- one case only 1/3 scoreable → INCONCLUSIVE;
- Model picks wrong Tool but scorer records it honestly → scoreable quality failure, not infrastructure failure;
- Tool returned success but final state wrong → task completion FAIL for that run;
- exact request identity missing and timestamp matching used → FAIL;
- any Workspace/Policy/Approval bypass → FAIL;
- credential appears in output → FAIL;
- passive History counted as selection accuracy → FAIL.

---

# 6. E2E Exit Gate

M04 E2E is PASS only when:

- M04-E2E-01 through M04-E2E-04 all PASS;
- the real frozen Model adapter actually executed;
- the full 72 logical-run batch was executed;
- baseline validity gate passes;
- exact runtime_id + request_id correlation is proven;
- deterministic final-state scoring is used;
- all required metrics retain raw denominators;
- zero security violations occur;
- no provider/model/dataset/scorer/catalog identity drift remains;
- passive telemetry remains excluded;
- temp provider/Runtime/Workspace/DB resources are cleaned;
- no later code change invalidates the candidate.

E2E PASS still does not mark M04 ACCEPTED / FROZEN. Human Manual Acceptance remains required.
