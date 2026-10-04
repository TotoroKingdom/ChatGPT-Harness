# M8 TASKS — UI/UX Simplification & Visual Redesign

> Status: DRAFT FOR REVIEW / ALL TASKS NOT STARTED  
> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Date: 2026-10-04  
> Scope: UI implementation planning; 本轮仅生成文档，不执行任何 Task。

## Execution contract

先通读 SPEC，检查 G0，再开始修改 production UI。文件存在不代表实施已授权。用户后续明确接受这些文档与修订并授权实施时，可以满足 G0 的相关确认，不重复请求已经给出的批准。

- 所有复选框保持未完成，直到有实际证据。
- 实现限定 `ui/src/**` 与相关测试；禁止顺手修改 Runtime / Protocol / SecretStore 安全语义。
- 每项 Task 提供改动文件、覆盖 SPEC AC、实际验证命令/结果、未覆盖项与剩余风险。
- 测试优先覆盖状态、mutation、导航、保存和失败行为；不为每个 spacing/radius 编写镜像测试。
- mock/fixtures 只用于自动行为验证，不能冒充 Runtime、Portable 或人工验收。
- 主 Agent 负责共享关键路径及整合；委派时明确 ownership，不并行修改共享文件，不覆盖其他工作区修改。
- 不新增未经授权的 Computer Use、插件、External ChatGPT 操作或 M9 工作。

## Dependencies

```text
T0 → T1 → T2 → T3 → T4 → T5 → T6
                         └────────→ T7
                         └────────→ T8
T4 + T5 + T6 + T7 + T8 → T9
T9 → Q1 → P1 → H1 → F1
```

T0 为人工/产品确认门禁；T1–T9 为开发任务。Q1 是独立 QA；P1 是 Portable 交付阶段编号，不是审计严重度；H1 是人工验收；F1 是最终冻结。阶段不得混写完成状态。

## T0 — Design approval and scope gate

**Status:** NOT STARTED / NEEDS HUMAN CONFIRMATION  
**Ownership:** 主 Agent 汇总；确认来自用户，不由 Agent 自行批准。

- [ ] 核对 `UX_FREEZE.md`、`DESIGN_SYSTEM.md` 和 SPEC §4 D01–D07。
- [ ] 明确接受或修订每个候选决策，涉及冻结 UX 的项记录为 UX CHANGE REQUEST。
- [ ] 记录 DESIGN FROZEN / SPEC APPROVED / implementation authorization 的实际依据。
- [ ] 候选决策变化时同步三份实施文档；未通过前不修改生产 UI。

**Exit:** 后续用户授权与设计确认真实存在；不是由 Task checkbox 自证通过。

## T1 — Baseline inventory and verification mapping

**Depends on:** T0  
**Ownership:** 当前 source / contract / tests 只读清点和验证设计；不改后端。

- [ ] 核对 HEAD、工作区已有修改、AGENTS、当前 build/test scripts；登记已有修改，不覆盖。
- [ ] 清点六页面、First Run、Recovery、共享组件、i18n、Provider/router 与客户端接口。
- [ ] 记录 configured/effective、restart、availability、policy、approval、activity、credential 的实际来源。
- [ ] 将 SPEC AC01–AC15 映射到现有回归、新增行为测试、本地真实检查与人工检查。
- [ ] 明确 UI/API contract 缺口；不得使用 wiki 或旧 milestone PLAN 代替当前源码事实。
- [ ] 准备后续 integration / local black-box / manual checklist 的覆盖安排；不提前生成 PASS。

**Verify:** 清点引用可定位，现有接口与目标行为相符。  
**Exit:** 没有关键权威事实依赖前端猜测；发现缺口有明确处理边界。

## T2 — Shared state, mutation and navigation contracts

**Depends on:** T1  
**Primary files:** `ui/src/app/providers/HarnessDataProvider.tsx`, `ui/src/app/router/*`, `ui/src/services/harness/*`, `ui/src/i18n/presentation.ts`；可新增职责明确的 UI helper modules。  
**Ownership:** 主 Agent 持有或指定单一负责人；不并行修改这些共享模块。

- [ ] 构建 Harness / component / tool / restart / stale 的 display-only（仅展示）投影。
- [ ] 所有 full configSave 接共享 mutation coordinator；以局部用户意图在执行时合并最新 persisted config。
- [ ] Language、Reset、Tool/default、Rule Save、Connection Apply 串行协调，保留现有 Language 专用命令与行为。
- [ ] 区分 persistence success 与 refresh failure；Retry 不重放整份 stale config。
- [ ] 建立 dirty/saving/conflict 与导航 leave guard；不把只读关闭变成确认流程。
- [ ] 建立 selected-object、Settings section、Approval return context；不保存 secret 于 router state。
- [ ] generation / session 改变使旧 observation 失效；不重新评估 policy 或 approval eligibility。

**Meaningful verification:**

- [ ] 两个控件快速修改后两个值都保留；第一笔失败不覆盖第二笔成功意图。
- [ ] dirty Rule/Connection 与成功 Language/Tool/Workspace 交错，提交不回写旧值。
- [ ] Language 失败回滚且不引起 restart；Reset 与保存不并行。
- [ ] persistence 已确认后 refresh 失败不会变回未保存值或重复执行。
- [ ] 已观察到同对象变化时保留草稿并阻止盲写。
- [ ] dirty 的 Esc/Close/row switch/nav 处理一致，saving 阻止离开并有解释。

**Acceptance:** AC02、AC03、AC07、AC12。  
**Exit:** 页面不再自行从长期 draft 组装整份 config。

## T3 — Shared design system, shell and accessible surfaces

**Depends on:** T2  
**Primary files:** `ui/src/styles/global.css`, `ui/src/components/common/*`, `ui/src/components/layout/*`, `ui/src/components/status/*`, `ui/src/App.tsx`。  
**Ownership:** 一位组件/样式负责人；页面负责人使用共享契约，不自行添加等价组件。

- [ ] semantic tokens、typography、spacing、radius、surfaces、semantic colors 按已批准 DESIGN_SYSTEM。
- [ ] 共享 Button/Switch/Segmented/Select/Input/Row/Section/Feedback/Disclosure/Inspector/Sheet/Dialog/AsyncStates。
- [ ] 各组件 default/hover/selected/focus/disabled/saving/attention/error 状态清楚。
- [ ] 取消大 Topbar、装饰 Hero、gradient、Glow、默认 Card shadow；不删除功能信息。
- [ ] 六导航顺序、pending count、Harness global state 与 Overview 入口统一。
- [ ] shared Inspector 并排/覆盖、modal focus、返回焦点、滚动与 draft leave 行为。
- [ ] keyboard、radiogroup、field validation、aria-live 与 reduced-motion 成立。
- [ ] minimum 980×680、base 1280×820、Main available width 600px 阈值适配。

**Verify:** 组件交互与 Shell tests；双语言长文案 fixture；人工视觉 QA 留给 H1。  
**Acceptance:** AC01、AC07、AC13、AC14。  
**Exit:** 页面能共享一套控件与 surface，不制造多套永久 UI 系统。

## T4 — Overview and Tools

**Depends on:** T3  
**Primary files:** `ui/src/pages/overview/*`, `ui/src/pages/tools/*`，对应 i18n keys/tests。  
**Ownership:** 页面负责人，不改共享 coordinator。

- [ ] Overview 状态 / next action / component Inspector / Advanced 按 SPEC §7。
- [ ] Ready 下稳定 Secondary Stop；restart-required 下 Primary Restart；error/degraded 不隐藏可用 Stop 或 remediation。
- [ ] lifecycle in-flight/failed/ready 分开，不把 command ack 当最终 Ready。
- [ ] Tools compact rows：当前 Runtime、已保存 Enabled/default、具体 mismatch 说明。
- [ ] row 与 Inspector 共用 auto-save；Switch/segmented 不触发 row click。
- [ ] stopped/checking/unknown/stale、模式不可用、Denied、Requires approval 语义区分。
- [ ] Bash 常用 shell/auto-detect/override 保留；picker cancel 不写，保存不冒充验证成功。
- [ ] 精确 Running now 无权威来源时明确未确认，不编造 affected count。

**Verify:** action state、mismatch、save failure/refresh failure、picker cancel、row event separation 的组件/页面测试。  
**Acceptance:** AC02–AC04。  
**Exit:** 本页既不隐藏生效差异，也不恢复旧多列表格。

## T5 — Policies and safe Rule editor

**Depends on:** T3；与 T4 的 default mutation 契约一致  
**Primary files:** `ui/src/pages/policies/*`、Rule editor/matcher helpers、对应 i18n/tests。  
**Ownership:** Rule 模块负责人，不改 Runtime matcher。

- [ ] Tool defaults/User rules/System guardrails/Effective diagnostics 四层结构。
- [ ] default/Enabled auto-save；Inspector Decision 只读；Edit 为完整对象显式 Save。
- [ ] name/reason 可编辑默认值、稳定自动 ID、Retry 去重、target-ID Delete confirmation。
- [ ] 新草稿 Ask，用户 Create 前不保存；existing user metadata 和 ID 不自动重写。
- [ ] 自然输入实现 SPEC §9.3 保守单命令子集；unsupported 给明确 Advanced 路径，绝不 silently broaden（静默扩大范围）。
- [ ] normalized executable + token prefix；首层说明附加参数/空 prefix 的范围，不宣称 full-command equality。
- [ ] Advanced 参数项能无损保留 empty token、含空白 token 与现有 matcher。
- [ ] dirty编辑暂停同一 Rule enabled 操作，其他对象按队列保存。
- [ ] invalid policy fail-closed 为 Critical，系统护栏只读。

**Meaningful verification:**

- [ ] `git status` / `git status --short` 的 prefix 解释与 normalization 一致；`git statusx` 不混为匹配。
- [ ] quote/escape/expansion/compound/redirection 不在自然子集中被误解析。
- [ ] 64 token / 1024 UTF-8 bytes / NUL / empty token 与现有边界相符。
- [ ] 编辑非自然子集 Rule 未改 matcher 时原值 round-trip；已有 ID 不变，Retry 不重复创建。
- [ ] Save/Delete 失败保留正确对象；System guardrails 不能编辑；Rules empty 文案准确。

**Acceptance:** AC05–AC07、AC14。  
**Exit:** 规则范围在普通阅读层成立，底层 matcher 与 precedence 无变化。

## T6 — Approvals queue and Rule handoff

**Depends on:** T5  
**Primary files:** `ui/src/pages/approvals/*`，使用现有 Rule Sheet / navigation context。  
**Ownership:** Approval 页面负责人；规则创建通过 T5 契约。

- [ ] decision-first Pending + secondary History，request ID/structured evidence 后置。
- [ ] 风险标记人类说明、预览截断/脱敏提示、active workspace 语义、有效期按 SPEC 前置。
- [ ] session scope 文案真实，eligibility 使用 Runtime 字段；不从显示预览猜 grant key。
- [ ] per-request Allow once/Allow session/Deny/More 层级；无批量审批新功能，无多余确认。
- [ ] resolving/resolve failure/expired/cancelled/stale/disconnected 有明确状态与反馈。
- [ ] 到期只是本地检查触发，最终状态由 authoritative refresh；不新增周期轮询。
- [ ] mutation 成功提示是决定已提交，不承诺 execution success。
- [ ] Approval→Rule Sheet 只预填安全无歧义 matcher；多段/脱敏不可静默取第一段或升级整个工具。
- [ ] Create/Cancel 返回原 request context 并读取最新状态；不自动 approve/resolve。

**Verify:** 不完整/高风险/未知 eligibility、复用范围、到期竞态、generation change、双击去重、resolve failure、handoff 返回和无隐式 mutation。  
**Acceptance:** AC08–AC09、AC06–AC07。  
**Exit:** fixtures 证明 UI 行为，真实 resume/session grant 由 Q1 当前产物验证。

## T7 — Activity feed and evidence navigation

**Depends on:** T3，链接使用 T5/T6 稳定对象契约  
**Primary files:** `ui/src/pages/activity/*`, `ui/src/services/harness/control-read.ts` 展示适配部分、i18n/tests。

- [ ] 当前 bounded snapshot 显示 compact feed，保留非工具事件。
- [ ] All/Success/Needs attention/Tool 本地 filter，说明仅当前加载记录。
- [ ] feed 区分 policy deny、user deny、allowed execution failure、unavailable 等 outcome。
- [ ] Inspector 先解释发生什么/为什么/下一步，Advanced 保留所有现有 evidence。
- [ ] Refresh 保留内容，失败标 stale；空数据不与读取失败混用。
- [ ] stable selection；无 eventId 的旧 snapshot 不跨刷新误选对象。
- [ ] 仅对可定位对象提供 link；历史证据与当前 Rule 区分，缺失字段不编造。

**Verify:** outcome/filter/evidence projection、refresh failure、旧 snapshot 身份、历史对象无法定位与变更链接。  
**Acceptance:** AC10、AC14。  
**Exit:** 不新增数据库、全量历史、auto-refresh，也不减少 M7 evidence。

## T8 — Settings, First Run and Recovery

**Depends on:** T3、T2；使用 T4 的状态与 remediation 契约  
**Primary files:** `ui/src/pages/settings/*`, `ui/src/components/status/ConfigRecovery.tsx`, `ui/src/App.tsx` 的 setup/recovery 接入及相关 i18n/tests。  
**Ownership:** settings/recovery 页面负责人；共享 App/provider 接口变更与主 Agent 协调。

- [ ] General/Workspace/Connection/Security/Advanced/Danger Zone section/list。
- [ ] Language Select 专用 auto-save，立即应用/失败回滚；无 Theme 新功能。
- [ ] Workspace picker 后局部保存，configured / running 分开。
- [ ] Connection section draft + Apply/Cancel；API Key Configured 不冒充 Connected。
- [ ] Replace 密码仅随 Apply 提交；Cancel 清理输入；失败读回状态且不承诺 secret 整体回滚。
- [ ] Remove 为独立 clean-editor destructive confirm，结果不被 Cancel 撤销。
- [ ] secret 不进入日志/router/fixtures snapshot/持久 UI store；提交结束即清理，失败重输。
- [ ] Developer/Test 后果确认、security save/restart；返回 Normal 不添加不必要确认。
- [ ] Reset 提示既有实际损失并确认，与普通 Apply 物理分离。
- [ ] First Run 四段流程；Readiness 仅表示配置准备；启动失败在相关步骤修复。
- [ ] Recovery 显示问题/已知数据安全/下一步，原配置和 secret 边界继续保持。

**Verify:** Language 专用回归与 serialization/lifecycle、Connection Cancel/Apply/partial failure、Remove、picker cancel、security confirm、setup/recovery、Reset/in-flight 互斥。  
**Acceptance:** AC11–AC12、AC02、AC07。  
**Exit:** 没有通过 UI 承诺或补做新的后端事务能力。

## T9 — Integration, regression and Development Complete

**Depends on:** T4–T8 全部完成  
**Ownership:** 主 Agent 整合所有共享边界和页面，检查 scope。

- [ ] 全产品 default/configured/effective/restart wording 与 saved source 一致。
- [ ] dirty guard、saving lock、conflict、focus restoration、contextual navigation 全路径一致。
- [ ] 双语言、长内容、minimum/base/wide、focus、disabled/saving/error/reduced-motion 场景具备覆盖。
- [ ] 删除已证明未引用的旧展示样式/组件，保留不相关修改、测试与安全 contract。
- [ ] 执行本次必要 targeted tests，整合后 `npm run typecheck`、`npm test`、`npm run build`。
- [ ] 按 PLAN 执行 workspace 回归与 `git diff --check`；失败修复后只针对需要的范围重跑。
- [ ] 记录 Development Complete、实际结果与未覆盖项；不标 Manual Acceptance、Portable 或 M9 完成。

**Acceptance:** AC01–AC14 的开发验证映射。  
**Exit:** 代码与开发验证完成，进入独立 QA，当前尚未 ACCEPTED / FROZEN。

## Q1 — Independent automated QA

**Depends on:** T9  
**Ownership:** 独立 QA 阶段；不得把开发自测重命名成独立 QA。

- [ ] 准备独立 integration / local real black-box coverage；所有验证使用明确的当前 source/binary 身份。
- [ ] 对 SPEC AC01–AC14 复核配置读回、真实 Runtime observation、MCP/Approval/Activity 和 UI 行为的各自覆盖。
- [ ] 检查 Language、Policy/Bash/Approval、migration/recovery 安全回归，不修改冻结语义。
- [ ] 人工界面行为无法自动验证的项标 Manual pending，不虚报 PASS。
- [ ] 报告实际命令、退出码、失败和非阻断观察；Automatic QA PASS 仅覆盖已实际执行范围。

**Exit:** 明确 Automated QA PASS 或具体失败；失败修复后更新受影响证据再进入 P1。

## P1 — Build and isolate the latest M8 Portable

**Depends on:** Q1 PASS 且代码身份仍相同  
**Ownership:** 产物构建与身份核验；不清理其他 milestone 的产物。

- [ ] 最新被验证代码运行当前 `ui` 的 `npm run portable:windows`，不用旧 release binary。
- [ ] 当前 verifier 先在允许的 `target/release/bundle/portable/` staging 路径完成验证。
- [ ] 完整复制到新的 `artifacts/M8-UI-UX-Redesign/build-<YYYYMMDD-HHmmss>-<short-head>/`，时间格式不含 Windows 路径非法字符。
- [ ] 比较 staging/final file list、manifest、SHA-256；不得覆盖已有人工包或其他 milestone。
- [ ] 保存 source HEAD/相关 dirty diff 身份、构建命令/时间/结果、包的绝对路径及所有 binary hashes。
- [ ] 提供实际 package identity 与 UI 启动说明，记录 verifier 与 Runtime smoke 的有限覆盖。

**Acceptance:** AC15。  
**Exit:** 当前 M8 独立包可交人工验收；构建成功不等于 UX 接受。

## H1 — Manual UX Acceptance

**Depends on:** P1  
**Ownership:** 用户或其明确授权的真实验收人；Agent 不模拟人工结果。

- [ ] 启动对应 M8 final package UI，确认 binary 身份、embedded frontend/custom protocol 与本次构建一致。
- [ ] 按 SPEC AC01–AC15 检查主要任务、六原则、双语言、minimum/base/wide、长内容和键盘路径。
- [ ] 检查 Tool/default 自动保存、Rule prefix解释、dirty guard、审批风险/范围/失效、Activity 和 Connection 凭据边界。
- [ ] 保留失败/非阻断观察与证据；没有实际执行的项标 NOT RUN。
- [ ] 无明确 Computer Use 授权时给出人工步骤，不启动自动 UI 操作。
- [ ] External ChatGPT/M9 不在本阶段自行执行。

**Exit:** 明确人工接受或修订要求；修订涉及代码时回到相关开发验证/QA，并重建最新包。

## F1 — Acceptance record and freeze

**Depends on:** H1 人工接受  
**Ownership:** 主 Agent 做文档收口，不自行补验收结果。

- [ ] 记录实际 M8 ACCEPTED / FROZEN、人工依据、包身份和残余观察。
- [ ] 按最终源码更新当前事实文档，避免把计划写成已实现事实。
- [ ] 不自动开始 M9，不删除或覆盖旧 milestone 历史产物。

**Exit:** M8 真实完成；后续 milestone 等待独立任务指令。

## Current state

```text
SPEC / PLAN / TASKS  DRAFT GENERATED FOR REVIEW
D01–D07              PROPOSED / NOT HUMAN-CONFIRMED HERE
DESIGN               NOT CLAIMED FROZEN
T0–T9                NOT STARTED
Q1                    NOT RUN
P1                    NOT BUILT
H1                    NOT RUN
F1                    NOT COMPLETE
IMPLEMENTATION        NOT AUTHORIZED BY DOCUMENT GENERATION
```
