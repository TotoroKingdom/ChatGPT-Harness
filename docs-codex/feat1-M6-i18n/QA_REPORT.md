# M6 独立 QA 报告

日期：2026-10-04  
被测代码：HEAD `7083fbc4862340b73f083f4b7b4912b626909662`，加本轮必要修复及新增 QA 测试。  
依据：同目录的 SPEC.md、PLAN.md、TASKS.md、INTEGRATION_TEST_PLAN.md、E2E_TEST_PLAN.md；DEVELOPMENT_REPORT.md 仅作为开发交接参考。

结论：**Integration INT-01–INT-40 全部 PASS；Local Black-box E2E E2E-01–E2E-22 全部 PASS；无未解决 FAIL / BLOCKED。具备进入 M6 Portable + Manual Acceptance 阶段的条件。**

本轮未执行 MANUAL_ACCEPTANCE_CHECKLIST.md，未构建 M6 Portable 验收包，未进行人工验收或冻结，未把 M6 标记为 ACCEPTED / FROZEN。进入下一阶段前仍须从最新代码构建独立 M6 Portable 包。

## 执行边界与构建身份

本轮 E2E 使用真实 Vite production bundle、真实 React main/bootstrap/App、真实 TauriHarnessControlClient，以及子进程桥接的 production ConfigStore（含 atomic save）。语言写入复用 `set_ui_language` 所调用的同一个 `ConfigStore::set_ui_language`，没有通过完整 configSave 模拟语言保存。

前端 routes 的只读 Runtime 数据使用计划允许的 production-equivalent typed fixtures（生产等价类型化夹具）。Runtime PID/generation、active/latest revision、真实 Git Bash 状态及 MCP 比较另外通过真实 release Runtime subprocess（发布版子进程）、stdin/stdout Control RPC 和 loopback MCP HTTP 验证。没有以 mock Runtime 或普通 unit test 代替这些黑盒 case。

Tauri host 使用 `tauri build --no-bundle` 构建成功；本轮没有启动原生桌面 UI。生产前端在 jsdom 中自动执行，桌面视觉、文字质量和真实桌面交互留给人工验收。

| 产物 | SHA-256 / 身份 |
| --- | --- |
| release Runtime | `51D77CC332740A16CC3394C986BFD963CCEA32413960FA642EEDF865CCB8DD7F` |
| release Tauri host | `995F1C946C0E42C3904916A3D067E998EB2F4D96970EA0E3769EEFC456383CC5` |
| production UI JS | `index-CyAmrAec.js`，`13d6ac2e9d8c1dba588fb5488e60fa4dc04379c9bf719d02ab39e677f364a12f` |
| UI HTML + JS + CSS aggregate | `b9ac9f653704d78e71cbc348573a82c1b756acb0551931a83f261d997ff46ac2` |
| ConfigStore bridge | `e5e06af4fdb12986316434d67bcf9199450f7600d543fe91f155eaec6fe87645` |
| SettingsPage 修复后源码 | `4093FA1E6D932CC8439268BFB22541446CDA83EED9AD5957E279545E7446228C` |

身份记录：[build-identity.json](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/build-identity.json)。前端 runner 每次重新构建 UI 和 ConfigStore bridge，并记录 bridge 源码与二进制哈希。最终单独 UI build 输出与被测 bundle 哈希一致。

## 实际命令与 gate

以下命令从 `C:\AI\ChatGPT-Harness` 执行；注明 UI 的命令从 `C:\AI\ChatGPT-Harness\ui` 执行。所有最终命令 exit code 为 0。

| Gate | 实际命令 | 结果 / 日志 |
| --- | --- | --- |
| 格式 | `cargo fmt --all -- --check` | PASS；[fmt-final.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/fmt-final.log)（成功无输出） |
| Rust check | `cargo check --workspace` | PASS；[workspace-check-final.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/workspace-check-final.log) |
| Rust 全量 | `cargo test --workspace` | PASS；440 passed / 0 failed / 4 ignored；[workspace-test-final.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/workspace-test-final.log) |
| H：M6 host Integration | `cargo test -p chatgpt-harness-ui --lib m6_i18n_qa::m6_host_integration_cases -- --nocapture` | 12 个计划 case PASS；[m6-host-integration.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m6-host-integration.log) |
| U：M6 UI Integration（UI） | `npx vitest run src/m6-integration.test.tsx src/m6-runtime-boundary-integration.test.tsx` | 2 files / 29 tests PASS；[ui-integration-test.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/ui-integration-test.log) |
| UI 全量（UI） | `npm test` | 20 files / 206 tests PASS；[ui-test.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/ui-test.log) |
| UI typecheck（UI） | `npm run typecheck` | PASS；[ui-typecheck.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/ui-typecheck.log) |
| UI production build（UI） | `npm run build` | PASS；87 modules；[ui-build.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/ui-build.log) |
| release Runtime | `cargo build --release -p chatgpt-harness-runtime` | PASS；[runtime-release-build.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/runtime-release-build.log) |
| Tauri host（UI） | `& ./node_modules/.bin/tauri.cmd build --no-bundle` | PASS；[tauri-no-bundle-build.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/tauri-no-bundle-build.log) |
| R：M6 Runtime 黑盒 | `cargo test -p chatgpt-harness-ui --lib m6_i18n_qa::m6_host_runtime_release_black_box_e2e -- --ignored --nocapture` | 6 个计划 case PASS；[m6-runtime-e2e.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m6-runtime-e2e.log) |
| F：M6 production frontend 黑盒 | `node ui/scripts/m6-frontend-e2e.mjs` | 18/18 PASS；runner 内 fresh 执行 `npm run build` 及 `cargo build -p chatgpt-harness-ui --example m6_config_bridge`；[frontend-e2e-run.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/frontend-e2e-run.log) |
| Diff | `git diff --check` | PASS |

Rust workspace 的 4 个 ignored case 分别为 M2、M3、M4、M6 黑盒 runner；它们都已另用 `--ignored` 显式执行。没有把 ignored 当成通过。

更详细的逐案断言见 [UI Integration evidence](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/ui-integration-evidence.md)、[host/runtime evidence](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/host-runtime-evidence.md)、[production frontend evidence](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/frontend-e2e-evidence.md)。下表 H/U/R/F 对应上面的实际执行命令。

## Integration：40/40 PASS

| Case | 状态 | 实际执行与关键 evidence |
| --- | --- | --- |
| INT-01 | PASS | H：schema v1 缺 ui.language 可加载为 None；完整 AppConfig 保持，schemaVersion=1。 |
| INT-02 | PASS | H：逐次保存并 reload en / zh-CN / None，精确 round-trip。 |
| INT-03 | PASS | H：磁盘 fr / zh-TW 均触发 ConfigError::Invalid，未静默变成 saved English。 |
| INT-04 | PASS | H：en→zh-CN，磁盘 revision 1→1。 |
| INT-05 | PASS | H：zh-CN→None，偏好清除，revision 1→1。 |
| INT-06 | PASS | H：非 UI 的 tool 配置变化，revision 1→2；语言保留。 |
| INT-07 | PASS | H：同 transaction 修改语言和 security，revision 仅 1→2；既有 config tests 还覆盖 Tools/Tunnel/Workspace/Policies/Bash 组合。 |
| INT-08 | PASS | H + U language-lifecycle + R/E2E-11：active=1/latest=2 保持，双语 Restart Required 提示保留。 |
| INT-09 | PASS | H + U language-lifecycle + R/E2E-10：active=latest=1，无新增 restart debt。 |
| INT-10 | PASS | H + U language-lifecycle + F/E2E-12：未保存 Settings draft 留在页面；语言 mutation 不写入其它字段。 |
| INT-11 | PASS | H atomic IO failure + U Settings failure test：原 bytes/config/revision 不变；selector/html.lang 回滚，显示本地化失败说明。 |
| INT-12 | PASS | U：saved en + system zh-CN→en；F/E2E-05 还观察 production 首次 App render。 |
| INT-13 | PASS | U：saved zh-CN + system en-US→zh-CN；F/E2E-06 观察 production 首次 App render。 |
| INT-14 | PASS | U：zh-CN / zh-SG / zh-Hans / zh-Hans-CN / zh 均→zh-CN。 |
| INT-15 | PASS | U：zh-TW / zh-HK / zh-MO / zh-Hant / zh-Hant-CN 均→en。 |
| INT-16 | PASS | U：fr/ja、空、缺失、非法 locale→en。 |
| INT-17 | PASS | U：primary navigator.languages 顺序确定；zh-TW,zh-CN→en，反序→zh-CN。 |
| INT-18 | PASS | U：render callback 前 i18n=zh-CN、html.lang=zh-CN；F 首屏观察补 production wiring 证据。 |
| INT-19 | PASS | U bootstrap seam + real App/Provider：configGet 失败仍初始化语言、渲染 First Run ConfigError recovery；reset 可用并恢复。 |
| INT-20 | PASS | U + Settings Follow system tests + F/E2E-08：启动、即时切换、Follow system 后 html.lang 正确。 |
| INT-21 | PASS | U + resource gate：599 个 leaf keys 完全一致。 |
| INT-22 | PASS | U + resource gate：两语言无空/纯空白 leaf。 |
| INT-23 | PASS | U + resource gate：每 key 的插值 placeholder 名称一致。 |
| INT-24 | PASS | U：计划规定的 15 个 critical groups 全部存在。 |
| INT-25 | PASS | U AppShell + existing shell gate：nav、page label、eyebrow、topbar 即时更新，route callback/child DOM 保留。 |
| INT-26 | PASS | U + Settings/FirstRun tests：labels/help/buttons、validation、notice、confirmation/recovery copy 双语；语言选项自识别。 |
| INT-27 | PASS | U Tools + existing Tools tests：availability/decision/readiness/restart/help 本地化；五种 canonical tool 名称及 Bash path 保留。 |
| INT-28 | PASS | U Policies：system friendly copy/decision 本地化；raw system/user IDs、matcher、用户 name/reason 原样。 |
| INT-29 | PASS | U Approvals：运行中切语言后 request-m6-uuid/tool/status 保留，actions 本地化；resolve 调用数=0。 |
| INT-30 | PASS | U Activity：event/decision/outcome/failure 本地化，tool/timestamp 不变；category/lifecycle/policyReason/Bash flags 的 typed mappings 两语言可解析；原 DTO 未被改写。 |
| INT-31 | PASS | U：计划九种 representative error code 均映射到两语言已定义资源。 |
| INT-32 | PASS | U：unknown code→localized generic error；无 success/allow fallback，只保留显式 sanitized detail。 |
| INT-33 | PASS | U shared states + observability tests：Loading/Empty/Error/Disconnected/ControlRead、refresh/retry 双语。 |
| INT-34 | PASS | U：Settings label 与 navigation aria-label 切换，input DOM node/focus 保留；可见属性 source audit PASS。 |
| INT-35 | PASS | U + F/E2E-15：raw tool/policy/request/path/command 保持，未作为翻译内容改写。 |
| INT-36 | PASS | H：revision=17 的 en/zh RuntimeConfigSnapshot 相等、无 ui；R/E2E-17/18 比较真实 registry/policy/MCP 行为。 |
| INT-37 | PASS | U real App/Provider/client spies + F：语言写入只走 uiLanguageSet/config refresh；Runtime/Harness start/stop/restart 调用数=0。 |
| INT-38 | PASS | U Tools/Approvals + resources：两语言均说明 cwd 非沙箱、可访问工作区外文件/网络；未分类高风险不能 session allow，风险语义不弱化。 |
| INT-39 | PASS | full npm gate 内 source-coverage.test.ts 实际执行 AST primary-copy 与 direct-key audit；并非仅用 INT-39 marker 声称通过。 |
| INT-40 | PASS | U 新增中文动作回归 + existing English gate：Start/Stop/Restart、tool/Bash config、policy save、approval resolve、control refresh 调用同一操作；M0–M5 真 Runtime 回归另见下表。 |

Activity 当前 table 没有 category/lifecycle/policyReason/Bash flags 列；上述 INT-30 对这些字段验证 typed presentation mappings 和 DTO 保真，没有新增 UI 列，也没有声称这些字段已显示在当前 table。

## Local Black-box E2E：22/22 PASS

F 的 18 个 case 与 R 的 6 个 case 在 E2E-10/11 重叠；按计划合并后是 22 个唯一 case，全部执行。

| Case | 状态 | 实际执行与关键 evidence |
| --- | --- | --- |
| E2E-01 | PASS | F：fresh npm build 成功，真实 bundle 包含并能运行 en / zh-CN 资源。 |
| E2E-02 | PASS | F：新隔离配置目录、无 saved language，system en-US→en；English AppShell/nav、html.lang=en。 |
| E2E-03 | PASS | F：无 saved language，system zh-CN→zh-CN；中文 shell/nav、html.lang=zh-CN。 |
| E2E-04 | PASS | F：ja-JP / fr-FR / zh-TW 三项 production bootstrap 均 fallback en。 |
| E2E-05 | PASS | F：磁盘 saved en 覆盖 system zh-CN；第一个 AppShell render 已是 en，首屏 labels 正确。 |
| E2E-06 | PASS | F：磁盘 saved zh-CN 覆盖 system en-US；第一个 AppShell render 已是 zh-CN。 |
| E2E-07 | PASS | F：真实 command 写 zh-CN，关闭原 bootstrap DOM 后在新 Node process 重新 bootstrap，磁盘/首屏均 zh-CN，revision=1。 |
| E2E-08 | PASS | F：真实 command 清除 language（磁盘字段 absent）；重新 bootstrap：en-US→en、zh-CN→zh-CN。 |
| E2E-09 | PASS | F：en→zh-CN→en→Follow system，每步读取磁盘，revision 序列 [1,1,1]。 |
| E2E-10 | PASS | F 生命周期 mutation=0；R active/latest=1/1，PID 17896 / Runtime ID e6a4da5f-d00d-4005-b809-6a405c4f107f 不变，无 restart debt。 |
| E2E-11 | PASS | F 生命周期 mutation=0；R active=1/latest=2，PID 21380 / Runtime ID 4b202643-4981-4331-84b4-1bda21b0aa49 不变，restart debt 保留；有效 MCP write 仍来自旧代次。 |
| E2E-12 | PASS | F：未保存 tunnel draft 留在 React；语言保存期间 full Save disabled，磁盘 tunnel 不变；随后真实完整保存提交 draft 并保留 zh-CN。 |
| E2E-13 | PASS | F：真实 ConfigStore temporary-file 创建失败；原 config bytes/revision/preference 保持；UI 回滚 en，atomic config 可重新读取。 |
| E2E-14 | PASS | F：执行实际 production main/App，en/zh 各访问 overview/tools/policies/activity/approvals/settings 六 routes；无 raw translation-key 或 runtime render error。 |
| E2E-15 | PASS | F：两语言 DOM 中 14 个明确机器 token 相同，含 tool names、policy IDs、request/runtime/generation IDs、path、git status --short、active_workspace。 |
| E2E-16 | PASS | F：production-equivalent Pending write/Bash fixture，语言改变后同 request identity/status/actions，runtime_approvals_resolve 调用数=0。 |
| E2E-17 | PASS | R：两种语言启动真实 release generations；snapshot 无 ui.language；归一化掉新进程 ID/time 后 Runtime effective/status/policy/read 行为等价。 |
| E2E-18 | PASS | R：真实 loopback MCP tools/list（names/schema/annotations）及 read_file result、effective tools en/zh 完全相同。 |
| E2E-19 | PASS | F bridge 调用真实 RuntimeSupervisor unavailable/control-read 路径：两语言 machine code 都是 RUNTIME_UNAVAILABLE；UI error localization 由 INT-31 同时验证。 |
| E2E-20 | PASS | R：真实 Git Bash + Developer/Test + Bash enabled；available_with_approval/ready，PID 29232 下语言切换前后 effective readiness/decision/policy 完全相同。 |
| E2E-21 | PASS | F：fresh-built source/build 所用 599 keys、placeholder、nonempty 完整；检查 bundle 的实际 fallback=en、supportedLngs=[en,zh-CN] 配置。 |
| E2E-22 | PASS | R：单一 PID 22004 / Runtime ID cdf2ab03-442a-4064-adec-ac4ca9c9bdae 完成语言 transitions；process 已退出、loopback endpoint 已关闭，config 可 reload，无额外 generation。 |

LOCAL_M6_E2E=PASS

最终进程盘点没有本轮 `target/release/chatgpt-harness-runtime.exe` 实例残留。另有 PID 16272 指向 `artifacts/M5-Human-Approval/...portable/runtime/` 的独立旧 Portable 实例；本轮保留该实例，没有使用它作为 M6 QA 证据或停止它。盘点记录：[process-cleanup.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/process-cleanup.log)。

## M0–M5 regression 与旧 M3 INT-04

| 回归 | 实际命令 | 结果 / evidence |
| --- | --- | --- |
| M0 与通用 Rust/UI 回归 | `cargo test --workspace` + `npm test` | 440 Rust、206 UI tests PASS；文件、安全、policy、approval、Bash、control read/lifecycle 均在实际 gate 内。 |
| M1 真 Runtime | `cargo test -p chatgpt-harness-runtime --release --test m1_e2e -- --nocapture` | E2E-01–08 PASS；[m1-e2e.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m1-e2e.log) |
| M2 真 Runtime | `cargo test -p chatgpt-harness-ui --lib runtime::m2_e2e::m2_local_black_box_e2e -- --exact --ignored --nocapture --test-threads=1` | E2E-01–12 PASS；[m2-e2e.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m2-e2e.log) |
| M3 真 Runtime | `cargo test -p chatgpt-harness-ui --lib runtime::m2_e2e::m3_local_black_box_e2e -- --exact --ignored --nocapture --test-threads=1` | E2E-01–14 PASS；[m3-e2e.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m3-e2e.log) |
| M4 真 Runtime | 设置 `$env:HARNESS_M4_E2E_RUNTIME_BIN = (Resolve-Path target/release/chatgpt-harness-runtime.exe).Path`；`cargo test -p chatgpt-harness-runtime --release --test m4_e2e -- --ignored --nocapture --test-threads=1` | E2E-01–18 PASS，同请求恢复/session grant/clean shutdown；[m4-e2e.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m4-e2e.log) |
| M5 真 Runtime/Git Bash | 设置 `$env:HARNESS_M5_E2E_RUNTIME_BIN = (Resolve-Path target/release/chatgpt-harness-runtime.exe).Path`；`node artifacts/M5-Controlled-Bash/qa/m5_e2e.js` | E2E-01–25 PASS，含真实 shell、审批、session grant、cwd、concurrency/output/timeout/process-tree 清理及文件工具；[m5-e2e.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m5-e2e.log) |
| M3 旧 fixture 复现 | `cargo test -p chatgpt-harness-ui --lib m3_config_integration -- --nocapture` | 修复前真实 FAIL：INT-04；[m3-fixture-before.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m3-fixture-before.log) |
| M3 fixture 修复后 | `cargo test -p chatgpt-harness-ui --lib runtime::m3_config_integration::m3_config_integration_cases -- --exact --nocapture` | INT-01–05 PASS；[m3-fixture-after.log](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/m3-fixture-after.log) |

旧 M3 INT-04 属于**测试夹具过时，不是 M6 产品回归**。依据是：M6 之前的 `4e2514ed54b5eed72318da7d9a73fc5b4249d0d8` 已允许 tool matcher `bash`；当前 `PolicyConfig::validate_for_save` 及 `protocol/tests/m5_policy_contract.rs` 也明确支持合法 Bash rule。旧测试仍把 Bash 当成 M3 尚未实现的工具并断言保存失败。

最小修正：负向 fixture 改为真正未知的 `unknown-tool`，继续证明 reject + 原文件 bytes 不变；新增合法 Bash ASK rule 的保存/reload 正向断言，revision=2。没有放松 production validation，也没有忽略原失败。Normal/disabled Bash 的执行边界由 M5 真实黑盒回归继续验证。

## 必要修复、新增测试与遗留情况

产品修复仅涉及 [SettingsPage.tsx](C:/AI/ChatGPT-Harness/ui/src/pages/settings/SettingsPage.tsx)：语言保存 pending 时，完整 Save/Reset 原先仍可发起；反向完整保存 pending 时语言 selector 也仍可用。这会使旧 UI preference 与异步新偏好竞争写入，或让 Reset 后 pending 语言写入重建 defaults。本轮增加语言保存/完整保存/Reset 的 guard 与 disabled 互斥，不改变配置 schema、Runtime authority 或完整保存的字段语义。

新增 [language-mutation-serialization.test.tsx](C:/AI/ChatGPT-Harness/ui/src/pages/settings/language-mutation-serialization.test.tsx) 证明 pending 时不会发出竞争 mutation，完成后完整保存保留新语言和 draft。修复前 2 tests FAIL，修复后 2 PASS；日志为 [before](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/language-race-before.log) / [after](C:/AI/ChatGPT-Harness/artifacts/M6-i18n/qa/language-race-after.log)。生产 bundle E2E-12 还验证了真实磁盘保存路径。

新增 QA 支撑：

- [m6-integration.test.tsx](C:/AI/ChatGPT-Harness/ui/src/m6-integration.test.tsx) 与 [m6-runtime-boundary-integration.test.tsx](C:/AI/ChatGPT-Harness/ui/src/m6-runtime-boundary-integration.test.tsx)。
- [m6_i18n_qa.rs](C:/AI/ChatGPT-Harness/ui/src-tauri/src/runtime/m6_i18n_qa.rs)，复用既有 M2 runner 的 test-only process/MCP primitives。
- [m6-frontend-e2e.mjs](C:/AI/ChatGPT-Harness/ui/scripts/m6-frontend-e2e.mjs) 与 [m6_config_bridge.rs](C:/AI/ChatGPT-Harness/ui/src-tauri/examples/m6_config_bridge.rs)。

没有未解决 M6 FAIL/BLOCKED。现有 ts-rs 对 deny_unknown_fields 的解析 warning、unused/dead-code 与 linker warning 仍作为非阻塞记录保留，没有借 QA 清理无关代码。

执行工具方面，最初 npm/PowerShell 吞掉 `--no-bundle` 参数的 host build 已中止，改用直接 tauri.cmd 后成功；M4 runner 首次缺少显式 Runtime binary 环境变量，补齐当前 release binary 后成功。前端 runner 的 Node navigator 兼容、eligibility read 被误匹配为 lifecycle mutation、React input 事件与断言强度问题已在测试支撑代码内修正后重跑；最终 verdict 依据最终完整 gate，不依据早期 runner 结果。



