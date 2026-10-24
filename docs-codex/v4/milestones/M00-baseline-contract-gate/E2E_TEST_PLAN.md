# M00 E2E Test Plan — Baseline Chain, Controlled Eval and Production-Isolation Proof

> 存放规则遵守 [AGENTS.md](../../../../AGENTS.md)。本目录只保留六份正式文档，固定测试数据沿用 `fixtures/v4/m00/`。独立报告、独立 ADR、Evidence Index 与独立 POC 工程已按用户要求删除，不恢复；删除不改写已经实际执行的 Development / Integration / E2E-03 结果。未具备真实边界证据的验收项保持 BLOCKED，未执行项保持 NOT EXECUTED，不得记为 PASS。

> Status: E2E-01 PASS; E2E-02 DEFERRED BY USER (NOT EXECUTED; not a M00 blocker); E2E-03 PASS; M00 ACCEPTED / FROZEN under explicit user-approved exception; Manual Acceptance APPROVED BY USER
> Milestone: M00 — As-Is Baseline and Architecture Contract Gate  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: QA/Coding Agent for locally automatable steps; explicitly authorized external execution for ChatGPT/networked model steps  
> Purpose: prove M00 architecture assumptions from real external/black-box boundaries.  
> This plan does not replace Manual Acceptance.

---

## 1. Objective

M00 E2E proves three end-to-end facts:

1. the current product chain still works before V4 implementation:
   real ChatGPT → Tunnel → Runtime MCP → existing Tool → real Workspace/Activity result;
2. a real controllable Agent/Model can drive the isolated evaluation runner through the real Harness MCP and be graded against ground truth;
3. the M00 candidate still behaves like the current product at production boundaries — POCs did not secretly become production features or change config/tool/security semantics.

The E2E layer must not substitute mocks for the real boundary it is meant to prove.

This plan is frozen before implementation starts. A failed or blocked E2E boundary is an engineering/availability result, not permission to weaken the case after coding.

---

## 2. Entry Criteria

Before E2E:

- M00 T1–T6 Development Complete.
- M00 Integration = PASS.
- exact candidate commit is frozen.
- release/final-ish Runtime executable is available.
- controlled disposable Workspace/config fixtures exist.
- no unresolved REVISED ADR exists outside the upstream docs.
- cleanup script/steps are ready.

For external/networked cases:

- explicit user authorization has been obtained for the specific run where required by Roadmap;
- credentials/access are available without committing secrets;
- cost/step/time bounds are configured for M00-E2E-02.

If external authorization/access is unavailable:

~~~text
affected E2E case = BLOCKED
M00 = not ACCEPTED
~~~

Do not replace it with a mock and mark PASS.

Current E2E entry evaluation: Development Complete = PASS and Integration = PASS (12/12), so the E2E plan was entered. The user confirmed E2E-01 PASS; E2E-02 was not executed and is DEFERRED BY USER without a PASS claim; E2E-03 is PASS. The user approved Manual Acceptance and the M00 closeout exception; the later Agent Evaluation Milestone must revalidate the real model closed loop.

---

## 3. Candidate Identity

Before E2E, record:

~~~text
Production commit:
Evidence/test commit if different:
Dirty state:
App version/build:
Runtime path:
Runtime SHA-256:
Tunnel binary/version:
Protocol version:
CURRENT_SCHEMA_VERSION:
Workspace fixture:
Config revision:
Security mode:
Tool config:
Policy fixture:
Eval adapter/provider/model:
Eval case hash:
Eval fixture hash:
Eval scorer version:
Tester:
Date/time:
~~~

If production logic changes after a case, re-run affected cases.

---

## 4. E2E Boundary Rules

### Real ChatGPT case

Must use the actual external chain:

~~~text
ChatGPT
→ Tunnel
→ Runtime MCP
→ existing read_file or glob
→ controlled Workspace
→ Runtime/Activity evidence
~~~

Do not replace ChatGPT with MCP Inspector or a local SDK client.

### Controlled Eval case

Must use:

~~~text
real controllable Agent/Model
→ evaluation runner
→ real loopback Runtime MCP
→ existing Tool
→ isolated Workspace
→ deterministic final-state scorer
~~~

A provider mock/scripted agent is insufficient for final PASS.

### Production-isolation case

Must launch/use production candidate boundaries and prove M00-only code is not visible or active as product functionality.

---

## 5. Required E2E Cases

### M00-E2E-01 — Real ChatGPT → Tunnel → Runtime current-baseline Tool call

**AC:** M00-AC11; supports M00-AC01/02/03/04/05  
**Tasks:** T1/T2/T6  
**External authorization:** required according to Roadmap execution rules.

#### Setup

Use a controlled Workspace containing a known file, for example:

~~~text
README.md
~~~

or another stable fixture.

Prefer a non-destructive request.

Record:

- candidate commit/build;
- Runtime identity;
- Tunnel identity;
- Workspace root/fixture identity;
- effective Tool state;
- relevant Policy state;
- Runtime Ready status before request.

#### Action

From real ChatGPT, request an existing Harness MCP action that deterministically uses:

- read_file, or
- glob.

The instruction must require actual tool use, not a prose guess.

#### Assertions

- ChatGPT can discover/call the expected Harness tool.
- Tunnel forwards to the intended Runtime candidate.
- Runtime remains authoritative for Workspace/Policy/Tool checks.
- Tool result matches the controlled Workspace truth.
- current Activity/Runtime observation contains the corresponding request evidence available in the baseline.
- request identity can be related to the Runtime observation without timestamp-only guessing.
- no SQLite History/Trace/full V4 feature is claimed from this result.
- no file mutation occurs for the read-only path.

#### Failure cases

- direct local Runtime call without ChatGPT/Tunnel → not this E2E.
- ChatGPT answers from context without a tool call → FAIL/INCONCLUSIVE.
- wrong Runtime/Workspace instance → FAIL.
- missing external access → BLOCKED.

#### Cleanup

No Workspace mutation should remain. Stop only test-owned instances.

**PASS:** the current real external Harness chain is proven on the M00 candidate.

**Recorded execution verdict: PASS.** The user confirmed the real ChatGPT → Tunnel → Harness Runtime MCP → `read_file` → Workspace chain completed for M00-E2E-01.

---

### M00-E2E-02 — Real controllable Agent/Model → Eval runner → real Runtime MCP → ground-truth grade

**AC:** M00-AC10  
**Task:** T5  
**Upstream:** F12-AC01, F12-AC03, F12-AC04, A10  
**Network/paid authorization:** explicitly required before execution when applicable.

#### Setup

Use the versioned M00 eval smoke case from T5.

Record:

- provider/adapter/model exact identity;
- model parameters/randomness controls;
- max step/time/spend;
- case ID/hash;
- Workspace fixture hash;
- Tool schema identity/hash;
- Policy config;
- scorer version;
- Runtime candidate identity.

The Workspace must be isolated from the user’s normal active Workspace.

#### Action

Run the actual concrete Agent/Model adapter.

The model must be allowed to emit real Tool calls through the runner. The runner forwards those calls to the real Harness Runtime MCP endpoint and returns real Tool results to the model until stop/limit.

#### Required assertions

- at least one actual model turn is executed;
- the model’s emitted Tool call is captured;
- selected Tool is graded against the case allowed/expected set;
- arguments are validated against the real Tool schema;
- call travels through the real Runtime MCP path;
- Runtime outcome is recorded separately from final task outcome;
- scorer validates final observable state independently;
- evidence record distinguishes:
  - tool selection;
  - argument validity;
  - Runtime call outcome;
  - task completion;
  - call count;
  - termination reason;
- evidence record does not claim hidden reasoning or unobserved candidate tools;
- failure or transport success alone does not determine task success;
- secrets/credentials are not persisted into repository evidence;
- test Runtime/Workspace is cleaned after run.

#### Negative control

Run or use a pre-versioned negative final-state scorer case in which MCP/tool transport can succeed but expected final state is wrong.

The scorer must mark task failure.

#### Verdict

- real adapter/model + real MCP + independent scorer all execute successfully → PASS;
- only mocked model/provider executes → BLOCKED for M00 acceptance;
- network/model unavailable → BLOCKED;
- too little evidence to distinguish task success → INCONCLUSIVE.

**Recorded execution verdict: DEFERRED BY USER — NOT EXECUTED.** No real controllable Agent/Model endpoint, actual model turn, real Runtime MCP call or independent final-state scorer evidence is claimed. The deterministic/local adapter remains Integration evidence only and does not satisfy E2E-02. This item is not a M00 blocker under the explicit user-approved closeout exception and must be revalidated in the later Agent Evaluation Milestone; no E2E-02 PASS is claimed.

---

### M00-E2E-03 — Production candidate has no M00 POC leakage

**AC:** M00-AC12, M00-AC13, M00-AC14  
**Task:** T6

This is a black-box/release-boundary integrity test.

#### Setup

Build/use the M00 production candidate through normal repository build paths.

Use disposable app/config/Workspace state.

#### Assertions

At the production boundary:

- app/runtime start without requiring the M00 SQLite POC;
- root production build does not package/launch the POC as a product process;
- no production History DB is created merely because M00 exists;
- production AppConfig created/saved remains schemaVersion 2;
- current MCP tools/list contains only the pre-M00 product catalog;
- no grep/new M00 Tool is exposed;
- no production History/Trace/Eval API is exposed as a user feature;
- normal existing read-only Tool still works;
- Policy/Approval/Workspace semantics remain current behavior;
- production shutdown leaves no M00 test process;
- after E2E cleanup, no temp DB/config/Workspace residue remains.

Also compare 架构决策及阻塞项（在既有 SPEC/PLAN 中维护） with the candidate:

- any REVISED ADR must already be reflected upstream;
- BLOCKED ADR means M00 cannot pass.

**PASS:** production candidate remains behaviorally isolated from M00-only POCs and all resources clean up.

**Recorded execution verdict: PASS.** This result was previously executed and was deliberately not rerun during the current closeout. Its evidence covered the real release Runtime MCP catalog, read-only fixture correctness, no AppData mutation, graceful Runtime exit and cleanup.

---

## 6. E2E Evidence Format

For every case capture:

~~~text
Case ID:
Candidate commit:
Binary identities/hashes:
Fixture IDs/hashes:
External client/model identity:
Exact user/task instruction:
Actual tool calls:
Runtime request identity:
Expected result:
Actual result:
Cleanup:
Verdict:
Evidence files/logs/screenshots:
Notes:
~~~

For M00-E2E-01, preserve enough evidence to prove the call originated from real ChatGPT and crossed the Tunnel.

For M00-E2E-02, preserve actual model/adapter/tool-call/scorer evidence while respecting secret/privacy boundaries.

---

## 7. E2E Failure Semantics

Use:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Do not use SKIP as a successful result.

Examples:

- ChatGPT external access not authorized → BLOCKED.
- Real model credentials unavailable → BLOCKED.
- Model ran but final-state evidence was corrupted → INCONCLUSIVE.
- Tool returned success but scorer final state failed → FAIL for task completion.
- Production candidate unexpectedly creates a History DB → FAIL M00-E2E-03.

---

## 8. E2E Exit Criteria

M00 E2E is PASS only when:

- M00-E2E-01 PASS;
- M00-E2E-02 PASS;
- M00-E2E-03 PASS;
- cleanup is confirmed;
- all results are bound to the candidate identity;
- no production change invalidated earlier results.

Only after this may the Human Reviewer execute MANUAL_ACCEPTANCE_CHECKLIST.md.

E2E PASS still does not itself mark M00 ACCEPTED/FROZEN.

### M00 closeout exception — 2026-10-24

The user explicitly approved M00 acceptance/freeze with M00-E2E-02 deferred. This is a closeout exception only; it does not change the frozen E2E-02 test definition or its requirement for a real controllable Agent/Model closed loop. E2E-02 remains NOT EXECUTED and must be revalidated in the later Agent Evaluation Milestone.

---

## 9. Execution evidence location

Execution results are maintained in this plan, E2E 执行结论（对话交付，不自动新建报告） and 验收证据定位（沿用既有验收清单）. No standalone execution report or Evidence Index is recreated.

Current result: E2E-01 = PASS, E2E-02 = DEFERRED BY USER (NOT EXECUTED; not PASS), E2E-03 = PASS; M00 closeout = ACCEPTED / FROZEN by explicit user-approved exception; Manual Acceptance = APPROVED BY USER. The later Agent Evaluation Milestone must revalidate the real model closed loop.
