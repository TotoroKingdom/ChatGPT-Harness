# M6 TASKS — Chinese / English UI

> Inputs: `SPEC.md`, `PLAN.md`  
> Executor: Codex / Coding Agent  
> Baseline: M5 ACCEPTED / FROZEN  
> 每个 Task 必须包含实现 + Unit Test / code-level automated verification。  
> Integration / E2E / Manual Acceptance 不属于本文件。

## Dependency

```text
T1 UiLanguage / UI-only persistence semantics
        ↓
T2 i18n bootstrap / resources / locale resolution
        ↓
T3 shared status / error / machine-value presentation mapping
        ↓
T4 translate all primary UI surfaces + Settings switch
        ↓
T5 resource coverage / regression / development completion
```

---

## T1 — UiLanguage / UI-only Persistence Semantics

**Primary files:** `protocol/src/v1/config.rs`, generated TS/schema, `ui/src-tauri/src/config/mod.rs`, Tauri commands, Harness client/provider, tests.

### Implement

#### Typed language config

Add:

```text
UiLanguage
├ en
└ zh-CN
```

`UiConfig`:

```text
theme?
language?
```

Requirements:

- old schema v1 config missing language loads successfully。
- default language preference = absent / follow system。
- invalid serialized language fails config validation rather than silently becoming another locale。
- `RuntimeConfigSnapshot` remains unchanged and excludes UiConfig。

#### Config revision semantics

Refactor `same_semantics` into restart-affecting semantics.

Language/theme-only changes:

```text
persist
configRevision unchanged
```

Runtime/Harness-affecting changes keep current behavior and increment once.

Do not exclude Workspace/Tunnel/Security/Tools/Policies/Bash from revision comparison.

#### Isolated language persistence

Add a focused React↔Tauri preference mutation, recommended:

```text
uiLanguageSet(language | null)
```

Implementation:

- load current config / safe defaults。
- update only `ui.language`。
- atomic save。
- return persisted config/preference。

Update real/mock/local client/provider surfaces.

Do not use Runtime Control RPC.

### Required Tests

At minimum:

- legacy config missing `ui.language`。
- round-trip `en`。
- round-trip `zh-CN`。
- clear language → None/follow system。
- invalid locale config fails closed。
- language-only save revision unchanged。
- theme-only save revision unchanged。
- language + tool change increments exactly once。
- language + security change increments exactly once。
- language + tunnel change increments exactly once。
- isolated language set does not overwrite unrelated persisted fields。
- isolated language set does not save page-local unsaved values。
- atomic save failure preserves previous config。
- Runtime snapshot serialization still has no UI language。

### Done

- Language preference has one typed local source of truth。
- UI-only preference does not create Restart Required debt。
- No M7 migration work introduced。

---

## T2 — i18n Bootstrap / Resources / Locale Resolution

**Primary files:** `ui/package.json`, lockfile, `ui/src/main.tsx`, new `ui/src/i18n/*`, tests.

### Implement

Add dependencies:

```text
i18next
react-i18next
```

Create:

```text
ui/src/i18n/index.ts
ui/src/i18n/resolve-language.ts
ui/src/i18n/en.json
ui/src/i18n/zh-CN.json
```

#### Locale resolver

Pure function with frozen precedence:

```text
saved language > system locale > English fallback
```

System mapping:

```text
en/en-* → en
zh-CN/zh-SG/zh-Hans* → zh-CN
plain zh → zh-CN
zh-TW/zh-HK/zh-MO/zh-Hant* → en
other/invalid/missing → en
```

Use `navigator.languages` / `navigator.language` or equivalent presentation-layer source.

#### Bootstrap before render

Refactor startup:

```text
configGet
→ resolve locale
→ initialize i18n
→ set document.documentElement.lang
→ render App
```

Config read failure:

```text
system detect / en fallback
→ still render App
```

Do not consume/clear the later ConfigError flow.

#### Base resources

Create full key skeleton by domain:

```text
app/common/nav/status
overview/tools/policies/activity/approvals/settings/firstRun
errors/policy/bash/validation
```

At this task, resources may contain keys required by shared shell/bootstrap plus placeholders for later page migration, but both locales must remain parity-valid at every commit.

### Required Tests

- saved en overrides system zh-CN。
- saved zh-CN overrides system en-US。
- no saved + en-US → en。
- no saved + zh-CN → zh-CN。
- no saved + zh-SG → zh-CN。
- no saved + zh-Hans → zh-CN。
- no saved + zh-TW/zh-HK/zh-Hant → en。
- unsupported fr/de/ja → en。
- empty navigator locales → en。
- configGet failure still initializes language and renders bootstrap path。
- `<html lang>` set at startup。
- fallback language configured as en。

### Done

- App can resolve and initialize language before primary React render。
- Runtime/Tauri does not produce translated strings。

---

## T3 — Shared Status / Error / Machine-value Presentation Mapping

**Primary files:** new `ui/src/i18n/presentation.ts` or equivalent, router/layout/shared status components, `ui/src/services/harness/control-read.ts`, providers/hooks, tests.

### Implement

#### Translation-key mapping

Centralize typed machine-value→translation-key mapping for at least:

```text
PageKey
ComponentStatus
OverallStatus
SecurityMode
PolicyDecision
PolicyReason
ToolAvailability
ToolReadiness
RiskLevel
ApprovalStatus
ApprovalDecision
ActivityCategory
ActivityEventType
ActivityLifecycleState
ActivityOutcome
ActivityFailureCategory
BashClassification
BashRiskFlag
known error codes
```

Mapping helpers must not return hard-coded English prose.

#### Routes / shell

Replace route `label` / `eyebrow` strings with keys or PageKey-based translated lookup.

Translate:

- Sidebar。
- AppShell topbar。
- Local only / Runtime data source copy。
- common Loading/Empty/Retry/Refresh copy。

#### Control-read error boundary

Current service helper returns English sentences.

Refactor to preserve machine meaning:

```text
unknown error
→ typed error code/descriptor
→ React presentation localizes
```

Known codes use localized resource explanations.

Unknown codes use localized generic failure plus safe fallback detail when available.

#### System policy friendly text

Add helper allowing:

```text
known system rule ID
→ localized friendly name/reason
```

Do not change raw rule ID.

User rule names/reasons remain raw.

### Required Tests

- representative mapping for every enum family。
- allow/ask/deny labels in en/zh-CN。
- availability labels bilingual。
- Activity outcome/failure labels bilingual。
- Bash risk labels bilingual。
- known error code maps to locale resource。
- unknown error never maps to success/allow。
- ControlReadView no fixed English error string dependency。
- raw tool name unchanged across locale。
- raw Policy ID unchanged across locale。
- user-authored Policy text unchanged across locale。
- AppShell/nav switches immediately without route identity change。

### Done

- Shared presentation layer can render machine data bilingually without mutating source data。
- Service/data layer no longer owns primary English copy for known typed errors/statuses。

---

## T4 — Translate Primary UI Surfaces / Settings Language UX

**Primary files:** all current page components under `ui/src/pages/**`, shared components, resources, tests.

### Implement

Translate **all** primary user-visible copy, not only headings.

#### First Run / Settings

Cover:

- Workspace setup。
- Tunnel ID/proxy/client/API Key。
- Security Mode。
- local storage facts。
- save/discard/reset/restart。
- validation messages。
- notices/confirm dialogs。
- First Run Wizard steps/buttons/help/errors。

Add Language section with:

```text
Follow system
English
简体中文
```

Language selector behavior:

1. keep previous preference/locale。
2. switch i18n immediately。
3. persist with isolated T1 mutation。
4. refresh config truth。
5. failure → rollback locale + localized error。

Language selector must work independently from `dirty` Settings form.

#### Overview

Translate:

- lifecycle labels。
- component status descriptions。
- actions/notices。
- diagnostics labels/help。

#### Tools

Translate:

- desired/effective labels。
- availability/readiness/policy labels。
- tool-toggle/restart copy。
- Bash config/readiness/remediation。
- Developer/Test restriction。
- no-sandbox warning。

Keep canonical tool names raw.

#### Policies

Translate:

- Tool Defaults/UI controls。
- User Rule CRUD controls。
- matcher labels。
- BashCommand fields/help。
- decision/status labels。
- validation/error copy。
- System Rule friendly presentation。

Keep:

- Policy IDs raw。
- user rule name/reason verbatim。
- matcher tool/executable values raw。

#### Approvals

Translate:

- statuses/actions。
- Allow once / session / Deny / Create rule。
- risk and expiry copy。
- Bash unclassified/session-ineligible warnings。
- resolve errors。

Keep request/tool/path/command/rule IDs raw.

#### Activity

Translate:

- page/refresh/empty copy。
- event type labels。
- decisions/outcomes/failure categories。
- Bash classification/risk labels if displayed。

Keep raw tool/request/rule/command/timestamp data.

#### Accessibility

Translate user-facing:

- aria-label。
- title/tooltips。
- form error descriptions。
- visually-hidden strings。

Update `document.documentElement.lang` on live language switch.

### Required Tests

Representative bilingual component tests at minimum:

- First Run English + Chinese。
- Settings English + Chinese。
- language switch immediate。
- language persistence success。
- persistence failure rollback。
- dirty Settings form remains unsaved after language switch。
- existing restartRequired stays visible across language switch。
- Overview status bilingual。
- Tools labels bilingual with raw `read_file/bash`。
- Bash no-sandbox warning semantically present in both locales。
- Policies system friendly text bilingual + raw ID。
- user rule text unchanged。
- Approval controls/status bilingual + same request ID。
- Activity typed labels bilingual + raw tool name。
- `<html lang>` live update。

### Done

- Every primary M6 surface is genuinely bilingual including secondary/error/empty states。
- No Runtime-facing behavior changed。

---

## T5 — Resource Coverage / Regression / Development Completion

**Primary scope:** resource/test audit + M0–M5 regression. No new product feature.

### Resource Integrity

Add automated resource audit:

```text
en keys == zh-CN keys
all values non-empty
placeholder names identical per key
supported locales exactly en + zh-CN
fallback locale en
critical page groups present
```

Do not allow supported-locale missing keys to silently fall back and still claim coverage PASS.

### Hard-coded Copy Audit

Review all current `.tsx` primary surfaces for remaining user-facing hard-coded English.

Allowed literals include only true machine/data values and test fixtures, e.g.:

```text
read_file
glob
write
edit
bash
protocol enum values
paths/commands
IDs
CSS class names
```

If adding an automated source scan, keep it targeted and maintainable rather than building a brittle JSX parser.

### Config / Lifecycle Regression

Verify:

- language-only save revision stable。
- no new Restart Required from language switch。
- existing Restart Required not cleared。
- current Runtime process identity unchanged during language switch。
- no runtimeStart/runtimeStop/harnessRestart call emitted by language switch。

### M0–M5 Functional Regression

Ensure unchanged behavior for:

- read/glob/write/edit。
- Tool Toggle。
- Policy evaluation。
- Approval same-request resume。
- session grants。
- Controlled Bash mode/policy/limits。
- Activity data parsing。
- Runtime/Tunnel lifecycle。

### Development Commands

Run:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm test
npm run typecheck
npm run build
git diff --check
```

All PASS.

### Done

T1–T5 completion only means:

```text
M6 Development Complete
```

Then independently generate/execute:

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E are Coding Agent responsibilities; only Manual Acceptance requires human action.

---

## Development Completion Checklist

- [ ] T1 UiLanguage/config/revision semantics PASS
- [ ] T2 i18n bootstrap/locale resolution PASS
- [ ] T3 shared typed presentation/error mapping PASS
- [ ] T4 all primary UI surfaces bilingual PASS
- [ ] T5 resource/regression PASS
- [ ] Supported locales exactly English + Simplified Chinese
- [ ] No saved language follows supported system locale
- [ ] Unsupported system locale falls back English
- [ ] Saved preference overrides system locale
- [ ] Follow system preference clears saved explicit locale
- [ ] Language switch immediate
- [ ] Language persists across relaunch/bootstrap
- [ ] Language-only save does not increment configRevision
- [ ] Language switch does not create Restart Required
- [ ] Existing Restart Required is preserved
- [ ] Persistence failure rolls locale back
- [ ] `en.json` / `zh-CN.json` key parity PASS
- [ ] interpolation placeholder parity PASS
- [ ] AppShell / FirstRun / Overview / Tools / Policies / Activity / Approvals / Settings translated
- [ ] known error-code explanations localized
- [ ] raw tool names unchanged
- [ ] raw protocol/machine values unchanged on wire
- [ ] Policy internal IDs unchanged
- [ ] user-authored Policy text unchanged
- [ ] Bash no-sandbox warning accurate in both languages
- [ ] `<html lang>` correct/live
- [ ] M0–M5 regression PASS
- [ ] cargo fmt/check/test PASS
- [ ] npm test/typecheck/build PASS
- [ ] git diff --check PASS
- [ ] TASKS does not claim Integration/E2E/Manual Acceptance PASS
