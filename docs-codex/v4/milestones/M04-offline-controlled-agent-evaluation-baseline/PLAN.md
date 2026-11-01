# M04 PLAN — Offline Controlled Agent Evaluation Baseline

> Input: SPEC.md  
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M03 ACCEPTED / FROZEN  
> Scope: F12 only  
> Dependency: M03 ACCEPTED / FROZEN  
> Layout: use runtime/src/evaluation, runtime/src/bin and fixtures/v4/m04-eval; no new top-level project.  
> External rule: real provider calls require explicit user authorization; development defaults to local deterministic mechanics until authorized.

---

## 1. Delivery Strategy

M04 is split so the quality contract is fixed before any real baseline is observed:

~~~text
M03 ACCEPTED / FROZEN
        ↓
M04 six docs frozen
        ↓
T1 Dataset / manifest / scorer contract
        ↓
T2 Real Runtime + MCP harness + request correlation
        ↓
T3 Model adapter + controlled Agent/Approval loop
        ↓
T4 Deterministic scoring + aggregate + M05 comparator
        ↓
T5 Full local dry-run / scripted controls / real-run preflight
        ↓
T6 Regression / packaging boundary / Development Complete
        ↓
Formal Integration
        ↓
Authorized real-model E2E baseline
        ↓
Manual Acceptance
~~~

Real provider execution belongs to formal validation unless the user explicitly authorizes it earlier.

---

## 2. Module Ownership

| Capability | Owner |
| --- | --- |
| Eval CLI entry | runtime/src/bin/m04_eval.rs |
| case/dataset manifest | runtime/src/evaluation/case.rs + dataset.rs |
| fixture materialization | runtime/src/evaluation/fixture.rs |
| ModelAdapter trait | runtime/src/evaluation/adapter.rs |
| OpenAI Responses adapter | runtime/src/evaluation/openai_responses.rs |
| controlled Agent loop | runtime/src/evaluation/agent_loop.rs |
| real MCP client | runtime/src/evaluation/mcp_client.rs |
| expected Approval resolver | runtime/src/evaluation/approvals.rs |
| M03 request correlation | runtime/src/evaluation/correlation.rs + minimal Runtime MCP additive metadata |
| deterministic scoring | runtime/src/evaluation/scoring.rs |
| aggregate/comparison | runtime/src/evaluation/aggregate.rs |
| stdout JSON/human output | runtime/src/evaluation/output.rs |
| fixed dataset | fixtures/v4/m04-eval |
| Runtime authority | existing Runtime modules, unchanged |
| M03 evidence read | existing evidence-store reader / canonical DTO |

Evaluation code must not move authorization logic out of Runtime.

---

## 3. Phase A — Dataset / Manifest / Scorer Contract

### Dataset schema

Implement EvalCaseV1 exactly from SPEC.

Validate:

- unique case IDs;
- version fields;
- fixture reference exists;
- hashes match;
- category in frozen set;
- selection strategy known;
- policy/approval expectation coherent;
- final-state scorer type closed/typed;
- run limits do not exceed global maxima.

### Fixtures

Create 24 small synthetic cases.

Use no real user paths/content.

Every Workspace fixture must contain explicit sentinels proving unrelated files remain unchanged.

Content-search fixtures should be deliberately useful for M05 grep comparison while remaining solvable with current glob + read_file.

### Ground truth

Prefer exact deterministic assertions:

- exact answer token;
- file exact bytes;
- file exists/not exists;
- unchanged sentinel;
- typed expected Runtime outcome.

Do not use LLM-as-judge.

### Golden scorer fixtures

Create positive and negative controls:

- correct Tool + correct final state;
- Tool success but wrong final state;
- wrong Tool but coincidentally correct final state;
- valid schema but semantically wrong path;
- correct denial/no mutation;
- unexpected mutation.

Exit: dataset/scorer parser tests PASS before MCP/model implementation depends on them.

---

## 4. Phase B — Production Runtime / MCP Eval Harness

### Isolated Runtime child

Runner creates:

- temp Workspace;
- temp AppData/evidence location;
- generated Eval config;
- production runtime executable child.

Use production initialization/acknowledge and real loopback MCP endpoint.

### Real tools/list

Fetch actual Tool definitions and canonicalize/hash.

Reject baseline run if catalog is not exactly the frozen profile:

- read_file enabled;
- glob enabled;
- write enabled;
- edit enabled;
- bash disabled;
- no grep.

### Explicit request correlation

Add the smallest additive Runtime MCP result metadata necessary to return:

- runtime_id;
- request_id.

Use MCP result metadata if current rmcp supports it.

Keep Tool structured_content unchanged.

Runner strips/withholds correlation metadata from model-facing Tool result when feasible.

Contract tests prove metadata corresponds to M03 Activity/History exact request.

If SDK cannot support the frozen logical contract safely, stop and revise SPEC; do not use timestamp matching.

### Evidence

Initialize Runtime with temp M03 EvidenceStore enabled.

After every call, read/verify evidence using exact request key.

Exit: scripted MCP client can create success/ASK/DENY/error requests and map them exactly.

---

## 5. Phase C — Controllable Model Adapter and Agent Loop

### ModelAdapter trait

Provider-neutral API for:

- turn input;
- tools;
- output Tool calls;
- final message;
- usage/identity;
- cancellation.

### OpenAI Responses adapter

Implement frozen profile:

- adapter openai-responses-v1;
- requested model gpt-6.1-terra;
- built-in tools disabled;
- max output 2048 per turn;
- no explicit temperature/seed.

Credential only from secure execution-time source.

Never write credential to logs/results.

### Tool loop

For each model-emitted call:

1. preserve original name/arguments;
2. schema validate;
3. semantic grade;
4. if schema-valid, call real MCP;
5. handle only expected Approval;
6. collect explicit request identity;
7. normalize Runtime outcome;
8. return real Tool result/error to model;
9. continue within limits.

Schema-invalid call is not repaired.

### Approval controller

Poll/query real pending approvals for isolated Runtime.

Resolve only the exact current expected request.

Unexpected request remains unapproved and terminates safely.

Exit: scripted fake adapter tests all loop mechanics; real adapter compiles and dry-run preflight performs no network call.

---

## 6. Phase D — Scoring, Aggregate and Regression Comparator

### Run scoring

Compute independently:

- selection pass;
- valid argument calls;
- expected Runtime outcomes;
- final task pass;
- unnecessary calls.

Preserve numerator/denominator.

### Aggregate

Aggregate 72-run format but allow local small scripted suites for mechanics.

Report:

- total;
- categories;
- cases;
- termination;
- infra;
- security;
- tool-call counts.

### Baseline validity

Implement §16 as deterministic validator.

No mutable CLI flag may weaken thresholds.

### M05 comparator

Implement frozen formulas in SPEC §§18–19.

Tests must include deliberate boundary fixtures:

- exact pass boundary;
- one metric just beyond regression allowance;
- category-only failure;
- stable-case failure;
- security violation overriding aggregate improvement;
- model drift causing INCONCLUSIVE.

Comparator thresholds are constants/versioned contract, not result-file inputs.

Exit: scorer + comparator golden tests PASS.

---

## 7. Phase E — Dry-run and Preflight

Before any real provider call, Runner supports:

~~~text
m04_eval validate
m04_eval preflight
m04_eval run --adapter scripted ...
~~~

Exact CLI spelling may follow Clap conventions, but semantics are frozen.

### validate

- parses dataset;
- verifies hashes;
- validates scorer/strategy;
- no Runtime/model needed.

### preflight

- locates/builds production Runtime;
- creates temp fixture;
- starts isolated Runtime;
- lists Tool schema;
- verifies catalog/config/hash;
- verifies M03 evidence/correlation;
- prints proposed provider/model/run count/limits/spend cap;
- stops/cleans;
- does not contact model provider.

### scripted run

Uses deterministic local adapter for positive/negative loop tests.

Cannot be called formal baseline PASS.

Exit: QA can prove the real baseline is executable without paying for it yet.

---

## 8. Phase F — Development Completion

Run full local suite without paid/provider execution:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Add focused commands for evaluation tests/CLI as implemented.

Run repository milestone delivery/layout scripts required by AGENTS.

M04 does not need to ship Eval Runner in the user-facing Portable unless existing build packaging naturally includes auxiliary binaries and the SPEC validation plan explicitly needs it. The formal Eval CLI can remain a repository QA/development binary.

Do not expose a production UI button.

Development Complete output must report:

- candidate SHA;
- Runner version;
- dataset/scorer/system-prompt hashes;
- Runtime binary hash;
- Tool schema hash from preflight;
- exact local test commands/counts;
- deterministic comparator/golden results;
- proof preflight made no provider request;
- formal real-model baseline = NOT EXECUTED unless explicitly authorized.

---

## 9. Formal Baseline Execution Sequence

Formal validation later performs:

1. explicit user authorization for paid/networked run;
2. verify candidate clean/frozen;
3. validate/preflight;
4. confirm requested/resolved model identity;
5. execute all 72 logical runs;
6. no cherry-picking/replacement;
7. compute metrics;
8. evaluate baseline validity gate;
9. inspect security/cleanup;
10. independent QA reruns required E2E subsets/full suite as defined by validation plan;
11. Manual Acceptance.

If provider/model changes mid-batch, discard the batch as INCONCLUSIVE; do not merge partial runs from different identities.

---

## 10. Concurrency / Isolation

Default formal runner concurrency is frozen to:

~~~text
1 logical run at a time
~~~

Reason:

- avoids provider parallelism changing model behavior/cost;
- simplifies real Approval isolation;
- ensures explicit request/evidence correlation;
- avoids test Runtime/DB cross-talk.

The runner may parallelize pure fixture hashing/scorer unit tests, not formal model runs.

Every run gets unique:

- Workspace;
- Runtime process;
- runtime_id;
- MCP endpoint;
- evidence DB;
- output run ID.

---

## 11. Output / Persistence

Default CLI:

- prints concise human summary to stderr;
- prints one JSON result/aggregate to stdout when requested;
- does not automatically write repository files.

Raw provider response storage is disabled by default.

If debug capture is explicitly enabled for local troubleshooting:

- store under system temp;
- redact credentials;
- remove hidden/reasoning fields;
- bounded size;
- clean after run unless user explicitly chooses to retain outside repo.

No baseline report is auto-added as a seventh milestone document.

---

## 12. Security Review

Before T6:

- prove no provider credential in git diff/test output;
- prove Runner cannot attach to user active Runtime by default;
- prove Workspace path is generated temp;
- prove bash disabled;
- prove unexpected ASK not auto-approved;
- prove no direct ToolExecutor call in formal run;
- prove M03 evidence matches exact request IDs;
- prove result JSON contains no physical Workspace root;
- prove teardown kills only test-owned Runtime.

---

## 13. Rollback

M04 has minimal production-surface changes:

- evaluation modules/binary;
- optional additive MCP result metadata for safe correlation.

If M04 is reverted:

- Runtime business Tool result remains compatible;
- M03 schema/database is unchanged;
- AppConfig schema remains v3;
- no migration is needed.

If correlation metadata causes client incompatibility, revert only that additive transport change and stop M04 until an approved explicit identity path is designed. Do not switch to fuzzy time matching.

---

## 14. Task Order

| Task | Main owner | Depends on | Output |
| --- | --- | --- | --- |
| T1 | evaluation case/dataset/scorer contracts | M03 gate + six docs | m04-core-v1 fixtures/contracts |
| T2 | Runtime/MCP/evidence integration | T1 | isolated real Runtime harness + exact correlation |
| T3 | Model adapter/agent loop/Approval | T1/T2 | controllable real adapter path |
| T4 | scorer/aggregate/comparator | T1/T2/T3 contracts | metrics + M05 regression gate |
| T5 | CLI validate/preflight/scripted run | T1–T4 | no-cost executable readiness |
| T6 | cross-cutting regression/package boundary | T1–T5 | Development Complete candidate |

---

## 15. Development Completion Gate

M04 Development Complete requires:

- T1–T6 implemented;
- dataset 24 cases frozen/hashed;
- scorer golden tests PASS;
- real Runtime/MCP scripted loop PASS;
- exact M03 request correlation PASS;
- no fuzzy timestamp join;
- OpenAI adapter compiled/tested at protocol boundary;
- no secret leakage;
- validate/preflight/scripted run PASS;
- M05 comparator threshold tests PASS;
- full workspace regression PASS;
- formal provider E2E explicitly remains NOT EXECUTED unless authorized.

M04 Development Complete does not equal M04 ACCEPTED / FROZEN and does not unlock M05 until formal M04 acceptance.
