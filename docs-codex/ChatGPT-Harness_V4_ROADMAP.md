# ChatGPT-Harness V4 Roadmap

> **Status:** V4 UPSTREAM PLANNING BASELINE / FROZEN FOR M00 (2026-10-23)  
> **Product:** [V4 PRD](./ChatGPT-Harness_V4_PRD.md)  
> **Design:** [V4 Architecture](./ChatGPT-Harness_V4_ARCHITECTURE.md)  
> **Reference:** [V3 historical Roadmap](../docs/archive/v3-2026-10/ChatGPT-Harness_V3_ROADMAP.md)  
> **Scope:** 里程碑、交付顺序、三层验收与版本 Gate；不代表已开发或已通过验收。  
> **Freeze semantics:** 本 Roadmap 作为 M00 的上游交付基线冻结；M00 验证若要求改变依赖、Milestone scope、release gate 或 canonical AC owner，必须先显式修订并重新冻结 Roadmap，而不是在下游 TASK 中临时改路线。

---

# 1. Release Structure and Rules

V4 依照 **架构契约 → 具体验收能力 → Core Gate → Full Gate** 逐步推进。一个 Milestone 必须有独立可观察的产品或工程交付，不能仅以代码合并、图表完成或单测通过宣称 ACCEPTED。

| Release | Mandatory Milestones | Gate |
| --- | --- | --- |
| **V4 Core (P0)** | M00–M06；M06 是综合发布 Gate | M00–M05 能力独立验收 + M06 真实完整链路，才能称 V4 Core — ACCEPTED |
| **V4 Full (P0+P1)** | M07–M11，且先具备通过的 M06 | M07–M10 独立通过 + M11 总验收，才能称 V4 — ACCEPTED / FROZEN |
| **Optional (P2)** | O01 F09、O02 F15 | DEFERRED 不阻断 Core/Full；如交付必须独立 PASS |

**功能唯一交付归属：** M01 F01/F03/F04；M02 F05/F06；M03 Shared Evidence Contract/F07；M04 F12；M05 F10；M07 F02；M08 F08/F14；M09 F11；M10 F13；O01 F09；O02 F15。M00 承担架构/契约先决门禁；M06/M11 只负责集成发布 Gate，不重复声称功能开发。

原则：**M04 受控 Evaluation baseline 必须先于 M05 新增 grep 的质量回归；Shared Evidence Contract 必须先于 F07/F08/F14 的实现及 Eval 对接。** F05 性能目标、F12 质量准入阈值必须在各自 Milestone 的 SPEC 编写阶段完成，并在进入 `SPEC_READY`、创建任何实现型 TASK 或开始候选实现之前冻结；不得在候选实现或验收后倒填。V4 Roadmap、Architecture 和 PRD 均是规划文件，不能当作代码当前事实。

# 2. Mandatory SPEC → PLAN → TASKS Delivery Protocol

每个 M00–M11 及任何实际启动的 Oxx 必须建立专属目录：

~~~text
docs-codex/v4/milestones/Mxx-short-name/
├── SPEC.md
├── PLAN.md
├── TASKS.md
├── INTEGRATION_TEST_PLAN.md
├── E2E_TEST_PLAN.md
└── MANUAL_ACCEPTANCE_CHECKLIST.md
~~~

文件布局修订（2026-10-24，用户授权）：每个里程碑目录仅允许上述六份文件，不含子目录。报告、ADR、ACCEPTANCE.md、EVIDENCE_INDEX.md 及需保留的脱敏验收日志/截图统一放在 `docs-codex/v4/evidence/Mxx-short-name/`；测试数据放在 `fixtures/v4/<milestone>/`；候选便携包及随包清单、散列和包校验记录放在 `artifacts/V4-Mxx-short-name/<candidate>/`，该目录不存放其他材料；编译缓存和临时验证材料统一放 `target/`。本修订不改变原有功能范围及验收门槛。

此 Roadmap 不预先生成空模板；**启动对应里程碑才创建文件**。流程：

~~~text
Frozen upstream planning baseline: PRD + Architecture + Roadmap
     → SPEC (AC编号/正反例/接口/安全/可量化阈值)
     → PLAN (owner/实现顺序/依赖/迁移/回退)
     → TASKS (小任务、影响模块、完成定义、受影响单测)
     → 三份测试/验收计划事先冻结
     → Development + Unit/Contract
     → Integration (真实组件边界)
     → E2E (真实端到端入口与结果)
     → Manual Acceptance (实际用户操作)
     → ACCEPTANCE + EVIDENCE_INDEX
     → ACCEPTED / FROZEN，或明确 BLOCKED/FAIL/NOT_VERIFIED
~~~

**SPEC** 必须以 PRD §4.1 的 Canonical AC-ID 为上游追踪键，并建立 Canonical AC-ID → Derived AC（如有）→ 实现 Task → Integration/E2E/Manual Test-ID 的双向追踪；说明 Scope/Non-goals、DTO/Protocol、错误/取消/超时/数据库失败、安全不变量、回归与阈值冻结。**PLAN** 指明唯一模块 owner、可回退分段、迁移与资源清理。**TASKS** 每条应足够小，能指定影响范围、验证命令和证据，禁止两个 Agent 并发修改同一模块及覆盖他人未提交文件。

测试计划是 **PLANNED** 而非 PASS；实际运行后填真实时间、OS/App/Runtime/Tool/Config 版本、commit、Workspace fixture、执行命令、实际结果及脱敏日志/截图。**Unit → Integration → E2E → Manual** 四层逐级区分，不能用 mock、process exists、MCP Inspector、自动 UI 取代真实 ChatGPT 与人工验收。外部 ChatGPT/付费模型、重启受管实例、Windows 安装及影响持久状态的 E2E 在执行前需要用户明确授权；无授权/环境不可用标 BLOCKED，不擅自运行。

每个 Milestone 的 ACCEPTED/FROZEN 必须有：局部 Unit/Contract PASS、规定的 Integration PASS、实际 E2E PASS、Manual Checklist PASS、受影响 V3 安全回归 PASS、失败/恢复/回滚证据、无当前范围 blocker。M00/M09 等架构/准入型 Gate 仍需真实基线链路 E2E 和对实际准入结果的人工验证。

# 3. Dependency Graph

~~~mermaid
flowchart TD
  M00["M00 Baseline & Contracts"] --> M01["M01 Tray/Notifications"]
  M00 --> M02["M02 Startup/Diagnostics"]
  M00 --> M03["M03 Evidence/SQLite"]
  M03 --> M04["M04 Agent Eval Baseline"]
  M04 --> M05["M05 grep & Regression"]
  M01 --> M06{"M06 Core Gate"}
  M02 --> M06
  M03 --> M06
  M04 --> M06
  M05 --> M06
  M01 --> M07["M07 Auto Start"]
  M02 --> M07
  M03 --> M08["M08 Metrics/Trace"]
  M05 --> M09["M09 File Tools Admission"]
  M05 --> M10["M10 MCP Conformance"]
  M06 --> M11{"M11 Full Gate"}
  M07 --> M11
  M08 --> M11
  M09 --> M11
  M10 --> M11
  M08 -.optional.-> O01["O01 Token Estimates"]
  M03 -.optional.-> O02["O02 Policy Simulator"]
~~~

M01/M02/M03 可在 M00 验收后由不同模块 owner 并行；M04 必须在 M03 后，M05 必须在 M04 baseline 后。M07–M10 可按各自依赖并行，但 M11 必须以 M06 Core Gate 为前提。Oxx 不得反向阻塞 Mandatory Gate。

---

# M00 — As-Is Baseline and Architecture Contract Gate

**Scope:** Core prerequisite；确认当前源码真实状态、ADR、Shared Evidence Contract v1、SQLite Writer/Reader 原型和受控 Eval Runner 可用性。  
**Depends on:** 当前 main 代码/Tests/Manifests 与 V4 PRD/Architecture 草案。  
**Deliverable:** AS_IS_BASELINE.md、ADR_DECISIONS.md、可运行的 evidence/SQLite ownership POC、Data Contract fixtures、带来源的设计冻结结论；不是只写架构评论。

- **SPEC/PLAN/TASKS：** 核验现有 Runtime/Tauri/Tunnel/Approval/Audit ownership；制定 request/event/session IDs、Outcome/UNKNOWN、脱敏 allowlist；证明 Runtime 单写+Tauri 停机离线读+独占维护及 crash 恢复的可行性；演练 Config v2→v3 兼容 fixture；明确能真实运行 F12 的 controllable Agent/Model Adapter、ground truth 及合规测试成本。POC 只做必要隔离，禁止大规模重构。
- **Integration：** 真实 Runtime AuditSink → typed safe evidence projection；同请求事件关联/脱敏、请求间隔离；SQLite 双连接 WAL/只读/锁/迁移故障 POC；Config 迁移不丢 SecretReference 或现有 Policy。
- **E2E：** 在当前基线用真实 ChatGPT→Tunnel→Runtime MCP 调用一个受控 read/glob Tool，观察 Workspace 结果/Activity 及 Ready 状态；证明现有链路的事实，而非声称新增 V4 History 已完成。
- **Manual：** 核实当前 UI/Tray 是否存在及权威 Ready，审阅 ADR、POC、依赖与“已实现/未验证/历史文档仅参考”事实清单；签认下一 Milestone 的契约与安全边界。
- **Exit Gate：** Architecture 契约可用于 SPEC、SQLite 原型/数据契约集成 PASS、真实基线 E2E PASS、受控 Eval 能实际执行；外部 ChatGPT 或 Eval 能力不可得则 BLOCKED，不能用调研猜测替代。

# M01 — Tray, Background Lifecycle and Notifications

**Scope:** P0 F01/F03/F04。**Depends on:** M00。  
**Deliverable:** Windows App 隐藏窗口保持 Harness 执行、Tray Show/Start/Stop/Quit、审批与断连通知、真实 deep link 与失败 fallback。

- **SPEC/PLAN/TASKS：** 明确 CloseRequested→Hide vs explicit Quit、后台首次告知、Tray authoritative state/pending count、Windows Toast 可用性/点击不可靠 fallback、ASK ID 去重/过期导航、断连 episode 去重；Tauri Host 作为唯一 desktop owner，不允许 React 重算授权。
- **Integration：** 真实 Tauri-HarnessController 状态变化/operation mutex；隐藏后 Runtime/Tunnel 仍活；ASK→Toast 仅一次、过期/已审批映射；断连→恢复→新事件；OS 通知禁用、Quit 进程回收和并发操作。
- **E2E：** 打包 Windows App，隐藏窗口期间经**真实 ChatGPT→Tunnel→Runtime MCP** 调用 Tool；发起 ASK 验证 Windows Toast→具体审批，故障通知真实出现且不刷屏，显式 Quit 后无 owned orphan。
- **Manual：** 关闭主窗口、托盘重新打开/启动/停止/退出；点击通知、关闭 Windows 通知、审批已过期；核对隐私和权威状态。
- **Exit Gate：** F01/F03/F04 全部真实可用；不可投递 Toast 时 Tray/Approval 仍可处理；隐藏窗口不是“后台进程停止”的替代。

# M02 — Startup Performance and One-click Diagnostics

**Scope:** P0 F05/F06。**Depends on:** M00。  
**Deliverable:** 真正优化且可量测的 Start→Ready；只读一键诊断给出明确故障阶段、原因与修复入口。

- **SPEC/PLAN/TASKS：** 在开发前冻结 cold/warm 硬件/网络/版本、P50/P95、失败率、最小样本、优化目标和计时区间；确定 Runtime/MCP/Tunnel/Proxy/Credential/config 的正常/缺失/不通/stale fixture；Host monotonic recorder 不跨进程直接减 Instant。
- **Integration：** 真 Runtime/Tunnel 启动分段、recovery、stale readiness、缺 credentials/错误代理/缺 Runtime/MCP target 不可达的注入；typed PASS/WARNING/FAIL/UNKNOWN 和 remediation；诊断不得改写配置。
- **E2E：** 真实桌面 Start→Tunnel→ChatGPT Tool 可用时才能记 Ready；同类环境重复测量并达提前冻结阈值；故障→诊断→恢复→再次真实 Tool 可用。
- **Manual：** 检查耗时明细与报告；故障定位是否准确、错误是否可理解、诊断是否无副作用；过程不可凭进程存在伪报 Ready。
- **Exit Gate：** F05 已达预冻结性能改善目标且 Ready 正确，F06 故障覆盖 PASS；仅新增计时/诊断页面不能宣称优化成功。

# M03 — Shared Evidence and Durable SQLite History

**Scope:** P0 Shared Evidence Contract + F07。**Depends on:** M00；M04/M08 硬前置。  
**Deliverable:** Runtime 单写、Tauri 离线只读的 SQLite History，支持 Activity/Approval 安全历史、查询/筛选/分页/清理、retention、数据库失败可诊断及数据覆盖率说明。

- **SPEC/PLAN/TASKS：** 先冻结 event_id、runtime_id/request_id、host-session 映射、审批和重试边界、terminal/incomplete 结果分类及字段 allowlist；再设计 EvidenceProjector、SQLite DDL/index/version migration、WAL、writer queue、Tauri read-only facade、离线 maintenance 锁、保留及清理。30 天/256MiB 是 Architecture 建议默认值，必须在 SPEC 中确认后使用；不从历史恢复 Pending/Grant/Permit。
- **Integration：** 真实 Runtime Tool/ASK/DENY/CANCEL 审计流写库、跨请求关联、event_id 幂等、WAL writer+reader 并发；DB 占用/满盘/损坏/迁移失败/queue overflow；停机 Tauri 读、独占清理无孤儿、secret/raw payload 不入库。
- **E2E：** 真实 ChatGPT→Tunnel→Runtime MCP 完成成功/拒绝/审批请求；退出/重启 Runtime 后在真实桌面 UI 按 Tool/Policy/Outcome 查询、分页、清理；旧 Pending 无法再授权，缺失/不完整证据明确展示。
- **Manual：** 打开 History，核对每条记录与实际调用、审批、错误状态一致；停机时仍可查，查看保留期限和手工清理提示；验证敏感字段不泄漏。
- **Exit Gate：** Shared Evidence Contract 在真实链路上可复核，F07 持久化/查询/清理/降级 PASS，历史与实时授权严格隔离；SQLite 文件存在不等于验收。

# M04 — Offline Controlled Agent Evaluation Baseline

**Scope:** P0 F12。**Depends on:** M03（同一 Safe Evidence 契约）；**M05 必须等待 M04 完成**。  
**Deliverable:** 真实可运行的独立 Agent Eval CLI/Runner、固定数据集/ground truth、可控模型 adapter、Workspace fixture、baseline 报告与事先冻结的回归阈值。

- **SPEC/PLAN/TASKS：** 冻结多种任务的 Tool selection 允许集、合法参数、真实 Workspace 最终状态断言、Policy/Approval 期望、模型/adapter/Tool Schema/Policy/fixture hash、随机性与重复次数、样本量、比较阈值/退化范围。受控 Eval 和被动生产 telemetry 两类指标禁止混淆。
- **Integration：** 使用隔离 Runtime + 真实 MCP adapter + fixture，测试 tool selection、schema validity、执行 outcome、task final-state、失败/超时/重试；版本化 scorer 用预设正反例与故意引入的质量回退校验；受控写入后可清理。
- **E2E：** Runner 驱动**实际可控 Agent/Model** 通过 MCP 工具完成多步任务，以最终文件/状态断言计算 task completion，生成 baseline/candidate 对比；不得用只有工具返回 200 的日志冒充 Agent task success。
- **Manual：** 审阅 dataset 样本、ground truth、模型/版本、实际复跑步骤、结果解释、失败样本、成本与边界，确认无法观测模型内部时没有伪造选择准确率。
- **Exit Gate：** F12 baseline 可以在隔离环境复现且样本/阈值满足 SPEC；不可控模型、缺失数据集或样本不足即 BLOCKED/INCONCLUSIVE，不解锁 M05 正式准入验收。

# M05 — grep Tool and Quality Regression

**Scope:** P0 F10，必须执行 F12 regression。**Depends on:** M04/M03。  
**Deliverable:** Workspace 内有界全文检索 grep，可发现的 MCP Tool schema，统一 Typed Evidence，真实 Agent 质量回归报告。

- **SPEC/PLAN/TASKS：** 搜索 literal/regex、相对 scope、include/exclude、行号、二进制与编码、regex 时间/大小、结果/扫描上限及截断语义；将 grep 一起接入 ToolRegistry/ToolConfig/ToolPolicyDefaults、MCP Adapter、CapabilityGate、WorkspaceGuard、ExecutionPipeline 和 Audit/History。不能隐式调用 Bash 完成 grep。
- **Integration：** 真实 Runtime MCP 端点运行多文件/嵌套/超大文件/无效正则/截断测试；Windows symlink/junction/reparse 与 TOCTOU 越界拦截；Tool disabled/Policy DENY/ASK 后不会绕过 Pipeline；相邻老工具回归。
- **E2E：** 真实 ChatGPT→Tunnel→Runtime 调用 grep 并核对文件行号与受限输出；受控 Eval Runner 在同数据集上对比 M04 无 grep baseline 与含 grep candidate，证明不发生阻断级任务质量与安全退化。
- **Manual：** 在真实 ChatGPT 搜索多文件、零命中、异常正则、结果截断、Workspace 越界；核对 UI Activity 和 Eval 差异可理解。
- **Exit Gate：** F10 功能、安全、真实 ChatGPT 工具调用与 F12 regression 均通过，未触发越界或敏感内容泄漏；仅注册工具或本地 shell grep 不通过。

# M06 — V4 Core Release Gate

**Scope:** **Core 综合验收里程碑**，不新交付 P1/P2。**Depends on:** M01/M02/M03/M04/M05 均 ACCEPTED。  
**Deliverable:** 固定 commit、版本和配置的可用 Core Release Candidate、Core Product Acceptance Matrix、集成/E2E/人工签认及 Release Notes。

- **SPEC/PLAN/TASKS：** 为同一个 candidate 冻结全链路场景、二进制/hash、PRD P0 需求 AC、问题优先级、升级与回滚；任务以跨模块验证与缺陷收口为主，不再把未完成功能改名为低优先级。
- **Integration：** 真实 Runtime MCP / Tauri / Tunnel / Policy/Approval / SQLite 跨组件执行成功、拒绝、取消、崩溃/恢复；Config v2→v3 兼容；V3 Workspace、Tool Toggles、SessionGrant、Bash、SecretStore、lifecycle 受影响回归。
- **E2E：** **同一真实 ChatGPT→Tunnel→Runtime MCP→read/grep/ASK/DENY→Execution→SQLite History**，窗口隐藏时工具仍工作、Windows Toast 跳转、诊断恢复可复现；Eval baseline/regression 与 candidate identity 一致。
- **Manual：** 真实 Windows 安装/启动→隐藏主窗口→ChatGPT Tool→审批→断连诊断→历史查询→退出；逐项核对安全边界、无 orphan、旧批准不可恢复。
- **Exit Gate：** PRD 全部 P0（F01/F03/F04/F05/F06/F07/F10/F12 + Shared Evidence）及外部 E2E、人工验收 PASS；仅可标 **V4 Core — ACCEPTED**。任何 M00–M05 未通过、外部测试未运行均 BLOCKED，不能宣称 Full。

# M07 — Auto Start on App Launch

**Scope:** P1 F02。**Depends on:** M00/M01/M02。  
**Deliverable:** 新安装默认关闭、用户可开关的 App 启动自动 Start Harness；不是 Windows 登录自启动，不绕过安全校验。

- **SPEC/PLAN/TASKS：** 冻结 Host-only config、默认/升级值、UI 开关、StartupCoordinator 生命周期、初始化只调用一次、UI/Tray/Auto Start 并发冲突、失败和 SecretStore/eligibility 错误；明确无 Runtime restart debt。
- **Integration：** v2→v3 迁移保留已有偏好/credentials；启用/关闭 App reopen、双击启动、并发手动 Start、缺凭据与异常停止；旧 Config revision 语义正确。
- **E2E：** 完全退出并重新打开真实 Windows App，enabled 时一次 Start 且真实 ChatGPT MCP 调用成功；disabled 时不自动启动；缺失 secret 时可诊断而不误报 Ready。
- **Manual：** 设置开关→Quit→再次打开；检查只启动一次、可手动停止、关闭后不自启、权限提示清晰。
- **Exit Gate：** F02 PASS，默认关闭、用户配置持久、无重复启动、不绕过 eligibility 与 Runtime authority。

# M08 — Tool Metrics and MCP Tool Request Trace

**Scope:** P1 F08/F14。**Depends on:** M03；复用同一个 Shared Evidence Contract。  
**Deliverable:** 来源真实、可复核的 Tool calls、execution success denominator、avg/P95、审批等待以及 Request-local Trace/Span UI。

- **SPEC/PLAN/TASKS：** 冻结 received/attempted/terminal/success、DENY/cancel/timeout 的分母与分类；execution vs approval wait vs wall time；nearest-rank P95、最小样本、查询窗口与性能预算；只观察 Harness MCP Tool Request 阶段，缺失/崩溃标 INCOMPLETE。
- **Integration：** 真实 AuditEvent 混合成功、拒绝、ASK、超时、取消、crash 样本计算；同一 request_id 的 spans 与 History 交叉查询；无事件/库清理/故障后的统计 coverage；核算小样本 P95。
- **E2E：** 真实 ChatGPT 调用 read、ASK、DENY 与失败 Tool，History、Metrics 与 MCP Tool Request Trace 显示同一事实；确认审批等待不算 executor duration，无法观测的模型阶段不存在。grep 的跨功能统计/Trace 回归在 M11 使用 Full candidate 统一覆盖，不作为 M08 自身的前置依赖。
- **Manual：** 人工用样本复核 totals/success denominator/P95、Trace 时间线与错误节点，确认没有“模型选工具全过程”的虚假展示。
- **Exit Gate：** F08/F14 PASS，指标与 Trace 均可追溯到同一 Safe Evidence，样本不足/数据缺失时明确提示，不将图表存在作为成功。

# M09 — File Tool Admission and Qualified Expansion

**Scope:** P1 F11。**Depends on:** M04+M05，保留 Eval regression Gate。  
**Deliverable:** 对 mkdir/list/copy/move/rename/delete 每项给出 ADMIT/REJECT/DEFER 决策和实证；实际 ADMIT 的 Tool 交付可运行、可验收能力。

- **SPEC/PLAN/TASKS：** 对每个候选写任务价值、旧工具是否重复、Tool schema/参数与相对路径、危险与不可逆性、双路径 Guard、覆盖/跨卷/文件冲突、权限等级/ASK、测试边界；ADMIT 项新增其 own AC/Task，未准入项禁止混入 ToolRegistry。
- **Integration：** 所有候选 Admission Matrix 与 capability/evaluation 数据可审计；已实现的 ADMIT tool 真实 Runtime MCP、Workspace/Policy/Approval、安全错误和 Regression PASS；未准入项不暴露。
- **E2E：** 真实 ChatGPT 运行现有 read/grep/write/edit 回归；**每个实际交付的 ADMIT Tool** 再完成 ChatGPT→Tunnel→Runtime 调用、正常/拒绝/越界/审批与真实文件状态断言。
- **Manual：** 逐项审阅为何 ADMIT/REJECT/DEFER，验证实际 Tool list 与发布清单一致；对已交付工具操作成功、失败、权限和恢复路径。
- **Exit Gate：** F11 具真实 Admission 结果，所有 ADMIT/交付项各层验收 PASS。**不要求全部候选实现**；如全部 REJECT/DEFER，须有明确产品批准与安全/效能理由，不得称“扩展工具已全部实现”。

# M10 — MCP Protocol Compatibility and Conformance

**Scope:** P1 F13。**Depends on:** M05/M00；M05 后 catalog 含 grep。  
**Deliverable:** 锁定 MCP protocol revision、rmcp SDK、客户端/runner 版本的兼容报告，官方 conformance 与真实 ChatGPT E2E 分开提供。

- **SPEC/PLAN/TASKS：** 锁定 initialize/tools/list/tools/call、schema/invalid args、errors、disabled/unavailable、cancel/timeout（适用时）的测试范围；固定官方 conformance suite commit/tag、目标客户端矩阵、已知例外与 failure severity，禁止暗中 wire breaking。
- **Integration：** 真 loopback Streamable HTTP MCP Server 与 SDK/客户端测试；运行可执行的官方 server conformance，并保留原始 runner 输出和 NOT_VERIFIED 项；Harness Policy/Approval 自有语义另测。
- **E2E：** 真实 ChatGPT→Tunnel→Runtime 发现 Tool/执行/处理错误；第二受控 MCP Client 实际互通，清楚记录两条链路差异。
- **Manual：** 检查官方报告的 revision、运行命令、失败和偏差；在 ChatGPT 端确认工具列表、调用及可理解的错误提示。
- **Exit Gate：** F13 PASS，官方协议测试与产品自测/真实 ChatGPT E2E 均有实际证据；只运行自定义本地单测不能冒充官方 conformance。

# M11 — V4 Full Release Integrated Acceptance

**Scope:** Full P0+P1 综合 Gate。**Depends on:** M06 + M07/M08/M09/M10 全部 ACCEPTED。  
**Deliverable:** 同一版本 Full Release Candidate、15-feature Traceability Matrix、升级/安装/回归/真实 E2E/人工验收报告、最终 Verdict 与 Release Notes。

- **SPEC/PLAN/TASKS：** 固定 candidate commit/App/Runtime/Tunnel/config/DB schema 与 PRD 完整 AC；确定阻断优先级、升级回退和 P2 的 DEFERRED 或已交付状态；仅拆 Release integration 与缺陷任务，不在此时更改阈值/发布范围。
- **Integration：** 实际 Win App + Runtime + Tunnel + SQLite + Eval/Conformance 报告，配置 v2→v3 与 DB migration、History offline、Auto Start、Notification、Policy/Approval、Metrics/Trace、grep 与实际 ADMIT 工具的安全回归；只重新执行受影响与正式 Gate 要求的必要测试集。
- **E2E：** 同一候选版本真实 ChatGPT→Tunnel→Runtime→Tool/Policy/Approval→Execution→SQLite History/Stats/Trace；隐藏窗口/Auto Start/通知/断连恢复/崩溃重启/grep；F12 Eval 与 F13 conformance 保持独立证据。
- **Manual：** Windows 安装/升级/卸载相关路径，Config 恢复、App Start/Tray/Quit、通知导航、审批/搜索、历史/统计/Trace，逐项填写产品接受清单并确认没有持久测试残留。
- **Exit Gate：** F01–F08、F10–F14 与 Shared Evidence 的 Mandatory Gate 全部 PASS；F09/F15 未交付则 DEFERRED，实际交付则必须独立 PASS。符合 PRD 方可标 **V4 — ACCEPTED / FROZEN**。

---

# O01 — Optional Token / Data Volume Estimation

**Scope:** P2 F09。**Depends on:** M08；未列入正式开发则 DEFERRED，不阻断 M11。  
**Deliverable:** 可追溯 estimator version 的 Tool I/O bytes/chars/truncation 与明确标记为 estimated 的 Token 数量。

- **SPEC/PLAN/TASKS：** 冻结 estimator 算法与适用模型、误差说明、版本、输入输出统计口径、截断/未知条件和隐私上限。
- **Integration：** 真实 Runtime safe evidence 中的字节/字符/截断计算、版本升级兼容和不得存 raw file body/secret 的测试。
- **E2E：** 真实 ChatGPT Tool read/grep 请求→History/Statistics 展示带 estimator name/version 与 estimated 标签的数值，不冒充 provider billed tokens。
- **Manual：** 用已知大小的样本验证数据量、截断与无法估算时 UNKNOWN；核对说明明确而不泄露内容。
- **Exit Gate：** 若选择交付则 F09 独立 PASS；否则 DEFERRED。

# O02 — Optional No-side-effect Policy Simulator

**Scope:** P2 F15。**Depends on:** M03 的 Shared Evidence/authority 契约；未规划则 DEFERRED。  
**Deliverable:** UI→Tauri→Runtime 的 Policy Preview，使用真实 PolicyEngine 当前有效快照，绝不执行任何 Tool。

- **SPEC/PLAN/TASKS：** 冻结候选参数/Workspace/Tool readiness、ALLOW/ASK/DENY/UNKNOWN、匹配规则和原因、snapshot identity、不可安全判断时的提示；区分 preview 和真实执行。
- **Integration：** 同一 fixture 的 Simulator 与真实 Runtime PolicyEngine 决策一致；不产生 ExecutionPermit、Pending Approval、Session Grant、Bash 进程或 Workspace 副作用；配置变更不复用旧授权。
- **E2E：** 真实 Windows UI 模拟四种判定→Tauri→Runtime Preview；再用真实 ChatGPT 工具请求核对执行仍重新判定，旧 Preview 无法授权。
- **Manual：** 人工模拟允许/审批/拒绝/未知，核查 Approval 列表、Workspace 与 Tool metrics 不发生模拟执行污染。
- **Exit Gate：** 若交付则 F15 独立 PASS；否则 DEFERRED。

---

# 4. PRD ↔ Architecture ↔ Milestone Traceability

| PRD feature / gate | Priority | Milestone owner | Integration / E2E / Manual roll-up |
| --- | --- | --- | --- |
| Shared Evidence Contract | Core prerequisite | M00 design, M03 implemented | M03 / M06 / M11 |
| F01 System Tray | P0 | M01 | M01 / M06 / M11 |
| F02 Auto Start | P1 | M07 | M07 / M11 |
| F03 Approval Notifications | P0 | M01 | M01 / M06 / M11 |
| F04 Disconnect Notifications | P0 | M01 | M01 / M06 / M11 |
| F05 Startup Performance | P0 | M02 | M02 / M06 / M11 |
| F06 One-click Diagnostics | P0 | M02 | M02 / M06 / M11 |
| F07 SQLite History | P0 | M03 | M03 / M06 / M11 |
| F08 Tool Metrics | P1 | M08 | M08 / M11 |
| F09 Estimated Tokens | P2 | O01 or DEFERRED | O01 if delivered |
| F10 grep | P0 | M05 | M05 / M06 / M11 |
| F11 File Tool Admission | P1 | M09 | M09 / M11 |
| F12 Controlled Agent Evaluation | P0 | M04; baseline/candidate in M05 | M04 / M05 / M06 / M11 |
| F13 MCP Compatibility | P1 | M10 | M10 / M11 |
| F14 MCP Tool Request Trace | P1 | M08 | M08 / M11 |
| F15 Policy Simulator | P2 | O02 or DEFERRED | O02 if delivered |

**Architecture decision coverage**：A01/A02/A03→M01/M07；A04–A08/A16/A17→M00/M03/M08；A09→M08；A10→M04/M05；A11→M02；A12→M05；A13→M09；A14→O02；A15→M00/M07/M11；A18→M10。所有 Milestone 需在 SPEC 中引用对应 Architecture ADR 与 PRD AC，若产生冲突先修改上层契约，不在 TASK 内自行改语义。

# 5. Acceptance Records and Verdicts

每个 ACCEPTANCE.md 与 EVIDENCE_INDEX.md 至少记录：

| Evidence field | Required contents |
| --- | --- |
| Scope | Milestone ID、PRD Feature / AC、Architecture ADR、SPEC/PLAN/TASKS revision |
| Build / source | Git commit SHA、branch、dirty state、App/Runtime/Tunnel binary version 或 hash |
| Environment | Windows/Host、配置 schema/effective revision、Workspace fixture、ChatGPT/Client 版本 |
| Unit / Contract | 所运行的受影响单测/协议 tests、命令、实际输出与结果 |
| Integration | 组件边界、隔离端口/实例、真实交互、错误注入与 cleanup |
| E2E | 真实入口→Tunnel/Runtime→目标行为/业务输出、时间/trace/request identity、执行许可 |
| Manual | 测试人员实际操作步骤、观察结果、截图、交互/失败恢复清单 |
| Quality gate | 目标阈值及其**事前冻结**证据、baseline、候选对比、样本量、INCONCLUSIVE |
| Safety | Workspace/Policy/Approval/Bash/SecretStore 回归、敏感数据无泄漏、无残留测试资源 |
| Verdict | ACCEPTED/FROZEN 或 FAIL/BLOCKED/NOT_VERIFIED/INCONCLUSIVE，blocker owner 与下一动作 |

**阶段状态：** NOT_STARTED → SPEC_READY → TASKS_READY → IMPLEMENTED → UNIT_PASS → INTEGRATION_PASS → E2E_PASS → MANUAL_PASS → ACCEPTED/FROZEN。每层可以 FAILED/BLOCKED/INCONCLUSIVE；只有本范围全部实际通过才允许递进。**仅编写测试计划不等于执行测试。**

**M00/M09 的特殊说明：** 这类架构/准入决策里程碑仍按本表交付、集成、E2E、人工验收；其主要验收物可以是可运行的技术契约、受控 POC 或正式候选决策，但不得仅凭会议结论标记 PASS。

# 6. Execution Restrictions, Change Control and Release

1. **外部授权**：真实 ChatGPT、联网付费模型、Windows 安装、破坏性数据写入和实例重启均需在执行时再次明确获得授权；无权执行则对应 Gate BLOCKED，而不是变更验收定义。
2. **隔离与清理**：测试使用隔离 Workspace、受控 Runtime、可清理 fixtures；必须检查是否留下测试帐号、进程、Rule、DB 行、文件及环境配置。
3. **最小修改与测试**：只运行受影响的 Unit/Contract tests 与各 Milestone 必要的 Integration/E2E，禁止全仓无关格式化；多 Agent 不并发修改同一模块，不修改他人工作树。
4. **不擅自改变 Release 目标**：任何改变 P0/P1/P2、默认行为、安全模型、真实 E2E 范围或 Gate 阈值的决定，先修订 PRD/Architecture/Roadmap，再进入 SPEC/TASK。
5. **阻断等级如实**：官方 MCP conformance 没跑、Windows Toast 真机未验收、F12 Model Adapter 不可用、F05/F12 未有事前阈值，都不能用 UI 截图、Unit 或老版本结果充当 PASS。
6. **证据一致性**：所有结论绑定相同代码 SHA、有效配置和样本；测试后若改动影响安全或关键逻辑，针对受影响门禁重新执行。
7. **Release 语义**：M06 只允许 V4 Core — ACCEPTED；M11 才允许 V4 — ACCEPTED / FROZEN；Optional 未实现显示 DEFERRED，已交付但失败不得被忽略。
8. **失败回退**：Config/SQLite schema migration、ToolRegistry/Policy、Tauri 生命周期变更必须在 PLAN 中定义回退。不可逆数据迁移需单独授权。

# 7. M00 Validation Checklist for Implementation Freeze

当前 Roadmap 已作为上游 planning baseline 冻结。M00 要把三份上游文档提升为可供 M01+ 实现直接依赖的 implementation-frozen contract，至少确认：

- [ ] V4 PRD 的 F01–F15 和 Shared Evidence Contract 逐项唯一映射到里程碑；
- [ ] 核心 Architecture ADR 决策与源码基线可行，尚未证明的技术假设有 M00 验证 Gate；
- [ ] M03→M04→M05 的证据/质量基线链无依赖环；
- [ ] M01/M02/M03 能安全并行，M07–M10 不阻断 M06 P0 Core；
- [ ] 每个 M/O 有可运行的交付结果，均包含 SPEC/PLAN/TASKS + Integration/E2E/Manual；
- [ ] F05 性能与 F12 评测阈值在开发前冻结，不允许事后把未验证结论标 PASS；
- [ ] Windows 真机通知、真实 ChatGPT MCP、SQLite 崩溃恢复、受控 Eval 与官方 conformance 都有明确执行路径、授权责任和 BLOCKED 表达；
- [ ] M06/M11 版本验收范围及 P2 延期规则与 PRD 一致；
- [ ] 每次测试都能关联 commit、binary、fixtures 与可复核 Evidence，失败路径与清理均被覆盖。

**Document status:** V4 upstream planning baseline is FROZEN FOR M00。本文件只定义未来工作分解与完成标准，不表示已经创建 Milestone SPEC、完成代码开发或执行真实验收；M00 通过后，受验证的 Architecture Contract 与后续 Milestone 依赖再进入 implementation-frozen 状态。
