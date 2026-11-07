# M03 集成测试计划 — Shared Evidence 与持久化 SQLite History

> 状态：VALIDATION BASELINE FROZEN / NOT EXECUTED（验证基线已冻结 / 尚未执行）
> 里程碑：M03 — Shared Evidence and Durable SQLite History（共享证据与持久化 SQLite 历史记录）
> 输入：SPEC.md、PLAN.md、TASKS.md
> Owner：M03 Development Complete（开发完成）后由独立 QA / Coding Agent 执行
> 范围：P0 Shared Evidence Contract（共享证据契约）+ F07 SQLite History（SQLite 历史记录）
> 规则：Integration 是 Codex-autonomous local QA（Codex 自主本地 QA），必须验证真实 Runtime/Tauri/evidence-store/SQLite 组合；不得通过降低 identity、privacy、SQLite ownership、retention、queue 或 query-performance 契约取得 PASS。
> 外部边界：真实 External ChatGPT → Tunnel → Harness → Runtime 不属于本 Integration；由 Manual Acceptance 负责，缺少外部链路不阻塞本地 Integration。
> 存储：所有数据库、锁文件、迁移备份和 Workspace fixture 必须使用 disposable test locations（一次性测试位置）；不得使用或修改用户真实 History 数据库。

---

## 1. 目标

Integration 在打包 E2E 和人工验收之前，证明 M03 production implementation（生产实现）满足冻结的 Shared Evidence 与 History 契约。

本套件必须证明：

- canonical AuditEvent identity（权威审计事件身份）能进入 live Activity 和 durable History；
- runtime_id + request_id 是 Tool request 的唯一主要关联键；
- SafeEvidenceProjector 只持久化 typed / bounded / sanitized evidence；
- shared evidence-store library 不拥有任何 authorization authority；
- SQLite schema、migration、WAL 和 writer-lock 语义正确；
- Runtime 是正常运行时的唯一 writer，Runtime 活跃时 Tauri 保持 read-only；
- event + request summary + terminal Approval 更新具备 atomicity 和 idempotency；
- bounded queue / storage failure 降级的是 evidence，不是 Policy/Tool truth；
- crash/restart 产生 INCOMPLETE，而不是伪造 terminal result；
- filters、keyset pagination 和 history.query/history.get 性能达到冻结契约；
- retention 和 manual clear 按完整 request family 删除，且不恢复或改变 live authorization；
- corrupt / incompatible / busy / full / drop 状态可被观察；
- M01/M02/V3 行为不回归。

Integration 只验证本地可控边界，不声称 External ChatGPT、Tunnel 或人工用户体验已经通过。

---

## 2. Entry Criteria（进入条件）

正式执行 Integration 前必须满足：

- M03 T1–T6 Development Complete；
- candidate commit 已锁定，工作树状态已记录；
- M03 task-local tests 已有真实结果；
- schema v1/checksum 已固定；
- candidate 包含后续 E2E 将使用的同一 Shared Evidence/SQLite 实现；
- 正式 M03 QA runner 可运行，并能建立真实的本地 Runtime/Tauri/evidence-store/SQLite 边界；
- disposable database/profile、Workspace fixture、fault fixture 和清理边界可用；
- 不需要访问用户真实 History 数据库；
- 外部 ChatGPT、Tunnel、外部账号或网络授权不作为 Integration 进入条件。

基础 preflight（前置检查）：

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

这些命令的通过只能说明对应的格式、编译、单元/前端测试或构建状态；它们不能单独替代正式 M03 QA runner 的组合证据。

如果 candidate 无法把 destructive DB fixture 与用户数据隔离，corrupt/full/clear 等相关 case 必须 BLOCKED；不得改用真实用户数据继续。

---

## 3. QA Runner / Fixture Contract（QA runner 与 fixture 契约）

本节定义预期 runner 和 fixture，不填写执行结果；本文档保持 NOT EXECUTED。

### 3.1 正式 runner 要求

所有需要验证 real boundary（真实边界）的 Integration case，必须通过正式 M03 QA runner 执行。执行记录必须包含：

- candidate commit、版本、schema/checksum；
- runner 名称和 exact command（精确命令）；
- fixture identity、隔离根目录的 containment proof（不写入未脱敏物理路径）；
- expected、actual、verdict、原始但已脱敏的必要输出；
- 进程、SQLite connection、lock、backup、Workspace 和临时文件清理结果。

正式入口为：

~~~text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\ui\scripts\m03-integration.ps1 -Preflight
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\ui\scripts\m03-integration.ps1 -Case M03-INT-01
~~~

无 `-Case` 时 runner 按 `M03-INT-01`～`M03-INT-18` 顺序执行全部 case；任何 case 未匹配到恰好一个测试、命令失败或输出不是 PASS 时，整个运行必须失败。形式化 QA 测试默认标记为 ignored，避免被普通 `cargo test` 并行误执行；只能由该 runner 以 `--ignored` 精确调用。

静态/source-only evidence（源码或依赖图静态证据）不能满足 composition case（组合行为用例）。以下内容尤其不能单独 PASS：

- 只审阅 source、类型或依赖图，未执行真实 Runtime/Tauri/SQLite 边界；
- 只运行内部 executor、mock MCP、mock Tauri、Vitest/jsdom 或直接调用 store API；
- 只看到进程存在、端口开启、SQLite 文件存在或日志中有一行成功；
- 用本地可控调用声称 External ChatGPT → Tunnel 已验证。

INT-05 的 dependency graph 审计和 INT-18 的 repository/package layout 审计本身包含允许的静态部分；但同一 case 中要求 public API、Runtime、Tauri、SQLite 或回归行为的部分，仍必须由正式 runner 执行，不能以静态检查替代。

### 3.2 Real boundary matrix（真实边界矩阵）

| Case | 正式 runner 必须实际组合的边界 |
| --- | --- |
| INT-01～04、08～10、17 | 真实 Runtime/ExecutionPipeline、生产 MCP 或 Runtime API 边界、真实 evidence projection/store/health path；不得只测内部 executor。 |
| INT-05 | 依赖图/编译边界静态审计 + shared store public API fixture；不得构造 Policy/Approval authority。 |
| INT-06～07、16～17 | 真实 SQLite 文件、连接、WAL、锁、迁移、备份、maintenance 和容量/故障路径。 |
| INT-11～15 | 真实 Runtime/SQLite 与 Tauri History facade/maintenance path；涉及 UI/DTO 时使用真实序列化和宿主边界，不使用 mock UI 代替。 |
| INT-12 | 冻结规模 fixture、同一 candidate/environment、正式性能 runner 和 30 次样本。 |
| INT-18 | 实际回归命令/流程；repository layout 的静态检查只能覆盖 layout 部分。 |

### 3.3 Fixture 与隔离

- 每次执行创建唯一 disposable AppData/profile、Workspace、SQLite、lock、backup 和 evidence 根；
- fixture 只使用合成凭据、合成 sentinel 和安全 filesystem 文件；
- success、DENY、ASK-terminal、crash、queue overflow、busy/full、corrupt/incompatible、retention、clear 和 privacy fixture 必须可追溯；
- fault 注入必须作用于真实被测文件锁/SQLite/队列/容量路径；不能用 mock store 代替真实失败语义；
- runner 必须在成功、失败、超时和异常退出路径清理进程与锁；
- External ChatGPT/Tunnel 不属于 Integration fixture，相关证据留给 Manual Acceptance。

---

## 4. Evidence Header（证据头）

每次正式执行记录一次：

~~~text
Milestone: M03
Candidate commit:
Branch / dirty state:
App version:
Runtime version:
Local MCP boundary/version:
Evidence-store crate version:
SQLite library/version:
Schema version:
Schema checksum:
Windows version:
Package/runtime build identity:
Disposable AppData/profile fixture ID:
Workspace fixture ID:
SQLite/lock/backup fixture ID:
Formal M03 QA runner + exact command:
Config revision:
History desired config:
History effective config:
QA owner:
Execution time:
~~~

永久证据不得记录：

- API Key / SecretStore value；
- 生产 Workspace 的 physical path；
- raw command/environment/stdout/stderr；
- 用户真实 History 数据库内容；
- 未脱敏的 proxy credential 或完整文件正文。

每个 case 记录 exact command/runner、fixture identity、expected、actual、verdict 和 cleanup。允许的实际结论：

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

---

## 5. Required Integration Cases（必须执行的 18 个集成用例）

### M03-INT-01 — canonical AuditEvent identity 到达 live Activity 与 durable History

**AC：** M03-AC01
**Tasks：** T1/T3

通过 production ExecutionPipeline 产生真实 Runtime request。

对每个由 AuditEvent 派生的观察值验证：

- live ActivityEvent.eventId 等于源 AuditEvent.event_id；
- live ActivityEvent.runtimeId 等于当前 Runtime runtime_id；
- live ActivityEvent.requestId 等于 AuditEvent.request_id；
- durable evidence_events.event_id 是同一个 canonical event_id；
- durable request key 是 runtime_id + request_id；
- Activity sequence 不作为 durable request identity；
- 不需要 timestamp-proximity join。

至少运行 success、Policy DENY 和 ASK flow。

**PASS 条件：**一个真实 Runtime request 在各个 projection 中保持一个稳定的 request identity 和一个稳定的 canonical event identity。

---

### M03-INT-02 — Runtime identity、Host mapping 与 retry 边界

**AC：** M03-AC02
**Task：** T1

使用两个 Runtime process 和受控 Host launch context。

验证：

- 两个使用相同文本 request_id 的 Runtime 因 runtime_id 不同而保持区分；
- hostSessionId/supervisorGenerationId 只有显式提供时才存储；
- 缺少 Host mapping 时保持 null/unknown，不按时间推断；
- retry/new Tool request 获得新的 request_id，除非已有 execution path 明确 resume 同一 request；
- approval_id 只挂在产生它的 request 上；
- Runtime restart 不复用上一 runtime_id。

**PASS 条件：**Host/session/generation metadata 是辅助信息，不能替代 canonical request key。

---

### M03-INT-03 — HistoryConfig 校验与 generation-effective restart 语义

**AC：** M03-AC12
**Task：** T1

验证 AppConfig v3 defaults 仍为：

- enabled = true；
- retentionDays = 30；
- maxStorageMiB = 256。

Boundary tests：

- retentionDays 1 和 3650 接受；
- 0 和 3651 拒绝；
- maxStorageMiB 32 和 4096 接受；
- 31 和 4097 拒绝。

验证：

- 仅修改 HistoryConfig 会推进 Runtime-facing configRevision；
- running generation 报告 Restart Required；
- restart 前旧 generation 保留旧的 effective History policy；
- UI/Desktop-only preference change 保持 revision-neutral；
- DB path 不写入 AppConfig 或 RuntimeConfigSnapshot。

**PASS 条件：**M03 激活 History policy，但不改变 AppConfig schema version 或 desktop preference 语义。

---

### M03-INT-04 — Durable SafeEvidenceProjector privacy allowlist

**AC：** M03-AC03
**Tasks：** T1/T3

向受控 request/log/error fixture 注入 sentinel：

- synthetic API key；
- SecretStore value；
- physical Workspace path；
- 带凭据的 proxy URI；
- raw environment；
- raw stdout/stderr；
- large file contents；
- unrestricted error string；
- Bash command preview；
- arbitrary nested JSON metadata。

检查：

- EvidenceEventV1 DTO；
- 持久化 SQLite text/blob columns；
- History query/get DTO；
- UI-facing serialized result。

验证禁止 sentinel 均不存留。

同时验证允许的 typed evidence 仍存在，包括 logical Tool identity、decision、safe result scalar、bounded rule ID 和 coverage。

**PASS 条件：**持久化 History 是 closed typed projection，而不是 log dump。

---

### M03-INT-05 — evidence-store dependency 与 authority boundary

**AC：** M03-AC04
**Task：** T2

审计正式 runtime/evidence-store crate/module dependency graph 和 compile boundary。

验证它不 import 或 construct：

- PolicyEngine；
- ApprovalManager；
- SessionGrantStore；
- ExecutionPermit；
- Tool executor；
- Tauri UI state。

通过正式 runner 使用 fixture exercise public API，证明它只提供：

- DTO/schema；
- migration；
- query；
- retention；
- maintenance；
- store status/locking。

依赖图可以有静态审计证据，但 public API 行为不能只靠 source review。

**PASS 条件：**shared store 不能成为第二个 authorization engine。

---

### M03-INT-06 — SQLite schema v1、indexes 与 checksummed migration

**AC：** M03-AC05
**Task：** T2

在 disposable database 上验证：

- 缺失 DB 只能通过 writer/maintenance ownership 创建；
- schema_migrations、runtime_sessions、request_summary、evidence_events、approval_history 和 evidence_health 存在；
- frozen PK/FK semantics 存在；
- required query indexes 存在；
- request_spans 没有被静默引入 schema v1；
- schema version/checksum 与 candidate constants 一致；
- checksum mismatch 被拒绝；
- newer/incompatible schema 被保留并拒绝；
- migration 以 transaction 运行；
- upgrade 前创建 migration backup，且失败时保留；
- 执行 integrity verification；
- migration 失败不会自动删除/重建 source DB。

**PASS 条件：**database lifecycle 符合冻结的 v1 契约。

---

### M03-INT-07 — WAL、one-writer/read-only-reader/maintenance lock ownership

**AC：** M03-AC07、M03-AC15
**Tasks：** T2/T4

使用真实 file lock 和 SQLite connection：

1. 启动 Runtime writer 并获取 history.writer.lock；
2. 打开 Tauri/shared DAL read-only reader；
3. writer 持续提交时查询 committed rows；
4. 尝试第二个 writer；
5. Runtime writer 持有锁时尝试 offline maintenance；
6. 停止 Runtime writer；
7. 获取 maintenance lock 并执行允许的 maintenance。

验证：

- writer 启用 WAL；
- read-only reader 可以并发工作；
- Tauri connection 为 query-only；
- second writer 确定性失败；
- maintenance 不能夺取 writer ownership；
- 只有 Runtime 停止并释放 lock 后 maintenance 才成功；
- 不强制破坏 lock。

**PASS 条件：**A05/A07 ownership 由可执行行为强制，而非静态声明。

---

### M03-INT-08 — event + summary + terminal Approval 的 atomic transaction 与 idempotency

**AC：** M03-AC06
**Tasks：** T2/T3

对 success、DENY 和 terminal Approval event，验证一次 committed transaction 会同时更新全部 required durable projection。

在每个 logical mutation point 之后注入 failure。

验证：

- partial batch transaction 不提交该 event 所需的任何部分 rows；
- 重试相同 event_id 不插入 duplicate event；
- duplicate event 不会重复 increment/mutate request summary；
- duplicate terminal Approval 不创建 duplicate approval_history row；
- foreign-key integrity 保持有效。

**PASS 条件：**event persistence 是 atomic 且以 event_id idempotent。

---

### M03-INT-09 — bounded non-blocking writer queue 与 loss coverage

**AC：** M03-AC08
**Task：** T3

让 writer stall，并将 queue 填充超过冻结容量 1024。

验证：

- ExecutionPipeline producer 使用 non-blocking enqueue；
- Tool/Policy result 不等待 SQLite fsync；
- overflow 增加 dropCount；
- lastDropAt 更新；
- store status 进入 degraded；
- request identity 仍保留时，受影响 request 变为 INCOMPLETE；
- bounded loss mapping 自身饱和时，设置 global UNKNOWN coverage marker；
- overflow 不改变 ALLOW/DENY/ASK 或 execution outcome。

同时验证 batch max 64 和 batch-wait max 50 ms。

**PASS 条件：**evidence loss 可见且有界，但不会成为 authorization behavior。

---

### M03-INT-10 — graceful stop、bounded drain 与 unclean crash recovery

**AC：** M03-AC09
**Task：** T3

覆盖：

- 没有 pending durable work 的 normal stop；
- 有 queued evidence 的 normal stop；
- drain 在 2 秒内完成；
- drain 超过 2 秒 budget；
- 有 open request 时 hard Runtime termination；
- 下一 Runtime startup/recovery。

验证：

- normal stop 关闭 runtime_sessions；
- remaining open request 变为 INCOMPLETE；
- drain timeout 是 evidence degradation，不是 Tool failure；
- stale prior session 收到 unclean_restart 或等价 typed end reason；
- 不插入 synthetic terminal success/failure event；
- 新 Runtime 使用不同 runtime_id。

**PASS 条件：**crash gap 保持为 gap，不被伪造结果填补。

---

### M03-INT-11 — History filters、keyset pagination 与 cursor integrity

**AC：** M03-AC10
**Tasks：** T2/T4/T5

构造包含多个 Runtime、Tool、outcome、decision、Approval state 和 timestamp 的 mixed fixture。

分别及组合验证 filters：

- fromUtc/toUtc；
- tool；
- outcome；
- policyDecision；
- approvalState。

Pagination assertions：

- default 50；
- max 200；
- deterministic newest-first request ordering；
- 稳定数据下相邻页面没有 duplicate/missing row；
- opaque cursor 包含并校验 filter fingerprint；
- changed filters 会用 typed INVALID_CURSOR 使旧 cursor 失效；
- 正常 History browsing 不使用 OFFSET；
- React 不接收整个 DB 做 client-side pagination。

**PASS 条件：**History browsing 有界且稳定。

---

### M03-INT-12 — 冻结的 history.query/history.get performance SLA

**AC：** M03-AC10
**Task：** T2/T6

使用冻结的 representative fixture：

- 50,000 request_summary rows；
- 250,000 evidence_events rows；
- page size 50；
- steady-state DB 在配置的 256 MiB target 内。

在同一 candidate/environment 上，对 representative indexed history.query 和 history.get：

- 先执行 2 次不计数 warm-up call；
- 再测量 30 次 call。

必须满足：

- history.query P95 <= 500 ms；
- history.get P95 <= 250 ms。

验证不能通过删除 required filters、detail events 或 coverage fields 来伪装达到 SLA。

**PASS 条件：**两个冻结的 query target 都达到。

---

### M03-INT-13 — History detail 与 offline read path

**AC：** M03-AC11、M03-AC15
**Tasks：** T2/T4/T5

持久化 representative completed/incomplete request，停止 Runtime，然后通过 Tauri History facade 查询。

验证：

- 仅为读取兼容 DB，不需要启动 app/runtime；
- list result 包含 safe request-level summary；
- history.get 返回有序的 safe event timeline；
- terminal approval_history 正确关联；
- request/store coverage notice 存在；
- 缺失 DB 返回 empty/uninitialized state，且不创建 DB；
- read-only browsing 不运行 migration/delete/checkpoint/vacuum。

**PASS 条件：**Runtime 停止时兼容 History 仍然有用。

---

### M03-INT-14 — Historical Approval 不可操作且不能恢复 live authority

**AC：** M03-AC11、M03-AC14
**Tasks：** T1/T4/T5

持久化：

- ApprovalRequired observation；
- AllowOnce terminal Approval；
- AllowForSession terminal Approval；
- Denied；
- Expired；
- Cancelled；
- ApprovalRequired 后 crash 且没有 terminal event。

重启 Runtime/App。

验证：

- History 只暴露 observational state；
- 没有 durable actionable Pending object；
- history.get 永不返回 live ApprovalRequest；
- 没有 History API 可以调用 approvals.resolve；
- 不重建 SessionGrant；
- 不重建 ExecutionPermit；
- old approval/request ID 不能授权新的 Tool request；
- 仅 crash 的 request 是 INCOMPLETE，不是 Pending。

**PASS 条件：**F07-AC02 / SEC-AC03 得到强制。

---

### M03-INT-15 — Online/offline Clear History ownership 与 barrier 语义

**AC：** M03-AC14
**Tasks：** T3/T4

#### Runtime running

排入旧 terminal history，同时保持一个 request open，然后发出 Clear History。

验证：

- Tauri 将 online clear 路由到 Runtime；
- Runtime 在已排队 work 之后插入 writer barrier；
- barrier 之前的 terminal/incomplete request family 被删除；
- current active runtime session/open request 保留；
- barrier 之后的新 evidence 可以重新出现；
- 不取消/resolve request；
- 不改变 Policy/Grant state。

#### Runtime stopped

获取 maintenance lock，清除所有 evidence/session rows，同时保留 schema_migrations。

验证返回的 counts/footprint。

**PASS 条件：**clear 只是 storage maintenance。

---

### M03-INT-16 — Age retention、capacity retention 与 whole-request deletion

**AC：** M03-AC13
**Task：** T3

使用小型 deterministic test limits，同时保留 production algorithm。

验证：

- age cleanup 删除超过 retention window 且 eligible 的 closed/incomplete request family；
- current open request 被排除；
- FK cascade 删除关联 evidence/approval rows，且无 orphan；
- age cleanup 在 capacity cleanup 之前运行；
- capacity cleanup 删除最旧的 eligible whole request family；
- cleanup 目标为配置 maxStorageMiB 的 90%；
- 按 production footprint rule 计算 main DB + WAL；
- 无法达到 cap 时产生 EVIDENCE_CAPACITY_DEGRADED 和可见 loss/coverage state；
- cleanup 不触碰 Workspace 文件。

另外验证 production defaults 仍为 30d/256MiB。

**PASS 条件：**retention 有界、按 request family 原子删除且可观察。

---

### M03-INT-17 — busy/full/corrupt/incompatible/migration/drop health projection

**AC：** M03-AC15、M03-AC16
**Tasks：** T3/T4/T5

注入受控 store failure：

- read busy；
- writer lock conflict；
- SQLite busy；
- disk/full 或 deterministic IO failure；
- corrupt DB；
- incompatible newer schema；
- migration failure/checksum mismatch；
- queue overflow；
- capacity degradation。

验证 typed status/error mapping：

- disabled；
- healthy；
- degraded；
- busy；
- incompatible；
- corrupt；
- unavailable。

UI/DTO 不得把 corrupt/incompatible database 称为 empty history。

验证每一种 evidence fault 下 Tool/Policy outcome 保持不变。

**PASS 条件：**storage truth 与 execution truth 保持分离。

---

### M03-INT-18 — M01/M02/V3 regression 与 repository/package boundary

**AC：** M03-AC18
**Task：** T6

运行受影响的真实回归：

- Tray/background Close/Show/Start/Stop/Quit；
- Approval/disconnect notification；
- StartupPhaseRecorder/startup lifecycle；
- one-click diagnostics；
- AppConfig v3 migration/current fields；
- Workspace boundary；
- Tool enable/disable；
- Policy ALLOW/ASK/DENY；
- Approval/SessionGrant；
- controlled Bash；
- SecretStore；
- Harness Start/Stop/Restart。

同时验证：

- 没有新增 repository top-level directory；
- evidence-store 位于冻结的现有 module tree；
- Control Protocol 保持 v1 additive；
- M02 frozen performance thresholds/evidence 没有被 M03 修改。

layout 可以有静态证据，但实际功能回归必须用正式 runner/真实流程，不能只审 source 或 process existence。

**PASS 条件：**M03 增加 durable evidence，同时不回归既有 product authority 或 repository layout。

---

## 6. Integration Exit Gate（退出门槛）

M03 Integration 只有在以下条件全部满足时才能记 PASS：

- M03-INT-01～M03-INT-18 全部 PASS；
- 所有 DB/fault fixture 使用 disposable path 和隔离 profile；
- 需要真实 Runtime/Tauri/SQLite/MCP 边界的 case 都有正式 M03 QA runner 证据；
- 没有用 weaker mock、internal executor、source-only review、process exists 或日志片段替代 required composition；
- query SLA 使用最终 candidate，且 2 warm-up + 30 calls、fixture size 和 500/250 ms 阈值均保持；
- privacy sentinel 不出现在持久化或 UI；
- 临时 DB、backup、lock、Workspace fixture、SQLite connection 和进程已清理；
- 没有后续 production change 使 candidate evidence 失效。

External ChatGPT → Tunnel → Harness → Runtime 不属于 Integration Exit Gate；它由 Manual Acceptance 最终验证。Integration PASS 不等于 M03 ACCEPTED / FROZEN。

本文档当前只定义验证契约，未执行任何 Integration case，不填写 PASS、FAIL、BLOCKED 或 INCONCLUSIVE 的执行结果。
