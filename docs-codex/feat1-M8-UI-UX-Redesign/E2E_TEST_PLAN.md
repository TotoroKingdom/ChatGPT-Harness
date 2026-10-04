# M8 E2E Test Plan — Local Black-box UI/UX Regression

> Status: DRAFT / READY FOR IMPLEMENTATION QA DESIGN  
> Milestone: M8 — UI/UX Simplification & Visual Redesign  
> Owner: Independent QA / Coding Agent  
> Requirement: automated local black-box verification against current production build artifacts and real Runtime/Control/MCP behavior.  
> Explicit boundary: this is **not M9 External ChatGPT acceptance**.

## 1. Objective

M8 的 Local Black-box E2E 不重复视觉人工验收，而是验证 UI 重构后，最终 production build 仍然连接真实 Harness behavior，并且没有因为 presentation rewrite 破坏 M0–M7 的 authoritative state / mutation / approval / activity / recovery 语义。

目标链：

```text
production frontend build
+ Tauri host / real app command surface
+ real config persistence
+ real Runtime subprocess
+ real stdin/stdout Control
+ real loopback MCP
→ observable product state / mutation outcomes
```

核心原则：

> M8 E2E 必须使用真实 current-build Runtime / Config / MCP / Approval / Activity evidence；fixture-only page tests 只能算 Integration，不能算 Local Black-box E2E。

## 2. Entry Criteria

- G0 已批准。
- T1–T9 Development Complete。
- M8 Integration PASS。
- `npm run typecheck` PASS。
- `npm test` PASS。
- `npm run build` PASS。
- `cargo test --workspace` PASS 或不存在未解释的后端回归。
- final-ish current-source Tauri/Runtime build可启动。
- disposable config / Workspace test environment准备完成。

## 3. Black-box Boundary

### Required real components

至少使用：

```text
current production frontend assets
current Tauri host command surface
real ConfigStore on disk
real Runtime executable subprocess
real Control JSONL
real loopback MCP
real filesystem Workspace
real ApprovalManager / Activity
```

### Not allowed as substitution

- React fixtures代替真实 Runtime state。
- mock HarnessControlClient代替 real product path。
- mock MCP server。
- in-process PolicyEngine test。
- External ChatGPT/Tunnel作为本阶段必需条件。
- screenshot肉眼判断代替 automated system assertions。

### UI-driving note

如果仓库在实施时已有稳定 desktop automation/WebDriver harness，可用它驱动最终 app。

如果没有成熟 GUI automation infrastructure，不要求为 M8 新增高风险/脆弱的 native automation stack。此时：

- UI interaction semantics由 Integration自动验证。
- Local black-box重点验证 real app command/service boundaries + real Runtime/MCP chain + build identity。
- 视觉/空间/鼠标手感/键盘完整路径由 Manual UX Acceptance完成。

不得因为没有 GUI automation就把 fixture test重新命名为 E2E。

## 4. Test Build Identity

每次 E2E必须记录：

```text
source HEAD
relevant working tree diff identity
frontend production build identity
Tauri executable path
Runtime executable path
Runtime SHA-256
test config directory
test Workspace
```

若 source/build identity变化，受影响 E2E必须重新执行。

## 5. Required E2E Cases

### E2E-01 — Production app boot with real config path

使用 disposable config directory启动 current build。

验证：

- app host可启动。
- config metadata可读取。
- Runtime未启动时 UI/service层不伪造 Ready。
- config path指向 test environment。

### E2E-02 — Existing valid M7 config loads without semantic change

准备合法 schema v2 config。

启动 current M8 build：

- config值保持。
- configRevision不因 UI rewrite自动变化。
- Tool/Policy/Bash/Language现有值不被重置。

### E2E-03 — Real Harness Start

通过当前 app command/client surface启动 Harness。

验证：

- Runtime真实子进程启动。
- Runtime PID存在。
- Control initialize成功。
- MCP endpoint可用。
- authoritative Runtime status最终 Ready。

不能用 command ack冒充 Ready。

### E2E-04 — Real Stop

Ready状态发 Stop。

验证：

- Runtime process退出。
- MCP endpoint释放。
- status最终 Stopped。
- active/effective state不再显示为 current Ready。

### E2E-05 — Real Restart / restart-required convergence

在 Runtime running时修改 Runtime-affecting config，例如 safe Tool enabled/default change。

验证：

```text
persisted desired changes
→ configRevision advances
→ current Runtime revision unchanged
→ restartRequired true
→ restart
→ new Runtime revision = saved revision
→ restartRequired false
```

### E2E-06 — Language remains Runtime-neutral

运行中通过真实 language preference path切换 English/zh-CN。

验证：

- language持久化。
- current Runtime PID不变。
- configRevision不增加。
- restartRequired不新增。

### E2E-07 — Scoped Tool mutation does not overwrite independent state

准备 config：

- language有非默认值。
- Workspace非默认。
- multiple tool settings。

通过 current M8 mutation path只改变一个 Tool setting。

随后 real config readback：

- target field changed。
- Language/Workspace/其它 Tool fields保持。

### E2E-08 — Sequential independent mutations survive

快速执行两项不同 mutations，例如：

```text
read_file enabled OFF
write default ASK
```

最终真实 config readback必须同时包含两项。

### E2E-09 — Tool configured/effective mismatch is real

Runtime running时保存 Tool mutation但不 restart。

通过真实 config + Runtime effective reads确认：

- saved desired新值。
- Runtime effective旧值。
- restartRequired真实存在。

这是 M8 UI configured/effective presentation的后端真相证据。

### E2E-10 — Real Tool toggle after restart

disable一个安全 tool，restart。

真实 MCP `tools/list`：

- tool不再暴露。

stale/direct call：

- 返回既有 disabled error。
- 没有 side effect。

重新 enable + restart后恢复。

### E2E-11 — Real Policy default mutation

通过真实 config mutation修改某 Tool Default。

restart后：

- Runtime Policy effective read反映新值。
- MCP behavior与 M3 semantics一致。

M8 UI不得改变 precedence。

### E2E-12 — Real User Rule create/edit/disable/delete

通过当前 config/service path执行一条 safe user rule lifecycle：

```text
create
edit
disable
enable
delete
```

每一步真实 config readback验证：

- ID稳定。
- matcher无 silent rewrite。
- unrelated rules/settings不被覆盖。

### E2E-13 — Bash matcher round-trip

若 test host具备 M5 Git Bash环境：

创建已有支持范围的 BashCommand matcher。

真实 config readback + Runtime policy projection验证：

- executable/argvPrefix保持。
- M8 natural editor没有改变 Runtime matcher semantics。

若环境不具备 Bash，可标：

```text
BLOCKED_BY_ENVIRONMENT
```

但 config round-trip仍应执行。

### E2E-14 — Real ASK approval pending

选择安全 file tool/operation配置 ASK。

使用本地 MCP client发起真实 call。

验证：

- original call保持 Pending。
- real approvals.list出现 request。
- pending count真实。

### E2E-15 — Allow once same-request resume

对 E2E-14 request调用真实 approvals.resolve AllowOnce。

验证：

- 原 call成功恢复。
- 无第二次 MCP call。
- filesystem结果正确。
- Activity有 policy→approval→execution evidence。

### E2E-16 — Deny path

发起另一个 ASK mutation，真实 Deny。

验证：

- original call结束为 denied。
- no filesystem mutation。
- Activity outcome正确。

### E2E-17 — Approval session eligibility remains Runtime-owned

构造 classified eligible与不eligible（如现有 Bash unclassified fixture，环境允许时）approval。

验证 Control返回的 eligibility与 resolve behavior保持 M4/M5 semantics。

M8不得改变 grant eligibility。

### E2E-18 — Approval stale on generation change

制造 pending approval后重启/stop current Runtime generation。

验证：

- old request不再可 resolve。
- real queue refresh反映 stale/terminal/new generation truth。
- 不复用 old approval。

### E2E-19 — Activity evidence survives UI redesign

通过真实 MCP完成：

- success。
- policy deny。
- approval deny/approved。
- execution failure或 unavailable之一。

读取真实 Activity snapshot：

- event/request IDs存在。
- M7 sanitized evidence仍在。
- matched rules / approval / duration / outcome未被 M8 contract破坏。

### E2E-20 — Recovery: invalid config

在 disposable config写入 invalid config。

启动/调用真实 config load路径：

- 返回 existing typed config error。
- config metadata/path仍可获得。
- 原文件未被 UI silently reset。
- Recovery surface所需 facts可读取。

修复后 Retry能恢复正常。

### E2E-21 — Recovery: unsupported schema

写入 unsupported future schema。

验证：

- existing unsupported-schema machine error。
- no silent downgrade/reset。
- original file unchanged。

### E2E-22 — Recovery: migration-failure path remains intact

使用 M7 source-controlled failing migration fixture或等价 disposable fixture。

验证：

- failure不覆盖原文件。
- recovery facts存在。
- M8没有把 failure当 first-run/new install。

### E2E-23 — Credential configured vs connected boundary

在不暴露 raw secret的前提下验证：

- credential configured status可独立读取。
- Tunnel/Harness connected状态来自 runtime observation。

M8 service path不能把 configured=true当 connected=true。

### E2E-24 — Config reset real semantics

使用 disposable environment执行真实 reset。

验证：

- reset影响与既有 product contract一致。
- 不因为 M8 UI rewrite扩大/缩小实际 reset范围。
- reset后 app可进入正确 setup/recovery state。

### E2E-25 — First-run underlying path

从 clean disposable config开始：

```text
Workspace
→ Connection config facts
→ readiness
→ Start Harness
```

验证底层真实 sequence：

- Workspace保存。
- Connection保存。
- readiness不是 connected。
- Start后才得到 real Ready。

不要求 E2E对页面像素做断言。

### E2E-26 — M0–M7 real MCP regression

同一 M8 current build的 Runtime至少真实执行：

- read_file。
- glob。
- write。
- edit。

并验证：

- tool toggle。
- policy deny。
- approval。
- Activity。

若 Git Bash available，再执行一个 safe bash golden local path。

### E2E-27 — Production frontend bundle / Tauri packaging smoke

当前 M8 source执行 production frontend build / Tauri build所需命令。

验证：

- generated assets存在。
- app加载 production assets所需路径/manifest有效。
- custom protocol/embedded frontend没有明显缺失。

这不是人工视觉 acceptance。

### E2E-28 — Clean shutdown

最终：

- no pending approvals。
- no active Runtime。
- no orphan child process。
- disposable config/Workspace可清理。
- test-created processes全部退出。

## 6. Optional Desktop Automation Extension

若实施时存在可靠的 native/webview automation，可增加 automated desktop cases：

```text
navigate six pages
toggle one Tool
open/close Inspector
create/cancel Rule draft
resolve safe Approval
change Language
verify dirty-leave dialog
```

这些只能作为 E2E增强；视觉 quality仍由 Manual Acceptance决定。

不得因为 desktop automation不可用而阻塞 M8，只要 Integration + Local black-box + Manual UX 的职责边界完整。

## 7. Acceptance Mapping

| SPEC Acceptance | Local Black-box Evidence |
| --- | --- |
| AC01 | E2E-01, 03–05 + Manual UI |
| AC02 | E2E-03–05, 09 |
| AC03 | E2E-07–12 |
| AC04 | E2E-03–05 |
| AC05 | E2E-13 + Integration |
| AC06 | E2E-12–13 |
| AC07 | Integration + Manual |
| AC08 | E2E-14, 18 |
| AC09 | E2E-14–17 |
| AC10 | E2E-19 |
| AC11 | E2E-23 |
| AC12 | E2E-06, 20–25 |
| AC13 | Manual UX |
| AC14 | E2E-10–26 + diff review |
| AC15 | Portable stage / Manual identity |

## 8. Evidence Format

最终 QA report至少记录：

```text
Source HEAD
Working tree identity
Frontend build command/result
Tauri/Runtime binary paths
Runtime SHA-256
Disposable config path
Disposable Workspace
E2E-01 ... E2E-28 PASS / FAIL / BLOCKED
real MCP endpoints
request IDs for approval/activity cases
configRevision / runtime revision evidence
filesystem assertions
process cleanup
```

## 9. Exit Criteria

Local Black-box E2E PASS要求：

- E2E-01 ～ E2E-28完成；环境依赖项可明确 BLOCKED，但不得用 mock替代。
- real Runtime / Control / MCP链路 PASS。
- scoped mutation真实 config readback PASS。
- configured/effective/restart semantics PASS。
- real Tool/Policy/Approval/Activity regressions PASS。
- migration/recovery regression PASS。
- no unauthorized Runtime/Protocol behavior change。
- production frontend/Tauri build smoke PASS。
- clean shutdown PASS。

随后才允许构建独立 M8 Portable并进入 Manual UX Acceptance。

## 10. Non-Claims

Local Black-box E2E PASS不代表：

```text
visual design accepted
desktop spacing/layout accepted
all keyboard paths manually accepted
External ChatGPT golden paths accepted
M8 ACCEPTED / FROZEN
V3 ACCEPTED / FROZEN
```

External ChatGPT完整 golden paths属于 M9。
