# M02 SPEC — Startup Performance and One-click Diagnostics

> 布局与测试生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。M02 六份里程碑文档现已齐全并在实现前作为实施/验证契约冻结；目录必须继续严格保持六文件布局。不得新增独立报告、实验工程、仓库顶级目录或把执行日志写入本目录。  
> Status: DRAFT / DESIGN READY — IMPLEMENTATION BLOCKED UNTIL PREVIOUS MILESTONE ACCEPTANCE  
> Milestone: M02 — Startup Performance and One-click Diagnostics  
> Scope: P0 F05 Harness Startup Performance + F06 One-click Connection Diagnostics  
> Roadmap dependency: M00  
> Repository execution gate: 按当前 AGENTS.md 的串行里程碑规则，M01 未经用户明确人工验收并标记 ACCEPTED / FROZEN 前，不得开始 M02 实现型 Task。  
> Product baseline commit observed while writing this SPEC: 9530c6e0acfd35073281d6582acc550dfe65b697  
> Performance rule: 本 SPEC 冻结样本定义、计时区间、统计公式和候选改善阈值；实际 baseline 数值必须在 T1 instrumentation-only 后、T2 优化前采集并锁定，之后不得为让候选 PASS 而修改阈值。

---

## 1. Objective

M02 解决两个相互关联但职责不同的问题：

1. F05 — Startup Performance（启动性能）：把一次 Start Harness → authoritative Ready 拆成可解释的阶段，建立同环境 cold / warm baseline，并真正缩短用户等待时间。
2. F06 — One-click Diagnostics（一键连接诊断）：把现在分散在 Config、Runtime、Tunnel、SecretStore、HarnessStatus 中的状态聚合成一次只读诊断，让用户不用看内部日志就知道“哪里坏了、证据是什么、下一步去哪处理”。

M02 不允许用“更早显示 Ready”冒充性能优化，也不允许诊断模块成为第二套生命周期、Policy 或连接真相。

~~~text
User Start
   ↓
HarnessController
   ├─ StartupPhaseRecorder
   │    ├─ config / eligibility
   │    ├─ Runtime spawn / initialize
   │    ├─ MCP endpoint ready
   │    ├─ Tunnel resolve / launch
   │    ├─ control-plane connected
   │    ├─ MCP target ready
   │    └─ final authoritative Harness Ready
   └─ existing RuntimeSupervisor + TunnelProcessManager

User Run Diagnostics
   ↓
DiagnosticsCoordinator (Tauri Host, read-only)
   ├─ ConfigStore / Workspace
   ├─ Runtime distribution / process / runtime.status
   ├─ MCP endpoint
   ├─ Tunnel distribution / process / readiness / recovery
   ├─ control-plane route / stale evidence
   ├─ Proxy config / observed route
   ├─ SecretReference presence / safe credential state
   └─ HarnessController lifecycle
   ↓
PASS / WARNING / FAIL / UNKNOWN
~~~

---

## 2. Upstream Canonical Acceptance Criteria

### F05

- F05-AC01 — Start→Ready 必须记录可比较的总耗时和关键阶段耗时，并区分 cold / warm、P50 / P95、失败率和环境。
- F05-AC02 — 性能 baseline 与改善阈值必须按 PRD §3.7 在 SPEC_READY 前冻结，候选版本必须达到已冻结目标才能 PASS。
- F05-AC03 — 优化不得跳过 authoritative readiness checks，不能以 spawn success 或 process exists 冒充 Ready。

### F06

- F06-AC01 — 一键诊断至少覆盖 config、Workspace、Runtime、MCP、Tunnel、control-plane、proxy、credential 可安全判断状态和 lifecycle。
- F06-AC02 — 每项诊断返回 PASS / WARNING / FAIL / UNKNOWN、可读解释、安全技术证据与适用 remediation。
- F06-AC03 — 默认诊断只读；修改配置、重启、触碰密钥或文件的修复动作必须另行显式授权，并以故障 fixtures 验证。

相关 Architecture 决策：

- A01 — Tauri Host 保持 lifecycle owner；Runtime 保持 execution / authorization authority。
- A11 — Start→Ready 使用 Host monotonic clock；禁止跨进程直接相减 Instant。
- Control Protocol 继续 v1 additive evolution。
- DiagnosticsCoordinator 只聚合权威来源，旧 History 不能判定当前健康。
- F05 目标必须在优化实现前冻结。

---

## 3. Current Baseline to Preserve

在 9530c6e0acfd35073281d6582acc550dfe65b697：

- production AppConfig 已是 schema v3；
- HarnessController 负责串行 Start / Stop / Restart 和 authoritative HarnessStatus；
- HarnessStatus 已包含 Runtime / MCP / Tunnel component state、Tunnel health/readiness/recovery 与 last error；
- RuntimeSupervisor 已负责 Runtime spawn、initialize、MCP endpoint、runtime.status 与 diagnostics.run；
- Tauri 已有 local diagnostics，覆盖部分 Workspace / Tunnel config / credential presence / security；
- 当前 DiagnosticStatus 只有 PASS / WARNING / FAIL，没有 UNKNOWN；
- 当前 DiagnosticCheck 只有 id / status / summary / remediation，没有 typed safe evidence / scope；
- Settings 已有基础 diagnostics 展示；
- 当前 Start 路径在 UI、HarnessController、Runtime/Tunnel resolution 之间存在重复 eligibility/config/discovery 工作，M02 必须先测量再决定是否优化。

以上只代表当前代码事实，不代表 F05/F06 已完成。

---

## 4. Scope

### Included

M02 实现：

- Host-owned StartupPhaseRecorder；
- typed startup attempt / phase DTO；
- latest startup attempt detail；
- cold/warm baseline 和 candidate comparison；
- baseline instrumentation-only checkpoint；
- measured bottleneck optimization；
- DiagnosticStatus.UNKNOWN；
- typed DiagnosticScope / safe evidence / remediation；
- Tauri DiagnosticsCoordinator；
- config / Workspace / Runtime / MCP / Tunnel / control-plane / proxy / credential / lifecycle checks；
- safe copyable diagnostic summary；
- Overview 一键诊断入口；
- Settings 复用同一诊断协调器；
- fault fixtures；
- en / zh-CN；
- package/regression verification。

### Non-goals

M02 不实现：

- F02 Auto Start Harness；
- Windows 登录自启动；
- SQLite History；
- F08 Tool metrics；
- 新 daemon/service；
- 自动修改配置、Workspace、Proxy 或 Secret；
- 自动 Restart/Start/Stop 作为“运行诊断”的副作用；
- 新 Policy/Approval 逻辑；
- 通过放宽 stale/readiness 条件换性能；
- 把 process exists/spawn success 当 Ready；
- 持久化 raw logs 或敏感诊断 payload。

---

## 5. Startup Measurement Contract

### 5.1 Timing owner

所有跨组件 Start→Ready latency 由 Tauri Host monotonic clock 计算。

允许：

- Host Instant；
- 同一模块内部自产 duration；
- Host 收到 Runtime/Tunnel event 时记录 Host monotonic time。

禁止：

- 跨进程 Instant 相减；
- 使用 RFC3339 wall-clock 做 latency math。

### 5.2 Attempt identity

~~~text
StartupAttempt
- attemptId
- harnessSessionId
- cohort: cold | warm
- startedAt
- finishedAt?
- outcome: ready | failed | cancelled | incomplete
- totalDurationMs?
- phases[]
- failureCode?
- appVersion
- runtimeVersion?
- tunnelVersion?
~~~

attemptId 为 Host-local UUID；harnessSessionId 沿用真实 Harness session。

### 5.3 Frozen phase IDs

1. eligibility_prepare
2. runtime_spawn
3. runtime_initialize
4. mcp_endpoint_ready
5. tunnel_resolve
6. tunnel_launch
7. control_plane_connected
8. mcp_target_ready
9. harness_ready

~~~text
StartupPhaseTiming
- phase
- startedOffsetMs
- durationMs
- status: completed | failed | skipped | incomplete
- safeErrorCode?
~~~

规则：

- harness_ready 只在现有 HarnessStatus 真正进入 Ready 时完成。
- earlier prerequisite failure 后续 phase 必须是 skipped/incomplete，而不是 0ms success。
- stale generation/session event 不能关闭新 attempt。
- error 只记录 typed/bounded safe code。

### 5.4 Primary interval

~~~text
T0 = Host enters accepted Harness start operation
T1 = authoritative HarnessStatus first becomes Ready for same session
totalDurationMs = monotonic(T1 - T0)
~~~

计时包括 authoritative Start 内的 eligibility/config preparation，不包括 desktop app launch time 和用户等待时间。

---

## 6. Cold / Warm Definition

### Cold

M02 的 cold 指 application-process cold，不承诺 OS page-cache cold：

1. 桌面应用完全退出；
2. candidate-owned Runtime/Tunnel 均已退出；
3. 等待至少 3 秒；
4. 重新打开同一个 package；
5. app 初始化稳定后 Start；
6. Start→Ready timer 从 Host T0 开始，不包含 app launch。

### Warm

1. 同一个桌面 app process；
2. 上一次已 Ready；
3. 正常 Stop Harness；
4. 等 authoritative Stopped 后至少 2 秒；
5. 不修改 config/credential；
6. 再 Start。

### Environment lock

Baseline 与 candidate 必须使用：

- 同一物理机器；
- 同一 Windows build；
- 同一 power mode；
- 同一 package type；
- 同一 Workspace fixture；
- 同一 Tunnel ID；
- 同一 SecretReference/credential；
- 同一 proxy configuration；
- 同一 Security/Tool/Policy config；
- 同一 tunnel-client distribution；
- 同一 network access method；
- 同一 control-plane route mode（可观测时）。

必须记录 app/runtime/tunnel version、commit、package hash、Windows version、CPU class、power mode、proxy source/route mode、network type、configRevision、Workspace fixture identity，但不得记录 secret。

环境核心条件变化则比较无效，需要重跑。

---

## 7. Frozen Baseline Sample Contract

### 7.1 Product baseline

Product logic baseline 固定为：

~~~text
9530c6e0acfd35073281d6582acc550dfe65b697
~~~

T1 可以增加 measurement-only instrumentation，形成 instrumentation-only baseline commit B0。

B0 capture 前：

- 不允许 startup optimization；
- 不允许删除 validation/discovery；
- 不允许放宽 timeout/stale/readiness；
- recorder 必须在 B0 和 final candidate 都存在。

B0 SHA 必须在 T2 开始前记录。

### 7.2 Sample count

B0：

- 2 次 primer，不计统计；
- 30 次 counted cold；
- 30 次 counted warm。

Final candidate 同样：

- 2 次 primer；
- 30 cold；
- 30 warm。

所有 accepted start 都进入 failure-rate denominator。

Latency P50/P95 只使用真正到 authoritative Ready 的样本。

每 cohort 至少需要 29 个 Ready 样本；少于 29 则该 cohort FAIL/BLOCKED，不允许静默删除失败样本。

### 7.3 Percentile

Nearest-rank：

~~~text
rank(P50) = ceil(0.50 * N)
rank(P95) = ceil(0.95 * N)
~~~

1-based。

禁止：

- 平均多个 P95；
- 平均 phase P95 得 total P95；
- 删除看起来“太慢”的有效样本。

### 7.4 Failure rate

~~~text
failureRate = accepted starts not reaching Ready / all accepted counted starts
~~~

---

## 8. Frozen F05 Improvement Threshold

以下阈值现在冻结，B0 数值出来后不得为了 PASS 修改。

定义：

- Bc50 / Bc95 = B0 cold total P50/P95
- Bw50 / Bw95 = B0 warm total P50/P95
- Cc50 / Cc95 = candidate cold total P50/P95
- Cw50 / Cw95 = candidate warm total P50/P95

### Median improvement

~~~text
Cc50 <= 0.90 * Bc50
Cw50 <= 0.85 * Bw50
~~~

即 cold P50 至少改善 10%，warm P50 至少改善 15%。

### Tail guard

~~~text
Cc95 <= 1.05 * Bc95
Cw95 <= 1.05 * Bw95
~~~

任一 cohort P95 不得退化超过 5%。

### Tail improvement

至少一个 cohort 必须满足：

~~~text
candidate P95 <= 0.90 * corresponding baseline P95
~~~

即至少一个 cold/warm P95 真正改善 10%。

### Failure-rate guard

每个 cohort：

~~~text
candidate failureRate <= baseline failureRate
candidate failureRate <= 1/30
~~~

Baseline 若为 0 failure，candidate 也必须为 0。

### Readiness correctness

Final candidate 的 performance samples：

- known false Ready = 0；
- 不得在 Runtime/MCP/Tunnel authoritative conditions 之前计 Ready；
- 后续 E2E 必须证明 Ready 后真实 ChatGPT Tool call 可用。

### Material bottleneck requirement

还必须满足以下其一：

A. B0 中 P50 占 total P50 至少 10% 的最慢 Host-controllable phase，其 candidate P50 至少改善 25%；或  
B. cold 和 warm 两个 total P95 都至少改善 10%。

这防止优化一个几乎不影响用户等待的微小阶段后宣称完成 F05。

若外部网络波动导致达不到固定目标，不允许事后降阈值；应 FAIL/INCONCLUSIVE，并先显式修订 SPEC 后再做新候选。

---

## 9. Optimization Boundary

B0 锁定后允许：

- 消除单次 Start 内重复 config load/validation；
- 消除重复 Runtime/Tunnel distribution resolution；
- 单次 Start 内复用 immutable prepared values；
- 并行独立 read-only preparation；
- 去掉已被同一 authoritative evidence 覆盖的重复等待/查询，但不改变 freshness；
- 优化测得位于 critical path 的 Host/UI serialization。

禁止：

- 跨 Start 缓存 raw API key；
- 跳过 SecretStore presence；
- 跳过 Workspace/config validation；
- 跳过 Runtime initialize/version check；
- 跳过 MCP endpoint validation；
- 放宽 Tunnel stale/readiness；
- benchmark 代码直接设置 Ready；
- 看到结果后改变 cohort/threshold。

---

## 10. Startup DTO / API

继续使用 protocol v1 additive evolution。

建议共享类型：

~~~text
StartupCohort = cold | warm
StartupOutcome = ready | failed | cancelled | incomplete
StartupPhase = frozen phase IDs
StartupPhaseStatus = completed | failed | skipped | incomplete

StartupPhaseTiming {
  phase
  startedOffsetMs
  durationMs
  status
  safeErrorCode?
}

StartupAttempt {
  attemptId
  harnessSessionId
  cohort
  startedAt
  finishedAt?
  outcome
  totalDurationMs?
  phases[]
  failureCode?
  appVersion
  runtimeVersion?
  tunnelVersion?
}
~~~

Host 只需提供 latest startup attempt；持久化 startup history 属于后续 evidence/history scope。

---

## 11. Diagnostics Contract

### 11.1 Owner

DiagnosticsCoordinator 位于 Tauri Host，聚合：

- ConfigStore；
- Workspace/platform；
- SecretStore presence only；
- RuntimeSupervisor process/status/diagnostics；
- MCP endpoint；
- TunnelProcessManager status/readiness/recovery；
- HarnessController status。

它不重算 Runtime Policy、不重算 Tunnel health、不重新定义 Harness Ready。

### 11.2 Status

新增 UNKNOWN：

- PASS — 当前事实已正向验证；
- WARNING — 已验证为非理想/非 fatal 或 intentional stopped；
- FAIL — 已验证问题阻断当前目标；
- UNKNOWN — 当前条件下不能安全确认或 prerequisite 不存在。

UNKNOWN 永远不能当 PASS。

Overall precedence：

~~~text
FAIL > WARNING > UNKNOWN > PASS
~~~

### 11.3 Required check IDs

至少：

1. config
2. workspace
3. runtime_distribution
4. runtime_process
5. runtime_readiness
6. mcp_endpoint
7. tunnel_distribution
8. tunnel_process
9. tunnel_readiness
10. control_plane
11. mcp_target
12. proxy_config
13. proxy_route
14. credential_presence
15. credential_state
16. harness_lifecycle

现有 security check 可保留为额外 check。

### 11.4 Safe evidence

建议：

~~~text
DiagnosticScope =
  config | workspace | runtime | mcp | tunnel |
  control_plane | proxy | credential | lifecycle

DiagnosticEvidenceKey =
  configured | exists | version | process_phase |
  lifecycle | readiness | route_mode | proxy_source |
  last_success_age_seconds | error_code | endpoint_reachable

DiagnosticEvidence {
  key
  value
}
~~~

value 必须 bounded（建议 <=128 UTF-8 chars）且 sanitized。

禁止 evidence 包含：

- raw API key；
- SecretStore value；
- physical Workspace path；
- credential-bearing proxy URL；
- raw env；
- unrestricted stdout/stderr；
- raw control-plane response/error body；
- arbitrary JSON dump。

### 11.5 Remediation

建议：

~~~text
DiagnosticRemediation {
  code
  summary
  target?
}

target =
  overview |
  settings_workspace |
  settings_connection |
  settings_security
~~~

M02 remediation 只做说明/导航。

Run diagnostics 不得自动：

- save config；
- choose Workspace；
- replace key；
- start/stop/restart Harness；
- write/edit files。

---

## 12. Classification Rules

### Config / Workspace

- valid current v3 config → PASS；
- missing/malformed/unsupported config → FAIL；
- dependent checks → UNKNOWN；
- accessible Workspace → PASS；
- missing/inaccessible → FAIL；
- safe evidence 不输出 physical path。

### Runtime

runtime_distribution：

- packaged Runtime resolve + compatible → PASS；
- missing/incompatible → FAIL。

runtime_process：

- running → PASS；
- intentionally stopped → WARNING；
- crashed → FAIL；
- unstable transition cannot confirm → UNKNOWN。

runtime_readiness：

- authoritative ready → PASS；
- running but error/degraded → FAIL/WARNING according to typed state；
- stopped/unavailable → UNKNOWN。

### MCP

- current endpoint ready/reachable → PASS；
- Runtime running but endpoint unavailable → FAIL；
- Runtime stopped → UNKNOWN。

### Tunnel

distribution：

- resolved → PASS；
- missing/incompatible → FAIL。

process/readiness：

- Ready current instance → PASS；
- intentionally stopped → WARNING；
- failed/blocked → FAIL；
- no live instance/evidence → UNKNOWN。

### Control plane / MCP target

- current fresh connected/reachable → PASS；
- stale/disconnected/unreachable → FAIL；
- no live Tunnel evidence → UNKNOWN。

### Proxy

proxy_config：

- no proxy + direct allowed → PASS；
- valid configured proxy → PASS；
- malformed config → config FAIL。

proxy_route：

- observed route → PASS with route_mode/proxy_source；
- typed proxy failure → FAIL；
- no live evidence → UNKNOWN。

不得输出 proxy credential。

### Credential

credential_presence：

- SecretReference + stored secret exists → PASS；
- missing → FAIL；
- SecretStore unavailable → UNKNOWN/FAIL according to typed store state，但不输出 value。

credential_state：

- current control-plane connected proves accepted → PASS；
- typed Unauthorized/Forbidden → FAIL；
- otherwise UNKNOWN。

M02 不为了把 UNKNOWN 变 PASS 而额外发送 credential 到新 endpoint。

### Lifecycle

- Ready → PASS；
- Stopped → WARNING；
- Starting/Stopping → WARNING；
- Degraded/Error → FAIL with safe typed error code。

---

## 13. One-click UX

Overview 必须有明显入口：

~~~text
Run connection diagnostics
~~~

结果至少显示：

- overall；
- run time；
- check status；
- readable explanation；
- safe technical evidence；
- remediation；
- latest startup attempt（若有）；
- total duration；
- stage durations；
- slowest phase。

Settings 复用同一个 coordinator。

Copy diagnostic summary 只能复制 allowlisted safe fields；running diagnostics 本身不得自动写 clipboard。

---

## 14. Fault Fixture Matrix

自动化至少覆盖：

- all healthy / Ready；
- config missing/malformed；
- Workspace missing/inaccessible；
- Runtime distribution missing/incompatible；
- Runtime stopped/crashed；
- Runtime diagnostics/status unavailable；
- MCP endpoint unavailable；
- Tunnel distribution missing；
- Tunnel stopped/failed；
- control-plane disconnected/stale；
- MCP target unreachable；
- proxy direct；
- configured proxy failure；
- credential missing；
- SecretStore unavailable；
- typed Unauthorized/Forbidden；
- no live evidence → UNKNOWN；
- 修复外部条件后恢复 Ready。

固定输入只放既有 fixture/test 位置；运行产生数据使用 system temp 并清理。

---

## 15. Derived Acceptance Criteria

| M02 AC | Requirement |
| --- | --- |
| M02-AC01 | 每个 accepted Start 创建一个 Host-owned attempt，关联 Harness session，以 monotonic semantics 记录 total + frozen phase timing。 |
| M02-AC02 | Cold/warm、environment lock、30+30 sample、nearest-rank P50/P95 与 failure-rate denominator 严格按 SPEC。 |
| M02-AC03 | T1 instrumentation-only 的 B0 numeric baseline 在任何 T2 optimization 前采集并锁定。 |
| M02-AC04 | Final candidate 满足 §8 全部 frozen performance thresholds。 |
| M02-AC05 | Optimization 不削弱 config/Workspace/SecretStore/Runtime/MCP/Tunnel authoritative readiness。 |
| M02-AC06 | Startup detail 可显示 total/stages/outcome/slowest phase 且不泄露敏感 raw payload。 |
| M02-AC07 | One-click Diagnostics 通过一个 Tauri Host coordinator 覆盖 F06 必需 scope。 |
| M02-AC08 | DiagnosticCheck 支持 PASS/WARNING/FAIL/UNKNOWN、typed scope、bounded safe evidence 与 remediation。 |
| M02-AC09 | 各 check 按 §12 分类，prerequisite 不可用时 truthfully UNKNOWN。 |
| M02-AC10 | Running diagnostics 只读，不改 config、不启动/停止/重启进程、不碰 secret value、不写文件、不改 authorization。 |
| M02-AC11 | Diagnostic evidence/copy summary 不含 secret、raw env/output、physical Workspace path、credential-bearing proxy URL、unrestricted errors。 |
| M02-AC12 | Fault fixtures 覆盖 normal/missing/stale/failed/unauthorized/unreachable 并映射 deterministic status/remediation。 |
| M02-AC13 | Overview 有明显一键诊断入口；Settings 复用同一 coordinator；UI 支持 en/zh-CN 和 safe copy。 |
| M02-AC14 | M02 保持 M01 Tray/background/notification 以及现有 Start/Stop/Restart、Workspace/Policy/Approval/Bash/SecretStore 行为。 |
| M02-AC15 | Packaged candidate 报 Ready 后能完成真实 ChatGPT→Tunnel→Runtime Tool E2E；performance instrumentation 不能制造 false Ready。 |

---

## 16. Reserved Validation Traceability

| Upstream | M02 AC | Task | Integration | E2E | Manual |
| --- | --- | --- | --- | --- | --- |
| F05-AC01 | AC01, AC02, AC06 | T1, T5 | M02-INT-01/02/03 | M02-E2E-01 | M02-MAN-01 |
| F05-AC02 | AC03, AC04 | T1, T2, T6 | M02-INT-03/04 | M02-E2E-01 | M02-MAN-01/02 |
| F05-AC03 | AC05, AC15 | T1, T2, T6 | M02-INT-05/06 | M02-E2E-01/04 | M02-MAN-02/05 |
| F06-AC01 | AC07, AC09 | T3, T4 | M02-INT-07/08/09 | M02-E2E-02/03 | M02-MAN-03/04 |
| F06-AC02 | AC08, AC11, AC12 | T3, T4, T5 | M02-INT-10/11/12/13 | M02-E2E-02/03 | M02-MAN-03/04 |
| F06-AC03 | AC10 | T3, T4 | M02-INT-14 | M02-E2E-03/04 | M02-MAN-04/05 |
| Regression | AC13, AC14 | T5, T6 | M02-INT-15/16 | M02-E2E-04 | M02-MAN-05 |

---

## 17. Error / Cancellation Semantics

Startup:

- synchronous startup failure → failed；
- user Stop/Quit while Starting → cancelled unless prior failure already terminal；
- app/process exits before terminal → incomplete；
- stale old generation/session event ignored。

Diagnostics：

- independent checks continue after one failure；
- unavailable prerequisite → UNKNOWN；
- coordinator internal failure → bounded typed error/check，不是 empty PASS；
- navigation away/cancel does not mutate product state。

---

## 18. Security Invariants

M02 不得：

- expose SecretStore value；
- 把 API key 放入 timing/diagnostics DTO；
- copy raw env/stdout/stderr；
- copy physical Workspace path；
- copy proxy credentials；
- 创建 Approval/Permit/Grant；
- 修改 Policy/Tool availability；
- 通过 diagnostics 改 active config；
- 为性能放宽 readiness。

Telemetry failure 也不能把 Error/Unknown 变 Ready/Pass。

---

## 19. Exit Gate

M02 只有在以下条件满足时才能 ACCEPTED / FROZEN：

- repository milestone governance gate 已满足；
- M02-AC01～AC15 全部 PASS；
- B0 baseline 先于 optimization；
- final candidate 达到 §8 全部性能阈值；
- real ChatGPT Tool E2E 证明 Ready 真实可用；
- diagnostics normal + fault matrix PASS；
- default diagnostics read-only；
- safe-copy privacy PASS；
- M01/V3 affected regression PASS；
- packaged Integration/E2E/Manual PASS；
- no current blocker。

只增加 timer 或 diagnostics 页面，不算完成 M02。
