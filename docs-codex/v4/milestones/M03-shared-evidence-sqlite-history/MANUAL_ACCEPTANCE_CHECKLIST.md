# M03 人工验收清单 — 共享证据与 SQLite 历史记录

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M03 — Shared Evidence and Durable SQLite History（共享证据与持久化 SQLite 历史记录）  
> 对应范围：Shared Evidence Contract（共享证据契约）+ F07 SQLite History（SQLite 历史记录）  
> 验收人：用户 / Human Reviewer（人工验收人）  
> 前置条件：M03 Development（开发）、Integration Test（集成测试）和 E2E Test（端到端测试）已经完成并有真实结果。  
> 本清单只用于最终人工观察和签认；数据库损坏、满盘、队列溢出、事务回滚等故障注入由自动化/QA 负责，不要求用户手工破坏数据库。

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
| E2E | 端到端测试 |
| Integration Test | 集成测试 |

后文出现英文名称时，均按本表中的中文含义理解。

---

## 2. 验收前确认

请先确认：

- [ ] M03 Development（开发）已经完成。
- [ ] M03-INT-01～M03-INT-18 集成测试已有真实结果。
- [ ] M03-E2E-01～M03-E2E-04 端到端测试已有真实结果。
- [ ] 当前打开的 Candidate（候选版本）与 E2E 使用的是同一个版本。
- [ ] 已记录候选提交、安装/便携包路径和 SHA-256。
- [ ] 数据库 Schema（结构版本）为 M03 冻结的 v1，并已记录 checksum（校验值）。
- [ ] QA 已使用隔离的测试数据库完成 corrupt/full/busy/drop 等故障测试，没有破坏真实用户历史数据。
- [ ] 当前验收使用的是可安全操作的测试 Workspace（工作区）。

验收信息：

~~~text
候选提交：
包类型：
包路径：
SHA-256：
App 版本：
Runtime 版本：
Tunnel 版本：
SQLite Schema：
Schema checksum：
验收人：
日期：
~~~

---

## 3. M03-MAN-01 — 历史记录是否和真实工具调用对应，并且看得懂

**对应：** M03-AC01、M03-AC02、M03-AC03、M03-AC10、M03-AC16、M03-AC17  
**主要功能：** Shared Evidence（共享证据）、History（历史记录）

### 操作

1. 启动 Harness。
2. 用真实 ChatGPT 或已经完成 E2E 的同一候选执行几类明确请求：
   - 一个成功的 Tool（工具）调用；
   - 一个 Policy DENY（策略拒绝）；
   - 一个需要 Approval（审批）并已经得到终态的请求。
3. 打开 Activity（活动记录）页面。
4. 切换到 History（历史）标签。
5. 打开对应请求的详情。

检查：

- [ ] History 中能找到刚才的请求。
- [ ] Tool 名称、时间、Outcome（结果）、Policy Decision（策略判定）、Approval 状态与我刚才实际看到的行为一致。
- [ ] 成功、DENY（拒绝）、ASK/Approval（需要审批）没有混成一种“失败”或“成功”。
- [ ] 请求详情能看到一条有顺序的安全事件时间线。
- [ ] 能看到 runtime_id（运行时标识）和 request_id（请求标识），不会仅凭“发生时间差不多”来关联记录。
- [ ] COMPLETE / INCOMPLETE / UNKNOWN（完整 / 不完整 / 无法确认）有明确显示。
- [ ] 如果请求证据不完整，界面不会猜一个成功/失败结果补上去。
- [ ] History 页面没有显示不必要的原始文件内容、物理路径、密钥或原始日志。

### 重点判断

History 的价值是告诉我“当时实际发生了什么”，而不是重新解释 Policy 或执行结果。

**M03-MAN-01 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 4. M03-MAN-02 — 关闭/重启以后历史是否还在，存储设置是否真实生效

**对应：** M03-AC05～M03-AC10、M03-AC12、M03-AC13、M03-AC15  
**主要功能：** Durable History（持久化历史）、HistoryConfig（历史配置）、Retention（保留策略）

### 4.1 跨重启

在已有历史记录后：

1. 点击 Stop Harness（停止 Harness）。
2. 保持 Runtime 停止，查看 History。
3. 完全退出 ChatGPT-Harness。
4. 重新打开应用，但先不要启动 Harness。
5. 再次查看 History。
6. 最后重新启动 Harness。

确认：

- [ ] Runtime 停止时仍然能查看兼容的历史记录。
- [ ] 查看历史不会偷偷启动 Runtime。
- [ ] 完全退出/重新打开应用后历史仍然存在。
- [ ] 新 Runtime 启动后产生新的 runtime_id，不会把旧历史伪装成当前 Runtime 的记录。
- [ ] 旧记录仍能正常查看。

### 4.2 设置

进入 Settings（设置）的 History（历史记录）区域。

确认：

- [ ] 默认 Enabled（启用）为开启。
- [ ] 默认 Retention（保留期限）为 30 天。
- [ ] 默认 Maximum Storage（最大存储）为 256 MiB。
- [ ] 可以看到当前存储占用或可理解的不可用状态。
- [ ] 修改 Retention Days（保留天数）或 Maximum Storage MiB（最大存储）后，会明确提示 Restart Required（需要重启 Runtime 才能生效）。
- [ ] 修改 History 设置不会自动重启 Harness。
- [ ] 关闭 History 后，重启 Runtime 才停止新增持久化记录。
- [ ] 关闭 History 不会自动删除旧历史。
- [ ] 修改普通 UI/桌面偏好不会因为 M03 无故要求 Runtime 重启。

### 4.3 保留策略说明

查看 QA 提供的自动化结果：

- [ ] 已证明超期/超容量时删除的是整个 request family（请求及其关联证据），不会只删中间一部分造成孤儿数据。
- [ ] 已证明当前正在执行的请求不会被 Retention 清理。
- [ ] 已证明容量清理目标是回到上限的约 90%，避免频繁反复清理。

**M03-MAN-02 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 5. M03-MAN-03 — 筛选、分页、详情是否好用，并且历史审批绝不能重新授权

**对应：** M03-AC10、M03-AC11、M03-AC14、M03-AC15、M03-AC16  
**主要功能：** Query（查询）、Keyset Pagination（稳定游标分页）、Authorization Separation（授权隔离）

### 5.1 筛选与分页

在 History 页面检查：

- [ ] 可以按 Time Range（时间范围）筛选。
- [ ] 可以按 Tool（工具）筛选。
- [ ] 可以按 Outcome（结果）筛选。
- [ ] 可以按 Policy Decision（策略判定）筛选。
- [ ] 可以按 Approval State（审批状态）筛选。
- [ ] 默认只展示合理范围的数据，不会一次把整个数据库全部加载到页面。
- [ ] 下一页/加载更多后没有明显重复或跳过记录。
- [ ] 修改筛选条件后分页会从新的结果重新开始。
- [ ] 请求详情能返回到列表，不需要通过日志查 ID。

### 5.2 历史审批必须不可操作

找一条过去的 Approval（审批）记录，尤其是已经 Allow / Deny / Expired / Cancelled（允许 / 拒绝 / 过期 / 取消）的记录。

检查：

- [ ] History 可以让我查看当时发生过审批。
- [ ] History 页面没有 Approve（批准）按钮。
- [ ] History 页面没有 Deny（拒绝）按钮去修改旧请求。
- [ ] History 页面没有“恢复 Pending（待审批）”功能。
- [ ] History 页面没有“按旧授权重新执行”之类的按钮。
- [ ] 重启 Runtime 后，旧 Approval 不会重新出现在当前可审批列表里。
- [ ] 旧 SessionGrant（会话授权）不会因为数据库里有历史记录而恢复。
- [ ] 新 Tool 请求仍然根据当前 Workspace / Policy / Approval 重新判定。

如果任何旧 History 记录能够直接恢复授权，本项必须 FAIL。

**M03-MAN-03 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 6. M03-MAN-04 — 存储故障提示、清除历史与隐私是否符合预期

**对应：** M03-AC03、M03-AC07～M03-AC09、M03-AC13～M03-AC16  
**主要功能：** Evidence Store Health（证据存储健康）、Clear History（清除历史）、Privacy（隐私）

### 6.1 存储故障提示

本项不要求你手工损坏数据库。请查看 QA 已准备好的隔离故障场景和截图/现场状态。

确认产品能区分并解释：

- [ ] Degraded（降级）。
- [ ] Busy（数据库忙）。
- [ ] Corrupt（数据库损坏）。
- [ ] Incompatible（数据库版本不兼容）。
- [ ] Unavailable（不可用）。
- [ ] Queue Drop（队列丢失）/证据不完整。

同时确认：

- [ ] 数据库损坏时不会直接显示“没有历史记录”来掩盖问题。
- [ ] 数据库故障不会把原本 DENY 的 Tool 变成 ALLOW。
- [ ] 数据库故障不会把原本 ALLOW/ASK 的请求改成另一种 Policy 结果。
- [ ] 存储失败时会提示 History 可能不完整，而不是继续宣称 COMPLETE。

### 6.2 Clear History（清除历史）

在确认使用测试数据后：

1. 点击 Clear History（清除历史）。
2. 检查确认提示。
3. Harness 运行时执行一次。
4. Harness 停止后再执行一次离线清理。

检查：

- [ ] 清除前有明确确认，不会误触一次就删除。
- [ ] Harness 运行时清理不会 Stop/Restart Harness。
- [ ] 清除历史不会取消当前 Tool 请求。
- [ ] 清除历史不会替你批准/拒绝任何 Approval。
- [ ] 清除历史不会创建/删除 SessionGrant。
- [ ] Runtime 停止后的离线清理仍然可用。
- [ ] 清理后数据库结构仍可继续使用。
- [ ] 新请求可以重新产生新历史。
- [ ] 清理不会删除 Workspace 文件。

### 6.3 隐私检查

查看 History 详情以及 QA 的数据库 sentinel（哨兵）检查结果：

- [ ] 不包含 API Key / SecretStore（接口密钥 / 凭据存储值）。
- [ ] 不包含完整 Workspace 物理路径。
- [ ] 不包含带账号密码的 Proxy URL（代理地址）。
- [ ] 不包含原始环境变量。
- [ ] 不包含原始 stdout/stderr（标准输出 / 错误输出）。
- [ ] 不包含完整文件正文。
- [ ] 不包含不受限制的系统错误文本。
- [ ] Bash 的 Command Preview（命令预览）不会被持久化进 SQLite。
- [ ] Settings 明确说明数据保存在本机，并且产品没有宣称 SQLite 已做静态加密。

**M03-MAN-04 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 7. M03-MAN-05 — 已有功能回归与最终安全边界

**对应：** M03-AC14、M03-AC17、M03-AC18  
**主要功能：** Regression（回归）、Live Authority（实时权威状态）

快速检查：

### 7.1 M01 / 桌面功能

- [ ] 关闭主窗口仍然进入 Tray（系统托盘）而不是意外退出。
- [ ] Tray 的 Show / Start / Stop / Quit 正常。
- [ ] Approval Notification（审批通知）仍然正常。
- [ ] Disconnect Notification（断连通知）没有因为 History 写库出现明显异常。

### 7.2 M02 / 启动与诊断

- [ ] Startup Phase（启动阶段）详情仍可用。
- [ ] One-click Diagnostics（一键连接诊断）仍可运行。
- [ ] M03 没有修改 M02 已冻结的性能阈值或把 M02 的历史结果重新解释为 PASS。

### 7.3 V3 安全能力

- [ ] Workspace（工作区）边界仍正常。
- [ ] Tool Enable/Disable（工具启停）仍正常。
- [ ] Policy ALLOW / ASK / DENY（允许 / 审批 / 拒绝）仍正常。
- [ ] 当前 Approval（审批）仍只由 Runtime 实时状态决定。
- [ ] SessionGrant（会话授权）仍只属于当前 Runtime 会话。
- [ ] Bash（受控命令执行）仍遵守原有 Policy / Approval。
- [ ] SecretStore（凭据存储）没有被复制进 History。

### 7.4 真实链路确认

查看 M03 E2E 证据：

- [ ] 已经真实执行 ChatGPT → Tunnel → Runtime → Tool → History。
- [ ] 成功、DENY、Approval 终态均能回到对应 History。
- [ ] 重启后旧历史可查，但不能重新授权。
- [ ] Clear / Retention（清理 / 保留策略）后，新真实 Tool 请求仍能按当前 Policy 正常工作并产生新历史。

**M03-MAN-05 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 8. 最终人工验收汇总

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M03-MAN-01 历史与真实调用一致 |  |  |
| M03-MAN-02 跨重启与存储设置 |  |  |
| M03-MAN-03 筛选/分页/授权隔离 |  |  |
| M03-MAN-04 故障/清除/隐私 |  |  |
| M03-MAN-05 回归与最终安全边界 |  |  |

自动化/端到端结果：

~~~text
M03 Development：
M03 Integration：
M03 E2E：
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

### 最终结论

只能选择一个：

- [ ] M03 — ACCEPTED / FROZEN（验收通过 / 已冻结）
- [ ] M03 — FAIL（验收失败）
- [ ] M03 — BLOCKED（存在阻塞）
- [ ] M03 — INCONCLUSIVE（证据不足）

只有以下条件全部满足，才能选择 ACCEPTED / FROZEN：

- [ ] M03-AC01～M03-AC18 均有有效证据。
- [ ] M03 Integration Test（集成测试）M03-INT-01～18 全部 PASS。
- [ ] M03 E2E Test（端到端测试）M03-E2E-01～04 全部 PASS。
- [ ] M03-MAN-01～M03-MAN-05 全部 PASS。
- [ ] canonical identity（权威关联身份）已在真实 Tool / Approval 链路验证。
- [ ] Runtime-only Writer（仅 Runtime 写入）和 Tauri Read-only（Tauri 只读）边界已验证。
- [ ] History 跨 Runtime / App 重启仍可查询。
- [ ] 旧 Pending Approval / SessionGrant / ExecutionPermit 无法从 SQLite 恢复。
- [ ] Retention（保留策略）与 Clear History（清除历史）均通过。
- [ ] migration / busy / full / corrupt / drop（迁移 / 忙 / 满 / 损坏 / 丢失）等降级状态均能被观察。
- [ ] history.query / history.get 达到冻结的查询性能要求。
- [ ] 隐私 sentinel（哨兵）没有进入数据库或用户界面。
- [ ] M01/M02/V3 没有由 M03 引入的未解决回归。
- [ ] 没有未解决的 FAIL / BLOCKED / INCONCLUSIVE。

人工验收完成前，Coding Agent（编码代理）不得自行把 M03 标记为 ACCEPTED / FROZEN，也不得把“SQLite 文件存在”当作 M03 已验收。
