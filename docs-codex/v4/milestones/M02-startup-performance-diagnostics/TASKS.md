# M02 TASKS — Startup Performance and One-click Diagnostics

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: DRAFT / IMPLEMENTATION BLOCKED UNTIL PREVIOUS MILESTONE ACCEPTANCE  
> Scope: F05 + F06 only  
> Product baseline: 9530c6e0acfd35073281d6582acc550dfe65b697  
> Critical rule: T1 只允许 instrumentation；B0 numeric baseline 未锁定前，禁止 T2 修改 startup behavior。  
> Layout: 只修改既有 product/test 位置；不得新增顶级目录、独立 POC、per-run report、benchmark archive 或 milestone 子目录。

---

## 1. Dependency

~~~text
Previous milestone ACCEPTED / FROZEN
        ↓
T1 Recorder + B0 capture
        ↓
     HARD GATE
        ↓
T2 Startup optimization
        ├───────────────┐
        ↓               ↓
T3 Diagnostics        T4 Fault/remediation
        └───────┬───────┘
                ↓
T5 UI / i18n
                ↓
T6 Candidate benchmark / regression / delivery
~~~

---

# T1 — Startup Measurement Contract, Recorder and B0 Baseline Freeze

**AC:** M02-AC01, AC02, AC03, AC05, AC06  
**Primary areas:** protocol v1 startup/diagnostics types、ui/src-tauri harness/runtime/tunnel integration、existing tests。  
**Constraint:** 本 Task 禁止 startup optimization。

### Implement

1. Add protocol v1 startup DTOs:
   - StartupCohort
   - StartupOutcome
   - StartupPhase
   - StartupPhaseStatus
   - StartupPhaseTiming
   - StartupAttempt

2. Update existing Rust serde/schema/TS contract workflow.

3. Add Host StartupPhaseRecorder:
   - one attempt per accepted Start；
   - Host monotonic clock；
   - frozen phase IDs；
   - attempt ↔ Harness session；
   - stale generation/session rejection；
   - failure/cancel/incomplete；
   - latest-attempt API；
   - no raw secret/error。

4. Add minimal RuntimeSupervisor hooks for runtime_spawn / runtime_initialize / mcp_endpoint_ready。

5. Close Tunnel phases from existing managed status/readiness:
   - tunnel_resolve；
   - tunnel_launch；
   - control_plane_connected；
   - mcp_target_ready。

6. Verify instrumentation does not alter config/Workspace validation、SecretStore、Runtime version/init、MCP validation、Tunnel stale/readiness 或 final Ready logic。

### Unit tests

- success all phases；
- config/eligibility failure；
- Runtime spawn/init/MCP failure；
- Tunnel resolve/launch failure；
- delayed readiness；
- stale old generation ignored；
- Stop while Starting → cancelled；
- Ready closes once；
- process exists cannot imply Ready；
- no raw secret/error；
- monotonic offsets consistent。

### B0 hard gate

1. build instrumentation-only package；
2. record B0 SHA；
3. prove no optimization diff；
4. 2 primer；
5. 30 counted cold；
6. 30 counted warm；
7. record environment；
8. nearest-rank P50/P95；
9. failure-rate；
10. phase stats；
11. at least 29 Ready each；
12. false Ready=0；
13. summarize in conversation/current evidence channel；
14. clean raw temp benchmark data。

Do not modify SPEC thresholds after B0.

### Verification

~~~text
cargo test -p harness-control-protocol
cargo test -p chatgpt-harness-ui <focused startup/harness/runtime tests>
cargo check --workspace
~~~

plus real packaged B0 measurement.

### Done

B0 locked；T2 unlocked。

---

# T2 — Optimize Measured Start→Ready Critical Path

**AC:** M02-AC04, AC05; supports AC15  
**Depends on:** T1 B0 gate.

### Before editing

先报告：

- B0 cold/warm total P50/P95；
- phase contribution；
- slowest Host-controllable phase；
- selected bottleneck；
- why material。

### Implement

只优化 measured material work。

Possible hypotheses：

- duplicate frontend/Host eligibility；
- repeated config load/validation；
- repeated Runtime resolution；
- repeated Tunnel resolution；
- repeated SecretStore lookup；
- independent prep serialized unnecessarily。

B0 若显示 negligible，不为“看起来重复”而重构。

### Safety

Final Start 仍验证：

- current AppConfig；
- Workspace；
- security；
- credential presence；
- Runtime distribution/version/init；
- MCP endpoint；
- Tunnel distribution/launch；
- control-plane/target readiness；
- existing final Harness Ready。

PreparedStart if introduced：

- one-operation lifetime；
- immutable；
- no cross-session cache；
- no persistent raw secret；
- invalid after config change；
- same failure semantics。

### Tests

- resolver/check call count；
- typed failure boundaries；
- operation mutex；
- Stop/Restart/Quit；
- stale event isolation；
- recorder unchanged；
- M01 Tray Start/Stop。

### Verification

~~~text
cargo test -p chatgpt-harness-ui <focused harness/runtime/tunnel tests>
cargo check --workspace
~~~

### Done

Critical path improved without shortcut.

---

# T3 — Typed Diagnostics Contract and Host DiagnosticsCoordinator

**AC:** M02-AC07, AC08, AC09, AC10, AC11  
**Depends on:** T1 contract stable.

### Protocol

Extend DiagnosticStatus with unknown。

Add DiagnosticScope、DiagnosticEvidenceKey、DiagnosticEvidence、DiagnosticRemediation、DiagnosticRemediationTarget。

Keep v1 additive；update TS/schema/contracts。

### Coordinator

Aggregate existing authoritative sources:

- ConfigStore；
- Workspace/platform；
- Runtime distribution/process/diagnostics；
- MCP；
- Tunnel distribution/process/readiness/recovery；
- control plane；
- proxy；
- SecretReference/SecretStore presence；
- Harness lifecycle。

Expose one frontend product command，名称沿用现有 convention。

Existing local/runtime commands 可保留作为 source。

### Read-only

绝不调用：

- config save/reset；
- choose workspace/client；
- SecretStore set/delete；
- harness start/stop/restart；
- approvals.resolve；
- filesystem mutation。

### UNKNOWN

Unavailable prerequisite 必须显式 UNKNOWN。

### Overall

~~~text
FAIL > WARNING > UNKNOWN > PASS
~~~

### Tests

- all pass；
- fail；
- warning+unknown；
- unknown only；
- config absent；
- Runtime stopped/crashed；
- source throws；
- no mutation invoked。

### Done

Unified typed diagnostics ready.

---

# T4 — Fault Matrix, Safe Evidence and Remediation

**AC:** M02-AC09, AC10, AC11, AC12  
**Depends on:** T3.

### Mapping

Central map from typed states/errors to：

- check ID；
- scope；
- status；
- explanation；
- safe evidence；
- remediation。

Prefer typed code, avoid React string parsing。

### Cover

Config/Workspace：

- valid/missing/malformed/inaccessible。

Runtime/MCP：

- distribution good/missing/incompatible；
- stopped/ready/crashed；
- RPC unavailable；
- endpoint unavailable。

Tunnel/control-plane/target：

- distribution missing；
- stopped/starting/ready/degraded/failed；
- stale；
- blocked recovery；
- target unreachable；
- recovered。

Proxy：

- direct；
- configured；
- observed route；
- typed failure；
- no evidence→UNKNOWN。

Credential：

- exists；
- ref missing；
- value missing；
- store unavailable；
- connected accepted；
- typed unauthorized；
- no live proof→UNKNOWN。

### Privacy sentinels

Inject and prove absent：

- fake API key；
- physical path；
- credential-bearing proxy URI；
- long raw error；
- raw env；
- command output sentinel。

### Remediation

Only Overview / Settings Workspace / Settings Connection / Settings Security / retry instruction。

No auto-fix。

### Verification

Focused Rust tests for all SPEC fault cases；generated state uses system temp and cleanup。

### Done

Fault classification deterministic and privacy-safe。

---

# T5 — Overview Diagnostics, Startup Detail, Safe Copy and i18n

**AC:** M02-AC06, AC13; supports AC07–AC12  
**Depends on:** T1 + T3/T4.

### Client

Add typed methods for aggregate diagnostics + latest startup attempt。

### Overview

Add visible Run connection diagnostics。

Display overall、checks by scope、four statuses、summary、safe evidence、remediation、retry。

### Startup detail

Show cohort、outcome、total、phases、slowest phase。

Never treat one attempt as performance PASS。

### Settings

Use same aggregate result；no duplicate classification in React。

### Safe copy

Strict allowlist；future/unknown fields excluded by default。

### i18n

en/zh-CN for status/scope/phase/remediation/outcome/copy feedback。

### Tests

- run/loading/error/retry；
- four statuses；
- Unknown not Pass；
- remediation navigation；
- startup success/failure/incomplete；
- copy allowlist；
- Settings/Overview same model；
- language coverage。

### Done

User-facing one-click UX complete。

---

# T6 — Final Candidate Benchmark, Regression and Delivery

**AC:** M02-AC04, AC05, AC14, AC15  
**Depends on:** T1–T5.

### Freeze candidate

Record SHA/dirty state before final cohort。

Startup-affecting change invalidates cohort。

### Candidate measurement

- 2 primer；
- 30 cold；
- 30 warm；
- same environment；
- every attempt in denominator；
- ≥29 Ready/cohort。

Compute Cc50/Cc95/Cw50/Cw95、phase stats、failure rate、material bottleneck criterion。

Evaluate every SPEC §8 formula；no threshold edits。

Fail → return T2 → new SHA → rerun complete cohort。

### Readiness regression

Confirm Runtime initialize/version、MCP endpoint、Tunnel instance/readiness、control-plane freshness、MCP target、Harness Ready source。

Real ChatGPT E2E 留给后续 E2E plan unless explicitly authorized。

### Diagnostics regression

Run normal + fault cases、read-only assertions、safe-copy privacy。

### M01/V3 regression

Affected suites：

- Tray Start/Stop/Show/Quit；
- connection notifications；
- config v3 desktop prefs；
- Workspace；
- Policy/Approval；
- Bash；
- SecretStore；
- Start/Stop/Restart。

### Full checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Then run AGENTS required milestone delivery/layout scripts。

### Cleanup

- stop test-owned processes；
- clean temp benchmark data/config/workspace；
- keep only allowed Portable artifact contents；
- run layout checks。

### Development Complete response

在对话中报告：

- B0 SHA + baseline stats；
- candidate SHA + stats；
- threshold table；
- exact commands/counts；
- candidate path + hash；
- diagnostics coverage；
- Integration/E2E/Manual NOT EXECUTED。

Do not create seventh milestone file。

### Done

Frozen F05 thresholds PASS；diagnostics tests PASS；full regression PASS；fresh candidate delivered；no development blocker。

Do not mark M02 ACCEPTED/FROZEN。

---

## 2. Task → AC → Reserved Validation Traceability

| Task | M02 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01, AC02, AC03, AC05, AC06 | M02-INT-01/02/03/05, M02-E2E-01, M02-MAN-01 |
| T2 | AC04, AC05 | M02-INT-04/05/06, M02-E2E-01/04, M02-MAN-02/05 |
| T3 | AC07, AC08, AC09, AC10, AC11 | M02-INT-07/08/09/10/14, M02-E2E-02/03, M02-MAN-03/04 |
| T4 | AC09, AC10, AC11, AC12 | M02-INT-11/12/13/14, M02-E2E-03/04, M02-MAN-04/05 |
| T5 | AC06, AC13 | M02-INT-15, M02-E2E-02/03, M02-MAN-01/03/04 |
| T6 | AC04, AC05, AC14, AC15 | M02-INT-04/06/16, all M02 E2E, M02-MAN-02/05 |

---

## 3. Development Handoff

~~~text
M02 Development Complete
→ frozen Integration plan
→ packaged E2E / real ChatGPT readiness correctness
→ Human Manual Acceptance
→ user sign-off
→ M02 ACCEPTED / FROZEN
~~~

Performance PASS is necessary but not sufficient for milestone acceptance。
