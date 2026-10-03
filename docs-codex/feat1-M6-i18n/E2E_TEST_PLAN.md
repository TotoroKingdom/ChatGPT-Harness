# M6 E2E Test Plan — Local Black-box i18n / Persistence Regression

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M6 — Chinese / English UI  
> Owner: Coding Agent / QA Agent  
> Requirement: fully automated local validation against production-ish built artifacts and real local ConfigStore/Tauri boundary where practical.  
> External ChatGPT is not required because M6 does not change MCP capability semantics.

## 1. Objective

证明 M6 在真实应用配置边界和 production frontend build 上满足：

```text
no saved language
→ system locale resolution
→ UI locale

saved language
→ overrides system
→ persists across app/bootstrap restart

language switch
→ immediate UI preference change
→ persisted locally
→ no Runtime configRevision mutation
→ no Runtime restart requirement
```

同时验证 production build中的双语资源完整，并且 M0–M5 Runtime/MCP行为不因 language preference改变。

## 2. E2E Boundary

M6 E2E必须自动执行，不需要人工点击 GUI。

可以使用：

- real ConfigStore on disposable local app-data directory。
- real Tauri command/config mutation layer通过 test harness或等价 production command seam。
- production `npm run build`输出资源。
- production React bootstrap入口的自动化 harness。
- real Runtime subprocess做 language non-impact regression。

不得：

- 用手工桌面截图替代自动化 E2E。
- 用纯 `resolveLanguage()` unit test冒充 Local E2E。
- 为了测试语言把 UiConfig发进 Runtime snapshot。
- 要求 External ChatGPT切换语言。

真实桌面视觉/交互检查属于 Manual Acceptance。

## 3. Entry Criteria

- M6 T1–T5 development complete。
- Unit PASS。
- Integration PASS。
- `npm run build` PASS。
- release/final-ish Tauri/Runtime artifacts可构建。
- disposable config directory可指定或隔离。

## 4. Test Harness

建议提供：

```text
ui/src-tauri/tests/m6_i18n_e2e.rs
+
ui/src/i18n/m6-bootstrap.e2e.test.tsx
```

或等价自动化组合。

其中：

### Host/config black-box

验证真实：

```text
config.json
ConfigStore
isolated uiLanguageSet command/service
configGet
configRevision
```

### Production frontend bootstrap

验证真实 build使用：

- production resource files。
- production locale resolver。
- production bootstrap path。
- real App component / routing shell。

### Runtime non-impact

启动真实 release Runtime时，比较不同 `ui.language` 下产生的 Runtime snapshot/effective behavior。

## 5. Required E2E Cases

### E2E-01 — Production build contains both supported locale resources

运行真实：

```text
npm run build
```

验证 build成功且 bundle包含可达的：

```text
en
zh-CN
```

不要求资源必须作为独立 JSON文件输出，只要求 production runtime能加载两语言。

### E2E-02 — Fresh config + English system locale

在全新 disposable config目录，无 saved language，系统 locale harness设：

```text
en-US
```

启动 production bootstrap。

期望：

```text
resolved=en
html lang=en
English AppShell/nav
```

### E2E-03 — Fresh config + Simplified Chinese system locale

无 saved language：

```text
zh-CN / zh-Hans equivalent fixture
```

期望：

```text
resolved=zh-CN
html lang=zh-CN
Chinese AppShell/nav
```

### E2E-04 — Fresh config + unsupported locale falls back English

例如：

```text
ja-JP
fr-FR
zh-TW
```

至少逐项/参数化验证 fallback `en`。

### E2E-05 — Saved English overrides system Chinese

真实 config写入：

```text
ui.language=en
```

system fixture `zh-CN`。

重新执行 bootstrap，必须直接 English。

### E2E-06 — Saved Chinese overrides system English

真实 config：

```text
ui.language=zh-CN
```

system fixture `en-US`。

重新 bootstrap必须直接 Chinese。

### E2E-07 — Language persistence survives process/bootstrap restart

通过真实 UI-preference command从 English切 `zh-CN`。

关闭 harness/bootstrap实例并重新创建。

期望：

- `config.json`保存 `zh-CN`。
- restart后仍解析 `zh-CN`。
- 不依赖上一个 React in-memory state。

### E2E-08 — Follow system clears explicit preference

真实 command：

```text
zh-CN → Follow system
```

确认 config中 language absent/null的冻结 representation。

下一次 bootstrap：

- system `en-US` → English。
- system `zh-CN` → Chinese。

### E2E-09 — Language-only persistence preserves configRevision

准备真实 config revision N。

通过 production UI-preference mutation：

```text
en → zh-CN → en → Follow system
```

每一步读取真实 `config.json`，期望：

```text
configRevision == N
```

### E2E-10 — Language-only persistence does not require Runtime restart

启动真实 Runtime generation，使：

```text
activeRevision=N
latestRevision=N
restartRequired=false
```

切换 language。

重新获取真实 Harness/config state：

```text
latestRevision=N
activeRevision=N
restartRequired=false
Runtime PID/generation unchanged
```

不得调用 restart来让 case通过。

### E2E-11 — Existing restart mismatch is preserved

先真实修改 Runtime-affecting desired config，使：

```text
active=N
latest=N+1
restartRequired=true
```

不重启，切换 language。

期望：

```text
latest=N+1
restartRequired=true
Runtime generation unchanged
```

### E2E-12 — Isolated language save does not commit unrelated Settings draft

通过 production-facing test seam准备未提交的 Settings draft值，并切换语言。

读取真实 config，确认只有 `ui.language`改变；Workspace/Tunnel/Security/Bash持久值不变。

如果 draft完全只存在 React内存，则通过 real component + real language persistence adapter组合验证，不能通过调用完整 configSave模拟。

### E2E-13 — Persistence failure leaves prior config intact

使用 deterministic filesystem/ConfigStore failure fixture，例如 readonly/failing test seam。

切语言：

- mutation返回失败。
- config文件保持原偏好/revision。
- bootstrap/reload仍使用旧语言。
- atomic config不损坏。

### E2E-14 — Production primary shell can render both locales

对 production App bootstrap分别以 `en` / `zh-CN` render主 shell，并访问全部 route surface：

```text
overview
tools
policies
activity
approvals
settings
```

每页必须 render成功，无 missing-resource exception / raw translation-key placeholder。

这不是人工视觉验收，但可防 production bundle缺资源。

### E2E-15 — Machine identities are identical across locales

同一 production fixtures分别 English/Chinese render，比较 raw data identity：

```text
tool names
policy IDs
request/approval IDs
paths
commands
machine codes where technical detail shown
```

必须一致。

### E2E-16 — Pending approval survives language change unchanged

在真实/production-equivalent provider里保留一个 Pending approval，切换 locale。

验证：

- request ID/status不变。
- 无 resolve request。
- actions的显示语言变化。

若测试 harness可以连真实 Runtime Approval，则优先使用真实 Runtime pending approval；否则 Integration负责 Runtime identity证明，此 E2E至少验证 production App data flow不触发 resolve。

### E2E-17 — Runtime snapshot excludes UiConfig.language

分别用相同 Runtime-affecting config但不同 language启动真实 release Runtime generation fixture。

验证 initialize snapshot/Runtime effective结果等价；language不出现在 Runtime snapshot payload。

### E2E-18 — MCP contracts unchanged across language preferences

对 `en` / `zh-CN` host config分别启动等价 Runtime，使用真实 loopback MCP至少验证：

```text
tools/list tool names/schema identical
representative read_file call behavior identical
```

M6不得改变 MCP names/schemas/annotations。

### E2E-19 — Known control error remains same machine code across locales

制造一个 harmless known control error（例如 Runtime unavailable read）。

底层 machine code在两语言必须一致；presentation localized explanation可不同。

证明 translation未进入 Control protocol。

### E2E-20 — M5 Bash state is unchanged by language switch

在不执行危险 command的情况下，比较 Bash effective state / policy / readiness machine data before/after locale switch。

期望完全相同；只显示标签变化。

### E2E-21 — Resource integrity runs against shipped resources

E2E runner调用 resource integrity check确认 production source/build所用：

- key parity。
- placeholder parity。
- no empty leaf。
- supported locales exactly `en`, `zh-CN`。
- fallback = `en`。

### E2E-22 — Clean shutdown / no Runtime lifecycle side effect

完成 language tests后：

- 若 Runtime曾启动，可正常 stop。
- language switching没有创建额外 Runtime generation/process。
- no orphan process。
- config文件保持可重新读取。

## 6. Evidence Format

最终至少输出：

```text
production UI build identity
config directory fixture
saved language transitions
system locale fixtures
resolved locale per case
configRevision before/after
Runtime PID/generation before/after language switch
MCP comparison evidence where applicable
E2E-01...E2E-22 PASS/FAIL/BLOCKED
LOCAL_M6_E2E=PASS
```

## 7. Exit Criteria

Local M6 E2E只有在以下全部满足才 PASS：

- Fresh system-locale resolution verified。
- saved preference override verified。
- persistence across restart verified。
- Follow system verified。
- language-only configRevision invariant verified against real ConfigStore。
- no Runtime restart/generation change verified。
- production routes render in both supported locales。
- resources完整。
- Runtime snapshot/MCP machine behavior language-neutral。
- M5 Bash effective state non-impact verified。
- no manual GUI interaction required。

PASS后进入 `MANUAL_ACCEPTANCE_CHECKLIST.md`。

## 8. Non-Claims

Local E2E PASS不代表：

```text
visual typography/overflow quality accepted
Chinese wording quality accepted by human
all OS locale variants physically tested on separate Windows installs
External ChatGPT UI language controlled
M6 ACCEPTED / FROZEN
```
