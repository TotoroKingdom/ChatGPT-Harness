# M6 Manual Acceptance Checklist — Chinese / English UI

> Status: ACCEPTED / FROZEN
> Milestone: M6 — Chinese / English UI  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Owner: Human Reviewer  
> Purpose: M6唯一需要人工介入的最终验收层。

## 1. Acceptance Responsibility

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local E2E            → Coding Agent
Manual Acceptance    → Human Reviewer
```

人工验收重点：

- English / 简体中文整体可用性。
- 首次启动/Settings language UX。
- 即时切换时页面是否完整同步更新。
- 中文翻译是否准确、不弱化安全语义。
- 是否存在明显漏翻/混合语言。
- 是否有布局溢出、截断、按钮不可读。
- machine identity / user-authored data是否保持原文。
- language-only change是否没有误导性的 Restart Required。

人工不负责重复 ConfigStore revision矩阵或 machine enum mapping自动化；这些必须在 Integration/E2E先 PASS。

## 2. Entry Criteria

只有以下全部 PASS才进入人工验收：

```text
Development
Unit
Integration
Local E2E
```

必须提供：

```text
Production commit
Automated evidence commit
Acceptance build / Portable identity
Integration summary
LOCAL_M6_E2E=PASS
```

如果 resource parity、language-only configRevision、Runtime non-impact尚未自动化证明，不能靠人工点击补成 PASS。

## 3. Build / Source Identity

- [x] 记录 M6 production implementation commit。
- [x] 记录 automated evidence commit。
- [x] 记录实际验收 Portable/build identity。
- [x] 确认之后没有未经验证的 UI/i18n production change。

记录：

```text
Production commit: 7083fbc4862340b73f083f4b7b4912b626909662
Evidence commit: 3c51941c3ba4a4c8556bb76a703e1639e387704e
Acceptance build: artifacts/M6-i18n/ChatGPT-Harness_0.1.1_x64-portable (build HEAD: 3c51941c3ba4a4c8556bb76a703e1639e387704e)
Reviewer: Human Reviewer（用户）
Date: 2026-10-04
```

### Agent preparation record — 2026-10-04

本段仅记录进入人工验收前的 Portable 准备证据；准备阶段本身不代表人工验收通过。最终人工验收结论见第 18 节，checkbox 依据用户本次完成并确认通过的声明收口。

- 初始工作区：clean；构建 HEAD：`3c51941c3ba4a4c8556bb76a703e1639e387704e`。
- Production implementation commit：`7083fbc4862340b73f083f4b7b4912b626909662`。
- QA evidence / production fix commit：`3c51941c3ba4a4c8556bb76a703e1639e387704e`；包含 Settings language mutation serialization 修复，源码 SHA-256 与 QA_REPORT.md 一致。
- 构建版本：UI / Runtime `0.1.1`；protocol `1`；Tunnel client `0.0.13`。
- Acceptance Portable：`C:\AI\ChatGPT-Harness\artifacts\M6-i18n\ChatGPT-Harness_0.1.1_x64-portable`。
- 主 UI executable：上述目录的 `chatgpt-harness-ui.exe`。
- Runtime executable：上述目录的 `runtime\chatgpt-harness-runtime.exe`。
- 构建方法：先 `cargo clean --release -p chatgpt-harness-runtime -p chatgpt-harness-ui`，再从 `ui` 执行 `npm run portable:windows`；重新构建 release Runtime、production frontend 与启用 `custom-protocol` 的 release UI，无复用 M5 Portable。
- 最终包与本次 staging / release 产物逐文件 SHA-256 相符；10 个文件均非空，manifest 版本/路径与 6 个 icon entries 验证通过。
- 最终目录 Runtime subprocess（子进程）initialize → ready → runtime.stop accepted，exit 0。
- 最终目录 UI 启动 8 秒后进程仍存活，executable path 精确匹配，WebView2 子进程存在；测试 UI 已关闭，无该 Portable 目录的 UI/Runtime 进程残留。
- 此启动 smoke 不包含桌面渲染、语言、视觉、点击或 External ChatGPT 验收；没有执行或标记任何人工项 PASS。
- 未重新执行完整 Integration/E2E；既有结果保持原样。构建中的现有 ts-rs / dead-code / linker warnings 不阻塞打包。
- 日志、进程身份与完整清单：`artifacts/M6-i18n/portable-preparation/build.log`、`build-identity.json`、`runtime-smoke.jsonl`、`SHA256SUMS.txt`（均相对于仓库根目录）。
- Reviewer / 人工验收确认日期：Human Reviewer（用户）/ 2026-10-04。

| Portable file | Bytes | SHA-256 |
| --- | ---: | --- |
| `chatgpt-harness-ui.exe` | 10878976 | `6C0A3ED25592F407E5C14DCAE6C058898F860956BB6CE5137E47B0F562FAF882` |
| `distribution.json` | 518 | `B3ADEF887386FFD8728730CA211B897E054D0CF4A8CFCFF5C0E89A48F1DCF942` |
| `runtime\chatgpt-harness-runtime.exe` | 8031744 | `79B43A2C6A261CE6E42375B46C4203777C6BD86D48FC1FC38390F5F9281ADC80` |
| `tunnel\cloudflared-manifest.json` | 1059 | `0C30BCCDA73FD6D2F81BD60F748C8DE805F09C9482364584C8C28C49EA7AD219` |
| `tunnel\cloudflared.exe` | 39751680 | `CD0BC49AEC24D0D62A37B6E842DAD46000F7B401F6167B4CBACE4E170819C9A2` |
| `tunnel\LICENSE` | 10945 | `6507C654AA26B7F8BEDA9BC86C955F8B6FF77350CEBF6663FAFA8974C8AE5A7C` |
| `tunnel\NOTICE` | 564 | `D5A275A75B91D294AF22FE98B43A122AB4510CD4F922A7910E84D1C2AD97F811` |
| `tunnel\tunnel-client-v0.0.13-windows-amd64-licenses.txt` | 13574 | `1B0D874470433A7DCC1BA1F7FD121D328D3FD8638CB9D094526CB76A2D2AF879` |
| `tunnel\tunnel-client-v0.0.13-windows-amd64.spdx.json` | 240128 | `A80D4C4C09836C97258740C4B0E975AA38BAFBA9A61F2C59DBE5D27646151E01` |
| `tunnel\tunnel-client.exe` | 21753856 | `83F08FB39B1C154747DEBD31B81B65DD4EE834CACF5A073B6301B2688699BC76` |

## 4. Automated Evidence Review

### Development / Unit

- [x] T1 UiLanguage / persistence semantics PASS。
- [x] T2 bootstrap/resources PASS。
- [x] T3 presentation mappings PASS。
- [x] T4 primary UI migration PASS。
- [x] T5 resource/regression PASS。

### Integration

- [x] INT-01 ～ INT-40 PASS。
- [x] language-only save preserves configRevision。
- [x] isolated persistence does not save unrelated draft。
- [x] resource key parity / placeholder parity PASS。
- [x] machine identifiers remain raw。
- [x] known error localization PASS。
- [x] Runtime/MCP non-impact PASS。

### Local E2E

- [x] E2E-01 ～ E2E-22 PASS。
- [x] fresh locale detection PASS。
- [x] saved preference override PASS。
- [x] persistence across bootstrap restart PASS。
- [x] Follow system PASS。
- [x] no Runtime restart/generation mutation PASS。
- [x] production primary routes render both languages。
- [x] MCP contracts unchanged。
- [x] `LOCAL_M6_E2E=PASS`。

## 5. First Launch / System Language

建议在可控测试环境中分别验证至少 English 与简体中文系统语言；若修改真实 Windows display language成本过高，可使用团队既定的 acceptance fixture/build启动参数，仅用于首次语言 UX人工观察，locale resolver本身仍以自动测试为 authority。

### MA-01 — English system / no saved preference

清除测试配置中的 explicit language preference后启动。

- [x] UI首次可见主界面即为 English。
- [x] 没有明显先显示中文再闪回英文。
- [x] `<html lang>`对应 English（可通过开发工具/自动 evidence review确认）。

### MA-02 — Simplified Chinese system / no saved preference

无 saved preference，以简体中文系统/fixture启动。

- [x] 首次可见主界面即为简体中文。
- [x] 没有完整 English页面闪现后再切中文。

### MA-03 — Unsupported locale fallback

review自动化 evidence并在可行时抽查一个 unsupported locale。

- [x] fallback English。
- [x] Traditional Chinese不会静默当成 Simplified Chinese saved language。

## 6. Settings Language Selector

### MA-04 — Selector options

Settings必须清晰提供：

- [x] Follow system。
- [x] English。
- [x] 简体中文。

选项名称应即使在陌生当前语言下也足够辨认。

### MA-05 — English → 中文 immediate switch

在 Settings选择简体中文：

- [x] 无需点击普通 `Save Changes` 才生效。
- [x] 当前页面即时切换。
- [x] Sidebar / topbar同步切换。
- [x] 不重启 Runtime/Harness。
- [x] 不出现新的 Restart Required。

### MA-06 — 中文 → English immediate switch

反向切换：

- [x] 整体即时恢复 English。
- [x] 当前操作状态/页面位置保持合理。
- [x] 不重新启动 Runtime。

### MA-07 — Follow system

选择 Follow system：

- [x] 立即切换到当前 system-resolved locale。
- [x] 重启应用后继续跟随系统。

## 7. Persistence / Restart UX

### MA-08 — Preference persists across app restart

选择 `zh-CN`，正常退出并重新打开应用：

- [x] 仍为简体中文。

选择 English重复：

- [x] 仍为 English。

### MA-09 — No false Restart Required

在 Harness当前 configRevision完全收敛时切语言：

- [x] 不出现 `Restart Required / 需要重启`。
- [x] Runtime PID/session不因语言变化而重建（结合 automated evidence确认）。

### MA-10 — Existing Restart Required remains

先通过一个真实 Runtime-affecting desired change制造 Restart Required，再切语言：

- [x] Restart Required仍存在。
- [x] 文案随语言切换。
- [x] 不因语言 save把 pending runtime config change清掉。

## 8. Primary Surface Coverage — English

以 English依次浏览：

- [x] First Run Wizard（如可进入测试 fixture）。
- [x] Overview。
- [x] Tools。
- [x] Policies。
- [x] Activity。
- [x] Approvals。
- [x] Settings。

每页至少检查：

- 标题/副标题。
- buttons/actions。
- status badges。
- help/warnings。
- empty/loading/error states（能方便触发的状态）。
- form labels/placeholders。

不得出现 translation key如：

```text
settings.language.title
errors.runtimeUnavailable
```

直接显示给用户。

## 9. Primary Surface Coverage — 简体中文

以简体中文重复浏览主要页面：

- [x] First Run Wizard。
- [x] 概览。
- [x] 工具。
- [x] 策略。
- [x] 活动记录。
- [x] 审批。
- [x] 设置。

要求：

- [x] 主功能文案无明显 English漏翻。
- [x] 技术名词按冻结边界保留英文/raw时属于预期，不算漏翻。
- [x] 中文自然、可理解、没有明显机器直译歧义。

## 10. Raw Identity Boundary

### MA-11 — Tool names remain raw

中文模式下确认：

```text
read_file
glob
write
edit
bash
```

作为 tool identity显示时不被翻译成其它字符串。

周围的“可用/需要审批/已禁用”等标签应为中文。

### MA-12 — Policy IDs / request IDs remain raw

中文模式：

- [x] System/User rule ID不翻译。
- [x] request/approval ID不翻译。
- [x] matched rule IDs不翻译。
- [x] known System Rule friendly name/reason可以本地化。

### MA-13 — User-authored Policy text remains verbatim

创建一条包含 English/中文混合的 Policy name/reason。

切换语言：

- [x] 用户输入内容完全不变。
- [x] 不自动翻译/重写。

### MA-14 — Paths / commands / output remain raw

检查 Tools/Bash/Approval/Activity：

- [x] path保持原值。
- [x] Bash command保持原值。
- [x] stdout/stderr保持原值。
- [x] executable path保持原值。

## 11. Status / Machine-value Localization

### MA-15 — Tool state labels

中文模式抽查：

- Available。
- Available with approval。
- Disabled。
- Unavailable by mode。
- Unavailable by policy/runtime。
- Ready/Unavailable。

应显示准确中文解释，而 machine identity仍保持底层原值。

### MA-16 — Policy labels

检查：

```text
Allow / Ask / Deny
```

中文显示语义清晰、一致。

不得把 ASK翻译成容易误解为“已允许”。

### MA-17 — Approval states/actions

至少检查：

- Pending。
- Approved。
- Denied。
- Allow once。
- Allow for session。
- Deny。
- Create rule。

中英文切换后动作语义必须保持一致。

### MA-18 — Activity labels

抽查不同 Activity：

- Policy evaluated。
- Approval required/resolved。
- Execution completed/failed。
- Bash timeout/limit。

中文应能理解，tool/request/rule identity仍 raw。

## 12. Error Localization

### MA-19 — Known error

制造一个安全 known error，例如 Runtime未启动时刷新 Runtime-backed页面。

English：

- [x] 用户解释为 English。

切中文：

- [x] 同一 machine condition显示中文解释。
- [x] 不改变 underlying error code/行为。

### MA-20 — Config validation

在 Settings输入一个安全 invalid配置 fixture，例如 malformed proxy。

- [x] English validation自然。
- [x] 中文 validation自然。
- [x] 修复路径清晰。

### MA-21 — Unknown/generic error presentation

若 acceptance fixture可安全制造 unknown code：

- [x] 使用本地化 generic error。
- [x] 不展示 success/ALLOW fallback。
- [x] safe technical detail如保留，不被错误机器翻译。

否则以 Integration evidence验收本项。

## 13. M5 Bash Security Wording

这是 M6 的硬 acceptance gate。

### MA-22 — No-sandbox wording parity

English 与中文都必须明确表达：

```text
Bash working directory is Workspace
≠
Bash is filesystem/OS sandboxed to Workspace
```

中文不能写成类似：

```text
Bash只能访问工作区
Bash已被安全隔离
```

除非未来真正实现 sandbox。

### MA-23 — Developer/Test warning parity

中文不能弱化 Developer/Test Mode 风险；应明确这是高风险开发能力资格，而不是安全 bypass模式。

### MA-24 — Unclassified command warning parity

Bash unclassified/high-risk approval中文：

- [x] 明确需要审批。
- [x] Allow for session不可用的原因清晰。
- [x] 不暗示 analyzer已经证明命令安全。

## 14. Layout / Visual Quality

分别 English / 中文检查主窗口常见尺寸。

### MA-25 — No critical overflow

- [x] Sidebar文字不严重截断。
- [x] topbar不覆盖。
- [x] buttons可读/可点击。
- [x] StatusBadge不破坏布局。
- [x] tables headers合理。
- [x] approval action区域正常。
- [x] Settings labels/help不重叠。

允许普通文本自然换行；不接受导致操作不可用的 overflow。

### MA-26 — Long technical identifiers coexist with Chinese copy

使用较长 path/rule ID/request ID fixture：

- [x] 技术值仍可识别。
- [x] 中文 labels不导致整体崩坏。

## 15. Focus / Interaction Continuity

### MA-27 — Switching language does not destroy live UI state

在非敏感 form里输入未保存值或展开某 UI state，然后切语言：

- [x] 不意外提交 form。
- [x] 不丢失明显不应丢失的 local draft。
- [x] 不触发 Harness restart。

### MA-28 — Pending Approval language switch

有真实 Pending approval时切换语言：

- [x] Pending request仍是同一 request。
- [x] 状态未改变。
- [x] 没有自动 Allow/Deny。
- [x] Approval UI copy即时切换。

## 16. M0–M5 Regression Smoke

M6只需要人工做少量 high-value smoke；完整矩阵由自动化负责。

- [x] Start/Stop Harness仍正常。
- [x] read_file或glob正常。
- [x] write/edit任一正常。
- [x] Tool toggle能保存。
- [x] Policy editor能保存规则。
- [x] file Approval能 resolve。
- [x] Bash effective state页面正常。

分别至少在 English / 中文各完成部分 smoke，证明翻译没有覆盖 action wiring。

## 17. Rejection Conditions

出现任一项即不能 ACCEPT：

- saved language重启后丢失。
- language-only change产生新的 Restart Required。
- 切语言导致 Runtime restart/generation改变。
- Settings language action顺手保存其它未提交配置。
- Supported locale主要页面存在明显大量漏翻。
- translation key直接暴露。
- 中文把 tool name / policy ID / request ID改写。
- 用户自定义 Policy内容被自动翻译。
- known ASK/DENY/Approval语义被错误翻译。
- 中文弱化 Bash no-sandbox / high-risk安全警告。
- pending approval因语言切换改变状态。
- error localization改变 machine behavior。
- 主页面因中文长度导致关键操作不可用。
- Integration/E2E未 PASS。

## 18. Final Decision

全部满足后：

- [x] Development PASS
- [x] Unit PASS
- [x] Integration PASS
- [x] Local E2E PASS
- [x] Manual Acceptance PASS
- [x] M6 — ACCEPTED / FROZEN

最终记录：

```text
M6 Decision: ACCEPTED / FROZEN
Reviewer: Human Reviewer（用户）
Date: 2026-10-04
Production commit: 7083fbc4862340b73f083f4b7b4912b626909662
Evidence commit: 3c51941c3ba4a4c8556bb76a703e1639e387704e
Acceptance build: artifacts/M6-i18n/ChatGPT-Harness_0.1.1_x64-portable (build HEAD: 3c51941c3ba4a4c8556bb76a703e1639e387704e)
Windows display/system locale used: 用户未单独记录具体系统 locale；不补充推断。
Notes: 用户确认 M6 Manual Acceptance 已完成并通过；简体中文 / English 可正常切换、即时生效。
```

收口依据：用户于 2026-10-04 明确确认人工验收完成并通过；本记录按该确认收口，不补充逐项操作细节。既有 Integration 40/40 PASS、Local Black-box E2E 22/22 PASS（LOCAL_M6_E2E=PASS）保持不变。本次仅更新文档，没有重新执行测试、构建或修改 M6 production code。

冻结后的后续规划为 M7 Migration / Evidence / UX Hardening；本次未启动 M7。M7 可以扩展 config schema与最终 UX，但不得重新把 language preference塞进 Runtime protocol或让语言切换要求 Runtime restart。
