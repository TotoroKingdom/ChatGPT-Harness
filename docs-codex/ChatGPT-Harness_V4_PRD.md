# ChatGPT-Harness V4 Product Requirements Document

> Status: DRAFT / PRODUCT REQUIREMENTS  
> Baseline: current main after V3 documentation archive  
> Product input: V4 feature set F01–F15  
> Architecture: TBD — defined separately in ChatGPT-Harness_V4_ARCHITECTURE.md  
> Roadmap: TBD — defined separately in ChatGPT-Harness_V4_ROADMAP.md  
> Scope: V4 product behavior, quality targets, boundaries, and acceptance intent only

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
| P0 | V4 核心需求，优先保证完成 |
| P1 | 重要增强，应在 V4 范围内完成 |
| P2 | 可延后实现，但设计必须与 V4 基础能力兼容 |

---

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

## 3.5 Safe policy exploration

用户能够输入一个候选 Tool request，在不执行真实操作的情况下看到：

- capability 是否允许
- Workspace 检查结果
- 匹配的 Policy rules
- 最终 ALLOW / ASK / DENY
- decision reason
- 是否会要求审批

模拟结果必须来自真实 Runtime Policy semantics，而不是 UI 推测。

---

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
| F14 | 结构化调用链追踪 | Trace / Span 关联 MCP、Workspace、Policy、Approval、Execution、Activity | P1 |
| F15 | Policy Simulator | 使用真实 Policy semantics 模拟判定，不执行真实操作 | P2 |

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

默认值由最终 UX 决定，但必须允许用户关闭。

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

通知内容应提供可行动入口，例如打开 Overview / Diagnostics。

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
- PASS / WARNING / FAIL
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

## 7.4 F14 — Agent Trace

### Objective

让一次 MCP request 的关键阶段能够在一条结构化调用链中查看。

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

是否允许用户分别关闭各类通知，由后续 UX / Config 设计决定。

---

# 14. Configuration and Upgrade Requirements

V4 新增设置可能包括：

- auto start Harness on app launch
- notification preferences
- history retention / cleanup preferences
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

V4 不冻结绝对毫秒目标，直到建立真实 baseline。

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
- exact model-provider billed-token accounting without authoritative usage data
- restoring old Pending Approval after Runtime restart
- automatically implementing every candidate file operation in F11
- replacing Runtime Policy with a separate simulator-only rule engine

---

# 20. Product Acceptance Matrix

V4 最终 acceptance 至少需要覆盖以下能力。

| Area | Required acceptance intent |
| --- | --- |
| Tray | 关闭窗口后真实 Harness / MCP 继续工作；可重新打开；Quit 能 cleanup |
| Auto Start | 打开应用后按设置自动 Start；关闭设置后不自动 Start |
| Approval Notification | 新 ASK 请求产生通知；点击到正确详情；stale request 显示真实状态 |
| Disconnect Notification | 独立故障通知一次；持续故障不刷屏；恢复可提示 |
| Startup Performance | 有可重复 baseline；能定位阶段耗时；优化不降低 readiness correctness |
| Diagnostics | 能区分关键故障阶段并给 remediation |
| SQLite History | 重启后可查询历史；筛选、分页、清理有效；不会恢复旧权限 |
| Metrics | calls / success / avg / P95 / approval wait 语义正确 |
| Token Estimates | 明确标记 estimated；记录截断；不冒充真实模型 usage |
| grep | Workspace search / regex / filter / line number / limits 正确 |
| Tool Expansion | 候选 Tool 经过准入评估，新增 Tool 有安全和 Evaluation evidence |
| Agent Evaluation | baseline / candidate 可比较；四项核心指标可测 |
| MCP Compatibility | 关键 MCP discover / call / error cases 通过兼容性验证 |
| Agent Trace | 单 request 可关联关键阶段并定位失败 / 延迟 |
| Policy Simulator | 与真实 Policy semantics 一致；无执行副作用 |

---

# 21. Final V4 Acceptance Boundary

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
