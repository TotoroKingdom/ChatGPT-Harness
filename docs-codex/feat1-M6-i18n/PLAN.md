# M6 PLAN — Chinese / English UI

> Input: `SPEC.md`  
> Baseline: M5 ACCEPTED / FROZEN  
> Goal: 在不改变 Runtime / MCP 语义的前提下，将全部主要 React presentation迁移到 English / 简体中文双语，并实现即时、本地、无需 Runtime restart 的语言切换。  
> Rule: PLAN 只决定实现路径；Integration/E2E/Manual Acceptance另行生成。

## 1. Strategy

M6 采用 presentation-first incremental migration：

```text
UiConfig.language
→ UI-only persistence semantics
→ pre-render language bootstrap
→ i18next/react-i18next
→ typed machine-value presentation mapping
→ shared components
→ every primary page
→ Settings immediate switch
→ resource integrity / regression
```

核心约束：

- Runtime remains language-neutral。
- `RuntimeConfigSnapshot` continues excluding `UiConfig`。
- Language-only persistence must not advance `configRevision`。
- User-facing strings move to resources, machine data stays raw。
- Existing control-read/policy/tool/approval data remains authoritative。

## 2. Phase A — UiLanguage Protocol / Config Persistence

Extend `protocol/src/v1/config.rs`:

```text
UiLanguage
├ En
└ ZhCn
```

Wire values:

```text
"en"
"zh-CN"
```

`UiConfig`:

```text
theme?
language?
```

Requirements:

- `language` missing remains backward-compatible。
- `UiConfig::default()` → no explicit language。
- generated TS/schema update。
- invalid serialized language remains config-invalid rather than silently picking arbitrary language。

### ConfigStore revision semantics

Replace whole-AppConfig semantic comparison with explicit restart-affecting semantics.

Conceptually:

```text
same_restart_semantics(old, new)
```

Ignore:

```text
configRevision
UiConfig presentation-only fields
```

Compare all other existing AppConfig fields exactly as before.

Tests:

- language-only change persists and revision stable。
- theme-only presentation change also revision stable。
- language + tool/policy/security/bash/tunnel/workspace change increments once。
- already-mismatched revision is not repaired by UI-only save。

Exit: `UiConfig.language` is persistent without creating Runtime restart debt.

## 3. Phase B — Isolated UI Preference Mutation

Do not bind language switch to the Settings draft transaction.

Add a focused host-side mutation, recommended shape:

```text
ui_language_set(language?: UiLanguage)
```

or equivalent typed Tauri command.

Behavior:

1. load current config or safe defaults。
2. update only `ui.language`。
3. ConfigStore atomic save。
4. return persisted AppConfig / UiConfig truth。

Update:

- real HarnessControlClient。
- mock/local client。
- HarnessDataProvider。

Requirements:

- no Runtime Control RPC。
- no SecretStore mutation。
- no accidental save of Settings unsaved form。
- save failure leaves prior persisted config intact。

Exit: language can be persisted independently and safely.

## 4. Phase C — i18n Foundation

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

Configure:

```text
fallbackLng = en
supportedLngs = [en, zh-CN]
interpolation.escapeValue = false   # React escapes output
returnNull = false
```

Use stable resource key grouping from SPEC.

### Language resolver

Pure/testable function:

```text
resolveLanguage(savedLanguage, systemLocales)
```

Implement exact mapping frozen by SPEC.

Exit: locale resolution and i18n engine work independently from page components.

## 5. Phase D — Pre-render Bootstrap

Refactor `main.tsx` into async bootstrap:

```text
bootstrap()
→ create/use Harness client or minimal config-read seam
→ configGet()
→ resolveLanguage()
→ init i18n
→ set document.documentElement.lang
→ ReactDOM.render
```

If config read fails:

```text
system language resolution
→ continue rendering App
```

Do not hide ConfigStore errors that HarnessDataProvider must later display.

Testing seams:

- saved language fixture。
- system locale fixture。
- configGet failure fixture。
- non-Tauri/browser test fallback。

Exit: app starts in correct saved language before primary UI render.

## 6. Phase E — Shared Presentation Mapping

Before translating pages one-by-one, centralize machine-value display mapping.

Create helper(s), e.g.:

```text
ui/src/i18n/presentation.ts
```

Map machine values to translation keys:

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
known error code
```

Rules:

- helper returns translation key / display descriptor, not prose。
- raw identifier remains available separately。
- unknown safety-sensitive value never maps to success/allow。

### Control-read error refactor

Current `controlReadError()` returns English prose from service/data layer.

Refactor toward:

```text
normalizeControlReadError(error)
→ typed code / descriptor
```

Then `ControlReadView` localizes it.

Exit: shared code no longer hardcodes English presentation for typed states/errors.

## 7. Phase F — App Shell / Shared Components

Migrate first:

- `routes.ts` label/eyebrow → keys or PageKey-derived mapping。
- AppShell topbar labels。
- Sidebar navigation。
- common StatusBadge usages where labels are static。
- LoadingState / EmptyState / ControlReadView。
- common notices / generic retry / refresh / save / cancel copy。

Set/update `<html lang>` whenever i18n language changes.

Do not change routing keys or PageKey machine values.

Exit: global shell can switch instantly without page remount semantics changing.

## 8. Phase G — Settings / First Run

Translate all user-visible copy in `SettingsPage.tsx` and FirstRunWizard:

- workspace。
- Tunnel。
- proxy validation。
- API Key state/actions。
- Security Mode。
- local data。
- save/discard/reset/restart notices。
- first-run steps/errors/buttons。

### Language control

Add independent Language section/control:

```text
Follow system
English
简体中文
```

On selection:

1. capture previous preference/resolved locale。
2. switch i18n immediately。
3. persist isolated preference。
4. refresh saved config state。
5. on failure, rollback locale/preference and show localized error。

The language action must not touch the page-local dirty form.

Exit: language switching is visible immediately and persistence-truthful.

## 9. Phase H — Overview / Tools

Translate Overview:

- lifecycle/status labels。
- start/stop/restart controls。
- component cards。
- warnings/remediation。
- diagnostics labels。

Translate Tools:

- Desired/Effective/Policy/Reason columns。
- toggle/help/restart copy。
- Bash config/readiness/mode restrictions。
- availability/readiness labels via typed mapping。
- Bash no-sandbox warning with equivalent security meaning in both languages。

Keep raw:

```text
read_file
glob
write
edit
bash
paths/executable path
```

Exit: operational control pages are fully bilingual.

## 10. Phase I — Policies

Translate:

- page headings/help。
- Tool Defaults labels。
- User Rules controls。
- matcher type labels。
- BashCommand editor fields/help。
- decision labels。
- validation/errors。
- Restart Required copy。

System Rules:

- map known internal IDs to localized friendly name/reason。
- raw ID remains unchanged/visible where currently shown。

User rules:

- user `name` / `reason` remain verbatim。

Do not translate serialized matcher/tool values.

Exit: Policy editor is bilingual without changing policy semantics.

## 11. Phase J — Approvals

Translate:

- Pending/Approved/Denied/Expired/Cancelled display labels。
- Allow once / Allow for session / Deny / Create rule。
- risk labels。
- expiry/help text。
- Bash risk flags / unclassified explanation。
- session-ineligible explanation。
- stale/resolve error explanation。

Keep raw:

- request ID。
- approval ID。
- tool name。
- path/command/scope。
- matched rule IDs。

Switching language during a pending approval must not cause list mutation or resolve action.

Exit: M4/M5 human approval UX is fully bilingual while identity remains stable.

## 12. Phase K — Activity / Diagnostics

Translate typed Activity display:

- event type。
- category/lifecycle if displayed。
- decision。
- outcome。
- failure category。
- policy reason。
- Bash classification/risk flags。
- empty/help/refresh copy。

Keep raw:

- tool name。
- request ID。
- rule IDs。
- command preview。
- path/data values。
- machine timestamps unless UI explicitly formats them using locale-aware formatter.

Diagnostics:

- translate user-facing labels/status/remediation only。
- preserve raw codes/IDs/paths in technical data cells。

Exit: observability UI no longer depends on English enum strings as display labels.

## 13. Phase L — Error Localization

Create central error presentation map:

```text
error code
→ translation key
```

Cover currently user-visible codes across Config / Runtime / Tool / Policy / Approval / Bash / Control read.

Known code:

```text
localized summary
+ optional sanitized technical fallback detail
```

Unknown code:

```text
localized generic failure
+ safe raw code/fallback detail when available
```

Do not try to machine-translate arbitrary Rust messages.

Ensure service/provider layers no longer generate primary English UI prose for known codes.

Exit: user-facing error explanations meet Architecture machine-code translation boundary.

## 14. Phase M — Resource Integrity / Coverage Tests

Add pure tests that flatten resources and compare:

- exact key parity。
- non-empty values。
- interpolation token parity。
- critical page groups present。

Add locale-resolution tests.

Add component tests in both languages for representative surfaces:

- AppShell/nav。
- Settings language selector。
- Tools raw identity + translated labels。
- Policy raw ID/user content + translated friendly text。
- Approval action labels。
- Activity enum labels。
- error-code mapping。

Optional static audit:

- targeted grep/source test for newly introduced obvious hard-coded primary UI copy。
- maintain small allowlist for machine literals/test fixtures.

Do not create a brittle generic parser if targeted coverage is more reliable.

## 15. Phase N — Regression / Compatibility

Verify M6 does not alter:

- RuntimeConfigSnapshot。
- ToolRegistry/exposure。
- Policy decisions。
- Approval state/identity。
- Bash execution/limits。
- Runtime/Tunnel lifecycle。
- MCP schemas/annotations。
- file operations。

Critical config cases:

```text
no saved language + system zh-CN
no saved language + system en-US
no saved language + system fr-FR
saved en + system zh-CN
saved zh-CN + system en-US
language-only save during running Harness
language save while restartRequired already true
language persistence failure
```

## 16. Dependency Discipline

New npm dependencies expected:

```text
i18next
react-i18next
```

No Rust i18n dependency is needed.

Do not introduce Runtime localization crates or duplicate translation resources in Rust.

## 17. Change Discipline

M6 must not:

- start M7 schemaVersion v1→v2 migration。
- translate Runtime output at source。
- translate tool names on the wire。
- translate protocol field names。
- translate Policy IDs。
- auto-translate user-authored Policy text。
- change Runtime decision/execution behavior。
- force Harness restart after language-only change。
- change Bash security wording into sandbox claims。
- turn unknown errors into success/friendly silent fallback。

## 18. Development Completion Check

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

Core automated proof:

```text
protocol/config compatibility
UI-only revision semantics
OS locale resolution
pre-render saved locale
immediate switch/persist/rollback
resource parity/placeholders
shared status/error mappings
all primary page representative bilingual tests
machine identity preservation
M0–M5 regression
```

Development complete is not acceptance complete. After TASKS finish, generate/run M6 Integration / Local E2E / Manual Acceptance separately.
