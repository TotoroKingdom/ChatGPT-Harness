# M01 人工验收清单 — 系统托盘、后台生命周期与通知

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M01 — 系统托盘、后台生命周期与通知  
> 对应范围：F01 系统托盘、F03 审批通知、F04 断连/恢复通知  
> 验收人：用户 / Human Reviewer（人工验收人）  
> 前置条件：M00 已 ACCEPTED / FROZEN（验收通过并冻结）；M01 开发完成；Integration Test（集成测试）全部 PASS；E2E Test（端到端测试）全部 PASS。  
> 本文只用于人工观察和最终签认，不替代自动化测试，也不要求用户阅读源码。

---

## 1. 术语说明

为避免英文技术名称造成歧义，本清单统一使用以下写法：

| 技术名称 | 中文含义 |
| --- | --- |
| Tray | 系统托盘 |
| System Tray | 系统托盘 |
| Notification | Windows 系统通知 |
| Toast Notification | Windows 弹出通知 |
| Action Center | Windows 通知中心 |
| Pending Approval | 待审批请求 |
| Approval | 审批请求 |
| Approvals Page | 审批页面 |
| HarnessController | Harness 生命周期控制器 |
| Runtime | 运行时进程 |
| Tunnel | 隧道进程 |
| Ready | 就绪状态 |
| Degraded | 降级状态 |
| Error | 错误状态 |
| Stop Harness | 停止 Harness |
| Explicit Quit | 显式退出应用 |
| Deep Link / Navigation | 深度链接 / 定向导航 |
| AppConfig v3 | 应用配置版本 3 |
| Host-only Preference | 仅桌面宿主端生效的偏好设置 |
| configRevision | 运行时配置修订号 |
| Restart Required | 需要重启 Runtime 才能生效 |
| NSIS Installed Package | NSIS Windows 安装版 |
| Portable | 便携版 |
| ASK | 需要人工审批的策略结果 |
| SecretStore | 凭据存储 |
| Workspace | 工作区 |

后文出现以上英文名称时，均指本表中的中文含义。

---

## 2. 人工验收前准备

请先确认以下项目全部满足；任一项不满足时，不要开始最终人工签认：

- [ ] M00 已正式验收通过并标记为 ACCEPTED / FROZEN（验收通过 / 已冻结）。
- [ ] M01 开发任务 T1～T6 已完成。
- [ ] M01 Integration Test（集成测试）M01-INT-01～M01-INT-16 全部 PASS（通过）。
- [ ] M01 E2E Test（端到端测试）M01-E2E-01～M01-E2E-04 全部 PASS（通过）。
- [ ] 当前人工验收使用的 Candidate（候选版本）与自动化测试使用的是同一个生产候选，或者差异仅为不影响行为的验收记录。
- [ ] 已记录 Candidate Commit（候选提交）、应用版本、包类型和 SHA-256。
- [ ] Windows 通知功能当前可开启；后面会专门测试“通知被关闭时的降级行为”。
- [ ] 使用受控 Workspace（工作区）和测试策略，不在人工验收中使用高风险破坏性操作。

建议以 NSIS Installed Package（NSIS Windows 安装版）作为通知功能的主要验收对象。Portable（便携版）仍需要验证系统托盘和后台行为；如果 Portable 的 Windows 通知能力受平台限制，应如实记录，不能用“系统托盘可用”替代“通知可用”。

验收信息：

~~~text
候选提交：
应用版本：
包类型：
包路径：
SHA-256：
Windows 版本：
验收人：
日期：
~~~

---

## 3. M01-MAN-01 — 关闭窗口后后台驻留

**对应：** M01-AC01、M01-AC02、M01-AC15、M01-AC16  
**主要功能：** Tray（系统托盘）、Background Lifecycle（后台生命周期）

### 3.1 首次关闭提示

在一个尚未确认“后台驻留提示”的验收配置中：

1. 启动 ChatGPT-Harness。
2. 正常启动 Harness，并确认状态达到 Ready（就绪状态）。
3. 点击主窗口右上角关闭按钮 X。

检查：

- [ ] 第一次关闭时，没有直接退出应用。
- [ ] 出现一次清楚的后台驻留说明。
- [ ] 说明明确告诉我：关闭主窗口只是隐藏到 Tray（系统托盘），不是 Stop Harness（停止 Harness）。
- [ ] 说明明确告诉我：真正退出要使用 Quit（退出）入口。
- [ ] 选择“继续在后台运行”后，主窗口消失。
- [ ] 系统托盘中仍能看到 ChatGPT-Harness 图标。
- [ ] Harness 没有因为关闭窗口而停止。

### 3.2 第二次关闭

从 Tray（系统托盘）重新打开主窗口，然后再次点击 X：

- [ ] 第二次关闭直接隐藏到系统托盘。
- [ ] 首次说明没有反复弹出。
- [ ] 再次从系统托盘打开时，窗口能正常显示并获得焦点。
- [ ] 如果 Harness 原本处于 Ready（就绪状态），重新打开后显示的仍是当前真实状态，而不是因为窗口重新打开才“变成 Ready”。

### 3.3 用户体验判断

- [ ] 我能明确理解“关闭窗口”“停止 Harness”“退出应用”是三个不同动作。
- [ ] 主窗口隐藏后，应用仍表现为一个正常的后台桌面程序，不会让我误以为已经退出。

**M01-MAN-01 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 4. M01-MAN-02 — 系统托盘操作与显式退出

**对应：** M01-AC03、M01-AC04、M01-AC10、M01-AC15  
**主要功能：** Tray（系统托盘）、Explicit Quit（显式退出）

### 4.1 系统托盘菜单

右键或按当前实现方式打开 Tray（系统托盘）菜单。

检查至少存在并能理解：

- [ ] Show（显示主窗口）。
- [ ] 当前 Harness 状态。
- [ ] Pending Approval（待审批请求）数量或“不可用”状态。
- [ ] Start Harness（启动 Harness）。
- [ ] Stop Harness（停止 Harness）。
- [ ] Quit（退出应用）。

### 4.2 Stop 与 Start

在 Harness 为 Ready（就绪状态）时：

1. 从系统托盘选择 Stop Harness（停止 Harness）。
2. 等待状态收敛到 Stopped（已停止）。
3. 再从系统托盘选择 Start Harness（启动 Harness）。

检查：

- [ ] Stop 只停止 Harness，不退出桌面应用。
- [ ] 系统托盘仍然存在。
- [ ] Start 使用正常的配置和凭据检查，没有绕过原有启动条件。
- [ ] 启动过程中能看到合理的 Starting（启动中）/Ready（就绪）状态变化。
- [ ] 没有因为连续点击而出现两个 Runtime（运行时进程）或两个 Tunnel（隧道进程）。

### 4.3 Pending Approval 数量

如果当前有待审批请求：

- [ ] 系统托盘显示的 Pending Approval（待审批请求）数量与审批页面一致。
- [ ] 请求被批准、拒绝、过期或取消后，数量会更新。
- [ ] Runtime（运行时进程）不可用时，不会继续把旧数字伪装成当前真实数量；应显示不可用或等价状态。

### 4.4 Quit

让 Harness 再次达到 Ready（就绪状态），然后从系统托盘选择 Quit（退出应用）。

检查：

- [ ] 应用真正退出，不只是隐藏窗口。
- [ ] 系统托盘图标消失。
- [ ] 候选版本拥有的 Runtime（运行时进程）已经退出。
- [ ] 候选版本拥有的 Tunnel（隧道进程）已经退出。
- [ ] Windows 任务管理器中没有留下该候选版本的孤儿进程。
- [ ] 因为这是用户主动 Quit（退出），没有额外弹出“断连”或“恢复”通知。

**M01-MAN-02 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 5. M01-MAN-03 — 桌面通知设置与配置持久化

**对应：** M01-AC05、M01-AC14  
**主要功能：** AppConfig v3（应用配置版本 3）、Host-only Preference（仅桌面宿主端偏好设置）

进入 Settings（设置）中的桌面/通知区域。

检查：

- [ ] 可以看到“审批通知”开关。
- [ ] 可以看到“连接通知”开关。
- [ ] 新配置/迁移后的默认值符合设计：审批通知开启、连接通知开启。
- [ ] M01 中没有“应用启动后自动启动 Harness”的设置项；该功能属于后续 M07。
- [ ] 修改“审批通知”开关后，不出现 Restart Required（需要重启 Runtime 才能生效）提示。
- [ ] 修改“连接通知”开关后，不出现 Restart Required（需要重启 Runtime 才能生效）提示。
- [ ] 修改这两个开关时，正在运行的 Harness 不会因此被停止或重启。
- [ ] 退出应用后重新打开，两个通知开关保持我上次保存的值。
- [ ] 原有 Workspace（工作区）、Tunnel（隧道）、Tools（工具）、Policies（策略）、主题和语言设置没有因为升级到 AppConfig v3（应用配置版本 3）而丢失或重置。
- [ ] 应用中没有出现要求我重新输入本来已经保存到 SecretStore（凭据存储）的密钥，除非密钥本身确实失效/缺失。

说明：configRevision（运行时配置修订号）是否保持不变、DesktopConfig 是否进入 RuntimeConfigSnapshot 等内部契约，属于自动化集成测试，不要求用户手工检查文件。

**M01-MAN-03 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 6. M01-MAN-04 — 审批通知、点击跳转与通知关闭后的降级路径

**对应：** M01-AC06～M01-AC10、M01-AC15、M01-AC16  
**主要功能：** Approval Notification（审批通知）、Pending Approval（待审批请求）、Deep Link（定向跳转）

### 6.1 准备一个安全的 ASK 请求

使用验收专用配置，把一个非破坏性工具设置为 ASK（需要人工审批），例如 read_file（读取文件）或其他不会修改文件的工具。

确保：

- [ ] Harness 为 Ready（就绪状态）。
- [ ] “审批通知”开关已开启。
- [ ] Windows 当前允许 ChatGPT-Harness 发送 Notification（系统通知）。
- [ ] 开始前没有其他无关 Pending Approval（待审批请求）。

### 6.2 从真实 ChatGPT 发起请求

从真实 ChatGPT 发起一次会调用上述工具的请求。

检查：

- [ ] Runtime（运行时进程）中出现一个真实 Pending Approval（待审批请求）。
- [ ] Windows 出现一条 Approval Notification（审批通知）。
- [ ] 同一个请求只出现一次通知，没有因为轮询或重复状态更新连续弹相同通知。
- [ ] 通知能看出是 ChatGPT-Harness 的审批请求。
- [ ] 通知可以显示 Tool（工具）名称。
- [ ] 通知没有显示 API Key（接口密钥）、Secret（敏感凭据）、完整 Bash Command（Bash 命令）、raw argv（原始参数列表）、物理文件路径、文件正文等敏感内容。
- [ ] Tray（系统托盘）的待审批数量同步变为 1。

### 6.3 点击通知

在实时 Toast Notification（Windows 弹出通知）仍可点击时点击它。

检查：

- [ ] 主窗口被打开或聚焦。
- [ ] 自动进入对应的 Approvals Page（审批页面）。
- [ ] 打开的确实是刚才那一个请求。
- [ ] 页面显示的是 Runtime（运行时进程）重新查询后的当前状态。
- [ ] 点击通知本身没有自动批准或拒绝请求。
- [ ] 最终批准/拒绝仍然只能通过正常审批按钮完成。

完成审批后：

- [ ] 待审批数量回到 0。

说明：Windows Action Center（Windows 通知中心）中的历史通知在某些 Windows / Tauri 插件组合下可能无法稳定回调。M01 不要求“已经进入通知中心很久的旧通知”必须可靠跳转；这里必须验收的是当前平台明确支持的实时通知点击。已解决/已过期后的安全行为由 Integration Test（集成测试）覆盖，不能因为通知中心平台限制而伪造 Pending（待审批）状态。

### 6.4 关闭 Windows 通知后的降级路径

暂时在 Windows 设置中关闭 ChatGPT-Harness 的系统通知权限，然后再从真实 ChatGPT 触发一个安全 ASK（需要人工审批）请求。

检查：

- [ ] 即使没有 Windows 弹出通知，请求仍然真实存在于 Approvals Page（审批页面）。
- [ ] Tray（系统托盘）的待审批数量仍然正确。
- [ ] 我仍然可以从系统托盘/主窗口进入审批页面并正常处理请求。
- [ ] “通知发不出来”没有导致请求自动批准、自动拒绝、消失或被重新创建。

完成后请恢复 Windows 通知权限。

**M01-MAN-04 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 7. M01-MAN-05 — 断连、恢复与通知去重

**对应：** M01-AC11～M01-AC13、M01-AC15、M01-AC16  
**主要功能：** Disconnect Notification（断连通知）、Recovery Notification（恢复通知）、Incident Episode（故障事件周期）

### 7.1 准备

- [ ] “连接通知”开关已开启。
- [ ] Harness 已达到 Ready（就绪状态）。
- [ ] 当前使用的是验收专用/可控实例。
- [ ] 已确认要操作的 tunnel-client（隧道客户端）进程属于本次候选版本，不会误杀其他代理或系统进程。

### 7.2 触发一次真实连接故障

按照 QA 提供的验收步骤触发一次受控连接故障。允许的方式包括：仅结束当前候选拥有的 tunnel-client（隧道客户端）进程，让现有恢复机制处理；不要随意结束其他进程。

检查：

- [ ] Harness 从 Ready（就绪状态）进入真实 Degraded（降级）或 Error（错误）等非就绪状态。
- [ ] Windows 出现一次 Disconnect Notification（断连通知）。
- [ ] 故障持续期间，不会每次轮询都重复弹通知。
- [ ] 即使错误文本发生变化，也不会把同一个故障当成很多次独立故障。
- [ ] 点击实时断连通知后，主窗口打开/聚焦并进入 Overview（概览）。
- [ ] Overview（概览）显示的是此刻真实状态，而不是通知产生时缓存的旧状态。

### 7.3 恢复

让产品自身的正常恢复路径重新达到 Ready（就绪状态）。

检查：

- [ ] 最多出现一次 Recovery Notification（恢复通知）。
- [ ] 连续保持 Ready 时不会重复弹“已恢复”。
- [ ] 我能理解是“系统先真实恢复到 Ready，再发送恢复通知”，而不是“通知发送成功所以算恢复”。

### 7.4 第二次独立故障

在已经稳定恢复到 Ready 后，再触发一次相同类型的受控故障。

检查：

- [ ] 这一次可以再次出现新的断连通知，因为它已经是新的 Incident Episode（故障事件周期）。
- [ ] 新故障仍然只通知一次，不刷屏。

### 7.5 主动 Stop / Quit 不应误报

恢复 Ready 后：

1. 使用 Tray（系统托盘）正常执行 Stop Harness（停止 Harness）。
2. 再正常 Start Harness（启动 Harness）并恢复 Ready。
3. 最后使用 Quit（退出应用）。

检查：

- [ ] 主动 Stop 不弹“异常断连”通知。
- [ ] 正常 Start/Stop 的状态变化不会制造“恢复通知”。
- [ ] 主动 Quit 不弹“异常断连”通知。

**M01-MAN-05 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 8. 最终人工验收汇总

请填写：

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M01-MAN-01 关闭窗口后后台驻留 |  |  |
| M01-MAN-02 系统托盘操作与显式退出 |  |  |
| M01-MAN-03 桌面通知设置与配置持久化 |  |  |
| M01-MAN-04 审批通知与降级路径 |  |  |
| M01-MAN-05 断连/恢复通知与去重 |  |  |

自动化前置结果：

~~~text
M00：
M01 Development：
M01 Integration：
M01 E2E：
候选提交：
候选包：
清理结果：
~~~

### 最终结论

只能选择一个：

- [ ] M01 — ACCEPTED / FROZEN（验收通过 / 已冻结）
- [ ] M01 — FAIL（验收失败）
- [ ] M01 — BLOCKED（存在阻塞，无法完成验收）
- [ ] M01 — INCONCLUSIVE（证据不足，无法下结论）

只有在以下条件全部满足时，才能勾选 ACCEPTED / FROZEN（验收通过 / 已冻结）：

- [ ] M00 已 ACCEPTED / FROZEN（验收通过 / 已冻结）。
- [ ] M01-AC01～M01-AC16 均有有效证据。
- [ ] M01 Integration Test（集成测试）全部 PASS。
- [ ] M01 E2E Test（端到端测试）全部 PASS。
- [ ] M01-MAN-01～M01-MAN-05 全部 PASS。
- [ ] NSIS Windows 安装版的 Tray（系统托盘）和 Notification（系统通知）满足验收要求。
- [ ] Portable（便携版）的能力差异已经如实记录，没有用降级能力冒充完整通知能力。
- [ ] Explicit Quit（显式退出）后没有候选版本拥有的孤儿 Runtime（运行时进程）或 Tunnel（隧道进程）。
- [ ] 没有新增第二套 Ready（就绪状态）、Policy（策略）、Approval（审批）权威。
- [ ] 没有未解决的 FAIL / BLOCKED / INCONCLUSIVE 项。

人工验收完成前，不得由 Coding Agent（编码代理）自行把 M01 标记为 ACCEPTED / FROZEN（验收通过 / 已冻结），也不得自行启动下一个里程碑。
