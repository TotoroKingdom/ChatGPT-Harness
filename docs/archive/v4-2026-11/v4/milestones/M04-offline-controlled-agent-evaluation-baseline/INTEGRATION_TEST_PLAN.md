# M04 Integration Test Plan — Offline Controlled Agent Evaluation Baseline

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M04 — Offline Controlled Agent Evaluation Baseline  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M04 Development Complete  
> Scope: F12 controlled evaluation mechanics across dataset, Runner, real isolated Runtime/MCP, M03 evidence, scoring and M05 comparator  
> Network rule: formal Integration MUST NOT make paid/networked Model requests. OpenAI adapter behavior is tested through an injectable/mock HTTP boundary only. Real Model execution belongs to M04 E2E and requires explicit user authorization immediately before execution.  
> Safety: all Runtime, Workspace, SQLite and Approval state must be isolated test-owned state; never attach to the user's active Runtime or Workspace.

---

## 1. Objective

Integration proves that M04's evaluation system is mechanically trustworthy before any paid real-model baseline is executed.

The suite must prove:

- dataset m04-core-v1 is exactly 24 versioned cases in 6 frozen categories;
- every repetition begins from a fresh immutable fixture;
- Runner starts a real production Runtime child and uses real MCP tools/list + tools/call;
- formal Eval Tool catalog/profile is the frozen M04 profile;
- every forwarded call is explicitly correlated to exact runtime_id + request_id and M03 evidence;
- scripted/model-adapter mechanics cannot bypass Workspace, Policy or Approval;
- selection, argument, Runtime outcome and final task completion are independent metrics;
- deterministic scorer positive/negative controls behave exactly as frozen;
- 72-run baseline validity rules and raw denominators are deterministic;
- M05 regression thresholds and model-drift guards are immutable code contracts;
- passive production telemetry cannot be scored as Tool selection/task completion;
- no provider credential, physical path, hidden reasoning or unbounded content enters Eval output;
- Eval code is isolated from production Runtime behavior and all resources are cleaned.

Integration PASS does not prove the real Model baseline. Only E2E can do that.

---

## 2. Entry Criteria

Formal Integration may begin only when:

- M03 is ACCEPTED / FROZEN;
- M04 T1–T6 are Development Complete;
- candidate commit is frozen and working tree is clean;
- all task-local tests are PASS;
- m04_eval validate and m04_eval preflight exist;
- preflight can use the real candidate Runtime without contacting the Model provider;
- no formal Integration case needs the user's active Harness instance.

Run preflight development checks:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

If M03 is not ACCEPTED / FROZEN, formal M04 Integration is BLOCKED even if the M04 code exists.

---

## 3. Evidence Header

Record once:

~~~text
Milestone: M04
Candidate commit:
Branch / dirty state:
Runner binary hash:
Runner version:
Runtime binary hash:
Runtime version:
Evidence-store schema/checksum:
Dataset ID:
Dataset hash:
System prompt hash:
Scorer version/hash:
Eval profile hash:
Tool schema hash:
Adapter ID/version:
Requested model:
Resolved model: NOT EXECUTED in Integration
Windows version:
QA owner:
Execution time:
~~~

Never record:

- provider API key;
- SecretStore values;
- real user Workspace physical path;
- raw environment;
- hidden/model reasoning;
- unrestricted provider response bodies.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

---

# 4. Required Integration Cases

## M04-INT-01 — Dataset, case schema and immutable identity

**AC:** M04-AC03, AC04, AC12  
**Tasks:** T1/T5

Run the real dataset parser/validator against fixtures/v4/m04-eval.

Verify:

- dataset_id = m04-core-v1;
- exactly 24 cases;
- exactly 6 categories;
- exactly 4 cases per category;
- formal repetitions = 3;
- formal logical run count = 72;
- case ID/version is unique;
- every fixtureRef exists;
- every fixture hash verifies;
- prompt, ground truth, policy expectation, approvalPlan and scorer type are closed/typed;
- changing any scoring-relevant case field changes case/dataset hash;
- unknown category/scorer/field is rejected;
- a malformed/missing fixture is rejected;
- global per-run limits cannot be weakened by case data.

Categories must be exactly:

~~~text
exact_read
path_discovery
content_search
write_state
edit_state
enforcement_and_recovery
~~~

**PASS:** the baseline dataset is versioned and immutable before Model execution.

---

## M04-INT-02 — Fresh Workspace / evidence isolation per repetition

**AC:** M04-AC01, AC16  
**Tasks:** T1/T2/T5

Materialize the same mutation case at least three times.

Verify:

- each repetition receives a unique temp Workspace root;
- fixture source is immutable;
- initial fixture hash is identical;
- mutation in run A is absent from run B/C;
- each run receives a unique M03 database;
- each Runtime receives a new runtime_id;
- lock/database state cannot cross repetitions;
- untouched sentinel files remain unchanged;
- successful and failed runs both clean test-owned resources.

Inject cleanup failure and verify it is surfaced in cleanupStatus rather than silently ignored.

**PASS:** no run can inherit another run's file/database state.

---

## M04-INT-03 — Real Runtime child, frozen Tool/Policy profile and provider-free preflight

**AC:** M04-AC01, AC04, AC16  
**Tasks:** T2/T5

Execute m04_eval preflight with network/provider access disabled.

It must:

1. create an isolated fixture;
2. launch the real production Runtime binary;
3. initialize/acknowledge Runtime;
4. obtain the real MCP endpoint;
5. use real MCP initialize/tools/list;
6. canonicalize/hash the exposed Tool catalog;
7. execute a safe scripted correlation probe;
8. verify M03 evidence;
9. stop Runtime and clean temp state;
10. make zero Model-provider requests.

Verify effective formal profile:

- read_file enabled / ALLOW;
- glob enabled / ALLOW;
- write enabled / ASK unless explicit case DENY;
- edit enabled / ASK unless explicit case DENY;
- bash disabled;
- grep absent.

Unexpected catalog/profile drift must fail preflight.

**PASS:** formal Runner mechanics use production Runtime/MCP without contacting the provider.

---

## M04-INT-04 — ModelAdapter and OpenAI Responses protocol boundary without real network

**AC:** M04-AC02, AC05  
**Task:** T3

Use injectable/fake HTTP responses to test the concrete openai-responses-v1 adapter.

Verify:

- requested model is gpt-6.1-terra;
- built-in provider tools are not enabled;
- real Harness Tool schemas are encoded as model-callable tools;
- tool-call IDs, names and original arguments are preserved;
- multiple Tool calls are parsed deterministically;
- Tool result/error continuation maps correctly to the next provider request;
- final response and stop reason are captured;
- resolved model identity is parsed when supplied;
- usage/cost fields are only reported when provider semantics are available;
- max output tokens = 2048;
- no explicit temperature/seed is injected;
- credential comes from execution-time secure source;
- credential is absent from serialized Eval result/error text.

Provider 429/5xx/transport retry rules are tested here but no real provider request is sent.

**PASS:** concrete adapter protocol mechanics work without pretending this is real-model evidence.

---

## M04-INT-05 — Exact MCP call → M03 runtime_id + request_id correlation

**AC:** M04-AC10  
**Tasks:** T2/T5  
**Upstream:** SEC-AC01 / A08

Drive real isolated Runtime Tool calls for at least:

- read_file success;
- write ASK → allow_once;
- edit ASK → deny or explicit Policy DENY;
- Workspace violation.

For every forwarded call verify:

- Runner receives exact runtime_id;
- Runner receives exact request_id;
- exact key matches live Activity;
- exact key matches M03 request_summary/evidence_events;
- ASK Pending request_id is the same invocation identity;
- a new Runtime gets a new runtime_id;
- same textual timing or nearest timestamp is never used as the primary join;
- business structured Tool result is unchanged by correlation metadata.

Add a negative fixture where correlation metadata is absent/mismatched.

Required result:

~~~text
HARNESS_CONTRACT_ERROR
~~~

not fuzzy fallback.

**PASS:** all Eval call evidence uses exact canonical request identity.

---

## M04-INT-06 — Tool Selection oracle and semantic strategy scoring

**AC:** M04-AC06, AC13  
**Tasks:** T1/T4

Feed deterministic scripted call sequences for every frozen strategy.

Verify positive examples:

- exact_read: read_file;
- path_discovery: glob → read_file;
- content_search baseline: glob → read_file+;
- write_state: write, optional read_file;
- edit_state: optional read_file → edit → optional read_file;
- enforcement/recovery strategies as declared per case.

Verify negative examples:

- no Tool when Tool required;
- wrong Tool;
- disallowed extra Tool;
- correct final state achieved through wrong/disallowed Tool;
- duplicate unnecessary calls.

Confirm:

- selectionPass is independent from taskCompletionPass;
- only model-emitted calls are scored;
- Tool catalog existence is not interpreted as “considered”;
- hidden reasoning is never required.

**PASS:** Tool Selection Accuracy measures declared observable strategy only.

---

## M04-INT-07 — Argument schema validity + case semantic constraints

**AC:** M04-AC07, AC13  
**Tasks:** T1/T3/T4

For each Tool surface use the actual tools/list schema.

Test:

- schema-valid + semantic-valid;
- schema-invalid types/missing fields;
- schema-valid but wrong logical path;
- wrong target file;
- disallowed overwrite/content;
- invalid Workspace escape argument;
- extra unknown fields where schema forbids them.

Verify:

~~~text
argument valid
= real Tool schema valid
AND case semantic constraints valid
~~~

Runner must not silently repair invalid arguments.

Original emitted invalid arguments must remain scoreable in bounded/redacted form.

Schema-invalid call is not forwarded as a normal real MCP call.

**PASS:** Argument Validity is stricter than transport/schema success alone.

---

## M04-INT-08 — Runtime outcome normalization is separate from task success

**AC:** M04-AC08, AC13  
**Tasks:** T3/T4

Drive real Runtime paths producing:

- success;
- policy_denied;
- approval_denied;
- workspace_violation;
- invalid_arguments where applicable to forwarded Runtime input;
- target_not_found;
- execution_failed fixture if safely injectable;
- cancelled/timeout if existing Runtime supports deterministic fixture.

Verify normalized outcome matches frozen categories.

Critical positive control:

- expected Policy DENY/no mutation can yield runtimeOutcomeMatch = PASS and taskCompletion = PASS.

Critical negative control:

- Runtime success with wrong final filesystem state yields runtimeOutcomeMatch = PASS but taskCompletion = FAIL.

Unknown new Runtime error code must become typed UNKNOWN/contract failure, not success.

**PASS:** Runtime transport/execution outcome is not collapsed into final task quality.

---

## M04-INT-09 — Deterministic final-state scorer golden matrix

**AC:** M04-AC09, AC13  
**Tasks:** T1/T4

Execute scorer golden fixtures without a Model.

Required controls:

1. exact read answer correct → PASS;
2. exact read answer wrong → FAIL;
3. Tool returned success but file bytes wrong → FAIL;
4. required file created with exact bytes → PASS;
5. unrelated sentinel changed → FAIL;
6. edit changed exact target and nothing else → PASS;
7. DENY/no mutation expected and preserved → PASS;
8. blocked case mutates Workspace anyway → FAIL;
9. model text says “done” but filesystem wrong → FAIL;
10. filesystem correct but selection wrong → task PASS, selection FAIL.

Scorer must be pure/deterministic and make no provider request.

**PASS:** final business state is independently graded.

---

## M04-INT-10 — Controlled Agent loop, Approval control, retries and limits

**AC:** M04-AC05, AC11  
**Task:** T3

Use scripted adapters against real Runtime/MCP.

Verify:

- max 8 Model turns;
- max 12 Harness Tool calls;
- max 90s logical wall-clock guard;
- 2048 max provider output tokens/turn is passed to adapter;
- one allowed infrastructure retry only for 429/5xx/transport reset before Model output;
- no retry for a poor Tool choice;
- no retry to replace a completed bad-quality run;
- expected ASK allow_once resolves exact Pending request;
- expected ASK deny resolves exact Pending request;
- unexpected ASK is never auto-approved;
- no SessionGrant injection;
- no synthetic ExecutionPermit;
- limit_exceeded preserves metrics already observed and makes task completion FAIL.

**PASS:** controlled loop is bounded and authorization-safe.

---

## M04-INT-11 — Formal 72-run baseline validity gate

**AC:** M04-AC04, AC12  
**Tasks:** T1/T4/T5

Test the validator entirely with deterministic synthetic run summaries.

Frozen valid minimum:

~~~text
logical runs = 72
scoreable runs >= 70
each case scoreable repetitions >= 2 of 3
each category scoreable runs >= 11 of 12
~~~

Verify boundaries:

- 70/72 scoreable → eligible if all other guards pass;
- 69/72 → INCONCLUSIVE;
- one case 1/3 scoreable → INCONCLUSIVE;
- one category 10/12 scoreable → INCONCLUSIVE;
- one dataset hash drift → invalid batch;
- one scorer hash drift → invalid batch;
- one Tool schema hash drift → invalid batch;
- requested/resolved model drift mid-batch → invalid batch;
- one Runtime binary identity drift → invalid batch;
- any security violation → FAIL.

No completed quality failure may be removed to improve the aggregate.

**PASS:** sample completeness and identity rules are immutable.

---

## M04-INT-12 — Aggregate metrics, raw denominators and run classification

**AC:** M04-AC12, AC13  
**Task:** T4

Construct a mixed deterministic 72-run set.

Verify outputs separately report:

- tool_selection_accuracy numerator/denominator;
- argument_validity numerator/denominator;
- runtime_outcome_match_rate numerator/denominator;
- task_completion_rate numerator/denominator;
- unnecessary_call_rate numerator/denominator;
- per-category values;
- per-case 0/3..3/3 selection/task counts;
- SCOREABLE / INFRA_ERROR / LIMIT_EXCEEDED / SECURITY_VIOLATION / HARNESS_CONTRACT_ERROR;
- Tool-call count statistics;
- termination reason counts.

Verify:

- no opaque single quality score replaces required metrics;
- INFRA_ERROR before Model decision is excluded according to frozen rules;
- quality failures remain scoreable;
- LIMIT_EXCEEDED after a Model decision remains a task failure;
- cost/usage is omitted or marked unavailable when not actually provided.

**PASS:** aggregate result is auditable from raw counts.

---

## M04-INT-13 — Frozen M05 comparator and provider/model drift guard

**AC:** M04-AC15  
**Task:** T4

Test exact frozen aggregate thresholds:

~~~text
Ctask >= Btask - 0.02
Csel  >= Bsel  - 0.03
Carg  >= Barg  - 0.01
Cout  >= Bout  - 0.01
Cunnecessary <= Bunnecessary + 0.05
~~~

Also test:

- task category guard -0.08;
- selection category guard -0.10;
- baseline stable case 3/3 → candidate at least 2/3;
- any candidate security violation = FAIL.

Boundary fixtures must include exact threshold PASS and just-over-threshold FAIL.

Provider/model drift paired-control guard:

~~~text
abs(Ptask - Btask) <= 0.05
abs(Psel  - Bsel)  <= 0.05
abs(Parg  - Barg)  <= 0.02
abs(Pout  - Bout)  <= 0.02
unnecessary-rate delta <= 0.05
~~~

Exceeding drift guard → INCONCLUSIVE, not candidate PASS/FAIL due solely to tool change.

Comparator thresholds must be versioned constants, not permissive CLI input.

**PASS:** M05 cannot redefine its regression gate after seeing results.

---

## M04-INT-14 — Passive telemetry truthfulness and output privacy

**AC:** M04-AC14, AC17  
**Tasks:** T3/T4

Provide two data sets:

1. controlled Runner run with ground truth;
2. ordinary M03 production History records with no Eval case identity.

Verify ordinary History cannot be fed into F12 scoring as:

- Tool selection accuracy;
- task completion;
- un-emitted Tool alternatives;
- hidden Model reasoning.

Privacy sentinels:

- fake provider key;
- physical Workspace path;
- raw environment;
- raw file body beyond bounded final evidence;
- raw stderr/stdout;
- credential-bearing URL;
- hidden/reasoning fixture.

Inspect EvalRunResult JSON, aggregate JSON, errors and human summary.

**PASS:** no forbidden sentinel appears and passive telemetry remains observational only.

---

## M04-INT-15 — Authorization/evidence separation and security isolation

**AC:** M04-AC10, AC11, AC14, AC16  
**Tasks:** T2/T3/T6  
**Upstream:** SEC-AC01/02/03, A17

Verify:

- Eval reads M03 History/evidence only after exact request identity is known;
- old History Approval cannot be used as live Approval;
- no SessionGrant is restored from History;
- no ExecutionPermit is constructed by Eval;
- Runner cannot write into M03 DB as a second normal-running writer;
- unexpected Approval is not auto-resolved;
- bash remains disabled in Eval profile;
- direct ToolExecutor/ExecutionPipeline calls are absent from formal Runner path;
- Runner defaults cannot attach to the user's active Runtime;
- provider credential is never sent to Runtime;
- output paths are test-owned temporary locations.

Inject a stale History Approval with matching-looking Tool/path and prove it has no effect on current Runtime authorization.

**PASS:** Eval consumes evidence without becoming authority.

---

## M04-INT-16 — Production isolation, M03/M01/M02/V3 regression and repository layout

**AC:** M04-AC18  
**Task:** T6

Run affected regression coverage for:

- M03 canonical event/request identity;
- M03 History query/read-only/write ownership;
- M03 privacy and Approval-history separation;
- Workspace boundary;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- Approval/SessionGrant;
- write/edit atomic behavior;
- controlled bash;
- SecretStore;
- Harness Start/Stop/Restart;
- M01 Tray/background/notifications;
- M02 startup recorder and one-click diagnostics.

Audit:

- production Runtime startup does not invoke Eval Runner;
- desktop app does not require Model-provider credential;
- no Eval Agent loop is part of production MCP request handling;
- M03 schema remains v1 unless separately approved;
- no grep Tool exists because M05 has not started;
- no new top-level directory/project exists;
- M04 does not rewrite M02/M03 acceptance evidence.

**PASS:** M04 is a QA/evaluation capability, not a new production authority.

---

# 5. Integration Exit Gate

M04 Integration is PASS only when:

- M04-INT-01 through M04-INT-16 are all PASS;
- M03 dependency is ACCEPTED / FROZEN;
- no real paid/network Model call was made by Integration;
- real isolated Runtime/MCP and M03 correlation were exercised;
- exact request identity is proven with no fuzzy matching;
- scorer/comparator golden and negative controls pass;
- security/privacy sentinels are absent;
- temp Runtime/Workspace/DB/process resources are cleaned;
- no production regression is unresolved.

Integration PASS means the real-model E2E baseline is ready to run. It does not mean F12 baseline has been executed or M04 is ACCEPTED / FROZEN.
