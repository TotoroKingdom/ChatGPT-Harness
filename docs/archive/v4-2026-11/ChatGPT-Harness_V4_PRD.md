# ChatGPT-Harness V4 Product Requirements Document

> Status: V4 UPSTREAM PLANNING BASELINE / FROZEN FOR M00 (2026-10-23)  
> Baseline: current main after V3 documentation archive  
> Product input: V4 feature set F01–F15  
> Architecture: [ChatGPT-Harness_V4_ARCHITECTURE.md](./ChatGPT-Harness_V4_ARCHITECTURE.md) — upstream planning baseline  
> Roadmap: [ChatGPT-Harness_V4_ROADMAP.md](./ChatGPT-Harness_V4_ROADMAP.md) — upstream planning baseline  
> Scope: V4 product behavior, quality targets, delivery boundaries, and testable acceptance; architecture remains separate  
> Freeze semantics: this document is frozen as the product input to M00; any scope, priority, default behavior, security semantics, release gate, or canonical AC change requires explicit upstream document revision before downstream implementation proceeds

---

# 1. Product Summary

ChatGPT-Harness 已经从最初的只读 Harness 演进为具备 Workspace tools、Policy、Human Approval、controlled bash、Activity evidence、双语 UI 与主题能力的本地 Agent Runtime。

V4 的目标不再只是继续增加单点功能，而是把 Harness 提升为：

> **一个能够长期后台运行、主动提醒、快速诊断、持久记录、结构化追踪，并能系统评测 Agent 工具质量的本地 Workspace Agent Runtime。**

V4 重点解决五类问题：

1. 桌面体验仍依赖主窗口，后台运行和通知能力不足。
2. Runtime / MCP / Tunnel 的启动与故障链路缺少足够直观的性能和诊断能力。
3. Activity / Approval 证据仍缺少长期、可查询、可聚合的持久化历史基础。
4. 工具能力继续增加前，需要建立 Agent Tool Evaluation 与 MCP 兼容性基线，防止工具数量增加反而降低模型调用质量。
5. Policy 已经具备真实执行语义，但用户仍缺少不执行实际操作的安全模拟能力。

V4 共包含 15 项功能需求，优先级分为：

| Priority | Meaning |
| --- | --- |
| P0 | V4 Core Release 必达，缺失则阻断核心版验收 |
| P1 | V4 Full Scope 必达；不阻断独立的 Core Release |
| P2 | 可选增强；未实现须标 DEFERRED，不阻断 Core 或 Full 的交付 |

---

## 1.1 User jobs and baseline

V4 首要服务单机 Windows ChatGPT-Harness 用户的四类任务：后台持续使用 Agent Tools；收到准确、可返回原请求的审批/断连提醒；不阅读内部日志就能定位启动或连接故障；跨重启审计工具请求与使用受控评测证明工具扩展质量。

本 PRD 是未来产品契约，不是 V3 已全部实现的证明。开始 Architecture / Roadmap 前，必须依据当前源码、Tests、Manifests、最新 Wiki 和 V3 各 Milestone 的实际验收记录建立 **As-Is baseline**，分别标明 IMPLEMENTED、LOCAL_TESTED、EXTERNALLY_ACCEPTED、NOT_VERIFIED、KNOWN_ISSUE。V3 单个 Milestone 的 ACCEPTED 不等于整个 V3 的 ACCEPTED。

# 2. Product Principles

## P1 — Background continuity

Harness 是否持续运行，不应依赖主窗口是否可见。

关闭主窗口后，只要用户没有明确 Stop / Quit，Harness 可以继续在系统托盘后台运行，并保持：

- Runtime lifecycle
- MCP availability
- Tunnel connection
- Pending Approval state
- Notification capability
- Activity / Trace recording

“关闭窗口”与“停止 Harness”必须是两个不同的产品动作。

---

## P2 — Runtime authority remains authoritative

V4 新增的诊断、追踪、统计、评测和 Policy Simulator 都不能建立第二套执行真相。

以下行为仍以 Runtime 实际结果为准：

- Tool availability
- Workspace authorization
- Policy evaluation
- Approval requirement
- Execution result
- Error category
- Effective runtime state

UI、Evaluation、Simulator 和 History 都只能消费、投影或调用真实 Runtime 能力，不得重新实现一套与 Runtime 可能产生偏差的判断逻辑。

---

## P3 — One evidence foundation

Activity、Approval History、Agent Trace、Tool Statistics 和 Agent Evaluation 应建立在统一的结构化执行证据基础之上。

V4 不接受长期维护以下模式：

- Activity 一套记录
- Trace 另一套记录
- Statistics 再单独打点
- Evaluation 又自行解析日志

同一次真实请求必须能够通过稳定 identity 关联其：

- MCP request
- Workspace validation
- Policy evaluation
- Approval
- Execution
- Result
- Activity
- Trace
- Metrics

具体数据模型和持久化 schema 由 Architecture 定义。

---

## P4 — Measure before expanding tools

V4 不以“工具越多越好”为目标。

新增工具必须考虑：

- Tool selection accuracy
- Argument validity
- Runtime call success
- Task completion quality
- Safety semantics
- 与已有工具的职责重叠
- 是否增加模型选择歧义

因此 Agent Tool Evaluation 必须成为工具扩展的重要准入依据。

---

## P5 — Truthful metrics

V4 所有可观测性数据必须明确区分：

- 实际观测值
- 派生统计值
- 估算值
- 不可观测值

特别是 Token Statistics：

> Harness 无法从普通 MCP tool call 得到模型供应商真实计费 Token 时，不得把本地估算值展示为模型真实 Token 使用量。

UI 和导出数据都必须使用明确的 estimated / estimated token wording。

---

## P6 — Notify, do not spam

Windows 通知用于需要用户注意的异步事件，不是新的日志流。

通知必须具备：

- 状态去重
- 合理的恢复提示
- 可点击导航
- 不重复轰炸同一故障
- 不泄露 secret 或敏感原始参数

---

## P7 — Security boundaries survive observability

为了 Trace、History、Statistics 或 Evaluation，不得降低现有安全边界。

尤其不得把以下数据无边界持久化：

- API keys
- SecretStore secret
- raw environment
- 未经处理的敏感命令参数
- 不必要的完整文件内容
- 任意物理路径
- 未受控 stdout/stderr

历史记录和 Trace 必须继续使用安全、受限、可解释的证据投影。

---

## P8 — V4 should preserve proven V3 behavior

V4 的新增能力不得破坏已经存在的：

- Workspace boundary
- Tool enable / disable semantics
- ALLOW / ASK / DENY
- Human Approval
- Session grants
- controlled bash
- SecretStore boundary
- Runtime / MCP / Tunnel lifecycle ownership
- desired / effective state distinction
- i18n / Appearance behavior

如 V4 架构需要改变这些语义，必须先回到 PRD 重新评审，而不能在实现阶段隐式改变。

---

# 3. V4 Success Definition

V4 成功不等于“15 个页面或按钮全部完成”。

最终应能证明以下产品体验成立：

## 3.1 Always-available desktop Harness

用户启动桌面应用后，可以选择自动启动 Harness。

主窗口关闭后：

- Harness 继续运行
- 系统托盘可查看关键状态
- 用户可 Start / Stop
- 用户可重新打开主窗口
- Pending Approval 可以主动通知
- 连接故障可以主动通知

---

## 3.2 Diagnosable connection lifecycle

对于一次 Harness Start 或连接故障，用户可以回答：

- Runtime 是否成功启动？
- MCP 是否 ready？
- Tunnel 是否启动？
- Control plane 是否连接？
- MCP target 是否可达？
- Proxy 是否可用？
- Credential 是否缺失或失效？
- 哪个阶段耗时最多？
- 当前应该怎么修复？

不能只给出笼统的 “Connection failed”。

---

## 3.3 Durable and queryable evidence

关闭应用或重启 Harness 后，用户仍可以查看已持久化的历史证据，并按时间、Tool、结果、Policy decision 等条件查询。

历史记录必须能够支持：

- Activity history
- Approval history
- Trace inspection
- latency statistics
- success / failure aggregation
- Evaluation analysis

---

## 3.4 Measurable Agent tool quality

Harness 应能够建立稳定的 Evaluation baseline，并回答：

- 模型是否选对工具？
- 参数是否符合 Tool schema？
- Tool 调用是否成功？
- Tool 执行是否完成任务？
- 新增工具后这些指标是提升还是回退？

新增 grep 或其他工具后，可以与基线进行回归对比。

---

## 3.5 Safe policy exploration (Optional / F15 P2)

这是非阻断的可选增强；若交付则全部安全语义必须验收通过。

用户能够输入一个候选 Tool request，在不执行真实操作的情况下看到：

- capability 是否允许
- Workspace 检查结果
- 匹配的 Policy rules
- 最终 ALLOW / ASK / DENY
- decision reason
- 是否会要求审批

模拟结果必须来自真实 Runtime Policy semantics，而不是 UI 推测。

---

## 3.6 Version release gates

必须区分功能优先级与对外声称完成的版本范围：

| Gate | Mandatory scope | Verdict |
| --- | --- | --- |
| **V4 Core Release** | P0：F01、F03、F04、F05、F06、F07、F10、F12；Shared Evidence Contract；V3 已有 Workspace / Policy / Approval / Bash / SecretStore / lifecycle 回归；真实 ChatGPT MCP E2E | 满足全部后可标 **V4 Core — ACCEPTED** |
| **V4 Full Scope** | Core Gate + P1：F02、F08、F11、F13、F14。F11 必须完成候选准入/拒绝/延期评审，不承诺全部候选工具实现 | 全部通过后才可标 **V4 — ACCEPTED / FROZEN** |
| **Optional Feature Gate** | P2：F09、F15；可延期或单独交付 | 延期时标 **DEFERRED**；已交付则每项必须独立 PASS；不阻断 Core / Full |

任何未做的外部 E2E、缺少环境的兼容性测试、未达到已冻结性能阈值或统计样本不足，都不得记为 PASS；使用 BLOCKED、NOT_VERIFIED、INCONCLUSIVE 等如实表达。发布记录必须公开区分已交付、延期、未验证和已知问题。

## 3.7 Milestone acceptance threshold contract

PRD 在没有真实测量之前不虚构毫秒或准确率目标。**F05 启动性能与 F12 Agent Evaluation 的指标定义、baseline 样本、运行配置、判定阈值 / 允许退化、失败场景与证据形式，必须在对应 Milestone 的 SPEC 编写阶段完成，并在该 Milestone 进入 `SPEC_READY`、创建任何实现型 TASK 或开始候选实现之前冻结。** 若真实 baseline 测量导致阈值无法成立，应先修订并重新冻结 SPEC；不得在候选实现或验收完成后倒填阈值。未完成该前置工作时，对应 Gate 为 BLOCKED；仅建成面板、采集日志或观察到 Tool 返回成功都不足以验收。

## 3.8 Shared evidence dependency

F07 / F08 / F12 / F14 须共用的请求身份、事件结果、授权状态边界、脱敏和关联契约是 **Core Gate 基础前置条件**；可以并行开发托盘/连接功能，但不得分别建设彼此不可关联的持久化、统计与 Trace 管道。具体表结构、协议和存储 Owner 属于 Architecture。

# 4. V4 Feature Inventory

| ID | Feature | Core Requirement | Priority |
| --- | --- | --- | --- |
| F01 | 系统托盘 | 关闭主窗口后后台运行；打开窗口、Start/Stop、连接状态、待审批数量 | P0 |
| F02 | 应用启动时自动启动 Harness | 打开应用后可自动执行 Start Harness；用户可关闭 | P1 |
| F03 | 审批通知 | 新 Pending Approval 时发送 Windows 通知，点击进入对应审批详情 | P0 |
| F04 | 断连通知 | Tunnel / Harness 断连、恢复失败时通知；恢复可提示；避免重复通知 | P0 |
| F05 | Tunnel / Harness 启动性能优化 | 建立阶段耗时基线并缩短 Ready 等待时间 | P0 |
| F06 | 一键连接诊断 | 检查 Runtime、MCP、Tunnel、Proxy、Credential 等状态并给出原因与修复建议 | P0 |
| F07 | SQLite 历史记录 | 持久化 Activity / Approval 历史并支持查询、筛选、分页与清理 | P0 |
| F08 | 工具调用时长统计 | 调用次数、成功率、平均耗时、P95、审批等待时长 | P1 |
| F09 | Token 统计 | 统计数据量并估算 Token、记录截断，不冒充模型真实用量 | P2 |
| F10 | grep 工具 | Workspace 全文搜索、正则、路径过滤、行号、结果限制 | P0 |
| F11 | 扩展工具能力 | 评估目录管理、复制、移动、重命名、删除等候选工具并按准入结果扩展 | P1 |
| F12 | Agent Tool Evaluation | Tool selection、参数合法、调用成功、任务完成及回归对比 | P0 |
| F13 | MCP 协议兼容性测试 | 验证工具发现、调用、错误语义及客户端兼容性 | P1 |
| F14 | MCP Tool Request Trace | Trace / Span 关联 Runtime 可观测的单次 MCP Tool Request，不承诺模型隐藏推理或完整 Agent Loop | P1 |
| F15 | Policy Simulator | 使用真实 Policy semantics 模拟判定，不执行真实操作 | P2 |

## 4.1 Canonical Acceptance Criteria IDs

以下 AC-ID 是 V4 上游产品契约的稳定追踪键。Milestone SPEC 可以将单个 Canonical AC 细化为更具体的 Derived AC，但必须保留对上游 AC-ID 的映射；不得在 SPEC / PLAN / TASKS 中重新定义、删除或改变其产品语义。

| AC-ID | Canonical acceptance criterion |
| --- | --- |
| SEC-AC01 | 同一次真实 Tool request 必须以稳定 identity 关联 Activity、Approval、History、Metrics 与 Trace；不得依赖模糊时间匹配。 |
| SEC-AC02 | Durable evidence 只保存最小化、typed、bounded、sanitized 的安全投影；不得建立第二套 Policy / authorization truth。 |
| SEC-AC03 | 历史 evidence 不得恢复 Pending Approval、Session Grant、ExecutionPermit 或其他实时授权状态。 |
| F01-AC01 | Harness Ready 时关闭主窗口默认隐藏到托盘且 Runtime / MCP / Tunnel / Approval 状态继续工作；关闭窗口不等于 Stop 或 Quit。 |
| F01-AC02 | Tray 提供 Show、Start、Stop、真实连接状态、Pending Approval count 与 Quit，状态来自权威 lifecycle。 |
| F01-AC03 | Explicit Quit 走正常停止流程并回收受管 Runtime / Tunnel，不留下 owned orphan process。 |
| F02-AC01 | Auto Start on app launch 是独立持久化设置，新安装默认关闭，升级不得覆盖用户已有明确选择。 |
| F02-AC02 | 开启后 App 初始化完成只通过标准 Harness Start 流程自动尝试一次，不绕过 config / eligibility / secret / security 检查。 |
| F02-AC03 | Auto Start 失败进入真实 Error / Diagnostics flow，不重复启动、不伪报 Ready，也不等同于 Windows 登录自启动。 |
| F03-AC01 | 每个新的 Pending Approval 至多发送一次对应 Windows notification，并保留 Tray / Approval Center fallback。 |
| F03-AC02 | 点击通知后聚焦 App 并查询该 approval 的实时状态；resolved / expired 请求不得被恢复为 Pending。 |
| F03-AC03 | 通知失败不得丢失 Approval，通知内容不得泄露 secret、raw unrestricted args 或未处理文件内容。 |
| F04-AC01 | Disconnect / degraded 通知按真实状态转换与 incident episode 去重，持续同一故障不重复刷屏。 |
| F04-AC02 | 恢复后可发送一次 recovery notification；恢复后新的独立故障可形成新的 incident。 |
| F04-AC03 | 通知成功不等于连接恢复，通知入口必须指向真实 Overview / Diagnostics 状态。 |
| F05-AC01 | Start→Ready 必须记录可比较的总耗时和关键阶段耗时，并区分 cold / warm、P50 / P95、失败率和环境。 |
| F05-AC02 | 性能 baseline 与改善阈值必须按 §3.7 在 SPEC_READY 前冻结，候选版本必须达到已冻结目标才能 PASS。 |
| F05-AC03 | 优化不得跳过 authoritative readiness checks，不能以 spawn success 或 process exists 冒充 Ready。 |
| F06-AC01 | 一键诊断至少覆盖 config、Workspace、Runtime、MCP、Tunnel、control-plane、proxy、credential 可安全判断状态和 lifecycle。 |
| F06-AC02 | 每项诊断返回 PASS / WARNING / FAIL / UNKNOWN、可读解释、安全技术证据与适用 remediation。 |
| F06-AC03 | 默认诊断只读；修改配置、重启、触碰密钥或文件的修复动作必须另行显式授权，并以故障 fixtures 验证。 |
| F07-AC01 | Activity、终态 Approval 与统计 / Trace 所需 evidence 可跨 Runtime / App 重启持久查询，并支持时间、Tool、outcome、decision、approval、分页。 |
| F07-AC02 | History 与 live authorization 严格隔离，旧 Pending / Grant / Permit 不能从 SQLite 恢复。 |
| F07-AC03 | Retention、容量上限、手工清理、migration / full / busy / corrupt / drop 等降级语义必须可观察且不得改变 Policy truth。 |
| F08-AC01 | Metrics 至少提供 calls、success / failure、success rate、average、P95 与 approval wait，并按 Tool / time range 查询。 |
| F08-AC02 | execution duration、approval wait 与 request wall time 必须分离；DENY、cancel、timeout 与 executor failure 不得混为同一分母。 |
| F08-AC03 | 样本量、INCOMPLETE / UNKNOWN 与 P95 算法必须透明；样本不足不得伪装为稳定统计结论。 |
| F09-AC01 | 记录 bounded input / output bytes、chars、truncation 与 estimated input / output tokens。 |
| F09-AC02 | Token 数值始终标为 estimated，并记录 estimator name / version；不得冒充 provider billed / actual model usage。 |
| F09-AC03 | 不得为了 Token 估算额外持久化 raw 文件正文、secret 或无限 payload。 |
| F10-AC01 | grep 支持 text / regex、Workspace-relative scope、include / exclude、line number、bounded results 与 deterministic truncation。 |
| F10-AC02 | grep 必须走既有 Workspace / Policy / Approval / ExecutionPipeline，不得通过隐藏 shell grep 绕过安全链。 |
| F10-AC03 | grep 需覆盖越界、symlink / junction / reparse、TOCTOU、无效 regex、超大输入 / 输出，并通过 F12 baseline regression。 |
| F11-AC01 | mkdir / list / copy / move / rename / delete 等候选项逐项形成可审计的 ADMIT / REJECT / DEFER 决策，而不是默认全部加入。 |
| F11-AC02 | 所有 ADMIT 并实际交付的 Tool 必须具有明确 schema、Workspace / Policy / Approval 语义并走完整 Runtime pipeline。 |
| F11-AC03 | destructive / 双路径操作必须定义覆盖、冲突、跨卷、中间态、不可逆性和恢复边界，并通过真实 E2E 与 Eval regression。 |
| F12-AC01 | Evaluation 使用真实可控 Agent / Model adapter、版本化 dataset / ground truth、隔离 Workspace 与真实 MCP Runtime。 |
| F12-AC02 | 至少独立计算 Tool selection、argument validity、runtime call outcome 与最终 task completion；tool returned success 不等于 task success。 |
| F12-AC03 | baseline、candidate、样本量、随机性与回归阈值按 §3.7 在实现前冻结；不可控模型或样本不足必须 BLOCKED / INCONCLUSIVE。 |
| F12-AC04 | Passive production telemetry 不得冒充 Tool selection accuracy、未发出候选工具或模型隐藏推理。 |
| F13-AC01 | 固定 MCP protocol revision、rmcp / client / runner 版本并验证 initialize、tools/list、schema、tools/call 与错误 / 取消 / 超时适用语义。 |
| F13-AC02 | 官方 conformance 可用时实际执行并保留原始证据；未执行项目必须标 NOT_VERIFIED。 |
| F13-AC03 | 官方 protocol conformance、Harness-specific security semantics 与真实 ChatGPT→Tunnel E2E 必须作为三类独立证据。 |
| F14-AC01 | 每个 MCP Tool request 能关联 received、resolve、Workspace、Policy、Approval、execution 与 terminal / incomplete 阶段。 |
| F14-AC02 | Trace 与 Activity / Approval / History 使用稳定 request identity 关联，并能解释失败阶段、慢阶段和 executor 是否启动。 |
| F14-AC03 | 不伪造 ChatGPT 隐藏推理、完整 Agent Loop 或不可观测 Span；Trace 不得成为 raw payload dump。 |
| F15-AC01 | Simulator 使用 Runtime 当前 effective Tool / Workspace / Policy semantics 返回 ALLOW / ASK / DENY / UNKNOWN、原因、规则与 would-require-approval。 |
| F15-AC02 | Preview 不执行 Tool、不改 filesystem、不启动 bash、不创建 Permit / Pending Approval / Session Grant，也不污染真实 execution metrics。 |
| F15-AC03 | Preview 结果不是授权票据；真实请求必须基于当时有效配置重新经过权威 Runtime 判定。 |

---

# 5. Desktop Integration Requirements

## 5.1 F01 — System Tray

### Objective

让 ChatGPT-Harness 从“窗口型工具”升级为可长期驻留的桌面应用。

### Required Behavior

系统托盘至少提供：

- Open / Show main window
- Start Harness
- Stop Harness
- Current Harness connection state
- Pending Approval count
- Quit application

关闭主窗口时：

- 新安装默认关闭窗口时隐藏到托盘而非 Quit；首次应提示用户后台驻留和显式退出入口
- 默认不等价于 Quit
- Harness 处于 Running 时继续后台运行
- Pending Approvals 和连接状态继续维护
- Windows notifications 仍可发送

明确 Quit 时：

- 必须按照正常生命周期停止 Harness
- 清理 Runtime / Tunnel owned processes
- 不留下 orphan process

### State Truthfulness

托盘显示的状态必须来自产品真实 lifecycle state，不得仅根据窗口状态或进程存在推断 Ready。

### Acceptance Intent

用户在 Harness Ready 时关闭主窗口，随后真实 ChatGPT MCP 调用仍然能够工作；用户可以从托盘重新打开窗口并看到连续状态。

---

## 5.2 F02 — Auto Start Harness on App Launch

### Objective

减少每次打开桌面应用后额外点击 Start Harness 的步骤。

### Required Behavior

Settings 提供独立开关：

- Auto Start Harness on app launch

新安装默认关闭；用户可开启，升级不得覆盖已有明确设置。

当开启时：

- 应用初始化并读取有效配置后自动尝试 Start Harness
- 不需要用户再次点击 Start
- 不得绕过现有 eligibility / config / secret / security 检查
- 启动失败进入正常 Error / Diagnostic flow

本功能不是：

- Windows 开机自启动应用
- 登录系统后自动启动 ChatGPT-Harness

V4 的要求仅为：

> 用户主动打开 ChatGPT-Harness 应用后，应用可自动执行 Harness Start。

---

## 5.3 F03 — Approval Notifications

### Objective

当主窗口隐藏或用户正在其他应用中工作时，仍能及时处理 ASK 请求。

### Required Behavior

新增 Pending Approval 时发送 Windows notification。

通知至少表达：

- Harness 有待审批请求
- Tool identity
- 安全的简要上下文
- 可点击进入对应 Approval detail

不得在系统通知中暴露：

- secrets
- raw unrestricted command arguments
- 未处理文件内容
- 其他不必要敏感字段

### Navigation

用户点击通知后：

- 打开或聚焦主窗口
- 导航至对应 Pending Approval
- 若请求已 resolved / expired，则显示真实终态，而不是伪造 Pending

### De-duplication

同一个 approval request 不应重复发送相同通知。

### Delivery failure and acceptance

Windows 通知被禁用、权限不足或投递失败时，不得因此丢失 Pending Approval；Tray 计数与主窗口仍以 Runtime 真实请求为准。至少覆盖：窗口显示/隐藏、新 ASK/重复状态轮询、点击时请求已完成/已过期、系统通知不可用、点击跳转对应 request。通知只是提示通道，不是授权状态源。

---

## 5.4 F04 — Disconnect / Recovery Notifications

### Objective

Harness 在后台时，连接故障不能静默发生。

### Required Events

至少考虑：

- Tunnel disconnected
- Harness lost Ready state
- automatic recovery / retry failed
- connection restored

### Required Behavior

通知应体现状态转换，而不是轮询结果。

例如同一持续故障：

- 首次进入重要故障状态时通知
- 状态未变化期间不重复刷屏
- 恢复后可发送一次恢复提示
- 再次独立故障可以再次通知

通知内容应提供可行动入口，例如打开 Overview / Diagnostics。连续故障使用同一次 incident/episode 去重，恢复后再次独立断连可形成新通知；通知发送成功不能被误记为连接恢复。

---

# 6. Connection Reliability Requirements

## 6.1 F05 — Harness Startup Performance

### Objective

建立可重复的启动性能基线，并缩短从 Start Harness 到 Ready 的实际等待时间。

### Product Requirement

V4 不能只测一个总耗时。

必须能够区分关键阶段，例如产品层面的：

- eligibility / configuration preparation
- Runtime launch
- Runtime ready
- MCP ready
- Tunnel launch
- control-plane connection
- MCP target readiness
- final Harness Ready

最终阶段名称和测量点由 Architecture 定义。

### Optimization Rule

任何“优化成功”必须基于可比较数据，而不是主观感觉。

F05 开发前应测冷启动、热启动、Start→Ready 总耗时、阶段耗时、P50/P95、失败率和 readiness 正确性；明确样本量、硬件、配置与网络条件，在 SPEC 冻结改善阈值。**只增加埋点而没有达到已冻结目标，不算性能优化通过。**

至少需要支持：

- baseline measurement
- after-change comparison
- total startup duration
- slowest phase identification

### Quality Boundary

不得通过以下方式伪造更快 Ready：

- 跳过 authoritative readiness checks
- 把 spawn success 当 Ready
- 把 process exists 当 Ready
- 跳过 MCP reachability
- 提前隐藏失败状态

---

## 6.2 F06 — One-click Connection Diagnostics

### Objective

用户无需阅读内部日志即可知道“为什么连不上，以及下一步怎么处理”。

### Diagnostic Scope

至少覆盖：

- configuration validity
- Workspace validity
- Runtime distribution / launch
- Runtime readiness
- MCP endpoint readiness
- Tunnel distribution / launch
- Tunnel readiness
- control-plane connectivity
- configured proxy
- credential presence / usable state where safely determinable
- current Harness lifecycle state

### Required Output

每个检查项至少提供：

- check name
- PASS / WARNING / FAIL / UNKNOWN（不能安全确认或未执行时不得当作 PASS）
- user-readable explanation
- safe technical evidence
- remediation when applicable

### Safety

诊断不得输出：

- raw API key
- secret value
- unsafe environment dump
- unrestricted command output

### UX

一键诊断默认只读；任何会修改配置、重启 Runtime、触碰密钥或改变文件的修复动作必须另行显式授权。诊断结果需包含可验证的故障夹具覆盖，不应只在“全部正常”场景通过。

用户可以从明显入口执行一次诊断，并看到：

- overall result
- failed stage
- remediation
- copyable safe diagnostic summary

---

# 7. Persistent History and Observability

## 7.1 F07 — SQLite History

### Objective

将短生命周期内存 Activity 升级为可长期查询的本地历史能力。

### Required Data Domains

至少持久化：

- Activity history
- resolved / expired / cancelled Approval history
- execution evidence needed for statistics
- Trace metadata required by F14

Pending approval 的实时 authorization state 与历史证据必须区分。

### Critical Boundary

SQLite history 是 evidence store，不是 authorization restore mechanism。

应用或 Runtime 重启后：

- 历史 Approval 可以查看
- 旧 Pending request 不得因为数据库记录自动恢复为可批准请求
- 旧 Session Grant 不得因为历史记录自动恢复

### Query Requirements

至少支持：

- time range
- tool
- outcome
- policy decision
- approval-related state
- pagination

### Retention / Cleanup

用户能够清理历史数据。

V4 必须定义合理的：

- retention behavior
- manual clear
- size / age management

新安装默认不能在用户不知情下无限期积累历史。SPEC 开发前必须冻结并测试：保留时长、容量上限、超限处理顺序、手动清理范围与数据库错误时的降级说明；UI 应展示有效配置。History 删除和失败不能改变实时权限，也不能从旧记录恢复 Pending/Grant。

具体数据库 schema、indexes 和 migration 由 Architecture / SPEC 冻结。

---

## 7.2 F08 — Tool Call Metrics

### Objective

从真实执行证据中形成可用于性能和质量分析的统计视图。

### Required Metrics

至少包括：

- total calls
- successful calls
- failed calls
- success rate
- average execution duration
- P95 execution duration
- approval wait duration where applicable

统计应能按至少以下维度查看：

- Tool
- time range

可进一步支持 outcome / decision 等维度，具体范围由后续 SPEC 冻结。

### Semantics

必须明确区分：

- execution duration
- approval wait duration
- total request wall time

不能把审批等待混入执行耗时后仍称为 “Tool execution time”。

须区分 received tool calls、execution attempts、executed success、DENY、cancel、timeout 与 system error。成功率分母、P95 样本量和小样本处理必须由 SPEC 明确；拒绝与取消不可悄悄混同执行失败。

---

## 7.3 F09 — Estimated Token / Data Volume Statistics

### Objective

帮助用户理解 Tool input / output 的上下文体量和截断行为。

### Required Measurements

至少考虑：

- input bytes / characters
- output bytes / characters
- estimated input tokens
- estimated output tokens
- truncation occurred
- truncation amount or limit evidence where available

### Truthfulness Requirement

所有 Token 数值必须明确标注为：

- estimated tokens

除非未来 MCP / model platform 明确提供真实 usage 并且 Harness 能验证其语义，否则不得展示为：

- actual model tokens
- billed tokens
- ChatGPT token usage

### Estimator Transparency

产品需要能识别使用了哪种 estimator / estimator version，以便未来算法变化时避免错误比较历史数据。

具体估算算法由 Architecture / SPEC 决定。

---

## 7.4 F14 — MCP Tool Request Trace

### Objective

让一次 MCP request 的关键阶段能够在一条结构化调用链中查看。

**可观测边界**：仅承诺 Harness 实际处理的单次 MCP request，以及可关联的 Runtime 事件；不承诺 ChatGPT 的隐藏模型推理、模型未发出的候选工具、完整 Agent Loop 或供应商真实 Token Usage。无法观测的阶段不得伪造 Span，必要时标记 UNKNOWN。

### Trace Coverage

同一次 request 至少能关联：

- request received
- Tool / capability resolution
- Workspace validation
- Policy evaluation
- Approval lifecycle when applicable
- Execution
- completion / failure
- Activity evidence

### Required User Value

用户能够从一次失败或慢请求回答：

- 请求在哪个阶段失败？
- 哪个阶段耗时最高？
- Policy 为什么得到当前结果？
- 是否等待过 Approval？
- Executor 是否真正启动？
- 最终 outcome 是什么？

### Correlation

Trace 必须与 Activity / Approval / execution evidence 通过稳定 identity 关联。

### Privacy Boundary

Trace 默认只能保存完成诊断所需的安全结构化字段，不得成为 raw payload dump。

---

# 8. Tool System Requirements

## 8.1 F10 — grep Tool

### Objective

提供面向 Agent 的 Workspace 全文文本搜索能力，避免通过 bash 或大量 read_file 模拟搜索。

### Required Capabilities

grep 至少支持：

- text search
- regular expression matching
- Workspace-relative path scope
- path include / exclude filtering
- line number
- bounded result count
- deterministic truncation / limit indication

### Safety

grep 必须：

- 遵守 Workspace boundary
- 不搜索 Workspace 外目标
- 对不支持或无效 pattern 明确失败
- 对超大结果集进行限制
- 不通过隐藏 shell command 实现而绕过 Runtime tool enforcement

### Tool Quality

grep 的 Tool name、description、schema 和 result shape 必须接受 Agent Tool Evaluation。

只有功能能运行，不代表产品验收完成。

---

## 8.2 F11 — Extended File Tools

### Objective

评估并按真实 Agent 需求扩展文件系统操作能力。

### Candidate Capabilities

候选包括但不限于：

- directory create
- directory list / management
- copy
- move
- rename
- delete

这些候选项不是 V4 PRD 对“全部必须实现”的承诺。

### Admission Criteria

某个候选 Tool 进入正式 V4 工具集前，应至少回答：

1. 是否解决真实高频任务？
2. 是否能通过现有工具合理完成？
3. 是否与已有 Tool 产生高重叠或 Tool selection ambiguity？
4. 参数模型是否能让 Agent 稳定生成？
5. Workspace / Policy / Approval 语义是否明确？
6. destructive semantics 是否可清晰表达？
7. Evaluation 是否表明加入它不会显著降低整体 Tool quality？

### Destructive Operations

delete / destructive move 等能力必须有明确安全语义。

不能仅因为 Developer/Test Mode 开启就跳过 Policy / Approval / Workspace enforcement。

---

# 9. Agent Quality Requirements

## 9.1 F12 — Agent Tool Evaluation

### Objective

建立可重复、可比较的 Agent 工具质量评测体系。

### Required Evaluation Dimensions

至少包括：

- Tool selection accuracy
- argument schema validity
- tool-call success rate
- task completion rate

建议同时能够记录：

- unnecessary tool call
- wrong-tool selection
- recoverable vs terminal failure
- number of calls per task

最终评分公式由后续 SPEC 定义。

### Evaluation Dataset

Evaluation 应使用可重复的任务集 / fixtures。

任务必须覆盖：

- current core tools
- normal success
- invalid arguments
- ambiguous tool choice
- policy / approval cases where relevant
- failure and recovery scenarios

### Regression Comparison

至少支持：

- baseline run
- candidate run
- metric comparison
- regression detection

V4 新增 grep 和候选文件工具时，必须能够与扩展前 baseline 对比。

### Truthfulness

Agent Evaluation 的任务完成率不能只依据 “tool returned success”。

如果任务要求的最终目标未达成，应计为 task failure。

### Evaluation vs passive telemetry

Harness 作为 MCP Server，通常只能观察真实发起的 tools/call，不能由此推断模型“本应调用的工具”、未执行的候选、隐藏推理或用户最终目标是否完成。

- **Offline / controlled Agent Evaluation** 才能报告 Tool selection accuracy 与 task completion rate：使用标注的 ground truth、可重复任务集、受控 Agent runner、预期工具/参数、最终状态断言和回归对比。
- **Online / passive telemetry** 只能报告真实可观测的请求、参数合法性、Policy/Execution outcome、耗时等；不能将已被调用工具的成功率冒充工具选择准确率或任务完成率。
- 记录 dataset 版本/hash、fixtures、模型/Agent/工具 schema/Policy 配置、运行环境、样本量、随机性/重复运行策略、评测器版本。样本不足或结果不稳定时报告 INCONCLUSIVE。
- 在候选评测前冻结核心指标阈值、允许退化与阻断准则；写入型评测使用隔离 Workspace 和可清理 fixtures，不得越过真实 Policy/Approval。

---

## 9.2 F13 — MCP Protocol Compatibility

### Objective

降低 Harness MCP 在不同客户端、SDK 版本或协议边界上的兼容性风险。

### Required Coverage

至少验证：

- server initialization where applicable
- tools/list
- tool metadata / schema
- tools/call success
- structured errors
- unknown tool
- invalid arguments
- disabled / unavailable behavior where protocol permits
- cancellation / timeout semantics where applicable

### Conformance Boundary

如果 MCP 官方 Conformance tooling 可用且适配当前协议版本，应优先接入。

如果某一能力属于 Harness 产品语义而不是 MCP 标准语义，应明确区分：

- protocol conformance
- Harness-specific behavior

不得将内部测试全部命名为 MCP conformance。

SPEC 必须固定被测 MCP 协议版本、客户端/SDK 组合、官方工具可用性与可接受的兼容性缺陷。没有实际执行官方 conformance 的项目必须如实标为 NOT_VERIFIED。

---

# 10. Security and Policy Requirements

## 10.1 F15 — Policy Simulator

### Objective

允许用户在不产生实际副作用的情况下理解一项 Tool request 会被如何判定。

### Required Input

用户至少能够选择：

- Tool
- Tool arguments / relevant scope
- current effective configuration context

具体输入 UI 由后续设计决定。

### Required Output

模拟结果至少提供：

- Tool / capability status
- Workspace validation result where applicable
- matched rule IDs / user-readable rules
- effective ALLOW / ASK / DENY
- decision reason
- whether Approval would be required

### Authoritative Semantics

Simulator 必须复用 Runtime 的真实 Policy evaluation semantics。

禁止：

- React 单独复制 Policy precedence
- UI 根据 rule 列表自行推导 result
- 建立第二个只用于 Simulator 的简化 Policy Engine

### No Side Effect

模拟针对给定的有效配置快照及候选请求，不能创建能够被真实执行复用的 Permit；执行时配置改变必须重新使用权威 Runtime 检查。模拟不等同于未来操作一定被允许。

Simulation 必须保证：

- 不执行 Tool
- 不修改 filesystem
- 不启动 bash process
- 不创建真实执行 permit
- 不把模拟请求伪装成真实 Activity success
- 不创建可用于恢复真实执行的 Pending Approval

如需要记录 simulator usage，应与真实 execution evidence 明确区分。

---

# 11. Cross-feature Data Principles

## 11.1 Shared request identity

同一真实请求必须能够跨以下能力关联：

- Activity
- Approval
- Trace
- Metrics
- History

不得依赖模糊时间匹配来关联一次请求。

---

## 11.2 Historical evidence vs live authority

以下内容属于 live Runtime authority：

- current effective Tool state
- pending Approval
- session grants
- execution permit
- live Policy decision context

以下内容可进入 durable history：

- past Activity
- resolved Approval evidence
- completed Trace
- durations
- result summary
- safe policy evidence

历史记录不得反向恢复实时权限。

---

## 11.3 Data minimization

持久化与统计只保留满足产品目标所需的数据。

V4 不以“记录一切”为原则。

对于原始 Tool input / output，应优先使用：

- sanitized projection
- bounded preview
- byte / character counts
- hash / fingerprint where appropriate
- structural evidence

具体保留策略由 Architecture 定义。

---

## 11.4 Shared Evidence Contract Gate

在 F07/F14 具体 schema/Span 定型前，须明确并验证：

- stable request identity、Runtime generation/session、retry、Approval 的关联与边界；
- 请求阶段、时间口径、结果分类、执行与审批分别计时；
- 字段级的安全 evidence projection、secret/redaction、有界预览与禁存数据；
- immutable 历史证据与 live authority 独立；
- telemetry/database 失败、阶段缺失和无法判断时的 UNKNOWN/DEGRADED 语义。

这是 P0 产品契约，不在 PRD 中指定具体 SQLite 表或 Trace 库。

# 12. UI Product Requirements

V4 不要求重做 V3 已完成的信息架构和视觉系统。

新增 UI 应继续遵守已有的简洁桌面工具体验。

## 12.1 Tray

Tray 是后台生命周期入口，不替代完整主窗口。

需要提供高频动作和最重要状态，不把完整 Diagnostics / History 塞入 tray menu。

## 12.2 Overview / Connection

Overview 应能够进入：

- startup / connection status
- one-click diagnostics
- relevant remediation

## 12.3 Activity / History

用户需要能够：

- 查看近期与历史记录
- 搜索 / 筛选
- 分页
- 进入单个 request 的 Trace
- 清理历史

## 12.4 Statistics

应提供工具级基础统计，不要求在 V4 做复杂 BI Dashboard。

至少能够理解：

- calls
- success rate
- average / P95 latency
- approval wait
- estimated data / token volume when启用

## 12.5 Evaluation

Evaluation UI 必须区分有 ground truth 的离线评测与无完整 ground truth 的线上调用统计。没有可观测证据的选择准确率和任务完成率不能显示为确定值。

Evaluation UI / output 应以回归判断为核心，而不是只显示一堆原始日志。

至少能区分：

- baseline
- candidate
- improvement
- regression
- unchanged / inconclusive

## 12.6 Policy Simulator

Simulator 输出必须解释“为什么”，而不仅显示 ALLOW / ASK / DENY 三个字。

---

# 13. Notifications Product Requirements

Windows notifications 至少需要满足：

- 可由用户识别来自 ChatGPT-Harness
- 点击有明确 deep-link / navigation target
- 请求已过期时不会跳到不存在的伪状态
- 相同状态不持续重复通知
- 不包含 secret
- 支持 Approval 与 Disconnect 两类不同语义

用户可以分别开启/关闭审批通知与断连通知；新安装默认启用应用通知偏好，但遵守 Windows 系统权限。关闭通知不改变 Runtime Pending Approval、Tray 数量或实时连接状态。

---

# 14. Configuration and Upgrade Requirements

V4 新增设置可能包括：

- auto start Harness on app launch（新安装默认关闭）
- notification preferences（审批/断连分别配置）
- history retention / cleanup preferences（有限保留与手工清理）
- observability / estimation preferences where needed

具体 config schema 由 Architecture 决定。

升级要求：

- 现有有效 Workspace、Tunnel、Proxy、Credential reference、Security、Tools、Policies、Bash、UI preferences 不得无故丢失
- schema migration failure 必须可诊断
- migration failure 不得破坏原配置
- secrets 不得被迁移到普通明文配置

---

# 15. Reliability Requirements

V4 新能力必须满足以下可靠性目标：

## Background lifecycle

- close window 不终止 Harness
- explicit Quit 能完整 cleanup
- tray state 与真实 Harness state 收敛

## Notifications

- duplicate suppression
- stale approval handling
- no notification storm

## History

- app / Runtime restart 后已提交历史仍可读取
- database failure 有明确降级 /错误语义
- history failure 不得隐式改变 authorization decision

## Trace / Metrics

- telemetry failure 不得把本来可安全执行的 Tool request错误判成 ALLOW
- 也不得静默吞掉关键 security failure evidence
- exact failure strategy 由 Architecture 冻结

## Evaluation

- 同一 dataset / configuration 下可重复
- baseline 与 candidate 有明确 identity
- 失败任务不会因 tool transport success 被误记为成功

---

# 16. Security Requirements

V4 必须继续满足：

- Workspace boundary remains authoritative
- Policy remains fail-closed for security-relevant invalid state
- Approval cannot bypass System Guardrails
- stale disabled tools cannot execute
- secrets remain outside normal history / trace payloads
- destructive tools have truthful semantics
- Policy Simulator has no execution side effect
- Historical approvals do not restore authority
- Metrics / Token estimates do not require persisting unrestricted file contents

---

# 17. Performance Requirements

V4 不凭空冻结绝对毫秒目标；但 F05/F12 的样本方案、目标阈值和质量阻断条件必须在对应 Milestone SPEC 开发前冻结，最终验收不得临时豁免。

但产品必须支持以下评估方式：

- measure Start → Ready
- measure per startup phase
- compare before / after optimization
- measure Tool execution latency
- measure Approval wait separately
- compute P95 from clearly defined sample set

正式性能 threshold 在对应 Milestone SPEC 中基于 baseline 冻结。

---

# 18. Compatibility Requirements

## Platform

V4 继续 Windows-first。

除非 Roadmap 后续明确扩大范围，V4 不承诺：

- macOS official distribution
- Linux official distribution

## MCP

V4 应保持当前真实 ChatGPT MCP 链路可用，并通过 F13 增加协议兼容性验证。

## Existing Tools

现有 Tool 行为不能为了新增 grep / file tools 被无意改变。

对已有 Tool schema 或 error semantics 的 breaking change 必须显式评审。

---

# 19. Non-Goals for V4

除非后续 PRD 修订，V4 不包含：

- Windows OS login auto-start of the entire application
- cloud-hosted history database
- multi-user remote authorization
- multi-device sync
- remote approval from phone / web
- full OS-level sandbox
- network sandbox
- generic plugin marketplace
- arbitrary user-defined MCP tool scripting
- full IDE replacement
- ChatGPT 模型内部推理链、未发出的 Tool 选择或完整跨模型 Agent Loop tracing
- 以被动 MCP telemetry 冒充受控 Agent Tool Evaluation
- exact model-provider billed-token accounting without authoritative usage data
- restoring old Pending Approval after Runtime restart
- automatically implementing every candidate file operation in F11
- replacing Runtime Policy with a separate simulator-only rule engine

---

# 20. Product Acceptance Matrix

以下 Gate 与第 3.6 / 3.7 节共同构成验收契约。所有阈值、样本、失败场景和证据链接必须在对应 Milestone SPEC / Acceptance 冻结；不使用未执行的自动化、mock UI 或进程存在冒充真实 E2E。

| Scope | Feature | Minimum verifiable acceptance |
| --- | --- | --- |
| Core/P0 | F01 Tray | 窗口关闭后真实 ChatGPT MCP 调用继续；Tray/恢复窗口/Start/Stop/Quit 状态正确，Quit 不留 owned orphan |
| Core/P0 | F03 Approval Notification | 新 ASK 通知、去重、正确 deep link、stale 显示真实终态；OS 通知不可用不丢 Pending |
| Core/P0 | F04 Disconnect Notification | 故障 episode 首次通知、持续去重、恢复与新故障区分，状态以 Runtime 为准 |
| Core/P0 | F05 Startup Performance | 冷/热 baseline、分阶段 P50/P95 与失败率可复测，满足 SPEC 预冻结目标且 Ready 语义不退化 |
| Core/P0 | F06 Diagnostics | 常见配置/Runtime/MCP/Tunnel/Proxy/Credential 故障夹具能定位与建议修复；UNKNOWN 不伪称 PASS，诊断无默认副作用 |
| Core/P0 | F07 SQLite History | 重启后可查询/分页/筛选/清理；retention/容量上限有效；数据库失败可诊断；历史不恢复 Pending/Grant |
| Core/P0 | Shared Evidence Contract | 单请求 identity、事件分类、脱敏、重试/重启、历史与实时权限边界有可验证契约 |
| Core/P0 | F10 grep | Workspace 边界、regex、路径过滤、行号、结果限制、截断与异常处理通过，Agent Evaluation 无阻断回归 |
| Core/P0 | F12 Agent Evaluation | 受控 dataset + ground truth；选择、参数、执行、任务完成独立测量；baseline/candidate 依冻结阈值判断 |
| Full/P1 | F02 Auto Start | 新安装默认关闭；开启/关闭动作可验证；不绕过 eligibility / SecretStore / security |
| Full/P1 | F08 Metrics | 调用次数、明确 success denominator、avg/P95、审批等待/执行耗时、拒绝/取消分类一致 |
| Full/P1 | F11 Tool Expansion | 候选工具有正式准入/拒绝/延期决定和安全/重叠/质量证据；已接纳实现的工具独立验收 |
| Full/P1 | F13 MCP Compatibility | 已锁定协议版本和客户端组合，协议 conformance 与产品专属语义分别有实际测试结果 |
| Full/P1 | F14 MCP Tool Request Trace | MCP 请求各 Runtime 可见阶段可关联且定位失败/延迟，未知外部阶段不虚构 |
| Optional/P2 | F09 Token Estimates | 若交付，estimated 明示、estimator version 与 truncation 可核验；否则 DEFERRED |
| Optional/P2 | F15 Policy Simulator | 若交付，真实 Policy semantics 一致且不执行、不产生 Permit/Approval；否则 DEFERRED |

**Cross-feature Real E2E**：验证实际 ChatGPT → Tunnel → Runtime MCP → Workspace / Policy / Approval → Execution → History / Trace / Statistics（后两项按实际交付范围）；并验证 Windows Hidden Window → Tray → Running Harness → Notification / Navigation。对于未覆盖阶段必须标 NOT_VERIFIED。

---

# 21. Final V4 Acceptance Boundary

最终验收先分别报告 V4 Core、V4 Full 和 Optional 各 Gate。P2 延期不妨碍 Full，但必须在发布说明中标 DEFERRED；P1 未通过时只能称 V4 Core — ACCEPTED，不能将整个 V4 称为 ACCEPTED / FROZEN。

V4 的最终验收不能只依赖：

- Unit tests
- mocked UI
- local data fixtures
- process exists
- synthetic statistics

对于涉及真实产品链路的能力，最终必须在适用范围验证：

ChatGPT → Tunnel → Runtime MCP → Tool / Policy / Approval → Execution → History / Trace / Statistics

并额外验证桌面后台体验：

Desktop App → Tray / Notification → Hidden Window → Running Harness

Evaluation 与 MCP Compatibility 属于工程质量 gate，不能被主观人工体验替代。

最终版本是否可标记为：

V4 — ACCEPTED / FROZEN

由后续 ROADMAP 中定义的各 Milestone Gate 和最终验收要求共同决定。

---

# 22. Architecture Questions Explicitly Deferred

以下问题属于 V4 Architecture，不在本 PRD 冻结具体答案：

- Tray ownership 与 Tauri plugin / native implementation 方案
- Windows notification implementation
- background window close / quit lifecycle wiring
- Auto Start Harness 的启动时序与 race handling
- Startup phase instrumentation 的具体 Trace / Span model
- SQLite owner 位于 Runtime、Tauri 或其他边界的最终决定
- SQLite schema、migration、indexes、retention implementation
- Activity / Audit / Trace / Metrics 的统一 event model
- synchronous / async persistence strategy
- request / trace / span identity 具体格式
- P95 aggregation implementation
- Token estimator algorithm / library
- grep implementation library / engine
- candidate file tools 的最终 API / Tool granularity
- Agent Evaluation runner architecture
- Evaluation dataset format
- MCP conformance tooling / version strategy
- Policy Simulator control protocol
- Simulator 如何调用真实 PolicyEngine 而不产生 ExecutionPermit
- observability failure 的 fail-open / fail-closed 边界
- V4 config schema version

Architecture 必须满足本 PRD 的产品语义。

如果 Architecture 发现某项产品要求在安全性、可靠性或平台能力上不可成立，应先修订 PRD，而不是在实现阶段静默改变行为。
