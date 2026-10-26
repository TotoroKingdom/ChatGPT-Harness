# M02 独立 QA 执行记录 — 2026-10-26

状态：`BLOCKED`（未执行人工验收；未标记 `ACCEPTED` / `FROZEN`）

执行依据：

- `docs-codex/v4/milestones/M02-startup-performance-diagnostics/INTEGRATION_TEST_PLAN.md`
- `docs-codex/v4/milestones/M02-startup-performance-diagnostics/E2E_TEST_PLAN.md`

本记录位于既有 `docs-codex/v4/evidence/` 根下；未向 M02 milestone 目录新增报告文件。原始临时 fixture、benchmark 目录和测试 profile 使用系统 `%TEMP%`，执行后已清理。

## 1. Candidate / environment header

| 字段 | 实际值 |
| --- | --- |
| Candidate commit | `e71a1aed61960e1c748bec4add0915fa33ac71e0` |
| Product logic baseline | `9530c6e0acfd35073281d6582acc550dfe65b697` |
| B0 instrumentation-only commit | **未提供** |
| Candidate source dirty | `false` |
| Candidate source state SHA-256 | `A4788004893255E9B99D462598C7E11C4D3ACAF325B0003E67F3A14C7E2BD879` |
| Package | `ChatGPT-Harness_0.1.3_x64-portable` |
| Package path | `artifacts/V4-M02-startup-performance-diagnostics/20261025-201908-fede1a23/ChatGPT-Harness_0.1.3_x64-portable` |
| App / Runtime | `0.1.3 / 0.1.3` |
| Tunnel client | `0.0.13` |
| Portable UI SHA-256 | `D18DE1CFAE82E7BCBD19FD8D4E7A4EEF6D2F742676D1E6ECB2C2388BF83C7E31` |
| Runtime SHA-256 | `9D74D75C7787A38730C4E69187834BB09D4E47D60E1A8191D1CEECD807810AAE` |
| Tunnel client SHA-256 | `83F08FB39B1C154747DEBD31B81B65DD4EE834CACF5A073B6301B2688699BC76` |
| Windows | `10.0.19045` / build `19045` |
| CPU / power | Intel i7-12700 / 卓越性能 |
| Workspace fixture | Disposable system-temp fixture; cleaned |
| Config | v3 test copy; credential reference presence only; no secret recorded |
| Tunnel identity | Configured reference only; redacted |
| Proxy / route | Tunnel control-plane route observed; credential-bearing values omitted |
| Product processes after cleanup | `0` |

交付清单还明确记录：`external_e2e = NOT_VERIFIED_BY_DELIVERY_SCRIPT`、`manual_acceptance = NOT_EXECUTED`、`acceptance_status = CANDIDATE_ONLY_MANUAL_ACCEPTANCE_NOT_EXECUTED`。

## 2. Entry audit

| 准入项 | 结果 | 证据 / 说明 |
| --- | --- | --- |
| Previous governance gate | INCONCLUSIVE | 历史 `M01-freeze` tag 存在，但本次未重新核验其完整 governance evidence |
| M02 Development Complete handoff | **BLOCKED** | 未找到包含 B0 SHA、B0 baseline stats、candidate stats 和 threshold table 的 handoff |
| Exact candidate identity | PASS | 当前 `HEAD` 与 Portable `build_sha` 均为 `e71a1ae...` |
| B0 exact SHA + numeric 30 cold / 30 warm baseline | **BLOCKED** | 交付 manifest 无 B0 字段；旧 M02 delivery 也记录 `performance: BLOCKED` |
| Candidate 30 cold / 30 warm statistics | **BLOCKED** | 未提供；不能用单次 probe 或本地单测替代 |
| Candidate Portable exists and verifies | PASS | `portable-verification.txt` 为 `PASS`；Runtime smoke 为 `PASS` |
| Task-local automation | **INCONCLUSIVE** | Rust/serial UI 全部通过；项目默认并发 `npm test` 出现可重复的非 M02 M7 UI timeout/flaky failure |
| Disposable fixture/profile | PASS | 真实 probe 使用临时 profile；执行后删除且无产品进程残留 |

准入结论：Development Complete、B0 和完整 candidate benchmark 条件未满足。因此性能比较、Integration exit gate 和 E2E gate 均不能放行。

## 3. Preflight commands

| 命令 | 结果 |
| --- | --- |
| `powershell -NoProfile -ExecutionPolicy Bypass -File ui/scripts/check-repository-layout.ps1` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS（既有 warning，无编译错误） |
| `cargo test --workspace` | PASS：Runtime 351、Tauri/UI 189、协议契约及其他测试通过；6 个既有 ignored |
| `cd ui; npm run typecheck` | PASS |
| `cd ui; npm run build` | PASS（仅有 bundle size warning） |
| `cd ui; npm test` | **FAIL / flaky**：一次 365/367，另一次 366/367；失败集中在并发负载下的 UI/M7 async assertion |
| `cd ui; npx vitest run --no-file-parallelism --maxWorkers=1 --minWorkers=1` | PASS：39 files / 367 tests |

失败的默认 UI 测试在单文件、目标文件联跑和单 worker 模式下通过，未形成稳定的 M02 产品缺陷复现；未修改测试配置或产品代码。

## 4. Integration results

| Case | Verdict | Actual evidence / boundary |
| --- | --- | --- |
| M02-INT-01 | **BLOCKED** | `startup::tests` 3/3 PASS，覆盖固定 phase、失败边界和 stale session；真实 Portable probe 未达到 authoritative Ready，无法完成成功 Start + failure cohort 的完整 Case |
| M02-INT-02 | PASS | Host recorder 的 3 个 startup contract tests PASS；offset/duration 单调、失败 bounded、stale session 不可关闭新 attempt |
| M02-INT-03 | **BLOCKED** | `startup-benchmark.test.ts` 的 nearest-rank/failure denominator/sample-gate 相关断言通过，但 B0 SHA/30+30 数值缺失，按计划不能 PASS |
| M02-INT-04 | **BLOCKED** | threshold evaluator 的现有 fixture tests 通过；没有 B0/candidate 数值，不能执行冻结阈值比较 |
| M02-INT-05 | INCONCLUSIVE | `harness::tests`、`runtime::tests`、`tunnel::tests` readiness/错误边界通过；真实 package probe 的 Tunnel child 以 exit code 1 反复退出，未形成成功 Ready |
| M02-INT-06 | PASS | Harness concurrency/stale-generation tests、Runtime restart/concurrency tests、Tunnel generation/recovery tests通过；未发现第二 authority 或跨 session 复用证据 |
| M02-INT-07 | INCONCLUSIVE | `DiagnosticsCoordinator` 源码包含 16 个必需 check IDs，另有 `runtime_diagnostics` additive check；缺少已满足真实 Ready 的 coordinator aggregate run |
| M02-INT-08 | INCONCLUSIVE | Coordinator 代码读取 ConfigStore、RuntimeSupervisor、TunnelProcessManager、HarnessController、SecretStore；未能在当前未 Ready 的真实包上完成完整 authority spy/aggregate run |
| M02-INT-09 | PASS | Tauri diagnostics tests 4/4、Runtime diagnostics tests及 full workspace tests通过；FAIL/WARNING/UNKNOWN/PASS precedence 与 unknown-safe evidence 断言通过 |
| M02-INT-10 | PASS | Protocol generated artifacts、schema/type contract tests 10/10；UNKNOWN/scope/evidence/remediation additive v1 contract 通过 |
| M02-INT-11 | INCONCLUSIVE | Runtime/Config/Workspace/MCP fault unit coverage通过；未完成 frozen plan 要求的完整 Host fault matrix 实例集 |
| M02-INT-12 | PASS | Tunnel tests 40/40；stale poll、unauthorized、proxy scope、MCP target/local readiness、generation/recovery 边界通过 |
| M02-INT-13 | PASS | SecretStore/reference、credential mapping、structured logger redaction 相关 workspace tests通过；未记录或输出 raw secret |
| M02-INT-14 | PASS | Diagnostics read-only/safe evidence tests通过；Coordinator 不调用 lifecycle mutation、save/reset、SecretStore set/delete、approval resolve 或 tool execution |
| M02-INT-15 | INCONCLUSIVE | 前端 M02 相关 tests 24/24、serial full UI 367/367；Overview/Settings 的真实 aggregate API equivalence 未在 Ready packaged instance 上完成 |
| M02-INT-16 | PASS | diagnostic-copy、M01/V3 lifecycle/config/policy/approval/SecretStore/runtime regression 自动化通过；sentinel-safe-copy 断言通过 |

### Real packaged probe

执行的真实 runner（使用临时 profile，不使用 Mock）：

```text
powershell -NoProfile -ExecutionPolicy Bypass -File ui/scripts/run-startup-benchmark.ps1 -Mode Cold -Count 1 -PrimerCount 0 -PortableDirectory <M02 candidate Portable path>
```

实际结果：`acceptedStarts=1`, `readySamples=0`, `failures=1`, `failureRate=1`。Portable UI 启动并拉起真实 Tunnel client，但 Tunnel client 在 control-plane metadata 之后以 exit code 1 退出并未达到 authoritative Ready；没有用本地 MCP、Mock 或内部单测替代这个结果。临时 profile、fixture 和 benchmark directory 已清理。

该 probe 不是 B0/candidate cohort，也不参与性能统计或 PASS 判定。

## 5. E2E results

E2E plan 的 Gate 是 `M02 Development Complete + Integration PASS`。该 Gate 未满足，且当前真实 packaged probe 未达到 Ready，因此没有模拟执行或替代外部边界。

| Case | Verdict | Reason |
| --- | --- | --- |
| M02-E2E-01 | **BLOCKED** | B0 identity/stats、candidate 30+30 stats 缺失；真实 candidate 未达到 Ready |
| M02-E2E-02 | **BLOCKED** | 无 healthy authoritative Ready，不能执行真实 Overview/Settings aggregate diagnostics |
| M02-E2E-03 | **BLOCKED** | Integration gate 未 PASS；没有已确认的受控 managed-instance fault mechanism；未执行破坏性故障注入 |
| M02-E2E-04 | **BLOCKED** | Integration gate 未 PASS；真实 ChatGPT→Tunnel Tool call 外部边界未可用；未用 direct local MCP 冒充 |

人工验收：`NOT EXECUTED`，按用户要求没有标记 `ACCEPTED` / `FROZEN`。

## 6. Remaining blockers

1. 提供并冻结 B0 instrumentation-only SHA、同环境 30 cold + 30 warm raw/summary、以及 candidate 同契约 30+30 statistics/threshold table。
2. 在受控测试 profile 上恢复真实 Tunnel/control-plane readiness，使 Portable candidate 能达到 authoritative Ready；当前观测到 Tunnel client exit code 1，但未有足够安全证据把它归因到产品代码而非外部 route/auth 环境。
3. 解决或在 CI/QA runner 中明确隔离默认并发 `npm test` 的既有 async/flaky failure；serial 367/367 通过不能把默认命令的红灯改写为 PASS。
4. 重新执行完整 INT-01～16 后，才能进入 E2E-01～04；E2E-04 还需要真实 ChatGPT Tool boundary，E2E-03 需要已授权的受控 fault mechanism。

## 7. Cleanup / workspace state

- Candidate source logic 未修改；未修改 SPEC、PLAN、TASKS、INTEGRATION_TEST_PLAN、E2E_TEST_PLAN 或 Manual checklist。
- 临时测试 profile、Workspace fixture、benchmark data 已清理。
- Portable UI/Runtime/Tunnel/Cloudflared 进程数为 0。
- 本次 QA 产生的 PowerShell `Microsoft/Windows/PowerShell/ModuleAnalysisCache` 仓库残留已在确认未被 Git 跟踪后删除。
- 本记录是 QA 执行证据，不是 M02 acceptance/freeze 记录。
