# M6 Development Report — Chinese / English UI

Status: **M6 ACCEPTED / FROZEN**（2026-10-04；开发阶段证据保留如下）
Date: 2026-10-04
Source: HEAD `4e2514ed54b5eed72318da7d9a73fc5b4249d0d8` plus current uncommitted M6 changes.
Baseline: M5 `ACCEPTED / FROZEN`; M5 acceptance records and Runtime production code remain unchanged.

## Task completion

Implementation proceeded in T1 → T2 → T3 → T4 → T5 order. Independent page work was delegated within T4 after the T3 checks passed.

| Task | Implementation and code-level proof | Result |
| --- | --- | --- |
| T1 | Typed `UiLanguage`, generated TS/schema, optional language in schema v1, UI-only revision comparison, isolated ConfigStore/Tauri/client/provider mutation. Legacy/strict enum/round-trip/clear/defaults/atomic failure/snapshot exclusion tests; language combined with Workspace/Tunnel/Security/Tools/Policies/Bash changes increments revision once. | PASS |
| T2 | i18next/react-i18next, pre-render config read and locale bootstrap, saved preference precedence, Simplified/Traditional locale boundaries, English fallback, config-error recovery read retained, startup `<html lang>`. | PASS |
| T3 | Typed machine-value → resource-key mappings; shared shell and state copy; known host/protocol/RPC-envelope errors; generic unknown errors; localized system-rule friendly text with raw identity. Control-read keeps its stricter error redaction. | PASS |
| T4 | AppShell, First Run, Overview, Tools, Policies, Activity, Approvals and Settings copy, forms, validation, notices and accessibility strings. Isolated immediate language switch, persistence/rollback, dirty-draft preservation, follow-system clear, live `<html lang>`, existing restart debt preserved. | PASS |
| T5 | Exact resource-key/nonempty/placeholder parity, bounded TypeScript-parser audit of primary JSX copy and visible attributes, direct literal-key resolution in both supported resources, M0–M5 unit/code-level regression. | PASS |

## Main changes and boundaries

- `UiConfig.language` persists `en` / `zh-CN`; absent means follow the primary OS/WebView locale. Unsupported and Traditional Chinese locales fall back to English. No schemaVersion or protocol-version migration was introduced.
- `same_restart_semantics` excludes only `configRevision` and presentation-only `UiConfig`; all remaining AppConfig fields continue participating in revision comparison.
- `ConfigStore::set_ui_language` reads the current persisted config or safe defaults, updates only the language preference and uses the existing atomic save. It does not invoke Runtime Control, SecretStore or lifecycle mutations.
- Bootstrap initializes i18n and DOM language before rendering React. A failed startup config read does not suppress the Provider's later ConfigError/recovery flow.
- Language changes are immediate and persisted independently of the Settings draft. Failure rolls back locale and selection. A later full Settings save uses the latest persisted UI object, including an absent language after choosing Follow system.
- Notifications and validation retain keys/descriptors, so an already-visible message changes with the current language. Known backend errors use localized summaries; arbitrary Runtime diagnostics are not machine-translated.
- RuntimeSupervisor's actual `RPC_<machine code>` envelope is interpreted only for display; the original code remains available. Unknown errors never become success/ALLOW.
- Tool names, wire fields/enums, Runtime/request IDs, paths/commands, policy IDs and user-authored rule content remain data. System-rule display text follows the current Runtime-owned IDs and semantics.
- Bash warnings retain the fact that cwd is an initial directory, not a filesystem sandbox; processes can still access files outside the workspace and the network.
- English resource copy preserves existing product semantics. Clearly obsolete milestone claims were removed from presentation only; no M7 feature was implemented.

## Final development verification

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace --lib --bins -- --skip m3_config_integration --skip m3_integration --skip m2_e2e` | PASS: Runtime 336 + host/UI library 83 + protocol 1 = **420 passed**, 0 failed, 4 explicitly filtered |
| `npm test` | PASS: **17 files / 175 tests**, 0 failed |
| `npm run typecheck` | PASS; also performed by the final build |
| `npm run build` | PASS: 87 modules, latest UI bundle `index-FShb6ZHK.js` |
| `git diff --check` | PASS |

Rust full workspace test invocation was intentionally replaced with the scoped command above for the final development regression because this request excludes Integration/E2E. This is not a claim that unfiltered `cargo test --workspace` passed.

Logs are under `artifacts/M6-i18n/development/`:

- `rust-unit.log`
- `ui-test.log`
- `ui-typecheck.log`
- `ui-build.log`

The host config suite includes 39 tests. The actual language mutation is tested against legacy/corrupt configs, missing-config defaults and a deterministic temporary-file creation failure; failure preserves prior bytes, config and revision.

The component tests use the actual Provider with a local mock adapter to prove immediate switch before persistence completion, unchanged persisted unrelated fields/revision, retained dirty inputs/focus identity, unchanged Runtime/session IDs and zero Runtime/Harness lifecycle calls. Bootstrap and ConfigStore round-trip are unit proof of preference persistence. Real Tauri relaunch, process PID/identity and UI rendering/layout checks remain independent QA responsibilities.

## Deviations and remaining observations

1. **Stage-boundary deviation:** during T1 a sub-agent mistakenly ran `cargo test -p chatgpt-harness-ui --lib`. This target embeds M3 Integration cases. Its result was 79 passed, 1 failed, 2 ignored; INT-04 failed, while INT-01/02/03/05 completed. No Integration gate is claimed PASS. The broad command was not repeated; subsequent and final runs explicitly exclude Integration/E2E modules.
2. Source inspection identifies the INT-04 stale assertion in `ui/src-tauri/src/runtime/m3_config_integration.rs`: it expects saving a user rule matching `bash` to fail, although M5 allows this matcher. The fixture and M5 baseline were left untouched. Independent QA must account for this known fixture mismatch rather than treating it as a new M6 product regression or hiding the failure.
3. Compilation emitted `ts-rs` warnings about `deny_unknown_fields` and existing unused/dead-code warnings. Commands succeeded; these were not cleared by expanding M6 scope.
4. No known unresolved M6 development failure remains. No Local Black-box E2E or Manual Acceptance was executed; no M6 Portable acceptance package was built. After independent automated QA passes, the required fresh M6-specific Portable build must precede Manual Acceptance.

## Handoff

开发阶段报告与 TASKS 仅证明开发完成，原 INT-04 fixture observation 保留，后续处理见 [QA_REPORT.md](QA_REPORT.md)。最终状态：Integration 40/40 PASS、Local Black-box E2E 22/22 PASS；Human Reviewer（用户）于 2026-10-04 确认人工验收完成并通过，简体中文 / English 可正常切换、即时生效。M6 已正式 **ACCEPTED / FROZEN**，最终记录见 [MANUAL_ACCEPTANCE_CHECKLIST.md](MANUAL_ACCEPTANCE_CHECKLIST.md)。本次仅收口文档，未重新执行测试或修改生产代码。
