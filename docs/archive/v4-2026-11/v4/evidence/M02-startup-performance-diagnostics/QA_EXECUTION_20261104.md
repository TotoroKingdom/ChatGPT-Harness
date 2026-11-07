# M02 QA Execution — 2026-11-04

状态：`CANDIDATE_ONLY` / 未验收。本记录区分开发级自动化、正式打包验证与外部/人工边界；未标记 `ACCEPTED` / `FROZEN`。

依据：`docs-codex/v4/milestones/M02-startup-performance-diagnostics/` 下已冻结的 SPEC / PLAN / TASKS / INTEGRATION_TEST_PLAN / E2E_TEST_PLAN / MANUAL_ACCEPTANCE_CHECKLIST。未修改这些文档。

Candidate identity 与 F05 数据见 [CANDIDATE_20261104.md](CANDIDATE_20261104.md)。Candidate commit `9d49a245ffcc385b5fdce8bc1ea216539e8cb9c4`，`source_dirty=false`。

## 1. Preflight

| 命令 | 结果 |
| --- | --- |
| `ui/scripts/check-repository-layout.ps1` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS：702 passed / 26 ignored / 0 failed |
| `cd ui; npm run typecheck` | PASS |
| `cd ui; npm test` | PASS：40 files / 379 tests |
| `cd ui; npm run build` | PASS（仅既有 bundle size warning） |

## 2. Integration INT-01～16

| Case | Verdict | Evidence |
| --- | --- | --- |
| M02-INT-01 | **PASS** | 60 个真实打包 Start 产生唯一 attempt、绑定同一 session、9 个固定 phase、单调 offset；真实 Starting 中 Stop 得到 `cancelled` + `incomplete/skipped`，未伪造 0 ms；`startup::tests` 3/3 |
| M02-INT-02 | **PASS** | Host monotonic 计时；60 个 attempt 无非负/单调异常；`startup::tests` 与 age-only event 测试通过 |
| M02-INT-03 | **PASS** | 2 primer 排除、30+30 counted、失败率分母、nearest-rank P50/P95、≥29 Ready 全部按冻结协议复算；B0 `6a24185` 早于 T2 `9d49a24` |
| M02-INT-04 | **PASS** | `startup-benchmark.test.ts` 边界/公式测试通过；仓库真实 `compareStartupBenchmark` 读取 B0/候选 aggregate 后八项全 true、`pass=true`；阈值未从运行时配置读取 |
| M02-INT-05 | **INCONCLUSIVE** | 已验证：真实启动中间态 process ready 但 control-plane 未连接，lifecycle 仍 starting；取消语义正确；Runtime/MCP/Tunnel/control-plane 边界测试通过。未执行：真实打包的 invalid config/Workspace/missing credential 等 fault matrix；不得记为 PASS |
| M02-INT-06 | **PASS** | 未引入 PreparedStart/cache；仅增加一次性 launch 参数；config/secret 不跨 Start 复用；并发/代际/Stop-Restart 测试通过 |
| M02-INT-07 | **PASS** | 真实打包 Ready 实例聚合诊断返回 17 个 check，包含全部 16 个必需 ID + `runtime_diagnostics`；overall PASS |
| M02-INT-08 | **PASS** | Coordinator 读取 ConfigStore / RuntimeSupervisor / TunnelProcessManager / HarnessController / SecretStore presence；真实运行结果与 source inspection 一致，React 未重算连接真相 |
| M02-INT-09 | **PASS** | precedence `FAIL > WARNING > UNKNOWN > PASS` 与 UNKNOWN 语义测试通过；真实健康态 PASS；未把 UNKNOWN 渲染为 PASS |
| M02-INT-10 | **PASS** | protocol v1 contract tests 12/12 + protocol lib 4/4；UNKNOWN/scope/evidence/remediation additive |
| M02-INT-11 | **INCONCLUSIVE** | Config/Workspace/Runtime/MCP 的单元边界测试存在并通过，但没有冻结计划要求的完整 Host fault matrix 实例集；真实打包本地故障未注入 |
| M02-INT-12 | **PASS** | Tunnel stale/unauthorized/proxy/target/generation/recovery 测试通过；真实健康态 control_plane/proxy/target PASS |
| M02-INT-13 | **PASS** | credential presence/state 区分与真实 PASS 结果一致；无 secret/raw error 进入 DTO；测试覆盖 presence 与 accepted 区分 |
| M02-INT-14 | **PASS** | 诊断前后同一 session `1bbe18df…`、configRevision `42`、无进程重启；只读测试与真实前后状态一致 |
| M02-INT-15 | **PASS** | 真实 Overview 与 Settings 展示同一 17 项结果和同一 startup detail；前端模型测试通过；无重复分类表 |
| M02-INT-16 | **PASS** | 真实“复制诊断摘要”文本仅含 allowlist 字段；不含 Workspace path、secret、proxy URL、raw logs/error；M01/V3 回归测试通过 |

Integration 结论：**14 PASS / 2 INCONCLUSIVE（INT-05、INT-11）**。按冻结 exit gate，Integration 未 PASS，不能宣称 M02 Integration 完成。

## 3. E2E

| Case | Verdict | Evidence |
| --- | --- | --- |
| M02-E2E-01 | **PASS** | 正式打包候选按 2+30 cold、2+30 warm 执行；全部冻结阈值 PASS；原始 aggregate 与哈希见 CANDIDATE 记录 |
| M02-E2E-02 | **PASS（case-level）** | 真实打包 UI：17/17 诊断 PASS、最新 startup detail（cold / ready / 8,916 ms / 最慢 control_plane 8,619 ms）、复制摘要 allowlist、Overview 与 Settings 结果一致；见 `raw-Candidate-20261104/diagnostics-run.json` |
| M02-E2E-03 | **BLOCKED** | 需要授权的 controlled test profile 与本地故障注入；不得修改真实用户 profile，未执行本地与连接侧故障矩阵 |
| M02-E2E-04 | **BLOCKED** | 真实 External ChatGPT → Tunnel Tool call 未执行；不能用 local MCP 替代 |

E2E 结论：**E2E-01 PASS、E2E-02 PASS（case-level）、E2E-03/04 BLOCKED**；E2E exit gate 未满足，且 Integration exit gate 尚未满足。

## 4. Manual Acceptance

`NOT EXECUTED`。需要用户人工完成：

- 审阅 Overview/Settings 的诊断信息与启动阶段展示；
- 确认连接降级/恢复通知行为符合预期；
- 在真实 ChatGPT 中完成一次受控 Workspace Tool call，验证 Ready 可用；
- 对 M02 作出 `ACCEPTED` / `FROZEN` 决定。

## 5. Cleanup

- 测试实例已 Stop/退出；无 `chatgpt-harness-ui.exe` / `tunnel-client.exe` / `chatgpt-harness-runtime.exe` / `cloudflared.exe` 残留。
- 临时 benchmark 目录、CDP 临时脚本与 profile 外临时数据已清理；仅保留本记录引用的原始统计。
- 未修改真实 config/credential/Workspace；benchmark 使用真实 profile 的只读配置。
