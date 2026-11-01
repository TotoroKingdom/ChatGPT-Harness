# M03 E2E 测试计划 — 打包候选的持久化历史与授权隔离

> 状态：VALIDATION BASELINE FROZEN / NOT EXECUTED（验证基线已冻结 / 尚未执行）
> 里程碑：M03 — Shared Evidence and Durable SQLite History（共享证据与持久化 SQLite 历史记录）
> 输入：SPEC.md、PLAN.md、TASKS.md、INTEGRATION_TEST_PLAN.md
> Owner：M03 Development Complete（开发完成）且 Integration 有真实结果后，由独立 QA / Coding Agent 执行
> 范围：Codex-autonomous local QA（Codex 自主本地 QA）；使用打包 Windows candidate（候选版本）、隔离的 AppData / Workspace / SQLite，以及真实 Runtime、MCP、filesystem、Activity、SQLite、Tauri 和 WebView/React UI。
> 外部边界：真实 External ChatGPT → Tunnel → Harness → Runtime 不在本 E2E 执行，由 MANUAL_ACCEPTANCE_CHECKLIST.md 负责；外部链路不可用不能阻塞本地 E2E。
> 安全：所有破坏性 DB / fault 场景必须使用一次性 Windows profile/AppData、Workspace 和 SQLite 边界；不得读取、清除、锁定或污染用户真实 History 数据库。

---

## 1. 目标与边界

M03 E2E 验证一个可交付的打包候选在本机真实产品链上的行为：

~~~text
打包 Windows candidate
→ 隔离 AppData / Workspace
→ Tauri desktop host + 真实 WebView/React UI
→ 本地真实 Runtime
→ 本地真实 MCP 边界
→ 受控 filesystem Tool fixture
→ ExecutionPipeline / Policy / Approval
→ canonical AuditEvent
→ Activity + SafeEvidenceProjector
→ Runtime SQLite writer
→ Tauri read-only History facade
→ 打包 History UI
~~~

本地 E2E 必须通过实际打包应用和真实本地链路证明行为。以下证据不能单独使 E2E PASS：

- Rust unit test、Vitest/jsdom、React component test 或内部 executor-only test；
- 直接调用 ExecutionPipeline / executor 而绕过生产 MCP 边界；
- mock MCP、mock Tauri、mock WebView、模拟进程存在或 source-only 静态检查；
- 仅凭 SQLite 文件存在、日志片段或进程列表推断 UI / Runtime / writer 已正确组合。

受控 fault fixture 可以用于制造 busy/full/corrupt/queue 等状态，但它只能补充故障触发；每个 E2E case 仍必须启动打包 candidate，并通过真实本地 Runtime、MCP、SQLite、Tauri 与 WebView/React UI 观察结果。

责任划分如下：

| 验证层 | 本计划中的责任 |
| --- | --- |
| Integration | 使用正式 M03 QA runner 验证本地 Runtime/Tauri/evidence-store/SQLite 组合与 18 个集成 case；不需要 External ChatGPT/Tunnel。 |
| E2E | 使用同一打包 candidate，验证隔离 AppData/Workspace/SQLite 上的真实本地端到端行为；不需要 External ChatGPT/Tunnel。 |
| Manual Acceptance | 由人工验证真实 External ChatGPT → Tunnel → Harness → Runtime 链路、外部隐私/权威观察和最终打包交互。 |

---

## 2. Entry Criteria（进入条件）

正式执行 M03 E2E 前必须满足：

- M03 T1–T6 Development Complete；
- M03 Integration 的正式结果可追溯，且其 required cases 没有以 mock 或 source-only 证据冒充 real boundary；
- candidate commit、打包类型、包 SHA-256、App/Runtime 版本和 SQLite schema/checksum 已锁定；
- 可启动一个与用户真实环境隔离的打包 candidate，而不是 dev server 或未打包源码；
- 可创建唯一且可验证的隔离 AppData/profile、Workspace、SQLite、lock、backup 和 process 边界；
- 本地真实 Runtime、生产 MCP 边界、controlled filesystem fixture、Activity、SQLite writer、Tauri facade 和 WebView/React UI 均可由 QA runner 驱动；
- E2E runner、fixture identity、故障恢复和清理步骤已准备；
- 不使用用户真实 History DB、真实 Workspace、真实 SecretStore 值或未脱敏的生产配置。

External ChatGPT、Tunnel、付费模型、外部账号和外部授权不是本地 E2E 的进入条件。缺少或未授权外部 ChatGPT 时，应在人工验收中记为 BLOCKED / NOT EXECUTED；不得把它写成本地 E2E 的 BLOCKED，也不得因此用 mock 外部链路冒充通过。

如果无法启动隔离的打包 candidate，或无法建立真实的本地 Runtime → MCP → Tauri/WebView 链路，本地 E2E 才可以记为 BLOCKED。其他 fixture 不可用、某个行为无法证明或证据不足，必须如实记为 FAIL 或 INCONCLUSIVE，不得扩大 BLOCKED，也不得降低边界。

---

## 3. Candidate Lock（候选版本锁定）

四个 case 默认使用同一最终 candidate。记录以下字段；永久证据只记录 fixture identity 或经脱敏的 containment proof，不记录 secrets、未限制的物理路径或真实用户数据：

~~~text
Candidate commit:
Package type:
Package SHA-256:
Package/App version:
Runtime version:
Local MCP boundary/version:
Evidence-store version:
SQLite version:
Schema version/checksum:
Windows build:
Isolated AppData/profile fixture ID:
Isolated Workspace fixture ID:
SQLite DB/lock/backup fixture ID:
Controlled filesystem fixture ID:
History desired/effective config:
Formal M03 E2E QA runner + exact command:
QA owner:
Execution time:
~~~

候选版本必须来自实际打包产物。若代码、打包内容、Runtime 或 schema 发生改变，受影响的正式 suite 必须在新 candidate 上重新执行；不得把不同 candidate 的结果拼成一个 PASS。

E2E 的 External ChatGPT / Tunnel 字段为 N/A — owned by Manual Acceptance；不以本地 runner 的 MCP 调用声称已经验证外部链路。

---

## 4. QA Runner / Fixture Contract（QA runner 与 fixture 契约）

本节只描述预期执行方式，不填充执行结果；本文档当前保持 NOT EXECUTED。

### 4.1 预期 runner 与辅助命令

正式 E2E 必须使用能启动打包 candidate 的 M03 QA runner，并在证据中记录精确命令、candidate identity、fixture identity、实际结果和清理结果。若需要构建或验证便携包，可按既有脚本的参数执行：

~~~text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\ui\scripts\m03-e2e-run.ps1 -Case all -TimeoutSeconds 300
~~~

该入口通过 `ui/scripts/m03-e2e-build.ps1` 使用同一源码构建 packaged candidate，并在 Tauri 配置中写入唯一的 disposable `appDirectoriesOverride`；`ui/scripts/m03-e2e-driver.mjs` 通过真实 WebView2/CDP、Tauri command、本地 Runtime/MCP、filesystem、SQLite 和 React UI 执行 case。正常 portable candidate 的验证仍可使用既有 `build-portable.ps1` / `verify-portable.ps1`，但不能替代隔离 packaged E2E。

以下命令只能作为开发/集成 preflight 或辅助证据，不能替代本计划的打包黑盒 E2E：

~~~text
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

正式 runner 至少应完成：

1. 从锁定的 package 启动真实 Tauri host、WebView/React UI、Runtime 和生产 MCP 边界；
2. 将 AppData、Workspace、SQLite、lock、backup、sentinel 和 fault fixture 放入唯一临时根目录；
3. 通过 MCP 发送本地 Tool 请求，经过 Runtime/Policy/Approval 和真实 filesystem fixture；
4. 通过 Activity、Tauri History facade、实际打包 UI 和只读 QA inspection 观察结果；
5. 对 Runtime/App 重启、offline read、maintenance、clear、retention 和 cleanup 做可追溯记录；
6. 在成功、失败、异常和清理路径都释放进程、锁、临时数据库和 fixture。

npm test、内部 executor test、mock UI 或直接调用 Rust API 即使通过，也不能作为上述 runner 的替代。

### 4.2 预期 fixture 矩阵

- success：受控、非破坏性的 filesystem read 或等价安全 Tool；
- DENY：当前 Policy 明确拒绝，且验证没有不应发生的 filesystem side effect；
- ASK-terminal：真实 MCP 请求进入 ASK，并通过安全的 allow-once / deny / expiry / cancel 之一到达终态；
- restart/offline：同一隔离 DB、Runtime stop、App quit/reopen、Runtime 新实例；
- filtering/detail：混合 runtime、Tool、outcome、Policy、Approval 状态和稳定时间；
- storage fault：真实 lock contention、busy/full、corrupt/incompatible、queue/drop 或正式 runner 支持的 production-equivalent fault；
- retention/clear：包含已结束 request family、open request、关联 evidence/approval 和 Workspace sentinel；
- privacy/authority：合成 API key、SecretStore sentinel、物理路径、proxy credential、环境变量、stdout/stderr、完整文件内容和 Bash preview sentinel。

所有 fixture 必须是合成数据。真实外部 ChatGPT/Tunnel、用户凭据和用户 Workspace 不属于 E2E fixture。

---

## 5. M03-E2E-01 — 真实本地 Runtime/MCP 的 success / DENY / ASK-terminal 跨 Runtime/App 重启持久化

**AC：** M03-AC01、AC02、AC03、AC10、AC11、AC17
**Tasks：** T1/T3/T4/T5/T6

### 前置条件

- 已锁定的打包 candidate 使用隔离 profile；
- History 使用冻结/default 或已记录的配置；
- 受控 Workspace 和 filesystem Tool fixture 可用；
- 本地真实 Runtime、生产 MCP 边界、Activity、SQLite writer、Tauri facade 和 WebView/React UI 可启动；
- Policy fixture 可以安全地产生 success、DENY 和 ASK-terminal。

### 操作

使用正式 M03 E2E QA runner，从本地 MCP 边界发起至少：

1. 一个允许的非破坏性 Tool 请求并访问受控 filesystem fixture；
2. 一个被 Policy DENY 的 Tool 请求；
3. 一个进入 ASK 并以安全终态结束的请求（allow once / deny / expiry / cancel，按 fixture 定义）。

然后：

4. 在 Runtime 活跃时通过打包 UI 查看 Activity 和 History，并记录 request/runtime/event identity；
5. 显式 Stop Harness；
6. 完全退出并重新打开打包 App；
7. 保持 Runtime 初始停止，读取 History；
8. 启动本地 Runtime，再次读取 History。

### 断言

- MCP 请求确实经过打包 candidate 的真实 Runtime 和生产 MCP 边界，而不是直接调用内部 executor；
- success、DENY、ASK-terminal 的 filesystem side effect、Policy 结果和证据状态正确区分；
- 每个请求的 runtime_id + request_id 正确，canonical event_id 在 Activity、SQLite 和 History 投影中保持一致；
- 不依靠时间接近或 Activity sequence 做模糊关联；
- terminal Approval evidence 归属于正确 request，不能把 success 混成 DENY/ASK；
- 记录在 Runtime stop、App quit/reopen 和新 Runtime identity 后仍可查询；
- 旧行保持原来的 runtime identity，不被新 Runtime 改写；
- coverage（COMPLETE / INCOMPLETE / UNKNOWN）真实反映证据；
- UI、SQLite 和 runner evidence 不暴露 secret、raw path、command body、原始环境或不受限文件内容。

本地 E2E 不因 External ChatGPT/Tunnel 缺失而 BLOCK。若隔离打包 candidate 或真实本地 Runtime/MCP/Tauri 链无法启动，才可 BLOCK；若本地请求执行或证据断言失败，应记 FAIL。

**PASS 条件：**真实本地 MCP 请求在 Activity、SQLite、Tauri History 和打包 UI 中正确关联，并跨 Runtime/App 重启保持可解释。

---

## 6. M03-E2E-02 — 打包 History 的筛选/详情/offline read 与旧 Approval 不重新激活

**AC：** M03-AC04、AC07、AC10、AC11、AC14、AC15、AC16
**Tasks：** T2/T4/T5

使用 E2E-01 产生的持久数据，必要时用同一隔离 candidate 和正式 runner 增加受控记录。必须在真实打包 WebView/React UI 中操作；React unit test 或 mock facade 不算本 case。

### 操作

1. 停止本地 Runtime，打开打包 App 的 Activity → History；
2. 按 time、Tool、outcome、Policy decision 和 Approval state 筛选；
3. 使用生产 cursor path 翻页超过一页；
4. 打开 success、DENY 和 Approval-related request 的 detail；
5. 尝试 History 提供的全部可见导航/操作；
6. 启动本地 Runtime，在 writer 活跃时重复代表性只读查询。

### 断言

- Runtime 停止时仍可读取兼容 History；
- 浏览 History 不会偷偷启动 Runtime；
- 筛选结果与 fixture 预期一致；
- 稳定数据下分页没有重复或遗漏，cursor 与筛选条件绑定；
- request detail 展示安全、有序的 timeline；
- COMPLETE / INCOMPLETE / UNKNOWN 在适用时明确显示；
- terminal Approval 只作为历史观察可见，不提供 Approve/Deny/replay-authorize；
- 带有旧 Pending-looking observation 的历史记录无法在重启后被批准；
- 不会重建 SessionGrant 或 ExecutionPermit；
- Tauri read-only 查询可与 Runtime writer 共存，不会出现第二个 writer；
- UI 不显示 DB physical path 或 Workspace physical path。

**PASS 条件：**打包 UI 在 offline 仍然有用，但 History 永远不能变成 live authorization。

---

## 7. M03-E2E-03 — 真实本地存储降级、恢复、Retention 与 Clear

**AC：** M03-AC05–AC09、AC13–AC16
**Tasks：** T2/T3/T4/T5/T6

只使用隔离 profile、Workspace 和 SQLite。Fault 触发必须作用于真实打包 candidate 使用的文件锁/SQLite/队列/容量路径；不得用 mock store 代替。

### Phase A — ownership / degradation

通过正式 runner 触发至少一种 production-equivalent writer/maintenance fault：

- second-writer lock contention；
- runner 支持的 deterministic SQLite busy/IO/full fixture；
- disposable DB 的 incompatible/corrupt fixture。

运行并观察打包 App，同时通过本地真实 MCP 发起可安全重复的 Tool 请求。

必须确认：

- History/evidence status 进入正确的 typed degraded/busy/corrupt/incompatible state；
- Runtime Policy/Tool outcome 仍由 Runtime authority 决定；
- DB 不会被静默删除或重建；
- 适用时 UI 显示 loss/coverage warning；
- fault 恢复后本地 Runtime/MCP 和 History 能按产品定义恢复。

### Phase B — Retention

在隔离 DB 中填充足够的历史数据，用 QA 配置的 bounded limit 或保留 production algorithm 的 accelerated fixture 运行真实 retention。

必须确认：

- 删除的是完整 request family；
- active/open request 不被删除；
- 没有 orphan child rows；
- age cleanup 后 capacity cleanup 目标为配置上限的 90%；
- 结果在打包 History/status 中可观察。

### Phase C — Clear

Runtime 运行时：

- 通过打包 UI 执行 Clear History；
- 验证 online Runtime-writer barrier；
- 验证当前 open request/session 不被改变。

Runtime 停止后：

- 通过打包 UI/正式 Tauri maintenance path 执行 offline clear；
- 验证 maintenance 获得 exclusive ownership；
- 验证 schema 仍可用、返回结果可解释；
- 验证 Workspace fixture 没有被触碰。

**PASS 条件：**真实打包 candidate 将存储降级、恢复、Retention 和 Clear 表达为可观察的 storage truth，同时保持 execution/authority truth 不变。

---

## 8. M03-E2E-04 — 打包隐私、权威隔离、维护后本地 Tool 回归

**AC：** M03-AC03、AC11、AC14、AC18
**Tasks：** T1/T4/T5/T6

### 前置条件

- 使用与 E2E-01～03 相同的最终打包 candidate；
- 隔离 Workspace 包含 privacy sentinels 和安全 read fixture；
- History 已经历至少一次 restart 以及 clear/maintenance cycle；
- 本地真实 Runtime/MCP/Tauri/WebView 链可用。

### 操作

1. 在打包 UI 中检查 History list/detail 和 Settings；
2. 复制 UI 允许显示的 request ID / evidence text；
3. 通过 QA 只读检查验证禁止 sentinel 不在 UI 或隔离 SQLite 中；
4. 确认旧 History record 不能 approve/replay-authorize；
5. 正常启动本地 Harness；
6. 使用本地正式 MCP QA driver 发起一个新的非破坏性 Tool call；
7. 在当前 live Runtime Policy 允许时，确认请求成功并独立产生新的 History；
8. 在不绕过真实桌面宿主的前提下，检查受影响的 Tray/notification/diagnostics flow。

### 断言

- 不出现 API key、SecretStore value、物理 Workspace path、raw env/stdout/stderr、完整文件内容或不受限错误文本；
- 不出现持久化 Bash command preview；
- 不从 History-derived data 创建 SessionGrant/ExecutionPermit；
- Clear/Retention 没有损坏当前 Runtime security semantics；
- post-maintenance local MCP Tool call 按当前 Policy 重新判定；
- 新证据使用新的 runtime/request identity，不覆盖旧历史；
- M01/M02/V3 受影响功能仍可用；
- 没有重复 Runtime、重复 local tunnel（若候选包含）、重复 writer 或未清理 orphan process；
- 以上结论来自真实打包 WebView/React UI、Tauri、Runtime、MCP、SQLite 和 filesystem 组合，不来自 internal executor-only test。

External ChatGPT/Tunnel 不属于本 case。缺少外部链路只能影响 Manual Acceptance；不能把本地 E2E-04 标为 BLOCKED 或用本地结果声称已完成外部验证。

**PASS 条件：**M03 的本地打包链路在维护后仍然隐私安全、不可越权，并可用新的本地 MCP Tool 请求回归验证。

---

## 9. E2E Verdict Rules（结论规则）

允许的实际结论：

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

规则：

- 只有无法启动隔离的打包 candidate，或无法建立真实本地 Runtime → MCP → Tauri/WebView 链，才可以将本地 E2E case 记为 BLOCKED；
- External ChatGPT、Tunnel、外部账号或网络不可用，永远不是本地 E2E 的 BLOCKED 条件；它们由 Manual Acceptance 另行记 BLOCKED / NOT EXECUTED；
- success/DENY/ASK、offline read、fault、retention、clear 或 privacy 断言失败，记 FAIL；
- 证据不足或 fixture 无法证明目标行为，记 INCONCLUSIVE；不得用 internal executor、mock UI、source-only 检查降级为 PASS；
- 破坏性 fixture 触及用户真实 DB/Workspace，立即停止；相关证据无效，case 至少 FAIL，并在事故复核完成前不得重跑；
- 不允许用本地 MCP 结果替代 External ChatGPT → Tunnel → Harness → Runtime 证据。

示例：

- 旧 Approval 可从 History 操作 → E2E-02 / E2E-04 FAIL；
- History 跨重启存在但 request identity 通过 timestamp 模糊拼接 → E2E-01 FAIL；
- storage fault 将 Policy DENY 变成 ALLOW，或反之 → E2E-03 / E2E-04 FAIL；
- runner 只运行 Vitest/jsdom 或直接调用 executor → 相关 E2E 不能 PASS，记 INCONCLUSIVE/FAIL；
- 外部 ChatGPT 未授权 → 本地 E2E 仍可继续；Manual Acceptance 的外部项 BLOCKED / NOT EXECUTED。

---

## 10. E2E Exit Gate（退出门槛）

M03 E2E 只有在以下条件全部满足时才能记 PASS：

- M03-E2E-01～M03-E2E-04 全部 PASS；
- 四个 case 使用同一最终 candidate identity，或因代码变化完整重跑受影响 suite；
- 每个 case 都有正式 M03 E2E QA runner 的真实打包、真实本地 Runtime/MCP/Tauri/SQLite 和真实 WebView/React UI 证据；
- success、DENY、ASK-terminal、restart/offline History、filter/detail、old Approval non-actionability、storage degradation、retention、clear、privacy、authority 和 post-maintenance local Tool regression 均有覆盖；
- 只使用隔离 AppData、Workspace、SQLite、locks、backups 和合成 sentinel；
- 临时 profile、数据库、备份、锁、Workspace、进程和 runner residue 已清理；
- 没有后续代码或候选变化使证据失效。

External ChatGPT/Tunnel 不属于本地 E2E Exit Gate；它是 Manual Acceptance 的必需最终外部边界。即使本地 E2E 将来 PASS，也不等于 M03 ACCEPTED / FROZEN。

本文档当前只定义验证契约，未执行任何上述 case，不填写 PASS、FAIL、BLOCKED 或 INCONCLUSIVE 的执行结果。
