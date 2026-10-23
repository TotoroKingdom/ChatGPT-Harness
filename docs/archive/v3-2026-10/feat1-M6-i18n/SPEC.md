# M6 SPEC — Chinese / English UI

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M6 — Chinese / English UI  
> Baseline: M5 — Controlled Bash, ACCEPTED / FROZEN  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, M0–M5 milestone docs

## 1. Objective

M6 为 ChatGPT-Harness V3 提供完整的双语 presentation layer：

```text
简体中文 (zh-CN)
English (en)
```

要求：

```text
首次启动无 saved language
→ 跟随 OS language
→ unsupported locale fallback English

Settings 手动切换
→ UI 即时更新
→ 本地持久化
→ 不需要 Runtime restart
→ 应用重启后保持
```

M6 只改变 React/Tauri presentation preference，不改变 Runtime authorization / execution / MCP semantics。

## 2. Baseline Facts

M5 后当前代码已经具备：

- M5 `ACCEPTED / FROZEN`。
- React UI 页面：Overview / Tools / Policies / Activity / Approvals / Settings / First Run。
- AppShell / Sidebar / shared status components。
- `HarnessControlClient.configGet()` / `configSave()`。
- `UiConfig` 当前只有 optional `theme`，没有 language。
- `RuntimeConfigSnapshot` 已经不包含 `UiConfig`。
- ConfigStore 当前 `same_semantics` 会把整个 `AppConfig`（除 revision）纳入 `configRevision` 比较。
- UI package当前没有 i18next/react-i18next。
- `main.tsx` 当前直接 render `<App />`。
- routes / Settings / Activity / shared async states存在大量 hard-coded English UI copy。
- Control/API errors通常已有 machine `code` + fallback `message`；部分 UI helper目前过早转换成固定 English string。
- Runtime control-read models已经大量使用 typed enum/category，可供 UI 本地化显示。

M6 应保留这些 transport/data contracts，只把用户可见 presentation统一迁移到 i18n resources。

## 3. Scope

M6 实现：

1. `UiLanguage` typed config。
2. `UiConfig.language?` persistence。
3. OS language detection。
4. English fallback。
5. pre-render locale bootstrap。
6. `i18next + react-i18next` React presentation foundation。
7. `en.json` / `zh-CN.json` resources。
8. Settings language selector。
9. immediate language switch。
10. language persistence without Runtime restart。
11. V3主要 UI 页面完整双语。
12. shared status / loading / empty / confirmation / form validation文案双语。
13. machine-code → localized error explanation mapping。
14. typed status/decision/availability/activity/risk labels双语。
15. Policy / Approval friendly presentation双语。
16. resource key parity / placeholder parity tests。
17. M0–M5 functional regression。

M6 不实现：

- Runtime中文输出。
- MCP tool name translation。
- MCP schema/field translation。
- Control protocol field translation。
- machine-readable enum/value translation on the wire。
- Runtime machine identifier translation。
- Policy internal ID translation。
- command/path/file content translation。
- user-authored Policy name/reason automatic translation。
- external ChatGPT locale control。
- documentation/README full translation。
- RTL language support。
- third product language。
- M7 config schema v1→v2 migration。
- M7 evidence/UX hardening。

## 4. Architecture Boundary

Architecture remains:

```text
React presentation
└ i18n

Tauri / Runtime / MCP
└ language-neutral machine data
```

Formal rule:

> i18n only exists in the React presentation layer. Runtime and Control remain language-neutral and continue returning machine codes/enums plus fallback diagnostic text where already supported.

Language preference is host/UI configuration and must never enter `RuntimeConfigSnapshot`.

## 5. Supported Language Contract

Add protocol/config type:

```text
UiLanguage
├ en
└ zh-CN
```

Recommended Rust/wire representation:

```json
"en"
"zh-CN"
```

`UiConfig` becomes conceptually:

```text
UiConfig
├ theme?
└ language?: UiLanguage
```

Semantics:

```text
language present
→ explicit saved user preference

language absent
→ follow system language at application startup
```

Old schema v1 config missing `ui.language` remains valid and uses system detection.

M6 does not increase `schemaVersion`.

## 6. Language Resolution

### 6.1 Precedence

At app bootstrap:

```text
saved UiConfig.language?
├ en     → en
├ zh-CN  → zh-CN
└ absent → resolve OS/browser WebView locale
```

If config cannot be loaded safely:

```text
resolve system locale
→ render recovery/config-error UI
```

Config read failure must not prevent the application from rendering.

### 6.2 OS locale mapping

Use `navigator.languages` / `navigator.language` (or an equivalent React-presentation-layer system-locale signal) as the OS/WebView locale source.

Deterministic mapping:

```text
en / en-*                    → en
zh-CN / zh-SG / zh-Hans*     → zh-CN
plain zh                     → zh-CN
zh-TW / zh-HK / zh-MO / Hant → unsupported → en
all other locales            → en
missing/invalid locale       → en
```

Traditional Chinese is not silently converted into Simplified Chinese because it is not an M6 supported locale.

### 6.3 Runtime OS-language changes

M6 does not require watching OS locale changes while the application is already running.

When `ui.language` is absent, a changed OS language is picked up on the next application launch.

## 7. Pre-render Bootstrap

Avoid rendering a full page in the wrong saved language and switching after React mounts.

Recommended flow:

```text
main.tsx
→ configGet()
→ resolveLanguage(saved?, system locales)
→ init i18n
→ set <html lang>
→ render React App
```

If `configGet()` fails:

```text
resolve system language
→ init i18n
→ render App
→ existing ConfigError recovery flow remains available
```

Tests/dev/mock environments without Tauri must have a deterministic bootstrap seam and default English fallback.

## 8. i18n Library / Resource Layout

Use Architecture-recommended:

```text
i18next
+
react-i18next
```

Add UI dependencies in `ui/package.json`.

Resource layout:

```text
ui/src/i18n/
├ index.ts
├ resolve-language.ts
├ en.json
└ zh-CN.json
```

Single resource files are sufficient for M6; no namespace splitting is required unless implementation size materially benefits.

Suggested key groups:

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

English and Chinese resources must have exact key parity.

## 9. Resource Authoring Rules

### 9.1 No sentence concatenation

Do not construct translated sentences by concatenating fragments such as:

```text
t("saved") + toolName + t("restart")
```

Use complete resource messages with interpolation:

```text
t("tools.restartRequiredFor", { tool: toolName })
```

### 9.2 Placeholder parity

For every translation key:

- same interpolation placeholder names in `en` and `zh-CN`。
- no missing placeholder。
- no extra placeholder。

### 9.3 Identifiers remain data

Interpolated identifiers are inserted verbatim and never translated.

Examples:

```text
read_file
bash
system-deny-outside
request UUID
C:\workspace\...
git status
```

## 10. Settings Language UX

Settings adds a Language control separate from Runtime-affecting configuration save/restart semantics.

Required choices:

```text
Follow system
English
简体中文
```

Persistence mapping:

```text
Follow system → UiConfig.language = None
English       → en
简体中文      → zh-CN
```

Language option names should remain self-identifying enough for a user to recover even if the current locale is unfamiliar.

### 10.1 Immediate switch

On selection:

```text
select preference
→ change React i18n language immediately
→ persist UI preference locally
→ refresh persisted config truth
```

No Runtime restart.

If persistence fails:

```text
rollback to previous language preference
→ show localized save failure
```

UI must not claim the preference is saved when persistence failed.

### 10.2 Do not commit unrelated Settings draft

Language switching must not accidentally save unsaved Workspace/Tunnel/Security/Bash edits from the Settings form.

Use an isolated UI-preference mutation path.

Recommended implementation:

```text
HarnessControlClient.uiLanguageSet(language | null)
→ Tauri command
→ ConfigStore load/create current config
→ update only ui.language
→ atomic save
```

This is React↔Tauri presentation/config plumbing only; no Runtime Control method is added.

## 11. Config Revision / Restart Semantics

This is a hard M6 invariant.

`configRevision` identifies Harness/Runtime-affecting desired configuration. `UiConfig` is presentation-only and excluded from Runtime snapshot.

Therefore:

> **Changing only `ui.language` (or another UI-only `UiConfig` field) must persist without incrementing `configRevision`.**

Refactor semantic comparison conceptually to:

```text
same_restart_semantics(old, new)
→ compare AppConfig excluding:
   - configRevision
   - UiConfig presentation-only fields
```

Rules:

```text
language-only save
→ persisted language changes
→ configRevision unchanged
→ restartRequired unchanged

language + Runtime-affecting change in one transaction
→ configRevision increments once because of Runtime-affecting change
```

If `restartRequired` is already true because of another config change, changing language must not clear it.

M6 must add regression coverage for these cases.

## 12. Translation Boundary — Translate

All major user-visible static/product copy must come from i18n resources, including:

- App navigation labels / eyebrows。
- page titles/descriptions。
- buttons。
- tabs/filters where present。
- StatusBadge labels。
- loading / empty / unavailable states。
- notices / warnings / confirmations。
- Settings labels/help/validation copy。
- First Run Wizard copy。
- Overview lifecycle/status explanations。
- Tools desired/effective/restart labels。
- Bash availability/remediation friendly explanations。
- Policies editor labels/help/decision labels。
- Approval Center actions/status/risk/friendly explanations。
- Activity event/outcome/failure category labels。
- Diagnostics user-facing labels。
- form placeholders。
- accessibility labels / titles / `aria-label` copy。
- known backend/control error explanations。
- known system-policy friendly names/reasons。

## 13. Translation Boundary — Do Not Translate

The following remain raw machine/user data:

### Tool identities

```text
read_file
glob
write
edit
bash
```

UI may translate a surrounding descriptor, but the canonical tool name remains visible/raw where identity is shown.

### Protocol / machine values

Do not mutate on the wire:

- protocol field names。
- serialized enum values。
- error codes。
- capability identifiers。
- activity event machine values。

UI may show localized **labels** for typed values while retaining raw identity in diagnostics where appropriate.

### Policy identity

Do not translate:

- Policy internal ID。
- matched rule IDs。

Known System Rule friendly display name/reason may be localized by ID mapping while the raw ID remains unchanged.

User-created Policy `name` / `reason` are user-authored content and are displayed verbatim.

### Runtime / request data

Do not translate:

- Runtime ID。
- request/approval IDs。
- timestamps as machine data。
- paths。
- commands / argv。
- stdout/stderr。
- file content。
- user-entered config values。

## 14. Typed Enum / Status Presentation

Centralize presentation mappings for machine values.

Examples:

```text
PolicyDecision: allow / ask / deny
ToolAvailability: available / available_with_approval / unavailable_by_mode / ...
ComponentStatus: stopped / starting / ready / degraded / error / stopping
ApprovalStatus / ApprovalDecision
RiskLevel
PolicyReason
ActivityEventType
ActivityOutcome
ActivityFailureCategory
BashClassification / BashRiskFlag
SecurityMode
ToolReadiness
```

Recommended helpers:

```text
policyDecisionKey(value)
toolAvailabilityKey(value)
activityEventKey(value)
errorCodeKey(value)
...
```

The mapping layer returns translation keys, not English prose.

Unknown enum values should already fail at typed adapter/protocol boundaries; UI must not invent an ALLOW/success fallback.

## 15. Backend Error Localization

Architecture contract:

```text
Runtime / Tauri
→ machine code + fallback message

React
→ machine code → localized explanation
```

### 15.1 Known codes

Known user-facing error codes map to i18n resources, for example:

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
...
```

Do not make service/data-layer helpers return fixed English UI strings.

For example current control-read error normalization should evolve from:

```text
error → English sentence
```

to:

```text
error → typed/machine error code
→ presentation translates code
```

### 15.2 Fallback diagnostic message

For a known code:

- show localized user-facing explanation。
- fallback/backend message may remain available as technical detail when useful。

For an unknown/unmapped code:

- use localized generic error text。
- preserve safe backend fallback message as diagnostic detail if already sanitized。
- never infer success/ALLOW from unknown error state。

M6 does not require translating arbitrary dynamic Rust error strings.

## 16. Policy Friendly Text

### System Rules

Known Runtime-owned System Rule IDs may map to localized friendly names/reasons:

```text
system rule ID (raw)
→ localized friendly name
→ localized friendly reason
```

Raw ID remains available and unchanged.

### User Rules

User-created:

```text
name
reason
```

remain exactly as entered. M6 does not machine-translate user policy content.

### Decisions

Machine values remain:

```text
allow / ask / deny
```

Display labels become localized:

```text
Allow / Ask / Deny
允许 / 询问 / 拒绝
```

## 17. Activity / Audit Presentation

Activity protocol/control-read data remains language-neutral.

UI localizes typed fields:

- event type。
- lifecycle state。
- policy decision。
- policy reason。
- outcome。
- failure category。
- Bash classification/risk flag labels。

Tool name, rule IDs, command preview and request IDs remain raw.

Do not change Runtime Activity evidence solely to store Chinese/English prose.

## 18. Approval Presentation

Approval Center localizes:

- status labels。
- Allow once / Allow for session / Deny / Create rule actions。
- risk labels。
- policy friendly explanation。
- expiry/help/warning copy。
- Bash unclassified/session-ineligible explanation。

Approval identity data remains raw:

- request ID。
- tool name。
- normalized scope/path/command。
- matched rule IDs。

Switching language while an approval is Pending must not alter approval identity or Runtime state.

## 19. Tools / Bash Presentation

Translate:

- desired/effective labels。
- readiness labels。
- unavailable-by-mode/policy/runtime explanations。
- Restart Required。
- Bash Auto Detect / Manual Override controls。
- no-sandbox warnings/remediation。

Do not translate:

- `bash` tool name。
- executable path。
- command examples。
- capability machine identifier in technical detail。

M5 no-fake-sandbox wording must remain semantically accurate in both languages.

## 20. Page Coverage

M6 primary coverage gate includes all current main surfaces:

```text
AppShell / Sidebar
First Run Wizard
Overview
Tools
Policies
Activity
Approvals
Settings
shared loading/empty/error/status components
shared confirmation dialogs/notices
```

A page is not considered translated if its title is localized but buttons/help/error/empty states remain hard-coded English.

## 21. Accessibility / DOM Language

On resolved language change:

```text
document.documentElement.lang = "en" | "zh-CN"
```

Translated accessibility strings include:

- `aria-label`。
- screen-reader-only text。
- button title/tooltips where user-facing。
- form error descriptions。

Language switching must not remove focus or recreate Runtime-facing state unnecessarily.

## 22. Resource Quality Requirements

### English

English resource represents the existing intended product semantics, with stale milestone copy corrected only where already clearly obsolete in the current M5 product.

M6 is not a general copy redesign.

### Simplified Chinese

Chinese translation must:

- use Simplified Chinese。
- preserve technical identifiers in English/raw form。
- use consistent security terminology。
- not weaken warnings。
- not translate Bash/Policy semantics into misleading claims。

Recommended terminology glossary:

```text
Workspace            → 工作区
Tools                → 工具
Policies             → 策略
Approval             → 审批
Allow once           → 仅允许本次
Allow for session    → 本次会话允许
Deny                 → 拒绝
Restart Required     → 需要重启
Developer/Test Mode  → 开发/测试模式
Activity             → 活动记录
Runtime              → Runtime（首次可解释为运行时）
Tunnel               → Tunnel
```

Tool names and internal IDs remain raw.

## 23. Resource Integrity Tests

Automated tests must verify:

1. `en.json` / `zh-CN.json` same key set。
2. no empty translation values。
3. interpolation placeholder names match per key。
4. supported locale list exactly `en`, `zh-CN`。
5. fallback locale = `en`。
6. critical key groups exist for every primary page。

Missing Chinese key must not silently ship as an accidental English-only primary UI surface.

Runtime fallback to English for an unknown locale is allowed; missing resource keys in supported locales are a test failure.

## 24. UI-only Persistence Error Semantics

Language preference persistence uses the same local atomic ConfigStore safety.

On failure:

- do not corrupt existing config。
- do not change Runtime configRevision。
- restore previous language selection/state。
- show localized error。

No SecretStore interaction is required.

## 25. Runtime / MCP Non-Impact

M6 must not change:

- RuntimeConfigSnapshot content except generated type changes needed to keep `UiConfig` excluded。
- ToolRegistry。
- PolicyEngine evaluation。
- Approval identity。
- Session grants。
- Bash process semantics。
- MCP tool names/schemas/annotations。
- Tunnel protocol。
- Runtime lifecycle。

Language switch while Harness is running must not send Runtime initialize/restart/control mutation.

## 26. Expected Code Touchpoints

```text
protocol/src/v1/config.rs
protocol generated TS/schema

ui/package.json
ui/package-lock.json
ui/src/main.tsx
ui/src/i18n/index.ts                 # new
ui/src/i18n/resolve-language.ts      # new
ui/src/i18n/en.json                  # new
ui/src/i18n/zh-CN.json               # new

ui/src/app/router/routes.ts
ui/src/app/providers/*
ui/src/components/layout/*
ui/src/components/status/*
ui/src/components/common/* as needed
ui/src/pages/overview/*
ui/src/pages/tools/*
ui/src/pages/policies/*
ui/src/pages/activity/*
ui/src/pages/approvals/*
ui/src/pages/settings/*
ui/src/services/harness/*

ui/src-tauri/src/config/mod.rs
ui/src-tauri/src/commands/* as needed for isolated language persistence

related tests
```

Runtime production modules should not need language-aware changes.

## 27. Development Verification Requirements

Each Task must include unit/code-level tests.

Development completion runs at least:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm test
npm run typecheck
npm run build
git diff --check
```

Automated coverage must include:

```text
UiConfig language missing → system detection
saved en overrides system zh-CN
saved zh-CN overrides system en
unsupported system locale → en
Traditional Chinese locale → en
language-only save preserves configRevision
language-only save does not create Restart Required
existing restartRequired remains true across language switch
persistence failure rolls UI locale back
resource key parity
placeholder parity
AppShell/nav bilingual
Settings/FirstRun bilingual
Tools bilingual labels with raw tool names
Policies bilingual friendly labels with raw IDs/user text
Approval bilingual controls with same request identity
Activity typed labels bilingual
known error-code localization
unknown error generic localized fallback
<html lang> update
M0–M5 functional regression
```

## 28. Post-TASK Verification Documents

After SPEC / PLAN / TASKS are frozen, generate separately:

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E remain Coding Agent responsibilities. Manual Acceptance is the only human layer.

M6 Local Black-box E2E should verify language persistence and no Runtime restart/revision mutation using the real Tauri/config boundary where practical; External ChatGPT is not required because M6 does not change MCP capability semantics.

## 29. Final Acceptance Criteria

M6 can be `ACCEPTED / FROZEN` only after proving:

- English and Simplified Chinese resources are complete and key-aligned。
- first launch without saved language follows supported OS locale mapping。
- unsupported OS locale falls back English。
- saved language overrides OS detection。
- Settings can switch immediately between English / 简体中文 and optionally restore Follow system。
- language choice persists across app restart。
- language-only change does not increment Runtime configRevision or require Harness restart。
- all primary V3 pages/statuses/prompts/error explanations use i18n resources。
- typed machine statuses are localized only at presentation layer。
- tool names/protocol fields/Runtime identifiers/Policy IDs remain unmodified。
- user-authored Policy text remains verbatim。
- known backend errors use localized explanation with safe fallback detail semantics。
- M5 Bash risk/no-sandbox messaging remains accurate in both languages。
- M0–M5 functional regression PASS。
- Integration / Local E2E / Manual Acceptance complete。

## 30. Open Questions

```text
0
```

Local component naming and exact translation wording may be refined during implementation, but supported locales, language resolution precedence, UI-only revision semantics, translation boundaries and Runtime/MCP non-impact are frozen by this SPEC.
