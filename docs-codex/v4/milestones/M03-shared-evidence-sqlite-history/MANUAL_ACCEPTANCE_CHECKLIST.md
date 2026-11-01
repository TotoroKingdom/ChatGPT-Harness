# M03 人工验收清单 — 共享证据与 SQLite 历史记录

> 状态：人工验收基线已冻结 / NOT EXECUTED（尚未执行）
> 里程碑：M03 — Shared Evidence and Durable SQLite History（共享证据与持久化 SQLite 历史记录）  
> 对应范围：Shared Evidence Contract（共享证据契约）+ F07 SQLite History（SQLite 历史记录）  
> 验收人：用户 / Human Reviewer（人工验收人）  
> 本清单的唯一外部责任：人工验证真实 External ChatGPT → Tunnel → Harness → Runtime → MCP Tool 链、外部隐私/权威观察，以及最终打包应用交互。
> 分层规则：本清单不重复执行本地自动化 E2E；M03-E2E-01～04 已负责隔离打包 candidate 上的本地 Runtime/MCP/SQLite/Tauri/WebView/React 组合。
> 说明：数据库损坏、满盘、队列溢出、事务回滚等故障注入由正式 QA runner 负责；用户只观察 QA 证据和安全的产品交互，不手工破坏真实数据库。

本清单当前只定义人工证据和勾选项，所有结果留空，不代表任何 PASS。

---

## 1. 术语说明

| 技术名称 | 中文含义 |
| --- | --- |
| Shared Evidence Contract | 共享证据契约 |
| Durable History | 持久化历史记录 |
| SQLite | 本地嵌入式数据库 |
| WAL / Write-Ahead Logging | 预写式日志模式 |
| Runtime | 运行时进程 |
| Tauri Host | Tauri 桌面宿主 |
| WebView / React UI | 实际桌面网页视图 / React 用户界面 |
| External ChatGPT | 外部真实 ChatGPT 会话或产品入口 |
| Tunnel | 连接 External ChatGPT 与 Harness Runtime 的真实传输边界 |
| AuditEvent | 运行时审计事件 |
| SafeEvidenceProjector | 安全证据投影器 |
| EvidenceEvent | 持久化证据事件 |
| runtime_id | 运行时实例标识 |
| request_id | 工具请求标识 |
| event_id | 事件标识 |
| Approval | 审批 |
| Pending Approval | 待审批请求 |
| SessionGrant | 当前会话授权 |
| ExecutionPermit | 执行许可 |
| Policy | 策略 |
| COMPLETE | 证据完整 |
| INCOMPLETE | 证据不完整 |
| UNKNOWN | 无法安全确认 |
| Retention | 历史保留策略 |
| Storage Cap | 存储容量上限 |
| Keyset Pagination | 基于稳定游标的分页 |
| Cursor | 分页游标 |
| Read-only | 只读 |
| Writer | 数据库写入方 |
| Maintenance Lock | 维护独占锁 |
| Restart Required | 需要重启 Runtime 才能生效 |
| HistoryConfig | 历史记录配置 |
| Evidence Store Status | 证据存储状态 |
| Degraded | 降级 |
| Corrupt | 数据库损坏 |
| Incompatible | 数据库版本不兼容 |
| Clear History | 清除历史记录 |
| Workspace | 工作区 |
| SecretStore | 凭据存储 |
| Bash Command Preview | Bash 命令预览 |
| AppData | Windows 应用数据目录 |
| Candidate | 已锁定的打包候选版本 |
| E2E | 本地打包端到端测试 |
| Integration Test | 本地集成测试 |
| QA Runner | 正式质量验证运行器 |

后文出现英文名称时，均按本表中的中文含义理解。

---

## 2. 验收前确认

请先确认：

- [ ] M03 Development（开发）已经完成。
- [ ] M03-INT-01～M03-INT-18 集成测试已经由正式 M03 QA runner 执行并有真实结果。
- [ ] M03-E2E-01～M03-E2E-04 已在隔离的打包 candidate 上执行并有真实本地结果。
- [ ] 当前打开的 Candidate 与本地 E2E 使用的是同一个版本。
- [ ] 已记录候选提交、包类型、SHA-256、App/Runtime 版本和 SQLite schema/checksum。
- [ ] QA 使用了隔离 AppData/profile、Workspace 和 SQLite 完成 fault/retention/clear 证据，没有破坏真实用户历史数据。
- [ ] 当前验收使用的是可安全操作的测试 Workspace。
- [ ] 已获得执行真实 External ChatGPT → Tunnel → Harness → Runtime 链的授权、账号/会话和可用 Tunnel route；不把本地 MCP driver 当作外部链路替代。
- [ ] 已明确人工验收使用的打包窗口、Tauri host、WebView/React UI，不是 dev server、mock UI 或内部测试页面。

缺少 External ChatGPT、Tunnel route 或外部授权时，Manual Acceptance 的相关项必须保持 BLOCKED / NOT EXECUTED；这不会把本地 E2E 标为 BLOCKED，但 M03 不能据此宣称完成最终人工验收。

验收信息：

~~~text
候选提交：
包类型：
包 SHA-256：
App 版本：
Runtime 版本：
Tunnel 版本/route identity（不得写入凭据）：
SQLite Schema：
Schema checksum：
隔离 profile / Workspace fixture ID：
External ChatGPT 会话/证据 ID（不得写入账号密钥）：
验收人：
日期：
~~~

---

## 3. M03-MAN-01 — 真实 External ChatGPT → Tunnel → Harness → Runtime 调用与 History 对应

**对应：** M03-AC01、M03-AC02、M03-AC03、M03-AC10、M03-AC16、M03-AC17  
**主要功能：** Shared Evidence、External ChatGPT link、History

本项必须使用真实 External ChatGPT 入口发起请求。Local MCP QA driver、直接 Runtime 调用、内部 executor test 或已有本地 E2E 证据不能替代本项。

### 操作

1. 启动已锁定的打包 candidate，并确认使用隔离 profile 和测试 Workspace。
2. 从真实 External ChatGPT 发起以下受控请求：
   - 一个成功的非破坏性 Tool 调用；
   - 一个 Policy DENY 请求；
   - 一个进入 ASK 并已经得到终态的请求。
3. 通过产品支持的连接/诊断/请求证据，确认请求确实由 External ChatGPT 发起，经 Tunnel 到达 Harness Runtime；记录脱敏证据定位，不记录密钥或完整原始内容。
4. 打开 Activity，再切换到 History，查看对应 request detail。
5. 对其中一个代表性请求记录 runtime_id、request_id、event_id 和结果状态。

### 检查

- [ ] 请求来源确实是 External ChatGPT，而不是本地脚本、MCP Inspector 或内部 executor。
- [ ] 可以建立 External ChatGPT → Tunnel → Harness → Runtime → MCP Tool 的真实链路证据。
- [ ] History 中能找到真实外部调用对应的请求。
- [ ] Tool 名称、结果、Policy Decision、Approval 状态与刚才真实观察到的行为一致。
- [ ] success、DENY、ASK/Approval 终态没有混成一种“失败”或“成功”。
- [ ] request identity 使用 runtime_id + request_id，不仅凭时间接近来关联。
- [ ] Activity 与 History 中同一 AuditEvent 的 event_id 一致。
- [ ] COMPLETE / INCOMPLETE / UNKNOWN 有明确且不猜测的含义。
- [ ] 请求详情不显示 API key、SecretStore value、物理 Workspace path、原始环境、完整 stdout/stderr 或完整文件正文。

如果不能证明 External ChatGPT 请求经过 Tunnel 到达 Harness Runtime，本项不得 PASS，必须记 BLOCKED / INCONCLUSIVE，并保留缺失原因。不得用本地 E2E 结果补齐外部证据。

**M03-MAN-01 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE
备注：

---

## 4. M03-MAN-02 — 打包应用重启后的 History 与设置语义

**对应：** M03-AC05～M03-AC10、M03-AC12、M03-AC13、M03-AC15  
**主要功能：** Durable History、HistoryConfig、Retention

本项只做代表性的最终打包 UI 观察；完整的 offline read、retention 算法、故障矩阵和锁语义由本地 E2E/Integration 负责，不在这里重复运行。

### 4.1 跨重启

使用 M03-MAN-01 已产生的代表性外部记录，或使用 QA 已提供的同一 candidate 证据：

1. 点击 Stop Harness。
2. 保持 Runtime 停止，打开 History。
3. 完全退出 ChatGPT-Harness。
4. 重新打开同一打包 candidate，但先不要启动 Harness。
5. 再次查看 History。
6. 最后重新启动 Harness，观察新 Runtime identity。

确认：

- [ ] Runtime 停止时仍能查看兼容的历史记录。
- [ ] 查看 History 不会偷偷启动 Runtime。
- [ ] 完全退出/重新打开应用后历史仍然存在。
- [ ] 新 Runtime 使用新的 runtime_id，不把旧历史伪装成当前 Runtime 记录。
- [ ] 旧记录仍能正常查看，且不改变旧的结果或 coverage。

### 4.2 设置

进入 Settings 的 History 区域，检查最终打包 UI：

- [ ] 默认 Enabled 为开启。
- [ ] 默认 Retention 为 30 天。
- [ ] 默认 Maximum Storage 为 256 MiB。
- [ ] 可以看到当前存储占用或可理解的不可用状态。
- [ ] 修改 Retention Days 或 Maximum Storage MiB 后明确提示 Restart Required。
- [ ] 修改 History 设置不会自动重启 Harness。
- [ ] 关闭 History 后，重启 Runtime 才停止新增持久化记录。
- [ ] 关闭 History 不会自动删除旧历史。
- [ ] 修改普通 UI/Desktop preference 不会因为 M03 无故要求 Runtime 重启。

### 4.3 QA 证据复核

查看 Integration/E2E 的正式证据，不重复注入故障：

- [ ] 超期/超容量删除的是完整 request family，没有 orphan child rows。
- [ ] 当前正在执行的 request 不会被 Retention 清理。
- [ ] 容量清理目标是回到上限约 90%。
- [ ] 证据明确来自正式 runner，而不是 SQLite 文件存在或源码推断。

**M03-MAN-02 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE
备注：

---

## 5. M03-MAN-03 — 打包 History 的筛选/详情与历史 Approval 不重新授权

**对应：** M03-AC10、M03-AC11、M03-AC14、M03-AC15、M03-AC16  
**主要功能：** Query、Keyset Pagination、Authorization Separation

本项通过最终打包 WebView/React UI 做代表性手工交互；不重复本地 E2E 的完整分页矩阵或自动化数据生成。

### 5.1 筛选与详情

在 History 页面检查：

- [ ] 可以按 Time Range 筛选。
- [ ] 可以按 Tool 筛选。
- [ ] 可以按 Outcome 筛选。
- [ ] 可以按 Policy Decision 筛选。
- [ ] 可以按 Approval State 筛选。
- [ ] 默认只展示合理范围，不一次把整个数据库加载到页面。
- [ ] 下一页/加载更多没有明显重复或跳过记录。
- [ ] 修改筛选条件后分页从新的结果重新开始。
- [ ] 请求详情显示安全、有序的事件时间线。
- [ ] 从详情返回列表不需要通过日志查找 ID。

### 5.2 历史 Approval 必须不可操作

找一条过去的 Approval 记录，尤其是 Allow / Deny / Expired / Cancelled 之一；必要时使用 QA 已生成的 representative record。

检查：

- [ ] History 可以让我查看当时发生过 Approval。
- [ ] History 页面没有 Approve 按钮。
- [ ] History 页面没有 Deny 按钮去修改旧请求。
- [ ] History 页面没有恢复 Pending 功能。
- [ ] History 页面没有“按旧授权重新执行”功能。
- [ ] 重启 Runtime 后，旧 Approval 不会重新出现在当前可审批列表。
- [ ] 旧 SessionGrant 不会因为数据库里有历史记录而恢复。
- [ ] 旧 History record 不能使新的外部或本地 Tool 请求跳过当前 Policy/Approval。
- [ ] 当前实时 Approval 仍由 Runtime 状态决定，而不是由 History 页面决定。

如果任何旧 History 记录能够直接恢复授权，本项必须 FAIL。

**M03-MAN-03 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE
备注：

---

## 6. M03-MAN-04 — 存储状态、Clear History 与外部隐私/权威观察

**对应：** M03-AC03、M03-AC07～AC09、M03-AC13～M03-AC16
**主要功能：** Evidence Store Health、Clear History、Privacy、External authority boundary

### 6.1 存储故障提示

本项不要求用户手工损坏数据库。查看正式 QA runner 准备的隔离 fault evidence、截图或现场状态：

- [ ] 能区分并解释 Degraded。
- [ ] 能区分并解释 Busy。
- [ ] 能区分并解释 Corrupt。
- [ ] 能区分并解释 Incompatible。
- [ ] 能区分并解释 Unavailable。
- [ ] 能显示 Queue Drop / evidence incomplete。
- [ ] corrupt/incompatible database 不被 UI 伪装成“没有历史记录”。
- [ ] storage failure 不会把原本 DENY 的 Tool 变成 ALLOW。
- [ ] storage failure 不会把原本 ALLOW/ASK 的请求改成另一种 Policy 结果。
- [ ] storage failure 会提示 History 可能不完整，而不是继续宣称 COMPLETE。

### 6.2 Clear History

自动化 E2E 已覆盖完整 online/offline barrier 语义；人工只在测试数据上确认最终打包交互：

1. 点击 Clear History。
2. 检查确认提示。
3. 在 Harness 运行时执行一次代表性清除。
4. 在 Harness 停止后执行一次代表性离线清理，或查看 QA 已提供的离线清理 UI evidence。

检查：

- [ ] 清除前有明确确认，不会误触一次就删除。
- [ ] Harness 运行时清理不会 Stop/Restart Harness。
- [ ] 清除历史不会取消当前 Tool 请求。
- [ ] 清除历史不会替用户批准/拒绝任何 Approval。
- [ ] 清除历史不会创建/删除 SessionGrant。
- [ ] Runtime 停止后的离线清理仍然可用。
- [ ] 清理后数据库结构仍可继续使用。
- [ ] 新请求可以重新产生新历史。
- [ ] 清理不会删除 Workspace 文件。

### 6.3 隐私与外部权威观察

结合 M03-MAN-01 的真实 External ChatGPT 请求、最终打包 UI 以及 QA 的隔离 sentinel evidence：

- [ ] External ChatGPT 看到的 Tool/结果与 Harness 实际执行的受控行为一致，没有由 History 反向伪造结果。
- [ ] External ChatGPT 请求没有把 API Key / SecretStore value 暴露到 History、Activity、UI 或证据。
- [ ] 不包含完整 Workspace 物理路径。
- [ ] 不包含带账号密码的 Proxy URL。
- [ ] 不包含原始环境变量。
- [ ] 不包含原始 stdout/stderr。
- [ ] 不包含完整文件正文。
- [ ] 不包含不受限制的系统错误文本。
- [ ] Bash Command Preview 不被持久化进 SQLite。
- [ ] Settings 说明数据保存在本机，且产品没有宣称 SQLite 已做静态加密。
- [ ] External ChatGPT 的请求不能通过旧 History、旧 Approval 或旧 SessionGrant 直接恢复授权。
- [ ] 任何外部隐私/权威观察都有真实外部链路证据，不以 local MCP 或 mock UI 替代。

**M03-MAN-04 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE
备注：

---

## 7. M03-MAN-05 — 最终打包交互、已有功能回归与真实外部 post-maintenance Tool

**对应：** M03-AC14、M03-AC17、M03-AC18  
**主要功能：** Regression、Live Authority、Final packaged/manual interaction

本项是人工验收的最后一项。它必须在同一锁定的打包 candidate 上完成；E2E-04 的本地 MCP Tool 结果不能替代这里的真实 External ChatGPT call。

### 7.1 M01 / 桌面功能

- [ ] 关闭主窗口仍然进入 Tray，而不是意外退出。
- [ ] Tray 的 Show / Start / Stop / Quit 正常。
- [ ] Approval Notification 正常。
- [ ] Disconnect Notification 没有因为 History 写库出现明显异常。

### 7.2 M02 / 启动与诊断

- [ ] Startup Phase 详情仍可用。
- [ ] One-click Diagnostics 仍可运行。
- [ ] M03 没有修改 M02 已冻结的性能阈值，或把 M02 的历史结果重新解释为 PASS。

### 7.3 V3 安全能力

- [ ] Workspace 边界仍正常。
- [ ] Tool Enable/Disable 仍正常。
- [ ] Policy ALLOW / ASK / DENY 仍正常。
- [ ] 当前 Approval 仍只由 Runtime 实时状态决定。
- [ ] SessionGrant 仍只属于当前 Runtime session。
- [ ] Bash 仍遵守原有 Policy / Approval。
- [ ] SecretStore 没有被复制进 History。

### 7.4 最终真实外部调用

在已经完成 QA 的 retention/clear/maintenance 后：

1. 保持同一打包 candidate 和隔离测试 Workspace。
2. 从真实 External ChatGPT 发起一个新的、非破坏性的 Tool call。
3. 确认请求经过 Tunnel → Harness → Runtime，并在当前 live Policy 下重新判定。
4. 观察 Activity、History、Approval/Notification 和受控 filesystem 结果。
5. 确认该请求产生新的 runtime/request identity 和新的历史证据。
6. 确认旧 History、旧 Approval、旧 SessionGrant 没有使新请求绕过当前授权。

检查：

- [ ] 已真实执行 External ChatGPT → Tunnel → Harness → Runtime → Tool → History。
- [ ] External ChatGPT 的真实请求来源和 Tunnel crossing 有脱敏证据。
- [ ] post-maintenance 请求没有依赖本地 E2E 或旧历史授权。
- [ ] 新请求按当前 Workspace / Policy / Approval 正常工作并产生新历史。
- [ ] 真实外部调用的 UI/History 观察不泄露隐私 sentinel。
- [ ] 没有重复 Runtime/Tunnel/writer 或未清理的 orphan process。
- [ ] 最终打包窗口、Tray、通知、History 和 Settings 的人工交互完成。

如果 External ChatGPT/Tunnel 不可用，本项必须 BLOCKED / NOT EXECUTED；不得用 local MCP 结果勾选“真实链路”。

**M03-MAN-05 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE
备注：

---

## 8. 最终人工验收汇总

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M03-MAN-01 真实 External ChatGPT → Tunnel → Runtime 与 History |  |  |
| M03-MAN-02 打包重启与存储设置 |  |  |
| M03-MAN-03 筛选/详情/历史授权隔离 |  |  |
| M03-MAN-04 故障提示/Clear/外部隐私与权威观察 |  |  |
| M03-MAN-05 最终打包交互与外部 post-maintenance Tool |  |  |

自动化/本地 E2E 结果（只引用正式证据，不在人工验收中重跑）：

~~~text
M03 Development：
M03 Integration（INT-01～18）：
M03 E2E（本地打包 E2E-01～04）：
Candidate commit：
Package：
Package SHA-256：
SQLite schema/checksum：
history.query P95：
history.get P95：
Privacy sentinel：
Storage failure matrix：
Cleanup：
~~~

真实外部边界与人工结果：

~~~text
External ChatGPT → Tunnel → Harness → Runtime evidence：
External privacy observation：
External authority/non-reactivation observation：
Final packaged/manual interaction：
Manual acceptance：
~~~

### 最终结论

只能在实际执行后选择一个；当前不勾选任何结论：

- [ ] M03 — ACCEPTED / FROZEN（验收通过 / 已冻结）
- [ ] M03 — FAIL（验收失败）
- [ ] M03 — BLOCKED（存在阻塞）
- [ ] M03 — INCONCLUSIVE（证据不足）

只有以下条件全部满足，才能选择 ACCEPTED / FROZEN：

- [ ] M03-AC01～M03-AC18 均有有效证据。
- [ ] M03 Integration M03-INT-01～18 全部 PASS，且 required real boundary 使用正式 QA runner。
- [ ] M03 本地打包 E2E-01～04 全部 PASS，且没有以 mock/internal executor/source-only evidence 冒充真实本地组合。
- [ ] M03-MAN-01～M03-MAN-05 全部 PASS。
- [ ] 已真实观察 External ChatGPT → Tunnel → Harness → Runtime → Tool 链路；不是 local MCP 替代。
- [ ] External ChatGPT 的隐私和 authority boundary 有人工观察与脱敏证据。
- [ ] 最终打包/manual post-maintenance Tool interaction 已完成。
- [ ] canonical identity 已在真实外部 Tool / Approval 链路验证。
- [ ] Runtime-only Writer 和 Tauri Read-only 边界已验证。
- [ ] History 跨 Runtime / App 重启仍可查询。
- [ ] 旧 Pending Approval / SessionGrant / ExecutionPermit 无法从 SQLite 或 History 恢复。
- [ ] Retention 与 Clear History 均通过。
- [ ] migration / busy / full / corrupt / drop 等降级状态均能被观察。
- [ ] history.query / history.get 达到冻结的查询性能要求。
- [ ] 隐私 sentinel 没有进入数据库或用户界面。
- [ ] M01/M02/V3 没有由 M03 引入的未解决回归。
- [ ] 没有未解决的 FAIL / BLOCKED / INCONCLUSIVE。

人工验收完成前，Coding Agent 不得自行把 M03 标记为 ACCEPTED / FROZEN，也不得把“SQLite 文件存在”、本地 E2E PASS 或外部链路缺失当作人工验收完成。

本文档当前保持 NOT EXECUTED；未执行结果不得填写为 PASS。
