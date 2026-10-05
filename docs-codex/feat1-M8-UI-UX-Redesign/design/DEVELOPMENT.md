# M8 development evidence

用户于本次任务明确批准 DESIGN_SYSTEM、SPEC D01–D07 和 SPEC，并授权实施；T0 / G0 已由人工完成。原规划文件的历史草案 header 不替代该授权。本记录只涉及 T1–T9 开发，不声明 M8 ACCEPTED / FROZEN。

## Baseline / T1

- Source HEAD: `2a793e5d58e03989cf532ce2457e2b34e2687ae5`，分支 `main`。
- 开始前 `git status --short` 为空。后续 diff 均为本次开发，不覆盖其他工作。
- 当前 manifests：`ui/package.json` 的 typecheck / test / build；根 Cargo workspace 包含 protocol、runtime、ui/src-tauri。
- 当前页面：Overview、Tools、Policies、Approvals、Activity、Settings；FirstRunWizard 在 SettingsPage，ConfigRecovery 独立组件。
- 持久配置与 Language：AppConfig + `configGet/configSave/uiLanguageSet`；Credential 只使用 status boolean 与既有受控写入入口。
- Runtime：RuntimeState + Harness lifecycle observation；Tool：runtimeGetEffectiveTools；Policy：policies.list effective snapshot；Approval：ApprovalRequest 字段；Activity：runtimeGetActivitySnapshot bounded safe projection。
- 缺口边界：无可证明的精确 running default 时只展示未确认；不计算 Policy / eligibility / Bash risk；前端 queue 不提供跨进程 CAS 或 SecretStore/config 事务。

## Development coverage mapping

| SPEC | 开发验证 | 后续阶段边界 |
| --- | --- | --- |
| AC01 | Shell 顺序、状态、双语、导航 tests | 人工路径留 H1 |
| AC02–04 | 状态投影、mutation queue、Tools/Overview lifecycle tests | 当前真实 Runtime observation 留 Q1 |
| AC05–07 | matcher 子集/边界/round-trip、Rule CRUD、dirty/saving/conflict tests | 人工范围说明留 H1 |
| AC08–09 | request validity、expiry、Runtime eligibility、handoff tests | 真实 resume/session grant 留 Q1 |
| AC10 | feed/filter/stale/evidence/identity/link tests | 当前真实 Activity 读取留 Q1 |
| AC11–12 | Language、Connection、credentials、setup/recovery 及 M6/M7 tests | 凭据真实运行边界留 Q1/H1 |
| AC13 | shared focus、ARIA、responsive/reduced-motion static/component checks | 实际尺寸、视觉、系统缩放留 H1 |
| AC14 | scope review、typecheck、完整 UI tests/build、Cargo regression | 不变更冻结后端语义 |
| AC15 | 本轮 NOT RUN | P1 当前 M8 独立包身份核验 |

## Progress

| Task | Status |
| --- | --- |
| T1 | COMPLETE — source inventory and mapping |
| T2 | COMPLETE — scoped serialized intents and navigation contract |
| T3 | COMPLETE — shared controls/surfaces and neutral shell |
| T4 | COMPLETE — Overview actions and compact Tools |
| T5 | COMPLETE — safe Rule editor and scoped defaults |
| T6 | COMPLETE — decision queue and safe Rule handoff |
| T7 | COMPLETE — bounded feed, filters and evidence links |
| T8 | COMPLETE — scoped Settings, four-step setup and recovery |
| T9 | COMPLETE — integrated development regression |

T2/T3 targeted verification 已执行：

- `npm run typecheck`：通过。
- `npm test -- src/app/providers/config-mutations.test.ts src/app/providers/HarnessDataProvider.test.tsx src/components/common/primitives.test.tsx src/components/layout/AppShell.test.tsx src/i18n/resources.test.ts src/i18n/source-coverage.test.ts`：当时 19 tests 通过；其后增加 conflict test。
- `npm test -- src/app/providers/m8-mutations.test.tsx src/app/router/DraftNavigation.test.tsx`：8 tests 通过。
- `npm test -- src/app/providers/config-mutations.test.ts src/components/common/primitives.test.tsx`：7 tests 通过。
- `npm test -- src/i18n/state-presentation.test.ts`：2 tests 通过。

Q1 / P1 / H1 / F1 / M9 本轮均不执行。

T4 targeted verification：`npm test -- --run src/pages/tools/ToolsPage.test.tsx src/pages/observability.test.tsx`，44 tests 通过；`npm test -- src/pages/overview/OverviewPage.test.tsx`，4 tests 通过。T5 parser：`npm test -- src/pages/policies/matcher.test.ts`，19 tests 通过；Task 本身尚未完成。

T5 最终验证：`npm test -- src/pages/policies/PoliciesPage.test.tsx src/pages/policies/matcher.test.ts src/i18n/resources.test.ts src/i18n/source-coverage.test.ts src/app/router/DraftNavigation.test.tsx`，45 tests 通过；typecheck 通过。共享状态新增 `m8-observations.test.tsx` 3 tests 通过，验证晚到状态不会覆盖已观察 Stop，Runtime 读回不重置 Tunnel。凭据删除 7-item mutation suite 保留真实 persisted reference。

T6 final targeted：`npm test -- src/pages/approvals/ApprovalsPage.test.tsx src/i18n/source-coverage.test.ts src/pages/approvals/approval-handoff.test.ts`，37 tests 通过；typecheck 通过。Cold First Run mutation 追加回归，`m8-mutations.test.tsx` 当前9 tests通过。

T7 root整合验证：`npm test -- src/pages/activity/activity-feed.test.ts src/pages/activity/ActivityPage.test.tsx src/i18n/source-coverage.test.ts`，10 tests通过。T8 FirstRun初版5tests通过，共享 mutation/provider/FirstRun21tests通过。

T8 root整合：`npm test -- src/pages/settings/SettingsPage.test.tsx src/pages/settings/FirstRunWizard.test.tsx src/pages/settings/language-lifecycle.test.tsx src/pages/settings/language-mutation-serialization.test.tsx src/i18n/source-coverage.test.ts src/i18n/resources.test.ts`，25 tests通过；typecheck通过。

## T9 final development result — 2026-10-05

**M8 Development Complete: YES.** This is development completion only. M8 is not ACCEPTED / FROZEN.

- UI full suite: 36 test files, 334 passed, 0 failed, 0 skipped.
- Rust workspace: 460 passed, 0 failed, 5 existing ignored tests; 12 reported suites including doc-test suites.
- Frontend production build: success from the current modified UI source.
- Changes are limited to `ui/src/**` and this development record. No Runtime / Protocol / Policy / Approval / Bash / Migration backend code, manifests, dependencies or schema were changed.
- All product configSave call sites remain inside the shared Provider queue. Page auto-save uses scoped intents; the first configuration uses a seed only after configGet returns null.

### Actual final commands

| Directory | Command | Result |
| --- | --- | --- |
| ui | `npx vitest run --reporter=json --outputFile=.m8-development-tests.json` | 334/334 PASS; same full suite as the npm test script (`vitest run`) |
| ui | `npm run typecheck` | PASS; final build also re-ran both TypeScript project checks |
| ui | `npm run build` | PASS, 98 modules; current CSS/JS assets generated |
| repository root | `cargo fmt --all -- --check` | PASS |
| repository root | `cargo test --workspace` | PASS: 460 passed, 0 failed, 5 ignored |
| repository root | `git diff --check` | PASS |

The first full UI run reported 305 passed / 29 failed. Legacy UI interaction assertions and incomplete fixtures were updated while preserving their safety, language, privacy, identity and lifecycle assertions. A subsequent run exposed 3 async/recovery races, which were fixed and verified. The final full run is 334 passed / 0 failed; failures were not hidden by skips or removed gates.

### Main implementation and shared boundaries

- Six-page minimal workspace shell, neutral semantic tokens, thin dividers, compact rows and page-local headers. Old Hero/Topbar/gradient/glow/card-wall style rules removed.
- Shared Button, Switch, Segmented, Select, TextField, Row, Section, InlineFeedback, Disclosure, Inspector, Sheet, Dialog, focus lifecycle, modal background inertness and reduced-motion handling.
- Scoped serial config intents, latest persisted snapshot merge, Language/Reset coordination, cold-start seed handling, object conflict protection and one shared draft leave guard.
- Configured/effective/unknown/restart are sourced separately. Late lifecycle/resource replies cannot revive old sessions or reset a confirmed Tunnel observation.
- Tool/default auto-save; Rule explicit Create/Edit/Save/Delete with stable IDs and lossless literal matcher handling; natural command parsing remains conservative.
- Decision-first approvals with Runtime eligibility, front-of-decision evidence, expiry checks, per-request locks, guarded preflight, truthful resolution feedback and safe Rule handoff.
- Bounded Activity feed with local combined filters, stable snapshot identities, all M7 safe evidence in Advanced and links only to currently locatable objects.
- Settings section/list with Connection Apply/Cancel, independent credential removal and dangerous confirmations. Four-step First Run and Recovery distinguish confirmed facts from unknown storage outcomes.

### Unresolved limitations and non-blocking observations

- Real visual/manual window, scaling and UX acceptance are not run. Automated focus, language, state and responsive-branch tests do not substitute for H1.
- Real current Runtime/MCP/Approval black-box checks are reserved for Q1; fixture tests do not prove real execution or external acceptance.
- Frontend JavaScript minified chunk is approximately 510.72 kB; Vite reports its existing 500 kB chunk-size warning. Build succeeds; the warning was not suppressed.
- Cargo reports ts-rs serde-attribute parsing and dead-code warnings; tests pass. Five ignored tests require explicitly built release Runtime black-box runs and were not executed in this development phase.
- The UI queue does not provide cross-process CAS or a ConfigStore/SecretStore transaction. Partial credential failures are read back without claiming whole-operation rollback.
- Activity contains historical matched IDs rather than complete immutable rule definitions; current-rule navigation labels that distinction without claiming the content is unchanged.

### Stop boundary

Q1 Independent QA: NOT RUN. P1 Portable: NOT BUILT. H1 Manual UX Acceptance: NOT RUN. F1 Acceptance/Freeze: NOT COMPLETE. M9 External ChatGPT Acceptance: NOT RUN.

T1–T9 are complete. Work stops at the authorized development boundary.
