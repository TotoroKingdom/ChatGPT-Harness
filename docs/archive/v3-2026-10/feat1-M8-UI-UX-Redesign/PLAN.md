# M8 PLAN — UI/UX Simplification & Visual Redesign

> Status: DRAFT FOR REVIEW / EXECUTION BLOCKED UNTIL G0  
> Input: `SPEC.md`  
> Date: 2026-10-04  
> Implementation: NOT STARTED  
> Rule: 本文件定义实施顺序，不自行改变 SPEC、批准设计或声明 QA 完成。

## 1. Strategy

先统一状态来源、保存边界与交互基础，再迁移页面。不能先把六个页面分别换皮，最后再拼接不一致的控件与保存逻辑。

依赖路径：

```text
G0 Design / audit decisions / authorization
  → T1 Baseline inventory and verification map
  → T2 Shared state / mutation / navigation contracts
  → T3 Design tokens / shared components / shell
  → T4 Overview and Tools
  → T5 Policies and safe matcher editor
  → T6 Approvals and policy handoff
  → T7 Activity
  → T8 Settings / First Run / Recovery
  → T9 Cross-page regression and development completion
  → Independent automated QA
  → Fresh isolated M8 Portable
  → Manual UX Acceptance
  → Acceptance record / freeze
```

T7、T8 在共享契约稳定后可独立推进，但仍需要最终跨页整合。主 Agent 保留状态真实性、mutation queue、router/draft guard、scope、关键路径与最终整合。使用子 Agent 时明确文件 ownership（归属），不得同时修改 Provider、router、global.css 或同一组件；不能覆盖其他人的工作区修改。

## 2. G0 — Reviewable implementation input

先阅读 SPEC §1、§4，不把生成文档视为设计冻结。

需要确认：

- `DESIGN_SYSTEM.md` 的候选视觉基线。
- SPEC D01–D07 的审计补充决策，尤其 prefix、审批信息、草稿、凭据边界。
- 后续实施授权；若后续用户已经明确接受这些决策并授权，记录并继续，不重复请求。

若修改候选决策，先同步 SPEC / PLAN / TASKS 的相关验收，不在代码中暗自选择其他方案。既有冻结输入是否需要补充由用户确认；本轮不改写原冻结记录。

Exit：有明确的 DESIGN FROZEN / SPEC APPROVED / IMPLEMENTATION AUTHORIZED 依据。未通过时只允许规划与只读取证。

## 3. Phase A — Current baseline and capability inventory

Primary locations（主要位置）：

| 区域 | 核查位置 |
| --- | --- |
| App / routing | `ui/src/App.tsx`, `ui/src/app/router/routes.ts`, `useAppRouter.ts` |
| Authority / mutation | `ui/src/app/providers/HarnessDataProvider.tsx`, `ui/src/services/harness/client.ts`, `local-client.ts`, `control-read.ts` |
| Shell / primitives | `ui/src/components/layout/*`, `ui/src/components/common/primitives.tsx`, `ui/src/styles/global.css` |
| Pages | `ui/src/pages/{overview,tools,policies,approvals,activity,settings}/` |
| State / recovery | `ui/src/components/status/*`, `ui/src/app/providers/useControlRead.ts` |
| i18n（国际化） | `ui/src/i18n/presentation.ts`, `en.json`, `zh-CN.json`, related tests |
| Read-only contract evidence | `protocol/src/v1/*`, `runtime/src/policy/*`, `runtime/src/session_grant.rs`, `ui/src-tauri/src/commands/mod.rs` |
| Distribution evidence | `ui/scripts/build-portable.ps1`, `verify-portable.ps1`, Tauri configs / manifests |

核查当前 HEAD、工作区修改和当前接口，不依赖旧里程碑计划或过时 wiki 描述。

确认至少：五个 built-in Tools、已保存 config、effective observation、policy diagnostics、approval eligibility / preview / expiry、Activity evidence、Language 路径、credential partial-result 边界。

测试映射以 SPEC AC01–AC15 为目标，登记现有可复用测试与新增行为测试。后续需要的 integration / black-box / manual acceptance 文档作为验证阶段交付，不把当前三份文档当作这些证据。

Exit：目标行为都有现有数据来源；缺口有明确记录，禁止未经授权改变 Protocol / Runtime。

## 4. Phase B — Shared presentation and mutation coordination

在 UI 层建立可测试的共享契约，模块文件名可以按实际结构调整，但职责不能混乱：

- status presentation：overall Harness、component、configured/effective/stale/restart、typed remediation。
- scoped config mutation coordinator：串行提交局部意图、合并最新 persisted snapshot、处理 Language / Reset / Rule / Connection 的交错。
- local feedback：saving / saved / restart-required / failed / refresh-unconfirmed。
- navigation context：选中对象、目标 Settings section、Approval→Rule→Approval handoff。
- draft guard：dirty、saving、conflict、Close/Esc/row switch/navigation 与受控 credential 输入清理。

不能建立新的 authoritative policy engine，不能修改 generation / revision 的意义。

必须先测试：

- 两个不同字段快速提交均保留。
- Rule Save / Connection Apply 不覆盖较新的 Language、Tool 或 Workspace。
- Language 失败 UI 与 preference 回滚；Language 不引起 Runtime restart。
- 保存成功但 refresh 失败不错误回滚 persisted mutation。
- generation 切换使旧 effective data / request stale。
- editor 已观察到同一对象变更时保留 draft 并阻止盲写。

Exit：页面可以调用统一操作与状态语义，不各自拼接整份 config。

## 5. Phase C — Shared visual grammar and shell

先实现 Design Tokens、Button/IconButton、Switch、Segmented、Select、TextField、Row、Section、StatusIndicator、InlineFeedback、Disclosure、Inspector、Sheet、Dialog、Empty/Loading/Error。

优先复用或演进现有 primitives，避免引入与现有组件并行的第二套永久系统。旧页迁移期间允许必要兼容，结束后只清理已证明无引用的旧 UI 样式；不删除不相关代码。

再实现统一 AppShell / Sidebar / PageHeader，保留真实状态与安全能力：

- 六导航按已冻结顺序展示；pending count 不编造。
- 状态点击到 Overview；无大 Topbar、Hero、gradient 或常驻 glow。
- Inspector 并排/覆盖按 SPEC 可用宽度阈值切换；focus lifecycle（焦点生命周期）一致。
- shared keyboard、validation、aria-live、reduced-motion 行为。

无需新增图标 / 动画库或字体依赖来完成既定设计。若确有必要，先说明收益和范围，不扩大基础设施重构。

Exit：共享组件行为与 shell tests 通过，最小窗口中结构可用；不宣称真实视觉人工验收通过。

## 6. Phase D — Overview and Tools

Overview 先建立 system state + next action：正常状态保留 Secondary Stop；mismatch 时 Primary Restart；异常先给 remediation；诊断后置。

Tools 迁移为 compact row + Inspector，接入统一 auto-save。保持配置值和运行状态的不同来源，包含 stopped/checking/stale/mismatch 及 Bash 常用配置。

测试操作与真实可读事实，不只测试 className（样式类名）。重点验证 OFF configured / ON running 的解释和保存失败、picker cancel。

Exit：AC02–AC04 的页面行为可验证；不以 mock 状态证明 Runtime 已生效。

## 7. Phase E — Policies and matcher editing

先迁移 defaults、user rules、guardrails、diagnostics，再实现完整 Rule Create/Edit/Delete。

- default / Enabled 接共享 auto-save。
- Inspector Decision 只读，edit mode 才可修改。
- ID 自动管理，已有 ID 与 matcher 不重写；Save / Delete 按目标对象执行。
- 自然 command conversion 限定 SPEC 保守子集；unsupported 拒绝并提供 Advanced literal tokens。
- prefix 范围摘要位于首层，不把 exact argv-prefix 表达为 exact full command。
- 非自然子集既有 matcher 使用无损 Advanced，不自动 split 或 stringify 重建。
- invalid snapshot 自动提升 Critical，而不是藏在 collapsed diagnostics。

测试自然输入成功/拒绝、normalization、literal empty token、含空白 token、UTF-8 bounds、Retry ID 与既有 matcher round-trip。

Exit：AC05–AC07 的 Rule 行为成立，Approval handoff 使用相同创建流程。

## 8. Phase F — Approvals and Activity

Approvals 优先实现 request validity，再处理视觉迁移：

- front-of-decision（决策前）信息例外、session scope 提示、Runtime eligibility。
- request-keyed resolving / failure、authoritative refresh、expiry hints、generation invalidation、终态只读。
- Create rule 不自动 resolve；不能从脱敏/多段命令静默创建更广 matcher。
- 返回原请求后读取最新状态；已过期请求不继续审批。

Activity 使用现有 bounded snapshot，feed + contextual Inspector；本地 filters 标明当前加载范围，保留所有 evidence、非工具事件和 stale feedback。

Exit：AC08–AC10 成立，navigation 携带上下文但不执行 mutation。真实 approval resume / grant 行为留给当前产物本地验证，不用页面 fixtures 代替。

## 9. Phase G — Settings, First Run and Recovery

Settings 使用 section/list，迁移 Language、Workspace、Connection、Security、Advanced、Danger Zone。

- 使用统一 scoped mutation，与其他页面的保存不会互相覆盖。
- credential removal 是独立确认动作；clean editor 后执行，后续 Cancel 不恢复。
- replacement 输入只在 Apply 提交；部分失败重新读取可得状态并诚实表达，无跨存储回滚承诺。
- Security capability 警示与确认保留，已保存模式不冒充 running mode。
- Reset 的后果与既有配置/secret 清理行为一致，避免未经验证的“数据都保留”承诺。

First Run 使用最短四段路径，Readiness 不假 Connected。Recovery 区分 invalid config / migration failure / unsupported，保留现有安全边界。

Exit：AC11–AC12 成立，旧 Language serialization/lifecycle、migration/recovery tests 不退化。

## 10. Phase H — Integration and development completion

T9 汇总跨页行为与所有 SPEC invariant：

- Tools ↔ Policies 默认值一致。
- Overview / row / Inspector / Settings 的重启表达不矛盾。
- dirty 关闭、切对象、切页与上下文回程一致。
- 多次快速 mutation 与 Language/Reset 不覆盖已保存意图。
- Runtime session 变化不让旧 observations / approvals 继续有效。
- 双语言、长内容、minimum/base/宽窗口、键盘 focus 与 reduced-motion。
- 旧 table/card/form-first 结构无不当回归；旧证据与可执行 remediation 不丢失。

Development Complete 只能表示实现任务与本次开发验证完成；独立 QA、Portable、Manual Acceptance 分开记录。

## 11. Verification commands and evidence

以下命令是后续计划，当前未执行。以实施时实际 manifests 与 `AGENTS.md` 为准；命令失败先定位，不原样重复。

| Working directory | Command | 证明范围 |
| --- | --- | --- |
| repository root | `git diff --check` | 差异格式 |
| `ui` | `npm run typecheck` | TypeScript 类型 |
| `ui` | `npm test` | 当前 UI automated tests |
| `ui` | `npm run build` | 类型 + frontend production build |
| repository root | `cargo fmt --all -- --check` | Rust 格式回归 |
| repository root | `cargo test --workspace` | 当前 workspace 后端/协议回归；不代表 UI 或 External 验收 |

Development 阶段按改动运行 targeted tests；整合后执行完整适当门禁。通过后不为报告数量反复重跑无变化检查。

本地真实验证必须确认所启动 Runtime / UI 来自当前修改，记录 binary path、hash、source HEAD/diff、配置与 generation。继续保留 Approval/MCP/Activity 的真实验证边界，不冒充 External ChatGPT 或人工操作。

独立 QA 需要：覆盖 AC01–AC14、实际 command 输出与退出码、真实本地产物身份、失败/非阻断观察和未覆盖项。仅 UI fixture PASS 不足以声明所有 acceptance 完成。

## 12. Fresh M8 Portable and manual handoff

严格遵循项目要求：Development Complete + Automated QA PASS 后，基于最新代码重新构建 Portable，再进入 Manual Acceptance。

1. 核对 build source 与 QA source；如果 QA 后代码变化，重新验证受影响行为后再构建。
2. 在 `ui` 使用当前 `npm run portable:windows`。该命令会构建正式 UI/Runtime 并执行现有 verifier，不能复用旧 milestone 包。
3. 当前 build script 的 staging 输出位于 `target/release/bundle/portable/`；当前 `verify-portable.ps1 -PortableDirectory <path>` 限定该根目录内路径。不要对 `artifacts/...` 直接调用然后伪报验证成功，也不要为此扩大 verifier 安全范围。
4. 先验证本次 staging 包，再复制完整包到新的 `artifacts/M8-UI-UX-Redesign/build-<YYYYMMDD-HHmmss>-<short-head>/`；使用 Windows 合法的时间格式，目标存在时用新的 build 目录，不覆盖上一份人工验收包。
5. 比较 staging 与最终副本的文件清单、manifest 和 SHA-256；记录 HEAD、相关未提交 diff 身份、构建命令、时间与结果。
6. QA 的旧进程不作为最终包验收依据。人工必须启动最终 M8 副本的 UI，验证 embedded assets / custom protocol 与运行二进制身份，不能仅检查 Runtime smoke。
7. 提供针对 SPEC §14 的人工检查清单、包的绝对路径和已知未覆盖项。没有 Computer Use 授权时由人完成界面验收；没有 External 指令时不开展 M9。

Runtime smoke / verifier / UI automated tests 各自说明覆盖，任何一项都不能单独升级为 Manual UX Acceptance PASS。

## 13. Risks and stop conditions

| 风险 | 控制 |
| --- | --- |
| frontend full config stale overwrite | shared queue + scoped intents + observed conflict handling；承认跨进程限制 |
| desired 冒充 effective | authoritative projection + stale/session tests；不重算 policy |
| prefix / secret / approval scope 被简化失真 | 首层范围说明 + 前置例外 + Runtime eligibility |
| 自动保存和 dirty editor 互相干扰 | 同对象编辑暂停直接保存；其他对象按最新 config 合并 |
| 设计需要新后端能力 | 记录缺口与现有 contract 的替代呈现，不扩大 M8 |
| minimum window / 中文长文案溢出 | shared responsive grammar + 人工尺寸矩阵 |
| QA 包与人工包不同 | 最新重建、独立目录、hash 与源码证据 |

必须暂停相关实现而非猜测的条件：G0 未确认、必要权威字段无现有来源、必须改变冻结 Runtime/Protocol 才能满足需求。可以继续不依赖该缺口的已授权工作；只有确实需要产品决策时提交具体、可评审的差异。

## 14. Deliverables and reporting

- 完成后的 UI diff 与本次必要行为 tests。
- SPEC AC01–AC15 覆盖映射、开发结果与独立 QA 结果。
- 当前最新独立 M8 Portable 及来源/manifest/hash 记录。
- Manual UX Acceptance 清单和人工结果；由真实执行人确认。
- 仅在验收后产生最终接受/冻结记录，并按实际源码更新当前事实文档。

当前仅生成 SPEC / PLAN / TASKS；上述实现、验证、构建、人工结果全部为后续工作。
