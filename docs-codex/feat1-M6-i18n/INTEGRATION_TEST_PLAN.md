# M6 Integration Test Plan — Chinese / English UI

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M6 — Chinese / English UI  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: UiConfig / ConfigStore UI-only persistence / locale resolution / i18n bootstrap / resource integrity / presentation mapping / React page integration. This is not Local E2E and does not require External ChatGPT.

## 1. Objective

验证 M6 各 Task 组合后形成一致的双语 presentation layer：

```text
saved UiConfig.language?
        +
system locale
        ↓
resolveLanguage
        ↓
i18n bootstrap before render
        ↓
React presentation mappings
        ↓
all primary pages
        ↓
Settings immediate switch / isolated persistence
```

核心不变量：

- i18n 只存在于 React presentation layer。
- Runtime / MCP / Control wire values保持 language-neutral。
- language-only persistence 不增加 Runtime `configRevision`。
- language-only persistence 不产生新的 Restart Required。
- 已存在的 Restart Required 不因切换语言被清除。
- supported locale 只有 `en` / `zh-CN`。
- missing saved language → system detection。
- unsupported system locale → English。
- machine identifiers和用户数据不翻译。
- supported resources不能靠 accidental English fallback掩盖漏翻。

## 2. Entry Criteria

执行前必须满足：

- T1–T5 development complete。
- Unit/code-level tests PASS。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- `npm test` PASS。
- `npm run typecheck` PASS。
- `npm run build` PASS。
- `git diff --check` PASS。

若实现未完成：

```text
Integration = BLOCKED_BY_IMPLEMENTATION
```

## 3. Integration Boundary

Integration 可以：

- 使用真实 ConfigStore + temporary `config.json`。
- 调用真实 isolated language persistence command/service。
- 使用 jsdom / React Testing Library渲染真实页面和 Provider。
- 使用真实 i18next resources。
- 注入 deterministic `navigator.languages` seam。
- 使用真实 typed Control read fixtures验证 presentation mapping。

Integration 不应：

- 修改 Runtime production logic以配合翻译测试。
- 通过 snapshot text replacement冒充 React即时切换。
- 只检查页面标题就声称页面完成翻译。
- 翻译 raw protocol values来让断言通过。
- 依赖人工桌面操作；人工属于 Manual Acceptance。

## 4. Required Integration Cases

### INT-01 — Legacy UiConfig without language remains valid

schema v1 config：

```text
ui.language missing
```

期望：

- load成功。
- `language = None/undefined`。
- schemaVersion仍为 1。
- 其它 M0–M5 config保持。

### INT-02 — UiLanguage round-trip

分别保存：

```text
en
zh-CN
None / Follow system
```

reload后精确保持。

### INT-03 — Invalid persisted language fails typed config validation

手工 config写入 unsupported serialized language，例如：

```text
fr
zh-TW
```

期望 typed config fail closed；不得静默当作 saved English preference。

系统 locale unsupported 与 persisted invalid config是不同语义。

### INT-04 — Language-only save preserves configRevision

已有 revision N，仅改变：

```text
ui.language: en → zh-CN
```

期望：

```text
persisted language = zh-CN
configRevision = N
```

### INT-05 — Follow-system save preserves configRevision

`zh-CN → None`：

- persisted preference清除。
- configRevision不变。

### INT-06 — Runtime-affecting config still increments revision

language保持不变，仅改变例如 Tool/Security/Bash desired config。

期望 revision按 M2–M5语义增加。

证明 M6没有把所有 config变化都排除在 revision之外。

### INT-07 — Language + Runtime-affecting change increments exactly once

同一个完整 config transaction同时改变：

- `ui.language`。
- Runtime-affecting field。

期望 revision只因 Runtime-affecting semantic change增加一次。

### INT-08 — Existing Restart Required survives language switch

fixture：

```text
active revision = N
latest persisted revision = N+1
restartRequired = true
```

切换 language。

期望：

- latest revision仍 N+1。
- restartRequired仍 true。
- UI语言即时改变。

### INT-09 — Clean Runtime does not become restart-required after language switch

fixture：active=latest=N。

language-only save后：

```text
active=N
latest=N
restartRequired=false
```

### INT-10 — Language persistence does not save unrelated Settings draft

Settings form内先编辑未保存的 Workspace/Tunnel/Security/Bash字段，再切换语言。

期望 persisted config只改变 `ui.language`；其它 draft不写入 ConfigStore。

### INT-11 — Language persistence failure rolls back locale

当前 `en`，尝试切 `zh-CN`，让 isolated persistence失败。

期望：

- UI恢复 `en`。
- selector恢复旧 preference。
- persisted config不变。
- localized error可见。
- configRevision不变。

### INT-12 — Saved English overrides Chinese system locale

```text
saved=en
system=zh-CN
→ en
```

### INT-13 — Saved zh-CN overrides English system locale

```text
saved=zh-CN
system=en-US
→ zh-CN
```

### INT-14 — Simplified Chinese locale mapping

至少：

```text
zh-CN → zh-CN
zh-SG → zh-CN
zh-Hans → zh-CN
zh-Hans-CN → zh-CN
zh → zh-CN
```

### INT-15 — Traditional Chinese locales fallback English

至少：

```text
zh-TW
zh-HK
zh-MO
zh-Hant
```

均 → `en`。

### INT-16 — Unsupported / missing locale fallback English

覆盖：

```text
fr-FR
ja-JP
empty/invalid
no navigator locale
```

均 → `en`。

### INT-17 — navigator.languages precedence is deterministic

若有多个系统 locale，验证 resolver按冻结顺序选择首个有效系统 preference，不被后续 locale意外覆盖。

### INT-18 — Bootstrap resolves locale before full App render

使用 bootstrap seam验证：

```text
configGet
→ resolve
→ i18n init
→ html lang
→ App render
```

已保存 `zh-CN` 时不得先完整渲染 English App再异步切换。

### INT-19 — Config read failure still renders recovery UI

`configGet`失败：

- 使用 system locale/fallback初始化 i18n。
- App仍可 render。
- existing config-error recovery可用。
- 不因 bootstrap异常白屏。

### INT-20 — `<html lang>` follows resolved language

初始化、即时切换、Follow system切换后分别验证：

```text
<html lang="en">
<html lang="zh-CN">
```

### INT-21 — Resource key parity

递归比较 `en.json` / `zh-CN.json`：

- exact same key set。
- no missing key。
- no extra primary-locale key。

### INT-22 — Resource values non-empty

所有 leaf translation string非空/非纯 whitespace。

### INT-23 — Interpolation placeholder parity

每个 key：

```text
{{tool}}
{{count}}
{{path}}
...
```

两语言 placeholder name集合完全一致。

### INT-24 — Critical resource groups exist

至少：

```text
app
common
nav
status
overview
tools
policies
activity
approvals
settings
firstRun
errors
policy
bash
validation
```

### INT-25 — Navigation / AppShell switches instantly

同一 mounted App：

```text
en → zh-CN → en
```

验证 Sidebar、page label、eyebrow、topbar shared copy即时更新，无 Runtime/provider reset。

### INT-26 — Settings / First Run bilingual integration

两语言分别验证：

- labels。
- help text。
- buttons。
- validation。
- confirmation。
- empty/error/success notices。
- language selector自识别。

### INT-27 — Tools presentation localizes labels but not tool identities

两语言下：

- availability/decision/readiness/restart/remediation本地化。
- `read_file/glob/write/edit/bash`保持 raw。
- Bash executable path保持 raw。
- Developer/Test/no-sandbox warning语义一致。

### INT-28 — Policies presentation preserves raw identity/user text

两语言：

- Allow/Ask/Deny labels本地化。
- matcher/help/system friendly copy本地化。
- System Rule raw ID保持。
- matched IDs保持。
- user-authored rule `name/reason`完全原样。

### INT-29 — Approval presentation switches without changing Runtime request identity

构造 Pending approval，运行中切换语言。

期望：

- actions/status/risk/help本地化。
- request ID/tool/scope/command/matched IDs不变。
- 不调用 `approvals.resolve`。
- pending状态不变。

### INT-30 — Activity typed values localize at presentation layer

两语言验证：

- event type。
- lifecycle state。
- decision。
- policy reason。
- outcome。
- failure category。
- Bash classification/risk flags。

同时：tool name/request ID/rule IDs/command preview保持 raw。

### INT-31 — Known backend errors use localized explanations

至少覆盖 representative machine codes：

```text
CONFIG_INVALID
CONFIG_WRITE_FAILED
RUNTIME_UNAVAILABLE
RUNTIME_NOT_READY
TOOL_DISABLED
POLICY_DENIED
APPROVAL_DENIED
BASH_UNAVAILABLE
BASH_TIMEOUT
```

English/Chinese分别得到对应本地化 user-facing explanation。

### INT-32 — Unknown error uses localized generic fallback

unknown machine code：

- 不渲染 success。
- 使用当前 locale的 generic error。
- 已 sanitized fallback message若展示，保持 diagnostic detail而不是被机器翻译。

### INT-33 — Shared loading / empty / error components bilingual

覆盖：

- ControlReadView。
- LoadingState。
- empty cards。
- generic notices。
- retry/refresh actions。

不允许 shared component继续硬编码 English primary copy。

### INT-34 — Accessibility strings switch language

验证 representative：

- `aria-label`。
- title/tooltips。
- form error descriptions。
- screen-reader-only text。

切换语言后更新，并保持 focus/state。

### INT-35 — Identifiers are never passed through translation as keys

对：

```text
read_file
system-deny-outside
request UUID
C:\workspace\x
git status
```

验证最终 DOM保持 verbatim，不发生 localized substitution。

### INT-36 — Runtime/MCP snapshot remains language-neutral

构造 `en` 与 `zh-CN` UiConfig，比较传给 Runtime的 snapshot/Control/MCP行为。

期望除 host-side AppConfig UI preference外：

- RuntimeConfigSnapshot相同。
- ToolRegistry相同。
- Policy behavior相同。
- MCP schemas/tool names相同。

### INT-37 — Language switch sends no Runtime lifecycle mutation

mock/spy真实 Harness client boundary：

切语言不得调用：

```text
runtimeStart
runtimeStop
harnessRestart
Runtime initialize/control mutation
```

只走 UI-preference persistence + config refresh。

### INT-38 — M5 Bash warning semantic parity

对 no-sandbox / Developer/Test / unclassified risk / session-ineligible等关键安全文案，验证英文与中文表达同一安全含义；中文不得弱化为“安全 Bash”或“仅工作区访问”。

### INT-39 — Primary-page hard-coded copy audit test

对 primary surfaces建立 source/resource guard（可用 targeted lint/test/allowlist）：

```text
AppShell / Sidebar
FirstRun
Overview
Tools
Policies
Activity
Approvals
Settings
shared status components
```

禁止新增明显用户可见 hard-coded English copy，允许 machine identifiers/test fixtures/technical constants。

### INT-40 — M0–M5 functional regression

切换语言前后验证核心 action callback仍调用同一 underlying operation：

- Start/Stop/Restart Harness。
- Tool config save。
- Policy save。
- Approval resolve。
- Bash config。
- read/control refresh。

语言只改变 presentation，不改变 authority/data flow。

## 5. Evidence Format

每个 case记录：

```text
Case ID
PASS / FAIL / BLOCKED
Test name / command
Saved language preference
System locale fixture
Resolved locale
Config revision before/after where applicable
RestartRequired before/after where applicable
Raw machine value vs displayed label where applicable
```

## 6. Integration Exit Criteria

M6 Integration只有 INT-01 ～ INT-40全部 PASS才为 PASS。

特别要求：

- locale resolution PASS。
- pre-render bootstrap PASS。
- language-only revision semantics PASS。
- isolated persistence/rollback PASS。
- resource key/placeholder parity PASS。
- all primary surfaces双语 PASS。
- identifiers/user content保持 raw PASS。
- error-code localization PASS。
- Runtime/MCP non-impact PASS。
- M0–M5 functional regression PASS。

随后进入 `E2E_TEST_PLAN.md`。

## 7. Non-Claims

Integration PASS不代表：

```text
real desktop visual acceptance PASS
all arbitrary Rust diagnostic strings translated
External ChatGPT language controlled
M7 config migration complete
M6 ACCEPTED / FROZEN
```
