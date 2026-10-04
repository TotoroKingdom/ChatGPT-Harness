# M8 SPEC — UI/UX Simplification & Visual Redesign

> Status: DRAFT FOR REVIEW / IMPLEMENTATION HANDOFF CANDIDATE  
> Date: 2026-10-04  
> Milestone: M8 — UI/UX Simplification & Visual Redesign  
> Inputs: `README.md`, `UX_AUDIT.md`, `INFORMATION_ARCHITECTURE.md`, `WIREFRAMES.md`, `WIREFRAMES_PART2.md`, `UX_FREEZE.md`, `DESIGN_SYSTEM.md`  
> Implementation: NOT STARTED / NOT AUTHORIZED BY THIS DOCUMENT

## 1. Purpose and authority

本规格定义后续 Codex 实施 M8 时的产品行为、边界与验收条件。

用户本轮明确要求生成 SPEC、PLAN、TASKS，因此提前生成这三份可评审草案。此授权不等于已确认视觉冻结、审计修订或 production UI rewrite（正式 UI 重写）。原有七份设计输入本轮不修改；不得把本文件的存在解释为原有冻结门禁已经通过。

判断当前已实现行为仍按 `AGENTS.md`：源码 / Tests / Manifests > wiki > docs-codex。本文描述目标行为，不声明功能已实现，也不声明 QA PASS。

本次只读取证快照：2026-10-04，HEAD `34204b489e825e30e14235723ba0ee3047bf958a`。此值用于追溯文档输入，不锁定后续实施版本；实施与验证必须重新确认当时的实际代码身份。

目标设计来源：

- `UX_FREEZE.md` 是既有已冻结 UX 基线。
- `DESIGN_SYSTEM.md` 是尚待确认的视觉基线。
- 本文 §4 是本次审计的补充决策及 UX CHANGE REQUEST（交互变更请求）候选，全部待确认。
- G0 通过后，本文明确的验收契约用于补齐旧线框；未覆盖部分继续遵循已确认设计输入。
- 如果本文与旧冻结记录冲突，G0 前不得静默选择本文；必须在 G0 中明确接受或修订对应决策。
- PLAN 不能修改 SPEC 产品行为；TASKS 不能用实施方便作为降低 SPEC 的理由。

审计结论为 NEEDS REVISION。本文把修订收敛为可执行候选，不把审计结论改写为 READY 或 DESIGN FROZEN。

## 2. Objective

在不改变 M0–M7 工具、安全、配置、Protocol（协议）和 Runtime（运行时）语义的前提下，形成统一的 Desktop Workspace（桌面工作空间）：

1. 每页有明确任务、位置与下一步操作。
2. 高频配置通过行内控件完成，减少打开编辑表单的次数。
3. 默认层展示当前任务必要的信息；内部证据保留并按需展开。
4. 已保存配置、当前运行事实与待重启状态始终真实且易懂。
5. 列表、详情、创建、确认和反馈使用同一套交互与视觉规则。
6. 移除无功能意义的渐变、Glow（发光）、大 Hero、卡片墙和持续阴影。

## 3. Scope and hard boundaries

### 3.1 In scope

- 稳定 Sidebar（侧栏）、page-local header（页面局部标题）及 contextual Inspector（上下文详情面板）。
- Overview、Tools、Policies、Approvals、Activity、Settings 六个工作域。
- First Run（首次配置）与 Recovery（恢复）现有流程的展示及交互重构。
- 共享视觉 token（设计变量）、控件、列表、反馈、状态、焦点与窗口适配规则。
- 纯 UI presentation adapter / selector / view-model（展示适配器 / 选择器 / 视图模型）。
- 支撑 auto-save（自动保存）的前端 mutation（变更）协调与局部反馈。
- 保留 M6 English / zh-CN / System 语言选择和 M7 evidence（证据）展示能力。
- 与上述变更有关的行为测试、回归、真实产物核验和人工 UX 验收。

### 3.2 Out of scope

- 新增工具、一级页面、多 workspace、Theme 产品能力或新字体资产。
- Runtime 热更新、自动重启、自动批准或后台创建规则。
- Policy precedence（策略优先级）、System Guardrails（系统护栏）、Bash enforcement（Bash 执行约束）与 session grant（会话授权）语义变化。
- Config schema / migration、generation / configRevision、Protocol contract、SecretStore 事务语义变化。
- 新增原始命令、文件内容、敏感参数或 API Key 的展示与导出通道。
- 新增持久审计数据库、历史全量查询、远程审计或新的自动刷新产品能力。
- M9 Full V3 External Acceptance；不得用本地请求或 mock（模拟客户端）代替 External ChatGPT 操作。

主要改动边界为 `ui/src/**`。默认不修改 `runtime/**`、`protocol/**`、`ui/src-tauri/src/**` 或其安全测试。如果既有 contract 无法表达设计，记录缺口与纯展示替代方案；不得借 M8 UI 重构改变后端权威行为。

## 4. Review decisions and G0

以下为本次审计的明确建议，状态均为 PROPOSED（待确认）。没有已获人工确认的声明。

| ID | 问题与候选决策 | 与旧输入的关系 | 影响 / 替代方案 |
| --- | --- | --- | --- |
| D01 | 全局指标表达 Harness 总体状态；Tools 用“当前运行状态 / 已保存配置”区分两类值；差异提供具体说明 | 补齐状态真实性原则，修订 `Runtime ready` 全局示例 | 不恢复 desired/effective 大表格；内部 ID 仍后置 |
| D02 | Bash rule 首层明确为命令参数前缀匹配，并说明附加参数覆盖范围 | 补齐自然命令创建流程；避免将 prefix 表达为完整命令匹配 | 保持 Runtime matcher；不新增 exact-full-command 能力 |
| D03 | Ready 保留稳定的次要 Stop；Rule Inspector Decision 只读，完整编辑显式 Save；按钮权重按上下文 | 解决 Overview 示例与 Rule Inspector 控件歧义 | 保留“一任务一主要操作”；不把所有 Rule 字段改成 auto-save |
| D04 | 重要风险、预览不完整、执行上下文和会话授权范围按 §10 前置；补齐过期、取消、失联与 stale 状态 | 对默认 evidence 折叠增加决策必要信息例外 | 原始 evidence 仍折叠；不新增每次 Allow/Deny 确认框 |
| D05 | 显式编辑统一 dirty（未保存）、saving、failed、leave（离开）行为 | 补齐 Inspector 切换、关闭、导航和 Connection Cancel 契约 | 不给只读浏览与已保存 auto-save 增加确认 |
| D06 | Connection 的配置草稿与凭据删除分开；不承诺 SecretStore 与 config 跨存储整体回滚 | 澄清旧线框“atomic edit”的实际能力边界 | 保留 explicit Apply；不修改后端事务或 SecretStore contract |
| D07 | 采用 §13 的窗口、长内容、控件状态和可访问性规则；Language 使用 Select | 补齐视觉稿；统一 Language 的两种控件示例 | 不扩展移动端、不新增 Theme |

G0 是后续实施的必要门禁：

- [ ] 人工确认 `DESIGN_SYSTEM.md` 的视觉方向及本文 D01–D07；涉及冻结 UX 的决策明确记录为 UX CHANGE REQUEST。
- [ ] 明确记录 DESIGN FROZEN / SPEC APPROVED，而不是依据文件生成日期推断。
- [ ] 用户明确授权后续实施，或同一条明确授权同时包含接受这些修订并开始实施。
- [ ] 后续 Agent 核对新的用户指令是否已满足上述授权，不重复索取已给出的批准。

当前所有门禁项均未在本文件中标记完成。后续若接受了不同方案，应先同步本文、PLAN、TASKS，再进入代码修改。

## 5. Product invariants

### 5.1 Authority and state truthfulness

- 已保存配置来自成功的 config response / refresh；运行事实来自 Runtime / Harness authoritative observation（权威观测）。
- 不从开关、默认策略、存在 PID、输入路径或上次成功提示推断 Runtime 已生效。
- 不在前端重新评估策略、批准资格、Bash 风险或系统护栏。
- Disabled、Unavailable、Denied、Ask/Pending、Execution failed 保持不同语义。
- Runtime 停止、数据加载、读取失败、旧 generation 的数据不能显示为当前 Available 或 Applied。
- `Restart required` 的权威判断继续使用既有生效模型；语言切换不得制造重启需求。
- 修复按钮只发起显式操作或导航，不因为进入详情、切页或选择对象隐式保存 / 重启 / 批准。

### 5.2 Evidence and privacy

- M7 已有 sanitized evidence（脱敏证据）、IDs、reason、duration、result summary 保留在 Detail / Advanced。
- 只展示既有安全 contract 提供的字段；不得额外读文件、读取 secret 或还原被脱敏命令。
- 缺失字段明确为未报告；空值不能被编造成“无风险”“成功”或“未匹配规则”。
- API Key 仅短暂存在于受控密码输入与现有受控保存调用，不能进入 router context、日志、Toast、测试快照或浏览器持久存储。

### 5.3 Mutation coordination

所有全量 configSave 调用必须按“用户意图的局部修改”组织，避免从长期保存的整页副本覆盖其他成功修改。

- 一个共享前端协调入口序列化 config mutation、Language mutation 与 Reset；操作队列保留不同对象的用户意图。
- 各次 mutation 在执行时基于最新确认的 persisted config，仅合并本操作负责的字段。
- UI 可以仅锁定当前控件，但不能并发提交可能互相覆盖的 config 快照。
- Tools 与 Policies 对同一 Tool default 使用同一保存源；切页不产生第二份 draft。
- Rule Save / Connection Apply 只合并自己对象或 section，不能覆盖后来保存的 Language / Workspace / Tool。
- 对 UI 已观察到的同一对象外部变化，dirty editor 进入 conflict（冲突）状态；保留草稿，禁止盲目 Save，提供 Reload / Cancel。
- 不声称前端队列提供跨进程事务或 compare-and-swap（比较并交换）保证。未观察到的外部写入仍受既有后端 contract 限制。
- Retry 必须重新基于最新配置执行，不能重放失败的整份旧 config。
- 保存成功但后续 refresh 失败时显示“已保存，状态暂未刷新”；不能把已确认持久化的修改回滚成未保存或重复提交。

## 6. Shared interaction contracts

### 6.1 Navigation and shell

导航顺序：Overview → Tools → Policies → Approvals → Activity；Settings 位于 System 组。

- Page Header 含标题、简短任务说明、必要的当前任务操作；不保留大尺寸全局 Topbar。
- 侧栏选中项只有一套清楚的 selected treatment（选中样式）。
- Approvals pending count 只来自最近确认的 pending 数据；未读取时不编造 `0`。
- 全局 Harness 状态可点击进入 Overview，不新增 status popover。
- `Harness ready · Restart required` 可以表达仍运行但配置待应用；错误或 Critical 状态不能被待重启提示覆盖。
- 全局观测不可读时显示 Unknown / Unavailable（未知 / 无法读取），不保留无时效提示的 Ready。
- Overview 主 attention 优先：Critical → system blocker → restart action → 非阻断提醒。Approval pending count 独立保留；一个提醒不得抹掉其他必要状态。

### 6.2 Inspector and Sheet

- row 非控件区域 / 专用详情触发器打开 Inspector；Switch、分段选择与菜单不触发 row navigation。
- 浏览对象时选中另一行原位切换，关闭后恢复滚动与触发位置焦点。
- row 与内置控件分别有合理键盘入口，不使用嵌套 button 或把整行包装成含控件的 button。
- 宽窗口 Inspector 非 modal（非模态）；中等窗口 overlay Inspector 按模态管理焦点及背景交互。
- Create Rule 使用 focused Sheet；Rule Edit 使用 Inspector edit mode；确认使用 Dialog。
- Sheet 的 mask（遮罩）点击不直接关闭；Close / Esc / navigation 统一经过 draft leave guard（草稿离开保护）。
- overlay 同时只处理一个主要编辑上下文；确认 Dialog 可临时覆盖编辑器并正确返回焦点。

### 6.3 Saving and leaving

| 类型 | 状态与行为 |
| --- | --- |
| 独立 auto-save | persisted → saving → saved / restart-required；失败恢复 persisted 值并显示局部错误 |
| 完整对象或 Connection draft | viewing → editing-clean → dirty → saving → saved；失败保留非敏感 draft 与字段错误 |
| dirty editor 离开 | Stay editing / Discard changes；不做隐式 Save，确认后才完成原始离开意图 |
| saving 时离开 | 暂停关闭 / 切对象 / 导航并就地说明正在保存；后台观测继续更新 |
| clean viewer 离开 | 直接离开，不确认 |
| 选择文件/目录后取消 picker | 无 mutation，无保存成功提示 |
| Delete / Reset / credential removal | consequence confirmation → in-flight → authoritative result；失败不声称完成 |

离开路径包括选另一对象、关闭按钮、Esc、侧栏导航与上下文跳转。M8 不新增后端进程关闭拦截能力；应用整体退出不保证恢复未保存草稿，界面不承诺跨退出 draft persistence（草稿持久化）。

已保存 auto-save 不受 Cancel 撤销。保存中阻止重复操作；配置写入串行不意味着整页必须变灰。危险操作执行前，先处理 dirty editor，避免 Reset 与保存并行。

## 7. Overview contract

主要问题：Harness 是否能工作；如果不能，下一步做什么。

| 状态 | 默认内容 | 操作 |
| --- | --- | --- |
| Stopped 且可启动 | 总体 Stopped、readiness 摘要 | Start Harness 为 Primary |
| Stopped 且不可启动 | 用户可理解 blocker、相关 readiness | Fix relevant settings 为 Primary；不只留下无解释的 disabled Start |
| Starting / Stopping | 明确进行中的 lifecycle | 受当前生命周期约束的按钮 disabled；不显示成功 |
| Ready 无 mismatch | Harness ready、Runtime/Tunnel/MCP、简短 recent activity | Stop Harness 为稳定 Secondary；可没有 Primary |
| Ready 且待重启 | 当前仍运行、保存配置未应用、已知 affected areas | Restart Harness 为 Primary；Stop 仍为 Secondary |
| Error / Degraded | 最重要原因、现有 remediation、组件状态 | 当前最合适修复为 Primary；存在可停止的 managed session / component 时保留 Stop |
| 数据不可读 | 最近事实的 stale 标识 / 未知状态 | Refresh / Retry；不推断可以启动或已连接 |

- Start/Stop/Restart 使用现有 Harness 生命周期入口，不能改成页面自行启动 Runtime/Tunnel。
- 操作接受、操作完成、最终 Ready 分开；最终状态来自生命周期刷新。
- Component row 打开 Inspector；route/proxy/IDs/technical errors 进入 Advanced。
- recent activity 是最近可读事实的小摘要，不新增数据能力；不可读时不写 No activity。
- affected areas 仅使用能证明的变化；不从 revision 差异虚构 `2 tool changes` 等数量。

## 8. Tools contract

默认固定少量 built-in Tools 的 compact rows（紧凑行），保留身份、当前运行状态、已保存 Enabled、已保存 Default policy、详情入口。

- `Current Runtime / 当前运行` 列不与 `Saved configuration / 已保存配置` 混用；配置列可由一个组标题统一说明。
- 每行 Enabled Switch 与 Allow/Ask/Deny segmented control（分段单选）直接 auto-save。
- 运行事实依 authoritative availability 显示 Available、Requires approval、Disabled、Unavailable、Denied 等对应状态，不把模式不可用都叫 Denied。
- Runtime stopped / checking / unreadable / not reported 明确显示；已加载 persisted config 仍可按既有约束编辑。
- OFF 已保存而运行仍可用：显示“关闭已保存，重启前仍可用”。反向或 Policy 差异同样使用具体说明。
- 精确字段对比仅在有效 Runtime observation 可证明时显示。只能证明 revision mismatch 时，说明“配置已保存，当前运行值待确认”，不得猜测 Running now 值。
- System capability / 模式约束导致不可用时，Enabled 的交互遵循现有允许保存的边界；不能通过 UI 强行启用被 Runtime 限制的 Bash。
- 已禁用工具的 configured default policy 仍是保存配置，允许修改不代表当前能执行。
- Inspector 中的 Enabled/Default 与 row 共用同一数据源；不存在第二份 auto-save draft。
- Bash Inspector 保留 shell、Auto detect、manual override 的常用配置；高级诊断后置。
- Bash override picker 返回后提交对应局部意图；保存路径不显示“Runtime validated”。验证结果按下一代 Runtime observation 展示。
- 错误紧邻发生位置；不会让单行错误或 Saved 变成全页彩色 banner。

## 9. Policies contract

结构：Tool defaults → User rules → collapsed System guardrails → Diagnostics / Effective policy。

### 9.1 Defaults and existing rules

- Tool defaults 与 Tools 使用同一 configured source 和 auto-save 契约。
- User Rule 列表显示 name、matcher 范围摘要、decision、configured Enabled、必要的待重启提示。
- Enabled Switch auto-save；Rule Inspector 中 Decision 是只读摘要，不画成可操作 segmented。
- Edit rule 才进入 name / matcher / decision / reason 的完整 draft，以 Save 原子提交此 Rule 对象。
- 编辑期间同一 Rule 的 auto-save enabled 控件暂停，并说明正在编辑；其他对象仍可排队修改。
- 原有 internal ID 不变；新规则 ID 自动管理，失败 Retry 不重复创建规则；不覆盖同名不同 ID 的规则。
- Delete 单独放置并确认后提交目标 ID；不凭位置删除，不改变 system-owned rule。
- 用户没有自定义 rule 时说明 Tool defaults 与 System guardrails 仍适用。
- System guardrails 只读，可展开人类说明；invalid snapshot / fail-closed 立即提升为 Critical，并保留 remediation。

### 9.2 Create / edit fields

1. 选择 Entire tool（整个工具）或 Bash command prefix（Bash 命令参数前缀）。
2. 选择 Tool 或输入自然 command。
3. 选择 Allow / Ask / Deny；新建草稿初始为 Ask，不隐式保存。
4. name / reason 提供可编辑安全默认值；用户可不手写，但提交值必须满足现有非空 contract。
5. Create / Save 前显示作用范围，成功后显示 Saved 或 Restart required。

默认名称、reason 是用户配置元数据，创建后保持原值；切换语言不重写既有规则。不可因为 DTO 字段存在就默认要求用户填写 ID 或 JSON。

### 9.3 Bash matcher scope and natural input

前缀指 argv 的 token prefix（参数 token 前缀），不是任意字符串前缀，也不是完整命令相等：

- executable identity 使用既有 normalized executable 语义。
- `git status` 可覆盖 `git status --short`，不匹配 `git statusx` 或 `git stash`。
- `npm test` 摘要明确说明可以包含附加参数。
- 参数前缀为空时明确“该 executable 的所有参数”；不能描述为“只匹配无参数命令”。
- 只解释 matcher 范围，不能承诺 Rule 的 Allow 一定压过其他规则、护栏或风险约束。

M8 自然输入仅支持确定性单命令的保守子集：普通空白分隔的字面 token，不支持引号、转义、变量展开、glob、重定向、管道、复合命令、命令替换、换行或环境赋值。候选 token 字符仅为字母/数字及 `_ - . / : @ + % ,`；不满足者拒绝并进入 Advanced literal-token editor（字面参数编辑器）。这项子集是创建体验约束，不是 Runtime 执行语法更改。

- 显示转换后的 executable 和每个参数 token，以及 matched / not-matched 说明后再让用户 Create。
- Advanced 使用独立参数项，不要求 JSON；可准确表示带空白和空字符串的 token。
- 限制复用当前 contract：最多 64 个 argvPrefix token，每 token 最多 1024 UTF-8 bytes，不允许 NUL；executable 验证和 normalization 与现有 protocol 一致。
- 不能把无法解析的命令降级为 entire-tool rule；不能静默删除不支持的部分。
- 编辑既有非自然子集 matcher 时进入可无损表达的 Advanced editor，不以拆空格重建原值。
- matcher preview 是配置解释，不在前端模拟 authoritative policy decision。

### 9.4 Approval handoff

- 创建草稿带 source request context；用户必须显式 Create。
- 仅在既有 display-safe evidence 足以无歧义构建 matcher 时预填。
- 脱敏、截断、unsupported 或多段命令不能静默选第一段生成规则，也不能自动升级为 entire-tool Allow。
- 无法安全预填时解释限制，要求用户显式选择作用范围；旧安全边界继续成立。
- Create 不批准当前请求，不创建 session grant。
- Create / Cancel 后提供返回原 Approval context 的路径，重新读取其最新状态；过期也不能伪装 pending。

## 10. Approvals contract

Approvals 是 decision-first queue（决策优先队列）。Pending 优先，History 同页次要区域。

### 10.1 Information before a decision

默认每个请求先显示 Tool、安全 target / command preview、为什么需审批、风险级别、有效决定。

必须前置的条件信息：

- `previewRedacted / previewTruncated / argvRedacted` 存在时，用短文明确预览不完整；不还原 secret。
- unclassified、高风险或任何已报告 risk flag，用人类说明表达重要风险，不仅显示 High / Medium 标签。
- Bash 工作目录按 contract 的 `active_workspace` 表达为“当前运行 workspace”。若当前有效物理路径不可证明，只显示该语义，不使用已保存新 workspace 冒充。
- expiresAt 显示简短有效期；临近或到达截止时间明显提示，时间格式不压过决策内容。
- 缺少风险或 Runtime 状态时显示未报告 / 待确认，不能写 No elevated flags。

原始 sanitized arguments、normalized segments、IDs、exact matched IDs 等仍进入 Advanced。Raw 字段展开不应成为每个低风险请求的必需操作。

### 10.2 Decision controls and reuse scope

- 每个 request 的 Allow once 为其 Primary，Allow for session 为 Secondary，Deny 为清楚的限制性操作。队列可有多个“每请求 Primary”；Page Header 不重复放 Allow all 等新功能。
- Deny 不等于删除配置；不要复用 Delete / Reset 的后果文案。
- 普通 Allow once / Deny 不增加第二次确认。Create rule 为 Quiet / More secondary workflow。
- Bash session eligibility 完全采用 Runtime 字段；UI 不以预览脱敏或自己的风险猜测替代 eligibility。
- 会话说明为“仅在当前 Runtime session 内复用相同工具和 Runtime 认定的相同作用范围；重启后失效”。
- file-tool scope 使用已报告逻辑路径/模式摘要；Bash exact normalized scope 保持私有。脱敏预览不能被描述为完整的 grant key。
- 不声称只批准相同文件内容；现有 write session scope 是工具与路径范围，不是内容 fingerprint。
- session 不可用时显示 Runtime 原因。多一层 Why 可补充说明，但不能只留下无解释 disabled 按钮。

### 10.3 Queue lifecycle

| 状态 | 展示与操作 |
| --- | --- |
| pending/current | 可以按 authoritative eligibility 决策 |
| resolving | 锁定此 request 的决策入口，显示 Resolving，阻止重复请求 |
| approved | 移入 History，局部“决定已提交”；不能声称工具执行成功 |
| denied / expired / cancelled | 终态只读，保留解释与历史 evidence，移出 actionable Pending |
| generation changed | 旧 request 标为 stale，不继续操作；刷新 authoritative queue |
| disconnected/read failure | 保留已有内容并显示 stale，不显示 No pending；暂停有效性无法确认的决定，提供 Refresh |
| resolution failed | 显示错误并刷新该请求；不得显示批准成功，亦不得自动重复授权 |

- 页面进入、显式 Refresh、决策结果、已知 session 变化后刷新；不新增周期轮询或新的 subscription protocol。
- 本地 expiresAt 计时只能触发“可能过期 / 检查状态”和一次刷新，不能替 Runtime 写终态。
- 决策时核对当前 session 与最新可读 request；Runtime resolve 的实际验证仍是最终权威。即使预检查通过，过期竞态失败也按失败处理。
- 一次 mutation 失败不让其他已确认有效 request 失去局部反馈。

## 11. Activity contract

- feed 使用 time、tool、安全 target/command、decision + outcome、适用 duration；不恢复默认六列审计表。
- 明确区分 Policy denied、Approval denied、Execution failed、Tool unavailable、Pending 与 Cancelled，不把它们都归为 Error。
- 第一层详情先回答发生什么、为什么、结果、可用 remediation；Advanced 保留全部已有 evidence。
- 支持 All / Success / Needs attention 与 Tool filter，作用于当前已加载 snapshot，并明确其范围。
- 保持现有 bounded snapshot 能力；不新增搜索数据库、无限滚动或全量历史承诺。
- Refresh 是低权重显式操作，已有内容刷新时保留并标识 refreshing；失败保留旧内容并标识 stale。
- eventId 是对象身份；旧数据无 ID 时只使用 snapshot-local 身份，不能跨 refresh 误开另一条记录。
- current Rule / Approval link 仅在对象仍可定位时启用；历史 snapshot evidence 不替换成当前配置。
- link 到当前 Rule 时若其内容已变化，说明当前配置与历史记录的区别。导航不可修改原事件。
- 无执行证据不能填 duration `0` 或推断 Success；非工具 lifecycle event 仍可阅读，不为其编造 command。

## 12. Settings, First Run and Recovery

### 12.1 Settings matrix

| 项 | 控件 / 保存 | 生效与反馈 |
| --- | --- | --- |
| Language | Select: System / English / 简体中文；独立 auto-save | 立即 UI 应用，失败回滚 UI 与 preference；不重启 Runtime |
| Workspace | Change → existing folder picker → 局部保存 | 取消不写；保存后显示 configured root，运行 root 单独说明 |
| Connection config | Configure 内 Tunnel ID / proxy / executable draft，Apply / Cancel | 保存只更新对应 section；Connected 来自 Harness/Tunnel observation |
| API Key replacement | Replace 只打开密码输入，随显式 Apply 提交受控 secret | Cancel before Apply 清除内存输入；不能承诺 Apply 失败一定回滚 secret |
| API Key removal | 独立立即 destructive action + confirmation，不在 dirty Connection editor 内执行 | 说明不会被后续 Cancel 恢复；完成后刷新 status/eligibility |
| Security | Normal / Developer-Test option group | 启用 Developer/Test 要后果确认；save 后按权威状态提示重启；返回 Normal 显式保存但无需额外风险确认 |
| Advanced | diagnostics / config location / secret provider | 只读，避免泄露原始 secret |
| Reset | 独立 Danger Zone，明确损失后确认 | 重置 workspace / rules / local credential 等现有行为；不承诺备份或恢复未实现的数据 |

Connection 的“原子编辑”只表示同一配置 section 不逐字段提交，不表示 ConfigStore 与 SecretStore 是一个事务。

- Apply 前依既有规则验证字段；只调用现有受控保存入口。
- 替换 secret 后配置写盘失败可能存在部分结果；错误后重新读取配置与 credential status，以“保存未完整确认，请检查连接”表达。
- 仅有 status boolean 不能证明旧 secret 未变，也不能证明新 secret 生效；不能显示“全部回滚成功”。
- 原始 API Key 提交完成或失败后均清除内存输入；失败重试需要重新输入，保留非敏感字段草稿。
- 删除 credential 需 clean editor / 先离开 draft，再独立确认和删除。既有运行 session 是否仍连接与下次启动资格分开，不能从删除推断已停止。
- API Key Configured 与 Tunnel Connected 不混用；Configured 不保证凭据有效。
- auto-detect 与 override 的互斥关系和路径规范复用现有 contract，不更改检测能力。

### 12.2 First Run

流程：Workspace → Connection → Readiness → Start Harness。

- 不复制完整 Settings。默认不展示 schema、revision、policy editor、proxy 或 executable diagnostics。
- Workspace picker 确认后的保存语义与 Settings 一致；Back 不撤销已经成功保存的配置。
- Connection Save & continue 显式提交；失败保留非敏感输入并清除 API Key。
- Readiness 显示“配置已准备，可启动”，不能在启动前宣称 Tunnel 已连接或 Harness Ready。
- Start 复用 Harness lifecycle，只有最终 observation Ready 才完成启动展示。
- 失败直接提供相关配置修复路径，保留已保存值；不让用户重做全部步骤。
- 已有可用配置不重复 setup；invalid / migration-failed 进入 Recovery，而不是视为全新安装。

### 12.3 Recovery

- 先说明无法安全读取/使用配置、已有数据的已知安全状况、可执行的下一步。
- 仅在既有错误 contract 能证明未写入时说明原配置未覆盖；未知错误不能泛称数据完全安全。
- Review problem / Technical details 保留 code、schema、config path 与现有 remediation。
- Reset 与普通审查物理分离，明确配置与凭据损失，并确认。
- 不新增自动修复、backup、schema downgrade 或 secret 明文化路径。

## 13. Visual, layout and accessibility acceptance

视觉基线沿用待 G0 确认的 `DESIGN_SYSTEM.md`：dark-first、灰阶、semantic color、system UI font + mono、4px spacing、6–10px radius、thin border、32px controls、compact rows。

- Page title 24/32、正文 13/20；11px metadata 只用于辅助信息。状态、风险后果、错误与决定按钮不能降为最弱 metadata。
- 全产品共享 semantic token / Button / Row / Switch / Segmented / Inspector / Sheet / Feedback，禁止页面自行复制等价样式。
- Primary 高对比灰阶，Danger 只用于正确语义；Restart 在重启任务中是 Primary，其他上下文可为 Secondary。
- Hover / Selected / Focus / Disabled / Saving / Error / Attention 可区分；Selected 不能只在 Hover 时出现。
- 不使用装饰 3D、gradient、Glow、pulse、默认 Card shadow 或巨大背景插画。
- 普通文本对比度目标至少 4.5:1，必要控件边界及状态指示至少 3:1；可读性不能靠持续 hover 提示补救。
- Switch track 约 28×16 不代表点击目标同样小；实际 focus/click target 至少 32×32，不侵入相邻控件。
- Segmented 为单选组，方向键改变 focus / selection；只对实际值变化提交一次，Tab 不触发保存，saving 时拒绝重复 mutation。
- 错误关联字段；局部成功用适度 aria-live，不能每次普通 refresh 都重复播报整页。
- modal overlay trap focus、Close / Esc 可达；非 modal Inspector 不锁死 Main。关闭后恢复触发位置；触发对象消失时落到合理相邻对象 / 页面标题。
- motion 仅用于状态与空间变化，尊重 reduced-motion；禁用动画后信息与操作不丢失。

### Window rules

M8 保留现有 desktop minimum 980×680；基准 1280×820。不为小于既有 minimum 的窗口新增移动端能力。

- Sidebar 目标 220px，page horizontal padding 28–32px，Inspector 目标 360px。
- 只有扣除侧栏、padding、Inspector 后 Main 可用宽度仍不少于 600px 时并排；不足时 Inspector overlay。1280px 基准可并排，980px 最小窗口使用 overlay。
- Row 的关键身份、状态、Switch、segmented 与错误提示在 minimum 下可见；需要时排列成受控的两行，不移除直接操作能力。
- Sheet 420–520px 且不越出 viewport；minimum 高度下内容可滚动，主要保存操作始终可达。
- Main 与 Inspector 各自管理必要滚动；避免为每个 section 再增加滚动区。
- 长路径使用受控摘要并提供完整值查看；工具名、状态与按钮不能被路径挤出。
- Approval 中会影响决策的长命令可换行 / 主动展开；UI 自身截断也必须标注，不能只靠 tooltip。
- raw evidence 使用局部换行或滚动，不撑宽整页；多行错误不覆盖邻行控件。
- English / zh-CN 与系统缩放下无按钮遮挡、文字裁切或不可达操作。宽度微调只能在上述功能验收成立的前提下进行。

## 14. Acceptance matrix

以下全部为计划验证，当前均 NOT RUN。Unit / component tests 可以使用 fixtures，但不能声明真实 Runtime、Portable 或 External Acceptance PASS。

| ID | 验收目标 | 必要证据 |
| --- | --- | --- |
| AC01 | 六导航、页面任务、全局 Harness 与组件状态清楚 | Shell 行为测试 + 人工路径检查 |
| AC02 | 配置与当前运行区别真实；Unknown / stale 不假 Ready | state projection 测试 + 当前产物真实 observation |
| AC03 | Tool / default / Rule toggle 局部保存、失败回滚、不覆盖其他成功修改 | 并发交错 mutation 测试 + 配置读回 |
| AC04 | Ready 可 Stop，待重启可 Restart；错误仍有 remediation | lifecycle UI 测试 + 当前包 Start/Stop/Restart |
| AC05 | Rule prefix 范围明确；自然子集拒绝歧义；高级编辑无损 | matcher 转换与既有规则 round-trip 测试 + 人工说明检查 |
| AC06 | Rule create/edit/delete、ID 稳定、无隐式批准 | page integration tests + 配置读回 |
| AC07 | dirty/saving/conflict 下关闭、切对象、切页行为一致 | 各离开路径行为测试 |
| AC08 | Approval 风险/不完整预览/范围前置，终态与 stale 不可继续决策 | queue/state 测试 + 真实 Runtime approval 证据 |
| AC09 | Allow once/session/Deny 反馈真实，grant eligibility 不重算 | 原有安全回归 + 真实 resolve / Activity evidence |
| AC10 | Activity feed / filters / refresh / links 保留全部已有证据 | fixture projection 测试 + 当前产物读取 |
| AC11 | Connection Apply/Cancel、凭据删除和部分失败表达准确 | 控制边界 failure injection tests；不得记录 secret |
| AC12 | M6 Language 行为与 M7 recovery/evidence 不回退 | 相关既有回归 + 双语言检查 |
| AC13 | minimum/base/宽窗口、长内容、焦点、reduced-motion 成立 | 人工 UX 检查；可用的组件与静态检查辅助 |
| AC14 | 无未授权后端/协议变更，无旧 Card/table/form-first 回退 | diff scope review + 完整适当回归 |
| AC15 | 最新 M8 独立 Portable 与被测源码一致 | source/build provenance、manifest、SHA-256、真实 UI 启动记录 |

核心交互预算：查看主要状态 0 额外操作；普通 toggle/default 1 次直接交互；Allow/Deny 1 次决定；普通详情 1 次；Advanced evidence 不超过 2 次；复杂 Rule 编辑与危险确认按语义保留必要步骤。不能以少一次点击为理由隐藏风险或隐式提交。

## 15. Delivery gates

1. G0：审计修订、视觉基线与实施授权明确确认。
2. Development Complete：TASKS T1–T9 完成，相关实现验证通过；不声明人工接受。
3. Automated QA PASS：完整本次适当回归与可自动执行的本地真实验证有证据；明确覆盖与未跑项。
4. Portable：基于最新被验证代码重建、验证、复制至独立 `artifacts/M8-UI-UX-Redesign/` build 目录，确认源代码与产物身份。
5. Manual UX Acceptance：人工使用对应最新 M8 包检查 §14；Computer Use 未获明确授权时由人执行，不模拟人工结果。
6. ACCEPTED / FROZEN：仅在人工接受后记录。M9 不随 M8 文档或实现自动启动。

本轮只交付三份规划文档，不修改原设计输入、不写生产代码、不执行 QA、不构建或启动产品、不标记上述门禁已通过。
