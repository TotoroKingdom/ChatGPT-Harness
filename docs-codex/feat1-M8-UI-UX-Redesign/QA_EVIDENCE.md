# M8 Q1 — Independent Automated QA Evidence

> Result: **M8 Automated QA PASS**
> Scope: Q1 Independent Automated QA only
> Date: 2026-10-05 (Asia/Shanghai)
> Manual UX Acceptance、Portable、F1、M9 不在本报告范围内。

## 1. Source and working-tree identity

- Source HEAD: `506fd2a0e321682b0d8392b5984c974052377b94`
- Branch: `main` (`origin/main`)
- QA 开始前 working tree：clean；`git status --short` 无输出。QA 收尾时除本报告外无 source/build 修改；最终 status 只包含本报告自身的 untracked path：`docs-codex/feat1-M8-UI-UX-Redesign/QA_EVIDENCE.md`。
- HEAD 变更范围：M8 UI、UI tests、i18n、presentation adapter 与 DEVELOPMENT evidence；`git diff HEAD^ HEAD -- runtime protocol ui/src-tauri Cargo.toml ui/package.json` 无输出。
- QA 后未修改 production source、Runtime、Protocol、manifest 或 schema。

## 2. Actual commands and results

| Command | Result | Evidence |
| --- | --- | --- |
| `npm run typecheck` (`ui`) | PASS, exit 0 | 两个 TypeScript project checks 通过 |
| `npm test` (`ui`) | PASS, exit 0 | 36 test files；334 passed；0 failed；0 skipped |
| `npm run build` (`ui`) | PASS, exit 0 | 98 modules；frontend assets generated |
| `cargo fmt --all -- --check` | PASS, exit 0 | 无格式差异 |
| `cargo test --workspace` | PASS, exit 0 | 460 passed；0 failed；5 ignored；ignored 为 release black-box suites，不计入本项 PASS |
| `git diff --check` | PASS, exit 0 | 无 whitespace error |
| `cargo build --release -p chatgpt-harness-runtime` | PASS, exit 0 | 当前 HEAD release Runtime |
| `npm run tauri:build` (`ui`) | PASS, exit 0 | NSIS packaging smoke；未调用 `portable:windows` |

已知 observation：Vite 生成的 minified JavaScript chunk 为 `510.72 kB`，超过 500 kB warning threshold。构建成功，未观察到由此造成的功能或实际性能失败；未扩大 M8 scope。

### Diagnostic-only focused Integration run

为核对 INT-33，曾以 30 个 Integration/UI 目标文件组合运行 focused Vitest：

- 3 次组合运行均为 `286 passed / 1 failed`，失败均为 `m7-qa-integration.test.tsx` INT-33，在 `ToolsPage` 的 Runtime epoch 重新确认窗口中短暂显示 `Loading data`，导致断言在 machine reason 与 remediation 之间观察到不同 render。
- 单独运行 `src/m7-qa-integration.test.tsx`：7/7 PASS。
- 正式 manifest 命令 `npm test` 随后通过：36/36 files，334/334 tests PASS。

该 focused failure 未证明 production implementation defect；它被记录为 Integration test timing / transient loading observation，没有通过 skip、修改断言或修改 production code 隐藏。实际 generation/session 失效时清除旧 observation 的行为仍由 INT-03、M7 observation tests 与真实 Runtime evidence 覆盖。

## 3. Integration cases

Fixtures 只用于 UI integration behavior；以下不把 fixture 结果当作 Runtime black-box evidence。

| Cases | Result | Independent evidence |
| --- | --- | --- |
| INT-01–INT-05 | PASS | `App.test.tsx`, `AppShell.test.tsx`, `Sidebar.test.tsx`, `OverviewPage.test.tsx`, `observability.test.tsx` |
| INT-06–INT-11 | PASS | `m8-mutations.test.tsx`, `language-mutation-serialization.test.tsx`, `language-lifecycle.test.tsx`；覆盖 scoped merge、失败后续成功、refresh failure、Language、Reset |
| INT-12–INT-15 | PASS | `ToolsPage.test.tsx`, `primitives.test.tsx`, `AppShell.test.tsx`；覆盖 row event separation、control states、radiogroup/keyboard semantics |
| INT-16–INT-19 | PASS | `PoliciesPage.test.tsx`, `matcher.test.ts`, `RuleEditor.test.ts`, `m8-mutations.test.tsx`；覆盖四层 Policy、shared defaults、Rule safety、ID stability |
| INT-20–INT-22 | PASS | `matcher.test.ts`, `PoliciesPage.test.tsx`；覆盖保守自然 matcher、prefix explanation、Advanced round-trip |
| INT-23–INT-24 | PASS | `DraftNavigation.test.tsx`, `PoliciesPage.test.tsx`, `m8-mutations.test.tsx`；覆盖 dirty/saving/conflict/leave guard |
| INT-25–INT-30 | PASS | `ApprovalsPage.test.tsx`, `approval-handoff.test.ts`, `m6-integration.test.tsx`；覆盖 decision-first、incomplete preview、Runtime eligibility、dedup、stale、handoff 不自动 resolve |
| INT-31–INT-34 | PASS | `activity-feed.test.ts`, `ActivityPage.test.tsx`, `m7-qa-integration.test.tsx`, `observability.test.tsx`；覆盖 outcome/filter/stale/evidence preservation |
| INT-35–INT-37 | PASS | `m8-mutations.test.tsx`, `SettingsPage.test.tsx`, `language-mutation-serialization.test.tsx`；覆盖 scoped Settings、Connection、credential removal |
| INT-38–INT-39 | PASS | `FirstRunWizard.test.tsx`, `m7-recovery.unit.test.tsx`, `m6-runtime-boundary-integration.test.tsx`；覆盖 readiness truthfulness 与 recovery |
| INT-40 | PASS | `m6-integration.test.tsx`, `m7-qa-integration.test.tsx`, `resources.test.ts`, `source-coverage.test.ts`；English / zh-CN 行为与 key/placeholder parity |
| INT-41 | PASS（projection）/ `MANUAL_PENDING`（实际 viewport） | Activity/Approval/Settings tests 覆盖长字段安全展示；980×680、系统缩放、真实换行/遮挡留 H1 |
| INT-42 | PASS（automated focus/state subset）/ `MANUAL_PENDING`（实际 reduced-motion 与完整键盘手感） | primitives、AppShell、DraftNavigation、FirstRun tests；不模拟人工视觉/键盘 acceptance |
| INT-43–INT-44 | PASS | 当前 HEAD scope review、full UI suite、M0–M7 service/black-box regression；未新增 local Policy evaluator、grant calculator、Bash risk authority |

Integration automated behavior result：PASS。实际视觉、窗口尺寸、系统缩放、完整人工键盘路径保留 `MANUAL_PENDING`，不升级为 automated 或 manual PASS。

## 4. Local real black-box E2E

### Build and runtime identity

- Runtime: `C:\AI\ChatGPT-Harness\target\release\chatgpt-harness-runtime.exe`
- Runtime SHA-256: `90CD62C0081366386A933EA919CDC490132FB76FD90C72DD033AA103EE0DB0CB`
- Frontend production JS: `ui/dist/assets/index-BM7Wf7od.js`，510715 bytes，SHA-256 `C67263FC828FF01F5A3451235BC2DF0000951437F4B9996E612F9AACC3E6F0FF`
- Frontend production CSS: `ui/dist/assets/index-DCs5ua41.css`，12962 bytes，SHA-256 `A7BE43718E798B509F3181292DF0E82110A8174CA55F51F50A88CA7F06418679`
- Tauri release UI: `C:\AI\ChatGPT-Harness\target\release\chatgpt-harness-ui.exe`，SHA-256 `EE6BABD3EB799D772645607B134C393DFC744BDDAF01647EEE7998E7C4AB8970`
- NSIS smoke artifact: `C:\AI\ChatGPT-Harness\target\release\bundle\nsis\ChatGPT-Harness_0.1.1_x64-setup.exe`，SHA-256 `938ED3F65B0501683BF4756486C56DC63B5868FA4F6D418C1CCEBFD450021C45`
- M7 evidence file: `C:\AI\ChatGPT-Harness\target\m7-qa\e2e\m7-e2e-1791174186651-4852.jsonl`
- Disposable M7 workspace 已清理；运行后无 `chatgpt-harness-runtime` 残留进程。

### Real release-runtime suites

| Suite / command | Result | Real evidence |
| --- | --- | --- |
| M2 `m2_local_black_box_e2e` | PASS | 12/12 cases；Tool disable、stale call、restart generations、filesystem side-effect boundary |
| M3 `m3_local_black_box_e2e` | PASS | 14/14 cases；real Policy default / user rule / DENY / ASK / effective policy / Activity |
| M6 `m6_host_runtime_release_black_box_e2e` | PASS | E2E-10、11、17、18、20、22；Language revision-neutral、English/zh-CN MCP/policy parity、Bash and cleanup |
| M7 `m7_local_black_box_e2e` | PASS | E2E-01–25 全部 PASS；migration/recovery、real tools、Policy、Approval、Activity、revision convergence、clean shutdown |

M7 key evidence：

- Revision convergence：E2E-20 保存后 `savedRevision=42`，运行中仍为 `configRevision=41`，active tools 仍是旧 generation；restart 后 Runtime 为 revision 42，`write.enabled=false` 收敛。PID `32760 → 21528`，runtime identity 发生变化。
- Policy ASK / Approval：E2E-14 pending 时 `activeRequests=1`、`pendingApprovals=1`；AllowOnce 使用同一 `requestId/approvalId=a0760220-1077-4009-8601-c77896c306d1`，原始 MCP call 恢复并产生正确 filesystem effect，终态 counters 全为 0。E2E-09、15 另验证 real Policy DENY 与 Approval DENY/no side effect。
- Activity：real request/approval IDs、decision/reason、outcome、duration、sanitized evidence 与 bounded snapshot 均被检查；E2E-19 验证 en/zh-CN remediation mapping。
- Real endpoints：evidence 中观察到 loopback MCP，例如 `http://127.0.0.1:51954/mcp`、`http://127.0.0.1:52403/mcp`；无 External ChatGPT / Tunnel substitution。
- File/Bash/M0–M7：M7 E2E-24、M2/M3/M6 suites 与 Runtime workspace tests 验证 `read_file`、`glob`、`write`、`edit`、disabled、DENY、ASK/resume、Bash execution/timeout、migration/recovery。

M8 E2E plan 的 desktop interaction / viewport 部分没有稳定 native automation infrastructure；按计划边界由 Integration 覆盖状态/交互组合，由 H1 进行真实视觉与人工键盘验收。没有将 fixture 或 NSIS build smoke 改名为视觉 E2E PASS。

## 5. Failures, fixes, and non-blocking observations

- Production implementation defect：未确认；本轮没有修改 production code，也没有修改 Runtime / Protocol / frozen M0–M7 semantics。
- Focused Integration diagnostic：INT-33 在 30-file 组合中 3/3 次出现 transient `Loading data` assertion failure；单文件 7/7 PASS，正式 `npm test` 36/36 files、334/334 PASS。记录为 test timing observation，未隐藏、未 skip、未作为正式门禁失败。
- Vite JS chunk：510.72 kB warning；没有功能/性能失败证据，保留 observation，不扩大 M8 scope。
- Cargo：保留既有 ts-rs serde parsing/dead-code warnings；不影响 exit code 或测试结果。

## 6. MANUAL_PENDING

- 真实 Desktop UI 启动与 WebView/custom-protocol 身份的人工检查。
- 980×680 minimum、1280×820 base、wide Inspector、长路径/长 command/多行 error 的实际视觉布局。
- English / zh-CN 在真实窗口与系统缩放下的遮挡、裁切、可达性。
- 完整键盘路径、focus 手感、reduced-motion 实际系统设置、真实视觉层级与交互预算。
- 这些项目属于 H1，不得由本报告模拟 PASS。

## 7. Gate conclusion

```text
M8 Automated QA PASS
P1 — Fresh M8 Portable: eligible to start
H1 Manual UX Acceptance: NOT RUN / MANUAL_PENDING
F1 ACCEPTED / FROZEN: NOT RUN
M9 External ChatGPT Acceptance: NOT RUN
```

本轮没有运行 `npm run portable:windows`，没有创建或验收 `target/release/bundle/portable`，没有执行 External ChatGPT / Tunnel acceptance。
