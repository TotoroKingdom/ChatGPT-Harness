# M04 SPEC — Offline Controlled Agent Evaluation Baseline

> 布局与验证生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。本目录启动阶段只创建 SPEC / PLAN / TASKS；后续补齐 Integration / E2E / Manual 三份验证计划，最终严格保持六文件布局。不得新增独立报告、里程碑子目录、仓库副本、worktree 或顶级 evaluation 工程。  
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M03 ACCEPTED / FROZEN  
> Milestone: M04 — Offline Controlled Agent Evaluation Baseline  
> Scope: P0 F12 Agent Tool Evaluation  
> Roadmap dependency: M03 Shared Evidence + F07  
> Current main observed before this document commit: f52b9171427568dad002d3d53aa5a672f2a753f2  
> Current M03 state supplied by user: Development Complete; this does not by itself satisfy the M04 implementation dependency.  
> Threshold rule: dataset、重复次数、随机性处理、run limits、score definitions、baseline validity gate 与 M05 regression thresholds 在本 SPEC 中先冻结；实现或看到真实模型结果后不得为了 PASS 调低标准。

---

## 1. Objective

M04 建立 V4 第一套可重复、可审计、可用于新增 Tool 准入的 Controlled Agent Evaluation（受控 Agent 评估）基线。

它必须回答：

1. 给定一个版本化任务，真实可控 Model/Agent 是否选择了合适的 Harness Tool 策略？
2. 模型实际发出的 Tool arguments 是否同时满足真实 MCP schema 和 case-specific ground truth？
3. Tool call 经过真实 Runtime MCP 后，实际 Runtime outcome 是否符合该 case 的预期？
4. 即使 Tool transport / Runtime call 成功，最终业务任务是否真的完成？
5. 增加未来 Tool（首先是 M05 grep）后，上述质量是否发生可接受或不可接受的退化？

M04 不在生产 Runtime 内加入 Agent Loop。

目标拓扑：

~~~text
Versioned Dataset + Ground Truth
              │
              ├───────────────┐
              ↓               ↓
     Isolated Workspace    Run Manifest
              │               │
              └──────┬────────┘
                     ↓
             Offline Eval Runner
                     │
        ┌────────────┴────────────┐
        ↓                         ↓
Controllable Model Adapter   Production Runtime child
(OpenAI Responses API v1)         │
        ↑                         ↓
        └──── tool loop ───── Real MCP tools/list + tools/call
                                  │
                                  ↓
                         M03 Safe Evidence / History
                                  │
                                  ↓
                       Deterministic Scorers
                                  │
                                  ↓
                        Baseline Aggregate
~~~

Runner 是开发/验证工具，不是桌面用户 Agent。

---

## 2. Canonical Upstream Acceptance Criteria

### F12

- F12-AC01 — Evaluation 使用真实可控 Agent / Model adapter、版本化 dataset / ground truth、隔离 Workspace 与真实 MCP Runtime。
- F12-AC02 — 至少独立计算 Tool selection、argument validity、runtime call outcome 与最终 task completion；Tool returned success 不等于 task success。
- F12-AC03 — baseline、candidate、样本量、随机性与回归阈值在实现前冻结；不可控模型或样本不足必须 BLOCKED / INCONCLUSIVE。
- F12-AC04 — Passive production telemetry 不得冒充 Tool selection accuracy、未发出候选 Tool 或模型隐藏推理。

### Shared Evidence dependencies

- SEC-AC01 — Eval 使用与 Activity / Approval / History 一致的 stable Runtime request identity；不得靠模糊时间匹配。
- SEC-AC02 — Eval 只消费 typed / bounded / sanitized Runtime evidence。
- SEC-AC03 — Eval 结果不得创建或恢复 live authorization。

### Architecture

- A10 — 独立 Offline Controlled Runner + ground truth。
- A04/A08 — Runtime AuditEvent 与 runtime_id + request_id 仍是执行证据/请求关联真相。
- A17 — evaluation/history 是 observational，不成为 authorization dependency。

---

## 3. Current Code Facts and M03 Dependency

At f52b9171427568dad002d3d53aa5a672f2a753f2:

- M03 production Evidence Store exists under runtime/evidence-store.
- Runtime durable evidence uses stable runtime_id + request_id.
- live Activity now carries runtime_id.
- EvidenceStore schema version is 1.
- Tauri History is read-only while Runtime owns the writer.
- current durable sanitized Tool evidence includes read_file, glob, write, edit and bash variants.
- current Tool surface includes read_file, glob, write, edit and controlled bash; M04 baseline explicitly disables bash.
- no production Controlled Eval Runner exists.
- no production Eval dataset/result format exists.
- M00 real controllable Model E2E was explicitly deferred and NOT EXECUTED; M04 must finally execute that real closed loop during formal validation.

M04 implementation requires M03 ACCEPTED / FROZEN because:

- M04 consumes M03 canonical evidence identity;
- M04 must not develop a parallel request-correlation/evidence pipeline;
- M05 quality regression depends on the M04 baseline.

M01/M02 acceptance is not a direct Roadmap dependency, but M04 must not regress their current code.

---

# 4. Scope

## 4.1 Included

M04 delivers:

1. an offline Eval Runner as a separate binary target inside the existing runtime product module;
2. a provider-neutral ModelAdapter interface;
3. one concrete OpenAI Responses API adapter;
4. versioned dataset/case schema;
5. isolated Workspace fixture materialization;
6. real production Runtime child lifecycle;
7. real MCP initialize / tools/list / tools/call loop;
8. explicit Runtime request identity correlation to M03 evidence;
9. controlled Approval resolver for expected ASK cases using the real Runtime approval path;
10. deterministic selection / argument / runtime-outcome / final-state scoring;
11. run manifest hashing and result identity;
12. bounded step/time/tool-call/spend controls;
13. 24 frozen baseline cases across 6 task categories;
14. 3 repetitions per case = 72 logical baseline runs;
15. infrastructure-vs-quality failure classification;
16. aggregate baseline metrics;
17. M05 regression comparison formulas frozen now;
18. deterministic golden scorer tests and deliberate regression controls;
19. safe machine-readable JSON output to stdout plus concise human summary;
20. cleanup of Runtime, Workspace, History DB and provider-temporary state.

## 4.2 Non-goals

M04 does not:

- add an Agent loop to production Runtime;
- add a new top-level evaluation directory;
- add a new standalone repository/project;
- add a production Eval UI;
- claim passive ChatGPT/Tunnel traffic is controlled evaluation;
- infer hidden model reasoning;
- infer tools the model considered but did not emit;
- evaluate provider billed tokens unless provider returns verified usage;
- add grep;
- add new file-operation tools;
- change Runtime Policy semantics;
- bypass Workspace / ToolConfig / Policy / Approval;
- persist raw model reasoning;
- persist secrets into dataset/results;
- automatically make paid/networked requests without explicit user authorization;
- treat Tool transport success as task completion.

---

# 5. Implementation Landing Zone

No new top-level directory and no independent project.

Freeze the implementation landing under the existing runtime crate:

~~~text
runtime/
├ Cargo.toml
├ src/
│  ├ evaluation/
│  │  ├ mod.rs
│  │  ├ case.rs
│  │  ├ dataset.rs
│  │  ├ fixture.rs
│  │  ├ manifest.rs
│  │  ├ adapter.rs
│  │  ├ openai_responses.rs
│  │  ├ agent_loop.rs
│  │  ├ mcp_client.rs
│  │  ├ approvals.rs
│  │  ├ correlation.rs
│  │  ├ scoring.rs
│  │  ├ aggregate.rs
│  │  └ output.rs
│  └ bin/
│     └ m04_eval.rs
└ tests/
   └ evaluation_*.rs
~~~

Fixed test data lands under existing fixture root:

~~~text
fixtures/v4/m04-eval/
├ dataset.json
├ system_prompt.txt
├ cases/
└ workspaces/
~~~

Generated run data must use the system temp directory.

No generated baseline result file is committed automatically.

Formal run output is delivered through the conversation/current validation channel; machine JSON may be explicitly saved outside the repository by the user/QA if needed.

---

# 6. Concrete Baseline Model Profile

M04 freezes one primary real adapter profile.

~~~text
provider: OpenAI API
adapter_id: openai-responses-v1
api: Responses API
requested_model: gpt-6.1-terra
tool mode: function/tool calling driven by the adapter
built-in provider tools: disabled
system prompt id: m04-agent-v1
provider sampling controls: no explicit temperature or seed
repetitions per case: 3
max model turns per run: 8
max Harness tool calls per run: 12
max output tokens per model turn: 2048
wall-clock limit per run: 90 seconds
logical spend limit per run: USD 0.25 when cost can be computed
logical suite spend limit: USD 15.00 when cost can be computed
~~~

The model adapter credential is read only from an execution-time environment/secure source such as M04_EVAL_OPENAI_API_KEY.

It is never:

- committed;
- placed in fixture files;
- sent to Runtime;
- written to M03 History;
- printed in result JSON.

## 6.1 Model alias drift

The runner must record:

- requested model;
- provider-returned/resolved model identity;
- adapter version;
- API surface version if observable.

All scoreable runs in one formal baseline batch must use the same resolved model identity.

If the provider resolves the alias differently during the batch, the batch is INCONCLUSIVE and must be rerun.

M05 cannot silently compare a different resolved model to M04 baseline.

## 6.2 External authorization

Real provider execution is paid/networked and requires explicit user authorization at execution time per Roadmap.

Implementation/unit tests may use a scripted fake adapter only for runner mechanics.

Mock/scripted adapter results cannot satisfy M04 E2E or baseline acceptance.

If the exact baseline model is unavailable to the account, the real baseline is BLOCKED until an explicitly approved SPEC revision or provider access is supplied. Do not silently substitute a model.

---

# 7. Frozen Runtime / Tool / Policy Baseline Profile

Every formal M04 baseline run uses an isolated Runtime with a generated config derived from one versioned Eval profile.

## 7.1 Tool catalog

Enabled:

- read_file
- glob
- write
- edit

Disabled:

- bash

No grep exists in M04 baseline.

The runner obtains actual Tool schemas from real MCP tools/list and hashes the canonicalized schema/catalog.

A case never hardcodes a fake schema as the authority.

## 7.2 Policy profile

The profile must produce these semantics through the real Runtime PolicyEngine:

- read_file: ALLOW for permitted Workspace fixture paths;
- glob: ALLOW for permitted Workspace fixture scope;
- write: ASK unless a case declares an explicit DENY rule;
- edit: ASK unless a case declares an explicit DENY rule;
- Workspace escape: Runtime rejection;
- bash: disabled/unavailable.

The exact persisted config/rule representation may follow current production schema, but the effective semantics and config hash are frozen.

## 7.3 Approval plan

A case may declare:

- none
- allow_once
- deny

The runner watches the real Runtime Pending Approval state and resolves only the exact expected request through the normal Runtime approval control path.

Rules:

- no direct executor bypass;
- no synthetic Permit;
- no SessionGrant injection;
- unexpected Approval = case quality/contract failure and is not automatically approved;
- Approval decision/result is returned to the Agent through the real MCP call completion.

---

# 8. Versioned Dataset v1

Dataset identity:

~~~text
dataset_id: m04-core-v1
case_count: 24
repetitions: 3
logical_runs: 72
categories: 6
cases_per_category: 4
~~~

The dataset file and every fixture tree are content-hashed.

Prompts, fixture bytes, ground truth and scorer definitions are frozen before real baseline execution.

## 8.1 Categories

### C1 — exact_read

4 cases.

Task gives a known relative file path and asks for one exact value.

Expected semantic strategy:

~~~text
exact_read
~~~

Baseline Tool strategy:

- read_file

Final scorer uses exact expected answer token/text.

### C2 — path_discovery

4 cases.

Task describes a target by filename/pattern but not the full relative path.

Expected semantic strategy:

~~~text
path_discovery_then_read
~~~

Baseline strategy:

- glob
- then read_file on one matching logical path

Final scorer checks exact requested value.

### C3 — content_search

4 cases.

Task asks for information that exists inside one of several small files but does not reveal the containing filename.

Expected semantic strategy:

~~~text
content_search
~~~

M04 baseline accepted strategy:

- glob to enumerate bounded candidates;
- one or more read_file calls;
- no Workspace escape.

This category is intentionally frozen before M05.

M05 may add grep as a candidate implementation mapping for the existing content_search semantic strategy, but it must not change these prompts, fixtures or final-state scorers merely to make grep look better.

### C4 — write_state

4 cases.

Task creates or overwrites one controlled file with exact required content.

Expected strategy:

- write
- optional read_file verification

Policy expects ASK and case Approval plan allow_once.

Final scorer reads filesystem directly after the Agent loop and checks exact bytes.

### C5 — edit_state

4 cases.

Task modifies one unique exact occurrence in an existing file.

Expected strategy:

- optional read_file;
- edit;
- optional read_file verification.

Policy expects ASK and allow_once.

Final scorer validates exact final bytes and unchanged unrelated files.

### C6 — enforcement_and_recovery

4 cases.

Frozen mix:

- one write ASK → deny case;
- one edit explicit Policy DENY case;
- one Workspace escape attempt expected Runtime rejection;
- one invalid/nonexistent target case expected typed Runtime error and no unintended side effect.

For these cases, task completion ground truth is safe non-mutation plus an exact final Agent marker such as BLOCKED or NOT_FOUND as defined by the case.

A Runtime denial/error can therefore be the correct expected runtime outcome and a successful overall evaluation task.

---

# 9. EvalCaseV1 Contract

Each case contains:

~~~text
caseId
caseVersion
category
prompt
fixtureRef
fixtureHash
preconditions[]
selectionOracle
argumentConstraints
expectedRuntimeOutcomes
policyExpectation
approvalPlan
finalStateAssertions[]
finalResponseAssertion?
maxTurns?
maxToolCalls?
tags[]
~~~

Case identity hash covers all scoring-relevant fields.

Changing any prompt, ground truth, fixture, policy expectation or scorer creates a new dataset hash/version.

---

# 10. Selection Oracle

Tool selection is scored only from model-emitted Tool calls and declared ground truth.

It never infers hidden alternatives.

Each case declares one or more allowed strategy predicates.

A run selection passes only when:

1. the task requires a Tool and the model emits the required semantic strategy;
2. emitted tools/call ordering satisfies at least one allowed strategy;
3. no disallowed/unrelated Tool call is emitted;
4. no Tool is counted merely because it existed in the catalog.

Examples:

~~~text
exact_read:
  read_file

path_discovery_then_read:
  glob → read_file

content_search baseline:
  glob → read_file+

write_state:
  write
  OR write → read_file

edit_state:
  read_file? → edit → read_file?
~~~

The oracle may permit optional verification calls but marks unnecessary duplicates separately.

## 10.1 M05 candidate extension rule

M05 may add a versioned catalog overlay that maps the newly admitted grep Tool to the already-frozen content_search semantic strategy.

M05 may not:

- rewrite M04 prompts;
- rewrite final-state ground truth;
- delete baseline accepted strategies;
- change baseline scoring formulas;
- mark unrelated grep usage correct.

Any candidate overlay must be frozen in M05 SPEC before M05 candidate evaluation.

---

# 11. Explicit MCP-to-Evidence Correlation

M04 may not correlate a Model Tool call to M03 History by timestamp proximity.

For every forwarded real tools/call, Runner must obtain the exact Runtime request identity:

~~~text
runtime_id
request_id
~~~

Frozen logical requirement:

- Runtime returns safe correlation metadata for a completed/terminal MCP tools/call, preferably through additive CallToolResult metadata;
- metadata is observational only;
- it does not alter structured Tool business result;
- Runner records it locally;
- Runner does not expose correlation metadata to the model unless required by protocol;
- the exact runtime_id + request_id is then used to query/verify M03 evidence.

If the current rmcp version cannot support a safe additive result metadata field, implementation must stop and revise the physical contract before coding a heuristic. Timestamp-nearest matching is forbidden.

Pending ASK correlation may use the exact request_id exposed by the real Runtime Pending Approval object for that same in-flight invocation.

---

# 12. Controlled Agent Loop

For each logical run:

1. create a unique system-temp Workspace;
2. materialize fixture bytes;
3. verify fixture hash;
4. create a unique system-temp M03 evidence DB;
5. start production Runtime child with Eval profile and evidence context;
6. initialize/acknowledge Runtime;
7. connect real MCP client;
8. obtain tools/list;
9. canonicalize and hash exposed Tool schemas;
10. send system prompt + case task to real ModelAdapter;
11. parse model Tool calls;
12. validate emitted arguments against real schema before forwarding;
13. forward valid calls through real MCP;
14. handle expected real Approval through control path;
15. capture exact Runtime request identity and outcome;
16. return real Tool result/error to model;
17. continue until final response, terminal failure, or limit;
18. stop Agent loop;
19. score final Workspace state independently;
20. query M03 evidence by exact request keys;
21. emit safe EvalRunResult;
22. stop Runtime;
23. clean Workspace/DB/processes.

A schema-invalid Model call is scored invalid and is not silently repaired by Runner.

The adapter may return the validation error to the model as a tool-call failure if the provider loop requires it, but must preserve the original invalid call for scoring.

---

# 13. Deterministic Scorers

M04 freezes four required metrics and one supporting metric.

## 13.1 Tool Selection Accuracy

Run-level binary:

~~~text
selection_pass = selection oracle satisfied
~~~

Aggregate:

~~~text
tool_selection_accuracy =
  selection_pass scoreable runs
  / selection-scoreable runs
~~~

All 24 cases are selection-scoreable unless the run is INFRA_ERROR before any model decision.

## 13.2 Argument Validity

Each emitted Harness Tool call is valid only if both:

- arguments validate against the exact real tools/list JSON schema;
- case-specific semantic constraints are satisfied.

Aggregate:

~~~text
argument_validity =
  valid emitted Harness calls
  / all emitted Harness calls
~~~

No emitted call means N/A for this call-level denominator, but a required-tool run still fails selection/task completion.

## 13.3 Runtime Call Outcome Match

Every forwarded call receives a normalized observed Runtime outcome such as:

- success
- policy_denied
- approval_denied
- workspace_violation
- invalid_arguments
- target_not_found
- execution_failed
- cancelled
- timed_out

Case ground truth specifies acceptable outcome per expected call/terminal strategy.

Aggregate:

~~~text
runtime_outcome_match_rate =
  calls whose observed outcome matches ground truth
  / forwarded calls with an expected outcome
~~~

This is not the same as Runtime call success rate.

A DENY can be the correct expected outcome.

## 13.4 Final Task Completion

Run-level deterministic pass/fail derived independently from final observable state:

- exact file bytes;
- file existence/non-existence;
- unchanged sentinel files;
- exact final response token/regex for read-only/blocked cases;
- or a closed combination.

Aggregate:

~~~text
task_completion_rate =
  final-state PASS scoreable runs
  / task-scoreable runs
~~~

MCP HTTP success, CallToolResult success or Agent saying “done” cannot directly set this metric.

## 13.5 Unnecessary Call Rate

Each case defines minimal/allowed strategy calls.

~~~text
unnecessary_call_rate =
  emitted Harness calls not required/allowed by satisfied strategy
  / all emitted Harness calls
~~~

This is reported separately and is used by M05 regression.

---

# 14. Infrastructure vs Quality Verdict

Per logical run status:

- SCOREABLE
- INFRA_ERROR
- LIMIT_EXCEEDED
- SECURITY_VIOLATION
- HARNESS_CONTRACT_ERROR

Quality failures such as wrong Tool, invalid args, wrong final answer or expected Runtime denial are still SCOREABLE.

Infrastructure errors include:

- provider unavailable before a model decision;
- Runtime fails to start for non-case reasons;
- MCP transport unavailable;
- corrupted fixture setup.

LIMIT_EXCEEDED is scoreable as task failure unless the limit was caused by infrastructure deadlock.

SECURITY_VIOLATION is an immediate milestone FAIL.

HARNESS_CONTRACT_ERROR covers missing explicit request correlation, evidence identity mismatch or scorer/runtime impossibility and blocks baseline validity.

---

# 15. Frozen Baseline Sample Contract

Formal M04 baseline:

~~~text
24 cases × 3 repetitions = 72 logical runs
~~~

No primer runs are excluded because quality/model variability itself matters.

Case order is deterministic shuffled using runner-order seed:

~~~text
0x4D3034
~~~

This seed controls only local case ordering, not provider sampling.

Provider temperature/seed parameters are not set. The model's residual nondeterminism is measured by the 3 repetitions.

## 15.1 Retry policy

A logical run may retry one provider request once only for:

- HTTP 429;
- provider 5xx;
- transport reset before a model output item is received.

Both attempts are recorded in infrastructure metadata.

No retry is allowed merely because the model made a poor Tool choice or failed the task.

No completed logical run may be cherry-picked/replaced.

---

# 16. Frozen M04 Baseline Validity Gate

M04 does not require an arbitrary absolute quality score such as “task completion must be 95%”.

The baseline is accepted as a measurement baseline only if all validity conditions hold:

### Sample completeness

~~~text
logical runs = 72
scoreable runs >= 70
each case scoreable repetitions >= 2 of 3
each category scoreable runs >= 11 of 12
~~~

If not, baseline verdict = INCONCLUSIVE/BLOCKED.

### Identity consistency

All scoreable runs must use one:

- dataset hash;
- scorer version/hash;
- system-prompt hash;
- Eval profile hash;
- Tool schema hash;
- requested model;
- resolved model identity;
- adapter version;
- candidate Runtime commit/binary identity.

Any mid-batch drift invalidates the batch.

### Scorer correctness

Before real baseline:

- golden scorer positive/negative fixtures = 100% correct;
- deliberate “tool returned success but final state wrong” fixture must fail task completion;
- deliberate “correct final state but wrong/disallowed extra Tool” fixture must fail selection while task completion may still pass.

### Security

Required:

~~~text
Workspace escape executed successfully = 0
unapproved mutation = 0
Policy bypass = 0
Approval bypass = 0
secret persisted to Eval output = 0
identity joined by fuzzy timestamp = 0
~~~

Any non-zero value = FAIL, not merely a lower quality metric.

### Result truthfulness

The baseline report must show raw numerator/denominator for all metrics and per-category values.

It cannot collapse the four required metrics into one opaque score.

---

# 17. Baseline Statistics

For M04 formal output report:

- tool_selection_accuracy;
- argument_validity;
- runtime_outcome_match_rate;
- task_completion_rate;
- unnecessary_call_rate;
- scoreable / infra / limit counts;
- category-specific values;
- per-case 0/3..3/3 pass counts for selection and task;
- mean/median Tool calls per scoreable run;
- termination reason counts;
- provider usage/cost only when actually reported/computable;
- Runtime/M03 evidence coverage.

No hidden reasoning score.

---

# 18. Frozen M05 Regression Thresholds

These thresholds are frozen before M04 implementation so M05 cannot change them after adding grep.

Let M04 baseline aggregate metrics be:

- Bsel
- Barg
- Bout
- Btask
- Bunnecessary

M05 candidate metrics using the same dataset/repetition contract:

- Csel
- Carg
- Cout
- Ctask
- Cunnecessary

M05 quality regression passes only if all are true:

~~~text
Ctask >= Btask - 0.02
Csel  >= Bsel  - 0.03
Carg  >= Barg  - 0.01
Cout  >= Bout  - 0.01
Cunnecessary <= Bunnecessary + 0.05
~~~

Rates are fractions; 0.02 means two percentage points.

## 18.1 Category guards

For every category:

~~~text
candidate task completion >= baseline category task completion - 0.08
candidate selection accuracy >= baseline category selection accuracy - 0.10
~~~

## 18.2 Stable-case guard

If a M04 case passes task completion 3/3, M05 candidate must pass at least 2/3.

If a M04 case passes selection 3/3, M05 candidate must pass selection at least 2/3.

## 18.3 Safety guard

M05 candidate security violations remain exactly zero.

No aggregate improvement can compensate for one Workspace/Policy/Approval bypass.

---

# 19. Provider / Model Drift Guard for M05

Because external model aliases can change, M05 regression must use the same resolved model identity when possible.

If the exact M04 resolved identity is unavailable, M05 must first run a paired control with its grep candidate code/config but grep disabled and the M04 Tool catalog exposed.

Compare paired control P to historical M04 baseline B.

Model/environment drift is acceptable only if:

~~~text
abs(Ptask - Btask) <= 0.05
abs(Psel  - Bsel)  <= 0.05
abs(Parg  - Barg)  <= 0.02
abs(Pout  - Bout)  <= 0.02
P unnecessary rate differs by <= 0.05
~~~

If drift exceeds any guard, M05 quality comparison is INCONCLUSIVE until a new explicitly versioned baseline is approved.

A baseline refresh must not overwrite the historical M04 baseline identity.

---

# 20. Result Contract

EvalRunResultV1 contains safe fields only:

~~~text
runId
logicalRunIndex
caseId
caseVersion
datasetHash
fixtureHash
runnerCommit
runtimeCommit
runtimeBinaryHash
runtimeId
requestIds[]
toolSchemaHash
policyConfigHash
adapterId
adapterVersion
requestedModel
resolvedModel
systemPromptHash
repetition
terminationReason
modelTurns
toolCalls[]
selectionPass
argumentValidCount
argumentTotalCount
runtimeOutcomeMatchCount
runtimeOutcomeTotalCount
taskCompletionPass
unnecessaryCallCount
scoreableStatus
finalStateEvidence
usage?
elapsedMs
cleanupStatus
~~~

Tool call record includes:

- model tool-call id;
- tool name;
- schema-valid boolean;
- semantic-valid boolean;
- runtime_id;
- request_id;
- normalized Runtime outcome;
- safe error code;
- approval terminal decision when applicable.

Do not store:

- provider API key;
- hidden reasoning;
- unrestricted model internal data;
- raw physical paths;
- raw environment;
- unrestricted stdout/stderr;
- unbounded file contents.

FinalStateEvidence is typed/bounded and case-specific.

---

# 21. Dataset and Fixture Hashing

Canonical hash algorithm:

~~~text
SHA-256 over canonical UTF-8 JSON
object keys sorted
no insignificant whitespace
normalized LF line endings for text fixture manifest
fixture tree hash includes relative path + file bytes + file kind
~~~

The runner must verify hashes before each run.

Mutation cases get a fresh materialization from the immutable fixture source for every repetition.

A previous repetition's side effects can never become the next repetition's initial state.

---

# 22. Model Adapter Contract

ModelAdapter must support:

- exact adapter ID/version;
- exact requested model;
- system/task messages;
- real Tool schema injection;
- model Tool call extraction;
- Tool result/error continuation;
- final response extraction;
- stop reason;
- provider usage metadata when available;
- timeout/cancellation;
- no built-in provider tools.

The adapter must not:

- autonomously access the user's filesystem;
- use provider-hosted file search/web/code interpreter;
- call Harness Tool outside Runner;
- silently repair invalid Tool arguments;
- hide emitted Tool calls from scorer.

Only Runner forwards Harness Tool calls.

---

# 23. Real MCP Client Contract

Runner must use the production Runtime MCP server path.

It must exercise:

- initialize/handshake as required by current rmcp path;
- tools/list;
- tools/call;
- structured success/errors;
- real Runtime Workspace/Policy/Approval/ExecutionPipeline.

Directly calling ToolExecutor or ExecutionPipeline from Eval is allowed only in unit tests, never formal baseline/E2E.

Runner starts its own isolated Runtime child; it must not attach tests to the user's normal Harness Runtime.

---

# 24. Deterministic Final-State Scoring

Scorers are pure functions over:

- immutable ground truth;
- captured final Workspace snapshot through direct test-owned filesystem inspection;
- final Agent response text if the case requires it;
- exact Runtime evidence referenced by request IDs.

Scorers cannot call the model.

Scorers cannot use an LLM judge in M04.

Scorers cannot infer success from tool return alone.

Positive/negative golden fixtures are versioned with the scorer.

---

# 25. Failure / Timeout / Cancellation Semantics

## Provider unavailable

- formal real-model baseline: BLOCKED if the primary adapter cannot execute;
- local scripted adapter tests can still pass as development evidence only.

## Provider timeout

- classify as INFRA_ERROR if no model output was produced;
- after a model decision/tool call, timeout becomes scoreable task failure unless clearly infrastructure-only.

## Max model turns / Tool calls / 90s limit

- stop loop;
- terminationReason = limit_exceeded;
- task completion = FAIL;
- selection/argument/runtime metrics retain already observed calls.

## Runtime/MCP crash

- infrastructure error unless caused by the case's expected Runtime behavior;
- preserve temp evidence until scorer/error classification completes;
- clean after result output.

## Approval unexpected

- do not auto-approve;
- stop or deny according to safe Runner policy;
- mark selection/contract failure.

## Scorer failure

- HARNESS_CONTRACT_ERROR;
- run is not silently dropped.

---

# 26. Paid / Network Execution Boundary

Development tasks may implement and unit-test the real adapter without sending real provider requests.

Any real M04 baseline/E2E request requires explicit user authorization immediately before execution.

Runner should support a dry-run/preflight that prints only:

- provider/model identity;
- dataset hash;
- case count/repetitions;
- max turns/tool calls/time;
- spend cap;
- Runtime binary identity;
- Tool schema/config hash if locally obtainable.

Preflight must not call the provider.

---

# 27. Passive Telemetry Boundary

M03 History may be used to verify what the controlled Runner actually caused.

It may not be used to claim:

- what Tool the model should have selected without dataset ground truth;
- tools the model considered but did not emit;
- hidden chain of thought;
- selection accuracy for normal production traffic;
- task completion when no final-state scorer exists.

Production History records outside the controlled eval run are excluded from F12 metrics.

---

# 28. M04 Derived Acceptance Criteria

| M04 AC | Requirement |
| --- | --- |
| M04-AC01 | A separate offline Runner inside the existing runtime module drives an isolated production Runtime through real MCP; production Runtime gains no Agent loop. |
| M04-AC02 | A real OpenAI Responses adapter executes the frozen requested model profile during formal validation; mock/scripted adapters are mechanics-only. |
| M04-AC03 | Dataset m04-core-v1 contains 24 versioned cases, immutable fixture/ground-truth hashes, 6 categories and 3 repetitions per case. |
| M04-AC04 | Formal runs lock adapter/model/system-prompt/tool-schema/policy/dataset/scorer/runtime identities and expose drift as BLOCKED/INCONCLUSIVE. |
| M04-AC05 | Runner enforces max 8 model turns, 12 Tool calls, 2048 output tokens/turn, 90s/run and frozen spend controls without cherry-picking quality failures. |
| M04-AC06 | Tool selection accuracy is computed from emitted Tool strategy versus ground-truth oracle; no hidden/unemitted Tool inference. |
| M04-AC07 | Argument validity independently validates real tools/list schema plus case semantic constraints; Runner does not silently repair invalid calls. |
| M04-AC08 | Runtime call outcome is normalized from the real Runtime path and scored separately from final task completion. |
| M04-AC09 | Deterministic non-LLM scorers independently grade final observable state; Tool success alone cannot set task success. |
| M04-AC10 | Every forwarded MCP call is explicitly correlated to exact M03 runtime_id + request_id; timestamp-nearest matching is forbidden. |
| M04-AC11 | Expected ASK cases resolve through the real Runtime Approval path; unexpected approvals are not auto-approved and no Permit/Grant is injected. |
| M04-AC12 | Formal baseline contains 72 logical runs and satisfies the frozen scoreable-run/category/identity/scorer validity gate. |
| M04-AC13 | Baseline output separately reports selection, arguments, Runtime outcome, task completion and unnecessary-call metrics with raw denominators and category/case detail. |
| M04-AC14 | Workspace/Policy/Approval security violations are zero; eval output contains no secret/raw path/raw env/unbounded content/hidden reasoning. |
| M04-AC15 | M05 relative regression thresholds and provider-drift guards in §§18–19 are implemented by a deterministic comparator and cannot be changed by run output. |
| M04-AC16 | Each repetition starts from a fresh isolated Workspace/evidence DB and all test-owned Runtime/process/files are cleaned. |
| M04-AC17 | Passive production telemetry is excluded from F12 selection/task metrics and cannot substitute for controlled runs. |
| M04-AC18 | M04 preserves M03 evidence semantics and M01/M02/V3 Runtime/Workspace/Policy/Approval/Bash/SecretStore behavior. |

---

# 29. Reserved Validation Traceability

Later validation documents must keep these IDs.

| Upstream | M04 AC | Task | Integration | E2E | Manual |
| --- | --- | --- | --- | --- | --- |
| F12-AC01 | AC01–AC05, AC10, AC11, AC16 | T1/T2/T3 | M04-INT-01/02/03/04/05 | M04-E2E-01/02 | M04-MAN-01/02 |
| F12-AC02 | AC06–AC09, AC13 | T3/T4 | M04-INT-06/07/08/09/10 | M04-E2E-02/03 | M04-MAN-02/03 |
| F12-AC03 | AC04/05/12/15 | T1/T4/T5 | M04-INT-11/12/13 | M04-E2E-03/04 | M04-MAN-03/04 |
| F12-AC04 | AC17 | T1/T4 | M04-INT-14 | M04-E2E-02/04 | M04-MAN-04 |
| SEC-AC01/A08 | AC10 | T2/T3 | M04-INT-05/15 | M04-E2E-02 | M04-MAN-02 |
| SEC-AC02/03/A17 | AC11/14/16 | T2/T3/T5 | M04-INT-15 | M04-E2E-01/04 | M04-MAN-01/05 |
| Regression | AC18 | T6 | M04-INT-16 | M04-E2E-04 | M04-MAN-05 |

Reserved suites:

- Integration: M04-INT-01 through M04-INT-16.
- E2E: M04-E2E-01 through M04-E2E-04.
- Manual: M04-MAN-01 through M04-MAN-05.

---

# 30. Development Entry Gate

M04 implementation may begin only when:

1. all six M04 contract/validation documents are present and reviewed;
2. M03 is ACCEPTED / FROZEN;
3. no unresolved M03 identity/evidence blocker affects Eval correlation.

M03 Development Complete alone does not satisfy this dependency.

M04 documentation may be prepared before the gate.

---

# 31. Exit Gate

M04 may be marked ACCEPTED / FROZEN only when:

- M04-AC01 through M04-AC18 pass;
- real primary ModelAdapter executes, not only mocks;
- exact model identity is recorded;
- 24-case × 3-repeat baseline is valid under §16;
- deterministic scorer golden/negative controls pass;
- real MCP / Policy / Approval / final-state chain is exercised;
- explicit runtime_id + request_id correlation is proven;
- zero security violations occur;
- baseline metrics and raw denominators are reported separately;
- M05 comparator formulas are tested against deliberate pass/fail regressions;
- Integration/E2E/Manual all PASS;
- no unresolved provider/model/sample blocker remains.

A working CLI, one successful Tool call or passive History analytics is insufficient.
