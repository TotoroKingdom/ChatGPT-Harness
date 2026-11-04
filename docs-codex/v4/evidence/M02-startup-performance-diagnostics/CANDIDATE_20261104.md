# M02 Candidate Record — 2026-11-04

状态：`CANDIDATE_ONLY`。F05 冻结公式在正式打包候选上通过；M02 未标记 `ACCEPTED` / `FROZEN`，人工验收未执行。

本记录位于既有 `docs-codex/v4/evidence/` 下；未修改 `SPEC.md`、`PLAN.md`、`TASKS.md`、任何冻结测试计划，也未修改 `B0_FREEZE_20261027.md`、`CANDIDATE_COMPARISON_20261027.md` 或 `QA_EXECUTION_20261026.md`。

## 1. 用户授权记录

2026-11-04，用户对该性能方案作出明确授权：

> M02 F05 技术裁决：正式批准 managed Tunnel 使用 5s idle long-poll timeout，作为本轮性能优化方案。保留当前已实测的修复，不回退代码。

该授权只覆盖本候选的 5s idle long-poll 参数；不构成 M02 验收结论，也不改变冻结阈值、Ready 谓词或安全检查。

## 2. 根因与修复

- `control_plane_connected` 仅在 tunnel-client 的 `commands_poll_last_successful_timestamp_seconds` 新鲜时关闭。
- tunnel-client 对空闲 commands long-poll 默认持有 30 秒；启动时无待处理命令，因此首个“成功轮询”约在启动后 30.2 秒才完成。
- 实测同一实例中，认证控制面 metadata 在约 1.1 秒已完成，MCP target 探针也已通过；30 秒等待是**证据延迟**，不是连接或初始化耗时。
- 修复：Host 启动 managed Tunnel 时传入 `--control-plane.poll-timeout 5s`。Ready 仍要求新鲜成功轮询（90 秒 freshness）、metadata 成功、MCP target 可达和 authoritative `HarnessStatus` Ready，未改变任何判定条件。

代码：`ui/src-tauri/src/harness.rs:26`、`:503`；`ui/src-tauri/src/tunnel/mod.rs:262`、`:301`、`:1756`；新增测试 `real_command_bounds_startup_control_plane_poll_timeout`。

## 3. Candidate identity

| Field | Value |
| --- | --- |
| Candidate commit | `9d49a245ffcc385b5fdce8bc1ea216539e8cb9c4` |
| Branch / dirty | `main` / `false` |
| Source state SHA-256 | `BFA5F4D019D604AC1E869951331A8B978523320746704A43F01303867711D6E5` |
| Product logic baseline | `9530c6e0acfd35073281d6582acc550dfe65b697` |
| B0 instrumentation-only commit | `6a2418521fb963934e1486b504df7d7ed06c20ae`（未改变） |
| App / Runtime / Tunnel | `0.1.3 / 0.1.3 / 0.0.13` |
| Portable path | `artifacts/V4-M02-startup-performance-diagnostics/20261104-221318-e8754dd5/ChatGPT-Harness_0.1.3_x64-portable` |
| Portable tree SHA-256 | `96A5B0749591874222204586C91509D3D3041CE4F004996AE7C0301347A6E688` |
| Portable UI SHA-256 | `950D55FD648BAB3950084506E4254F5CE5187DD5D99741C6E911B5CBC7BB74C0` |
| Portable Runtime SHA-256 | `72C0EAAA2D0888303380410E91F856BBFBE4F8B54AE5D93815263541F07D092D` |
| Portable Tunnel SHA-256 | `83F08FB39B1C154747DEBD31B81B65DD4EE834CACF5A073B6301B2688699BC76` |
| Portable Cloudflared SHA-256 | `CD0BC49AEC24D0D62A37B6E842DAD46000F7B401F6167B4CBACE4E170819C9A2` |
| `distribution.json` SHA-256 | `63F25327298144F0C6AEE3FF9C73C3031B74D3D6768487600B05F5357CC9F262` |
| Portable verification | `PASS` |

## 4. 环境锁

| Field | Value |
| --- | --- |
| Windows | `10.0.19045` / build `19045` |
| CPU / power | Intel i7-12700 / 卓越性能 |
| Config | 真实 v3 profile；revision `42`；benchmark 未修改配置 |
| Credential | Windows Credential Manager reference；未读取或记录 secret value |
| Proxy / route | `control-plane.http-proxy`；观测 route mode `proxy`；未记录凭据 |
| Workspace | 同一受控 Workspace |
| Network | 同一真实 proxy/control-plane route；无 Mock/local 替代 |

## 5. 正式队列（2 primer + 30 counted / cohort）

命令：

```text
powershell -NoProfile -ExecutionPolicy Bypass -File ui/scripts/run-startup-benchmark.ps1 `
  -Mode Cold -Count 30 -PrimerCount 2 `
  -PortableDirectory "artifacts/V4-M02-startup-performance-diagnostics/20261104-221318-e8754dd5/ChatGPT-Harness_0.1.3_x64-portable" -KeepRaw
powershell -NoProfile -ExecutionPolicy Bypass -File ui/scripts/run-startup-benchmark.ps1 `
  -Mode Warm -Count 30 -PrimerCount 2 `
  -PortableDirectory "artifacts/V4-M02-startup-performance-diagnostics/20261104-221318-e8754dd5/ChatGPT-Harness_0.1.3_x64-portable" -KeepRaw
```

| Cohort | Counted | Ready | Failures | P50 | P95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Cold | 30 | 30 | 0 | 7,394 ms | 16,483 ms |
| Warm | 30 | 30 | 0 | 7,387 ms | 8,974 ms |

对照未修改的 B0 基线：

| Cohort | B0 P50 | B0 P95 | Candidate P50 | Candidate P95 | P50 改善 | P95 改善 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Cold | 31,502 ms | 33,508 ms | 7,394 ms | 16,483 ms | 76.5% | 50.8% |
| Warm | 32,079 ms | 66,405 ms | 7,387 ms | 8,974 ms | 77.0% | 86.5% |

### 冻结阈值判定

| Rule | Frozen requirement | Actual | Verdict |
| --- | --- | --- | --- |
| Cold P50 | ≤ 28,351.8 ms | 7,394 ms | PASS |
| Warm P50 | ≤ 27,267.15 ms | 7,387 ms | PASS |
| Cold P95 guard | ≤ 35,183.4 ms | 16,483 ms | PASS |
| Warm P95 guard | ≤ 69,725.25 ms | 8,974 ms | PASS |
| At least one P95 ≤ 90% B0 | cold ≤ 30,157.2 或 warm ≤ 59,764.5 ms | 两者均满足 | PASS |
| Failure rate | ≤ B0 且 ≤ 1/30；B0 零失败则候选零失败 | cold `0/30`，warm `0/30` | PASS |
| Ready sample gate | 每 cohort ≥ 29 | 30 / 30 | PASS |
| Material bottleneck | clause A 或 B | clause B：两个 P95 均改善 > 10% | PASS |

### 原始证据

- [Candidate cold aggregate](raw-Candidate-20261104/Candidate-cold-aggregate.ndjson) — 32 行，SHA-256 `7C887A5F1D453FC8421192DDEC6E9C296F2705A7095D47E21FA5A464FFD04ADE`
- [Candidate warm aggregate](raw-Candidate-20261104/Candidate-warm-aggregate.ndjson) — 33 行（1 bootstrap cold + 32 warm），SHA-256 `444C3426EA2086395954D4F6C3357D33F73134A36C155E42D2E2863BB7D0DFC1`
- [Packaged diagnostics run](raw-Candidate-20261104/diagnostics-run.json) — 17 checks，SHA-256 `D5ADE3192FA5FB5FF4828D665A04192980148B513C70256031F49E0FD8509C3C`
- [15-minute soak samples](raw-Candidate-20261104/soak-15min.ndjson) — 30 samples，SHA-256 `0143577C8C85960C190AAC3100999961444C2A9E7DCDC898CD3DA851A19D6BF4`

60 个 counted attempt ID 全部唯一；phase 状态全部为 `completed`；无负 offset/duration；未删除任何慢样本。

仓库内真实阈值评估器（`ui/src/services/harness/startup-benchmark.ts`）直接读取 B0 与候选 aggregate 复算：baseline cold `31,502 / 33,508`、warm `32,079 / 66,405`（warm `29/30` Ready、1 失败）与冻结值一致；候选 `coldMedian/warmMedian/coldTailGuard/warmTailGuard/tailImprovement/failureRateGuard/readySampleCount/materialBottleneck` 八项全为 `true`，`pass=true`。

## 6. 5s 参数影响评估

在真实打包候选上完成 15 分钟单实例 soak（30 个采样点，间隔 30 秒）：

| 指标 | 实测 |
| --- | --- |
| Harness / Tunnel / control-plane 状态 | 30/30 采样均为 Ready |
| readiness / liveness / proxy health gauge | 30/30 均为 1 |
| 轮询周期 | 150 次 / 881.6 s = **10.21 次/分钟**（约 613 次/小时） |
| 平均轮询延迟 | 5.84 s（`poll_timeout_ms=5000`，`poll_deadline_ms=10000`） |
| 轮询超时 | 1 / 150（0.67%）；客户端 200 ms 后退避重试；readiness 未翻转 |
| HTTP 429 / rate limit | 0 |
| poll freshness age 最大值 | 9.42 s（远低于 90 s freshness 窗口） |
| CPU | 15 分钟内 5.80 s CPU，约单核 0.66% |
| RSS | 32.4 MB → 34.3 MB（最大 34.8 MB），无持续增长 |
| Goroutine / queue | 25–28 / 0 |
| 连接通知 | 无 incident transition；lifecycle 全程 Ready，未触发降级/恢复通知 |

结论：请求率约为默认 30 秒配置的 5 倍，但绝对量很低；超时在 10 秒 HTTP deadline 内偶发一次并自动恢复；未出现限流、重连风暴、状态翻转或通知回归。

观察项（非本次 5s 引入）：长时间 Ready 时，Host readiness watcher 每 500 ms 探测一次，因 poll age 变化而持续写入 `readiness_transition` 日志（15 分钟 1,944 行）。该行为与 30 秒或 5 秒轮询无关，属于既有日志量问题，留给后续独立改进，不纳入本候选。

## 7. Ready 正确性证据

- 真实启动中间态：Runtime ready、Tunnel process ready、localMcp ready，但 `controlPlaneConnected=false` 且 lifecycle 仍为 `starting`；process existence 未被当作 Ready。
- 真实 Starting 中 Stop：attempt outcome=`cancelled`，`control_plane_connected` 为 `incomplete`，`mcp_target_ready` / `harness_ready` 为 `skipped`，错误码为 bounded `CANCELLED`，未编码成 0 ms 成功。
- 60 个正式样本与 15 分钟 soak 均只在权威条件满足后才进入 Ready。

## 8. 对 M06 候选证据的影响

- 本次产品代码变更产生新的 UI 二进制：`950D55FD…`，取代 `8861251` 候选的 `A54A01DF…`。
- `artifacts/V4-M06-v4-core-release-gate/20261104-*` 下六个候选（`M06-6f778de…` 至 `M06-8861251-a54a01df`）均绑定变更前的 commit/UI hash，F05 equivalence 不再成立。
- M06 恢复后必须从包含 `9d49a24` 的 commit 重建最终 candidate，更新 `candidate_id`、portable/installer/installed hashes，重新执行 package audit、F05 equivalence 或 `FULL_M02_BENCHMARK_REQUIRED` 全量队列，以及受影响的 M06 Integration/E2E。
- 本次未修改任何 M06 冻结契约或候选产物；M06 发布验收保持暂停。
