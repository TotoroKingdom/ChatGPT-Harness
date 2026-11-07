# M02 PLAN — Startup Performance and One-click Diagnostics

> Input: SPEC.md  
> Status: DRAFT / DESIGN READY — IMPLEMENTATION BLOCKED UNTIL PREVIOUS MILESTONE ACCEPTANCE  
> Scope: P0 F05 + F06  
> Product baseline: 9530c6e0acfd35073281d6582acc550dfe65b697  
> Rule: baseline instrumentation and numeric capture must precede optimization；不得在看到 B0 后降低阈值。  
> Layout: 复用现有 protocol/runtime/ui/test 位置，不创建新顶级目录、独立 POC、per-run report 或 milestone 子目录。

---

## 1. Strategy

~~~text
Previous milestone ACCEPTED / FROZEN
        ↓
T1 Startup recorder + instrumentation-only B0 capture
        ↓
HARD GATE: lock B0 SHA + 30 cold + 30 warm
        ↓
T2 Measured startup optimization
        ├──────────────────────┐
        ↓                      ↓
T3 Diagnostics DTO +        T4 Fault mapping /
   Host coordinator            remediation / fixtures
        └──────────┬───────────┘
                   ↓
T5 Overview/Settings UX + startup details + i18n
                   ↓
T6 Candidate benchmark + regression + package delivery
~~~

T2 在 B0 数值锁定前不得开始。

T3/T4 可以在 T1 contract 稳定后进行，但不得改变 startup critical-path behavior，以免污染 B0 对比。

---

## 2. Ownership

| Capability | Owner |
| --- | --- |
| startup attempt lifecycle | Tauri Host / existing HarnessController integration |
| Runtime startup phase hooks | ui/src-tauri/src/runtime/* |
| Tunnel readiness phase closure | existing Tunnel status events consumed by Host |
| startup/diagnostic DTO | protocol/src/v1 existing layout |
| DiagnosticsCoordinator | ui/src-tauri existing Host/commands module |
| Config/Workspace facts | ConfigStore/platform helpers |
| Runtime facts | RuntimeSupervisor + existing diagnostics.run |
| Tunnel/control-plane/proxy facts | TunnelProcessManager/readiness/recovery |
| credential facts | SecretStore reference/presence only |
| frontend client | ui/src/services/harness/* |
| UI | Overview + existing Settings diagnostics |
| i18n | existing resources |
| persistent helper script if truly required | existing ui/scripts only |

任何模块都不能成为第二个 lifecycle/authorization authority。

---

## 3. Phase A — Recorder and Baseline Checkpoint

在 protocol v1 additive 增加 startup types，并更新现有生成物/契约测试。

Host recorder：

- accepted Start 前建立 attempt；
- frozen phase IDs；
- Host monotonic timing；
- attempt/session identity；
- stale generation/session rejection；
- failure/cancel/incomplete；
- no secret/raw error；
- latest attempt query。

RuntimeSupervisor 只暴露必要边界，不改 initialize/version/MCP validation 顺序。

Tunnel 使用现有 managed instance/generation/readiness evidence，不重写 readiness。

B0 gate：

- instrumentation-only package；
- 2 primer；
- 30 cold；
- 30 warm；
- locked environment；
- nearest-rank P50/P95；
- failure rate；
- phase stats；
- ≥29 Ready/cohort；
- false Ready=0。

结果在对话/既有验证渠道交付；raw data 使用 system temp 并清理。

---

## 4. Phase B — Measured Optimization

按 B0 排序：

- total P50/P95 contribution；
- phase P50/P95；
- failure contribution；
- Host-controllable vs external。

当前源码可能的 hypothesis：

- duplicate frontend/Host eligibility；
- repeated config load/validation；
- repeated Runtime resolution；
- repeated Tunnel resolution；
- repeated SecretStore lookup；
- independent prep serialized unnecessarily。

只有 B0 证明 material 才改。

PreparedStart 若使用：

- one-operation lifetime；
- immutable；
- no cross-session cache；
- no persistent raw secret；
- config change invalidates it；
- same authority checks。

Parallelization 只用于 independent read-only prep。

每个 optimization slice 后立即跑 lifecycle regression。

---

## 5. Phase C — Diagnostics DTO + Coordinator

Protocol additive：

- UNKNOWN；
- DiagnosticScope；
- DiagnosticEvidenceKey；
- DiagnosticEvidence；
- DiagnosticRemediation；
- navigation target。

新 product command 如 connection_diagnostics_run，按现有 naming 落地。

Existing local/runtime diagnostics 可保留为 source/backward-compatible API，frontend 不自行 merge。

Collection order：

1. config；
2. Workspace；
3. Runtime distribution；
4. credential presence；
5. Tunnel distribution/proxy config；
6. Harness lifecycle；
7. Runtime process/readiness；
8. MCP endpoint；
9. Tunnel process/readiness；
10. control plane；
11. MCP target；
12. observed proxy route；
13. credential state。

Independent checks continue；dependent unavailable → UNKNOWN。

Overall：

~~~text
FAIL > WARNING > UNKNOWN > PASS
~~~

---

## 6. Phase D — Fault Mapping

建立中心化 typed mapping。

优先使用：

- ConfigError code；
- SupervisorError code；
- TunnelProcessError/recovery reason；
- HarnessLifecycle；
- TunnelReadinessEvidence。

Remediation 仅说明/导航，不 auto-fix。

Fault matrix 按 SPEC §14 全覆盖。

Generated state 使用 system temp 并清理。

---

## 7. Phase E — UI

Overview 增加明显的一键诊断。

Result：

- overall；
- checks by scope；
- four statuses；
- summary；
- safe evidence；
- remediation；
- retry；
- safe copy。

Startup detail：

- cohort；
- outcome；
- total；
- phases；
- slowest phase。

单次 attempt 不显示成 benchmark PASS。

Settings 复用 aggregate result。

Copy formatter strict allowlist，未知新增字段默认不复制。

i18n：en + zh-CN。

---

## 8. Phase F — Final Candidate

冻结 candidate SHA 后完整跑：

- 2 primers；
- 30 cold；
- 30 warm；
- same environment；
- complete threshold comparison。

任何 startup-affecting code 变化都使 cohort 失效，需要重跑。

同时跑 diagnostics fault matrix、read-only/privacy、M01/V3 regressions。

Final checks：

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

随后执行 AGENTS 规定的 milestone delivery/layout scripts。

---

## 9. Concurrency

关键 race：

- recorder attach vs fast synchronous failure；
- Tunnel Ready event vs launch return；
- old generation event vs new attempt；
- Stop/Restart/Quit while Starting；
- diagnostics while lifecycle transition；
- config save while diagnostics；
- credential disappears between checks。

Rules：

- attempt first；
- generation/session mismatch discard；
- recorder lock 不跨 spawn/network；
- diagnostics 独立 snapshot，无法一致确认则 UNKNOWN；
- diagnostics 不持锁等待 Runtime/Tunnel；
- lifecycle mutation 仍只有 HarnessController operation mutex。

---

## 10. Rollback

- recorder 可保留而单独 revert optimization；
- optimization slices 应可独立回退；
- correctness regression 先回退 optimization，不回退 threshold；
- aggregate diagnostics/UI 可回退而不影响 startup；
- protocol 保持 v1 additive；
- M02 无 config migration。

---

## 11. Task Order

| Task | Owner | Depends on | Output |
| --- | --- | --- | --- |
| T1 | protocol + Tauri timing | prior milestone gate | recorder + B0 |
| T2 | measured startup critical path | T1 B0 | optimization |
| T3 | protocol diagnostics + Host coordinator | T1 contract | unified diagnostics |
| T4 | diagnostics mapping/tests | T3 | fault/remediation |
| T5 | React/client/i18n | T1 + T3/T4 | UX |
| T6 | QA/package | T1–T5 | candidate benchmark + regression + delivery |

---

## 12. Development Completion Gate

Before Integration：

- T1–T6 complete；
- B0 captured before T2；
- final candidate meets every frozen performance formula；
- recorder/readiness tests PASS；
- diagnostics matrix PASS；
- read-only/safe-copy PASS；
- full regression PASS；
- fresh Portable candidate delivered；
- no external E2E/manual falsely marked PASS。

M02 Development Complete 不等于 ACCEPTED / FROZEN。
