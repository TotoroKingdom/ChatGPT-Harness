# M04 TASKS — Offline Controlled Agent Evaluation Baseline

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: IMPLEMENTATION BLOCKED UNTIL M03 ACCEPTED / FROZEN AND M04 VALIDATION DOCS ARE FROZEN  
> Scope: F12 only  
> Rule: do not make paid/networked provider calls without explicit user authorization.  
> Layout: runtime/src/evaluation + runtime/src/bin/m04_eval.rs + fixtures/v4/m04-eval; no new top-level project.

---

## 1. Dependency

~~~text
M03 ACCEPTED / FROZEN
        ↓
M04 six docs frozen
        ↓
T1 Dataset/contract
        ↓
T2 Runtime/MCP/evidence correlation
        ↓
T3 Model adapter/agent loop/Approval
        ↓
T4 Scoring/aggregate/M05 comparator
        ↓
T5 CLI validate/preflight/scripted controls
        ↓
T6 Regression/Development Complete
        ↓
Formal Integration/E2E/Manual
~~~

M03 Development Complete alone does not unlock implementation.

---

# T1 — EvalCase v1, Frozen Dataset, Fixtures and Ground-truth Scorers

**AC:** M04-AC03, AC04, AC06, AC07, AC09, AC12, AC16  
**Primary areas:** runtime/src/evaluation case/dataset/fixture/scoring contracts; fixtures/v4/m04-eval.

## Implement

### 1. EvalCaseV1 parser/schema

Implement closed typed fields from SPEC.

Reject:

- unknown category;
- duplicate case ID/version;
- missing fixture;
- bad hash;
- unknown scorer;
- impossible approval plan;
- run limits above global frozen maxima;
- arbitrary JSON scorer payload.

### 2. Create dataset m04-core-v1

Exactly:

~~~text
24 cases
6 categories
4 cases/category
3 repetitions/case
72 logical formal runs
~~~

Categories:

- exact_read
- path_discovery
- content_search
- write_state
- edit_state
- enforcement_and_recovery

### 3. Fixtures

Use only synthetic small files.

Every fixture gets:

- immutable source tree;
- fixture hash;
- one or more untouched sentinel files;
- expected final-state data.

No secrets/real user content.

### 4. Ground truth

For each case freeze:

- prompt;
- accepted semantic Tool strategy;
- semantic argument constraints;
- expected Runtime outcomes;
- Policy expectation;
- Approval plan;
- final filesystem/response assertion.

### 5. content_search future compatibility

Baseline accepted current strategy is glob → read_file+.

Do not mention/use grep as an existing Tool.

Implement semantic strategy abstraction so a future M05 overlay can explicitly map an admitted grep to content_search without editing M04 dataset.

### 6. Scorer golden fixtures

Unit tests must prove:

- correct final state PASS;
- Tool success + wrong final state FAIL;
- wrong/disallowed Tool + coincidental final state means selection FAIL;
- schema-valid but wrong semantic path means argument invalid;
- deny/no mutation expected case PASS;
- unintended mutation FAIL.

### 7. Canonical hashing

Implement SHA-256 canonical hash contract for:

- dataset;
- case;
- fixture tree;
- system prompt;
- scorer version.

### Verification

At minimum:

~~~text
cargo test -p chatgpt-harness-runtime evaluation
cargo check --workspace
~~~

Use exact filters that match final module/test names.

### Done

Dataset/scorer contract is frozen in code and independent of model/provider.

---

# T2 — Isolated Production Runtime, Real MCP Client and Exact M03 Correlation

**AC:** M04-AC01, AC04, AC10, AC11, AC14, AC16  
**Depends on:** T1.

## Implement

### 1. EvalRuntimeHarness

Start the actual production Runtime executable as a child.

Generate per-run:

- temp Workspace;
- temp M03 DB;
- Eval AppConfig/control initialization;
- host/evidence context;
- unique process ownership metadata.

Never attach formal evaluation to the user's active Runtime.

### 2. Frozen Tool/Policy profile

Real effective profile:

- read_file enabled ALLOW;
- glob enabled ALLOW;
- write enabled ASK except explicit DENY cases;
- edit enabled ASK except explicit DENY cases;
- bash disabled;
- no grep.

Use current real Policy schema/rules to realize semantics.

### 3. MCP client

Use production MCP initialize/tools/list/tools/call path.

Canonicalize tools/list JSON schema and hash it.

Fail preflight if unexpected catalog drift occurs.

### 4. Exact request correlation

Implement safe additive Runtime correlation metadata:

~~~text
runtime_id
request_id
~~~

for each Tool call result/terminal response using the supported MCP metadata mechanism.

Do not change business structured_content.

Tests prove the returned key exactly matches:

- live Activity;
- M03 request_summary/evidence_events;
- Pending Approval request_id for ASK.

No timestamp-nearest matching anywhere.

If rmcp cannot support safe result metadata, STOP T2 and report SPEC implementation blocker; do not invent a heuristic.

### 5. M03 evidence verification

After Tool terminal state, use exact runtime_id + request_id to query temp evidence DB.

Verify COMPLETE/expected stage for normal terminal cases.

Evidence store failure is HARNESS_CONTRACT_ERROR for formal Eval unless the case specifically tests infrastructure failure.

### 6. Cleanup

Stop child via normal control path, then bounded kill only if needed.

Remove temp Workspace/DB.

Never kill user's Runtime.

### Tests

- tools/list exact baseline catalog;
- success read;
- glob;
- write ASK;
- edit ASK;
- policy deny;
- workspace violation;
- unknown/missing target;
- request ID correlation;
- runtime restart gets new runtime_id;
- two runs never share DB/Workspace;
- cleanup on panic/error.

### Done

Runner can drive a real isolated Runtime and map every call to M03 identity.

---

# T3 — ModelAdapter, OpenAI Responses Adapter, Controlled Agent Loop and Approval Controller

**AC:** M04-AC02, AC05, AC07, AC08, AC11, AC14, AC17  
**Depends on:** T1/T2.

## Implement

### 1. ModelAdapter trait

Typed interface for:

- start turn;
- supply system/task/tool schemas;
- receive Tool calls/final response;
- send Tool results;
- expose provider/resolved model identity;
- usage metadata;
- stop reason;
- timeout/cancel.

### 2. Scripted adapter

Implement deterministic scripted adapter for unit/integration mechanics.

Clearly mark adapter class NON_FORMAL.

It may not satisfy real baseline/E2E.

### 3. OpenAI Responses adapter

Frozen:

~~~text
adapter_id = openai-responses-v1
requested_model = gpt-6.1-terra
max_output_tokens = 2048
built-in provider tools = none
temperature/seed = omitted
credential source = M04_EVAL_OPENAI_API_KEY or equivalent secure execution injection
~~~

No network call in ordinary unit tests.

Use an injectable HTTP boundary/recorded safe protocol fixtures where needed.

### 4. Controlled loop

Enforce:

- max 8 model turns;
- max 12 Harness Tool calls;
- max 90s;
- spend caps when computable.

Do not repair model arguments.

Record invalid calls as emitted.

Only schema-valid calls reach MCP.

Return real Tool result/error to model.

### 5. Approval controller

For expected ASK:

- wait/query exact Pending request;
- verify expected Tool/request;
- apply case allow_once or deny via normal approval method;
- collect terminal decision.

Unexpected approval:

- never auto-allow;
- terminate safely;
- score/report contract/quality failure.

No SessionGrant injection.

### 6. Provider retry

Only one retry for 429/5xx/transport reset before model output.

No quality retry.

### 7. Privacy

Result/log redaction tests prove API key absent.

Do not retain provider hidden/reasoning data.

### Verification

- adapter parsing fixtures;
- Tool call extraction;
- invalid args preserved;
- provider error mapping;
- retry rule;
- max step/tool/time;
- approval allow/deny;
- unexpected approval;
- credential redaction;
- cancellation.

### Done

Real controllable adapter path exists without unauthorized paid execution.

---

# T4 — Deterministic Metrics, Baseline Validator and M05 Regression Comparator

**AC:** M04-AC06–AC09, AC12, AC13, AC15, AC17  
**Depends on:** T1–T3 contracts.

## Implement

### 1. Run-level scoring

Compute independently:

- selectionPass;
- argument valid/total;
- runtime outcome match/total;
- task completion;
- unnecessary call count;
- termination/status.

No composite hidden score.

### 2. Selection oracle

Implement frozen semantic strategies and optional verification-call handling.

Content-search baseline requires current strategy.

Future candidate overlay API must be explicit/versioned and cannot mutate dataset.

### 3. Argument scorer

Validate:

1. exact real Tool JSON schema;
2. case semantic constraints.

Both required.

### 4. Runtime outcome normalization

Map current Runtime result/error codes into the frozen normalized outcome set.

Unknown new code becomes typed UNKNOWN/contract result, not silently success.

### 5. Final-state scorer

Pure deterministic functions.

No model judge.

Filesystem scorer reads only test-owned Workspace.

### 6. Aggregate

Produce:

- required four metrics;
- unnecessary call rate;
- category metrics;
- per-case 0/3..3/3;
- scoreable/infra/limit/security;
- tool call counts;
- termination reasons;
- usage if available.

### 7. Baseline validity gate

Implement constants from SPEC §16.

Tests boundary:

- 70/72 scoreable passes;
- 69/72 fails;
- 2/3 per case passes;
- 1/3 fails;
- 11/12 category passes;
- identity hash drift fails;
- model identity drift fails;
- any security violation fails.

### 8. M05 comparator

Implement SPEC §§18–19 exactly.

Thresholds are code constants associated with comparator version.

Cannot be passed as permissive CLI arguments.

### Comparator negative tests

- task -2pp exact PASS; just below FAIL;
- selection -3pp exact PASS; below FAIL;
- arg/runtime -1pp;
- unnecessary +5pp;
- category guards;
- stable 3/3→1/3 FAIL;
- security violation FAIL;
- provider drift limit exact PASS / exceed INCONCLUSIVE.

### Done

M04 produces reproducible quality metrics and a frozen future admission gate.

---

# T5 — CLI validate / preflight / scripted run and Formal-run Readiness

**AC:** M04-AC01–AC05, AC10, AC12, AC16  
**Depends on:** T1–T4.

## Implement CLI

Binary:

~~~text
m04_eval
~~~

Required logical operations:

### validate

- parse dataset;
- verify hashes;
- verify case/scorer consistency;
- no provider;
- no Runtime required if not needed.

### preflight

- build/locate Runtime candidate;
- create one disposable fixture;
- start isolated Runtime;
- query real tools/list;
- verify Tool schema hash/profile;
- execute a safe scripted MCP correlation probe;
- verify M03 evidence identity;
- print requested provider/model/run limits/spend cap;
- clean up;
- NO provider request.

### run scripted

- deterministic fake adapter;
- selectable case subset for developer tests;
- output EvalRunResult/Aggregate;
- must label result NON_FORMAL.

### run real

Implement command path but require:

- explicit environment opt-in such as M04_EVAL_ALLOW_NETWORK=1;
- credential presence;
- exact frozen provider/model;
- full formal mode rejects case subset/skips by default.

This code path exists, but T5 must not execute it without user authorization.

## Output

- safe human summary;
- JSON stdout option;
- no automatic repository result file.

## Cleanup assertions

After every command:

- no owned Runtime;
- no pending approvals;
- temp Workspace removed;
- temp DB/lock removed where allowed;
- no provider credential printed.

### Done

Formal E2E can be started later with explicit authorization and without code changes.

---

# T6 — Cross-cutting Regression, Packaging Boundary and Development Complete

**AC:** M04-AC14, AC16, AC18; closes all M04 ACs locally  
**Depends on:** T1–T5.

## 1. Dependency / production isolation audit

Prove:

- production Runtime startup does not invoke Eval Runner;
- desktop app does not require provider API credential;
- no Eval Agent loop is linked into Runtime request handling;
- Eval cannot bypass Workspace/Policy/Approval;
- no new top-level directory;
- no new production Tool registration.

## 2. M03 regression

Run affected M03 suites:

- canonical request identity;
- Activity runtime_id/event_id;
- evidence writer;
- History query/read-only;
- Approval history separation;
- privacy projections.

Correlation metadata must not change M03 DB schema v1.

## 3. V3/M01/M02 regression

Affected tests:

- Workspace boundary;
- Tool toggle;
- Policy ALLOW/ASK/DENY;
- Approval/SessionGrant;
- write/edit atomic behavior;
- Bash remains controlled/disabled under Eval profile;
- SecretStore;
- Start/Stop/Restart;
- Tray/background/notifications;
- startup recorder;
- one-click diagnostics.

## 4. Local evaluation matrix

Run all 24 cases through scripted deterministic adapters sufficient to exercise:

- selection pass/fail;
- invalid args;
- success/deny/workspace error;
- Approval allow/deny;
- final-state pass/fail;
- limits;
- infra classification;
- cleanup.

This is not real-model baseline.

## 5. Full checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Run repository deliver-milestone/layout checks as required by AGENTS.

M04 formal runner itself need not be bundled into user Portable unless current delivery script requires all workspace binaries; if included, it must not surface as a user-facing production feature.

## 6. Development Complete response

Report in conversation:

- candidate SHA;
- Runner/adapter/scorer versions;
- dataset/system-prompt/fixture hashes;
- Runtime/Tool schema hashes from preflight;
- exact local commands/counts;
- comparator boundary tests;
- scripted aggregate;
- provider real run = NOT EXECUTED unless explicitly authorized;
- remaining formal Integration/E2E/Manual.

Do not create a seventh milestone file.

### Done

M04 Development Complete means implementation is ready for formal validation, not that F12 baseline has been accepted.

Do not mark M04 ACCEPTED/FROZEN.

---

## 2. Task → AC → Reserved Validation Traceability

| Task | M04 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC03/04/06/07/09/12/16 | M04-INT-01/06/07/09/11, M04-E2E-01/03, M04-MAN-01/03 |
| T2 | AC01/04/10/11/14/16 | M04-INT-02/03/04/05/15, M04-E2E-01/02, M04-MAN-01/02 |
| T3 | AC02/05/07/08/11/14/17 | M04-INT-04/07/08/14, M04-E2E-02/03, M04-MAN-02/04 |
| T4 | AC06–AC09/12/13/15/17 | M04-INT-06/07/08/09/10/11/12/13/14, M04-E2E-03/04, M04-MAN-03/04 |
| T5 | AC01–AC05/10/12/16 | M04-INT-01/02/05/11, M04-E2E-01/02, M04-MAN-01 |
| T6 | AC14/16/18 | M04-INT-15/16, M04-E2E-04, M04-MAN-05 |

---

## 3. Development Handoff

After T6:

~~~text
M04 Development Complete
→ independent Integration
→ explicit user authorization for real paid/networked model execution
→ M04 real-model E2E baseline
→ Human Manual Acceptance
→ user sign-off
→ M04 ACCEPTED / FROZEN
→ unlock M05 grep + quality regression
~~~

A scripted adapter or one local Tool call cannot unlock M05.
