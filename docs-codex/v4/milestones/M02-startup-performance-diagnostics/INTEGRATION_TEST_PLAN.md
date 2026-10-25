# M02 Integration Test Plan — Startup Performance and One-click Diagnostics

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M02 — Startup Performance and One-click Diagnostics  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M02 Development Complete  
> Scope: F05 Startup Performance + F06 One-click Diagnostics  
> Rule: Integration validates real product modules and packaged Host boundaries, but does not replace the real external ChatGPT E2E or human acceptance.  
> Storage: execution logs and temporary benchmark data use system temporary directories; do not add report/evidence files to this milestone directory.

---

## 1. Objective

Integration proves that M02 implementation is internally correct before external E2E:

- StartupPhaseRecorder measures the existing authoritative startup lifecycle rather than creating a second lifecycle.
- timing, cold/warm classification, P50/P95 and failure-rate math exactly match SPEC.
- B0 baseline was captured before optimization and candidate comparison uses the same measurement contract.
- optimization preserves Runtime/MCP/Tunnel readiness semantics.
- DiagnosticsCoordinator aggregates existing authorities and returns PASS / WARNING / FAIL / UNKNOWN truthfully.
- diagnostics are read-only and privacy-safe.
- Overview and Settings consume the same diagnostic model.
- M01 Tray/background/notification behavior remains intact.

Integration cases use the real Runtime/Tauri modules and real packaged components where required, but external ChatGPT is reserved for E2E.

---

## 2. Entry Criteria

Before execution:

- previous milestone governance gate is satisfied;
- M02 T1–T6 are Development Complete;
- exact candidate commit is frozen;
- B0 exact SHA and summarized baseline statistics exist in the Development Complete output;
- final candidate benchmark statistics exist and claim all SPEC §8 thresholds PASS;
- task-local tests are PASS;
- candidate Portable package exists;
- benchmark/fault-fixture temporary data is disposable and separate from the real user profile.

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

If a required candidate/B0 identity is missing, the affected performance cases are BLOCKED, not PASS.

---

## 3. Evidence Header

Record:

~~~text
Milestone: M02
Candidate commit:
B0 instrumentation-only commit:
Product logic baseline:
Branch / dirty state:
Windows version:
CPU / power mode:
App version:
Runtime version:
Tunnel version:
Package type / SHA-256:
Workspace fixture:
Config revision:
Tunnel ID identity:
Proxy source / route mode:
Network type:
QA owner:
Execution time:
~~~

Never record API Key or SecretStore values.

Each case records:

- Case ID
- related AC
- exact command/runner
- expected
- actual
- PASS / FAIL / BLOCKED / INCONCLUSIVE
- cleanup status

---

## 4. Required Integration Cases

### M02-INT-01 — Startup attempt identity and frozen phase lifecycle

**AC:** M02-AC01  
**Tasks:** T1

Exercise a real Host-managed successful Start and focused failure cases.

Verify:

- exactly one StartupAttempt for one accepted Start;
- attemptId is unique;
- harnessSessionId matches the active Harness session;
- all frozen phase IDs are present with truthful completed/failed/skipped/incomplete status;
- startedOffsetMs and durationMs are non-negative and monotonic;
- totalDurationMs closes only when the same session reaches authoritative Ready;
- rejected lifecycle conflict does not become a successful latency sample;
- Stop/Quit during Starting terminalizes as cancelled where appropriate;
- crash before terminal state is incomplete;
- old Runtime/Tunnel generation events cannot close a new attempt.

**PASS:** recorder observes the real lifecycle without inventing state.

---

### M02-INT-02 — Host monotonic timing and no cross-process clock arithmetic

**AC:** M02-AC01, M02-AC02  
**Task:** T1

Inspect/execute recorder contract tests proving:

- Start→Ready total is calculated from one Host monotonic clock;
- Runtime/Tunnel internal timestamps are never subtracted from Host Instant;
- wall-clock timestamps are display labels only;
- simultaneous control-plane/MCP-target evidence can close separate phase markers at the same Host observation time without negative duration;
- failed/skipped phases are never encoded as false zero-duration success.

**PASS:** latency math is valid and deterministic.

---

### M02-INT-03 — Baseline/candidate sample accounting and percentile algorithm

**AC:** M02-AC02, M02-AC03  
**Tasks:** T1/T6

Use the implementation benchmark/statistics code or independent QA calculator against synthetic raw sample sets.

Verify:

- 2 primer runs are excluded;
- 30 counted cold + 30 counted warm are expected;
- every accepted attempt enters failure-rate denominator;
- only Ready attempts enter latency percentile set;
- fewer than 29 Ready in a cohort cannot PASS;
- P50 and P95 use nearest-rank ceil formula;
- no averaging of P95;
- no averaging of phase P95 into total P95;
- valid slow samples cannot be silently filtered.

Then verify B0 evidence identity predates T2 optimization changes.

**PASS:** benchmark math and chronology match the frozen SPEC.

---

### M02-INT-04 — Frozen performance threshold evaluator

**AC:** M02-AC04  
**Tasks:** T2/T6

Feed threshold-boundary fixtures into the comparison evaluator.

Required assertions:

- cold P50 > 90% of baseline fails;
- warm P50 > 85% of baseline fails;
- either P95 > 105% of baseline fails;
- at least one cohort P95 must be <=90% baseline;
- candidate failure rate cannot exceed baseline;
- baseline zero failure requires candidate zero failure;
- candidate failure rate cannot exceed 1/30;
- material-bottleneck clause A or clause B must pass;
- exact boundary values pass;
- threshold constants are not read from mutable post-run user configuration.

**PASS:** evaluator implements SPEC §8 exactly.

---

### M02-INT-05 — Startup optimization preserves authoritative readiness

**AC:** M02-AC05  
**Tasks:** T1/T2

Run real Host/Runtime/Tunnel integration failures at each boundary:

- invalid config / Workspace;
- missing credential;
- Runtime distribution/version failure;
- Runtime initialize failure;
- MCP endpoint failure;
- Tunnel distribution/launch failure;
- control-plane disconnected/stale;
- MCP target unreachable.

Verify no case reaches Harness Ready merely because a process spawned or exists.

Also verify successful Ready still requires the current managed Runtime/MCP/Tunnel conditions.

**PASS:** optimization made startup faster without weakening truth.

---

### M02-INT-06 — Optimized preparation does not create stale/cross-session authority

**AC:** M02-AC05, M02-AC14  
**Task:** T2

If PreparedStart or equivalent optimization exists, verify:

- data lifetime is one Start operation;
- config change invalidates/rebuilds preparation;
- raw secret is not cached across starts;
- stale Runtime/Tunnel generation cannot reuse a prepared result;
- Stop/Restart/Quit serialization still uses HarnessController operation mutex;
- Tray Start calls the same authoritative path;
- concurrent Start attempts do not duplicate Runtime/Tunnel processes.

If no PreparedStart abstraction exists, prove equivalent invariants for the implemented optimization.

**PASS:** performance cache/preparation never becomes a second source of truth.

---

### M02-INT-07 — DiagnosticsCoordinator required check coverage

**AC:** M02-AC07, M02-AC09  
**Task:** T3

Execute aggregate diagnostics against a healthy controlled state.

Verify required IDs are present:

- config
- workspace
- runtime_distribution
- runtime_process
- runtime_readiness
- mcp_endpoint
- tunnel_distribution
- tunnel_process
- tunnel_readiness
- control_plane
- mcp_target
- proxy_config
- proxy_route
- credential_presence
- credential_state
- harness_lifecycle

Existing security check may be additional.

**PASS:** one coordinator returns the complete product diagnostic surface.

---

### M02-INT-08 — Diagnostic aggregation consumes authoritative sources

**AC:** M02-AC07, M02-AC09  
**Task:** T3

Instrument/spies verify coordinator obtains facts from:

- ConfigStore/platform validation;
- RuntimeSupervisor/current Runtime diagnostics;
- current MCP endpoint state;
- TunnelProcessManager readiness/recovery;
- HarnessController lifecycle;
- SecretStore presence/reference.

Verify React does not independently reclassify lifecycle or connection truth.

**PASS:** diagnostics aggregates; it does not reimplement authority.

---

### M02-INT-09 — PASS/WARNING/FAIL/UNKNOWN dependency semantics

**AC:** M02-AC08, M02-AC09  
**Tasks:** T3/T4

Table-drive states:

- healthy;
- intentionally Stopped;
- Starting/Stopping;
- Runtime missing;
- Runtime crashed;
- live Runtime evidence unavailable;
- Tunnel stopped;
- Tunnel failed;
- no current route evidence;
- missing prerequisite.

Verify:

- PASS only for positively verified facts;
- WARNING for verified nonfatal/intentional states such as stopped lifecycle;
- FAIL for verified blocking problems;
- UNKNOWN when the fact cannot safely be established;
- UNKNOWN is never omitted or rendered as PASS;
- overall precedence is FAIL > WARNING > UNKNOWN > PASS.

**PASS:** uncertainty is represented truthfully.

---

### M02-INT-10 — Diagnostic DTO typed scope, bounded evidence and remediation

**AC:** M02-AC08  
**Tasks:** T3/T4

Verify generated Rust/TS/schema contract includes:

- DiagnosticStatus.UNKNOWN;
- DiagnosticScope;
- typed evidence key/value;
- bounded safe value handling;
- typed remediation code/target.

Serialize/deserialize representative results.

Verify unknown enum/input fails safely according to current protocol conventions and no protocol/v2 is introduced.

**PASS:** wire/UI contract is additive and typed.

---

### M02-INT-11 — Config / Workspace / Runtime / MCP fault mapping

**AC:** M02-AC09, M02-AC12  
**Task:** T4

Fixtures:

- valid v3 config;
- config missing;
- config malformed/unsupported;
- Workspace inaccessible;
- Runtime distribution missing/incompatible;
- Runtime stopped;
- Runtime crashed;
- Runtime status/diagnostics unavailable;
- MCP endpoint unavailable/unreachable.

Verify exact expected status and remediation family.

Dependent checks with no valid prerequisite must become UNKNOWN, not synthetic FAIL/PASS unless the SPEC explicitly classifies the parent problem.

**PASS:** core local failures are deterministic and actionable.

---

### M02-INT-12 — Tunnel / control-plane / target / proxy fault mapping

**AC:** M02-AC09, M02-AC12  
**Task:** T4

Fixtures:

- Tunnel distribution missing;
- Tunnel intentionally stopped;
- Starting;
- Ready;
- Degraded;
- Failed;
- recovery blocked;
- stale control-plane poll;
- control-plane disconnected;
- MCP target unreachable;
- direct route;
- configured proxy;
- observed proxy route;
- typed proxy failure;
- no live route evidence.

Verify:

- current typed readiness drives classification;
- stale is not PASS;
- no evidence is UNKNOWN;
- proxy route does not expose credential-bearing URL.

**PASS:** network path diagnosis is truthful and safe.

---

### M02-INT-13 — Credential-state fault mapping without secret disclosure

**AC:** M02-AC09, M02-AC11, M02-AC12  
**Task:** T4

Cases:

- SecretReference + credential exists;
- reference missing;
- referenced credential missing;
- SecretStore unavailable;
- current control-plane connection proves accepted;
- current typed Unauthorized/Forbidden;
- no live acceptance/rejection evidence.

Verify:

- presence and usable-state checks are distinct;
- no extra credential-validation network request is made solely for diagnostics;
- UNKNOWN is used when usable state cannot be proven;
- raw secret never enters DTO/log/test failure output.

**PASS:** credential diagnosis is useful without reading/exposing the secret value.

---

### M02-INT-14 — Running diagnostics is strictly read-only

**AC:** M02-AC10  
**Tasks:** T3/T4

Use spies/instrumented adapters and filesystem/config snapshots.

Before/after one diagnostics run verify no invocation or state change for:

- config save/reset;
- choose Workspace/client;
- SecretStore set/delete;
- Harness Start/Stop/Restart;
- approvals.resolve;
- Tool execution;
- filesystem write/edit;
- Policy/Approval/SessionGrant state.

Verify only an explicit later user action may navigate or perform repair.

**PASS:** diagnosis itself has zero product mutation.

---

### M02-INT-15 — Overview/Settings share one diagnostic model and safe-copy surface

**AC:** M02-AC06, M02-AC13  
**Task:** T5

Frontend integration verifies:

- Overview visible one-click entry calls aggregate diagnostics;
- Settings calls the same aggregate API;
- no duplicate classification table exists in React;
- four statuses render distinctly;
- remediation navigation goes only to allowed targets;
- latest startup attempt renders total/phases/slowest phase/outcome;
- one startup attempt is not labeled benchmark PASS;
- en and zh-CN keys exist.

**PASS:** UI is a presentation layer over the same Host data.

---

### M02-INT-16 — Safe diagnostic copy + M01/V3 regression

**AC:** M02-AC11, M02-AC14  
**Tasks:** T5/T6

Inject sentinels:

- API key;
- physical Workspace path;
- credential-bearing proxy URI;
- raw environment;
- raw stdout/stderr;
- raw long control-plane error;
- unexpected future DTO field.

Verify none appears in copied diagnostic summary.

Then run affected regression for:

- Tray Start/Stop/Show/Quit;
- background continuity;
- connection notifications;
- AppConfig v3 desktop prefs;
- Workspace;
- Policy/Approval;
- Bash;
- SecretStore;
- Start/Stop/Restart.

**PASS:** M02 adds observability/performance without leaking data or breaking M01/V3.

---

## 5. Verdict Rules

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

Examples:

- threshold evaluator works but B0 identity is missing → M02-INT-03 BLOCKED.
- diagnostics omits UNKNOWN child and reports overall PASS → M02-INT-09 FAIL.
- optimized Start is faster but skips MCP readiness → M02-INT-05 FAIL.
- clipboard contains physical Workspace path → M02-INT-16 FAIL.

No SKIP is treated as success.

---

## 6. Integration Exit Gate

Integration PASS requires:

- M02-INT-01 through M02-INT-16 all PASS;
- same candidate identity for all production-behavior cases;
- B0 chronology proven;
- no weaker mock replaced a required real component boundary;
- temp fixtures/processes cleaned;
- no subsequent production change invalidated evidence.

Integration PASS does not equal M02 ACCEPTED / FROZEN.
