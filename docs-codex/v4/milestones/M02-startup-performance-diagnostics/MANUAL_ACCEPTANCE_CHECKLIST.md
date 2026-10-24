# M02 人工验收清单 — 启动性能与一键连接诊断

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M02 — 启动性能与一键连接诊断  
> 对应范围：F05 启动性能优化、F06 一键连接诊断  
> 验收人：用户 / Human Reviewer（人工验收人）  
> 前置条件：M02 Development（开发）、Integration Test（集成测试）和 E2E Test（端到端测试）均已完成并得到真实结果。  
> 本文用于人工观察和最终签认，不要求用户手工重复 30+30 次性能采样；性能统计正确性由自动化/端到端验证负责。

---

## 1. 术语说明

| 技术名称 | 中文含义 |
| --- | --- |
| Startup Performance | 启动性能 |
| Start→Ready | 从点击/执行启动 Harness 到真正就绪 |
| Ready | 就绪状态 |
| Cold Start | 冷启动：桌面应用进程重新启动后再启动 Harness |
| Warm Start | 热启动：桌面应用不退出，停止 Harness 后再次启动 |
| Baseline / B0 | 优化前的基线数据 / 仅增加计时器后的基线版本 |
| Candidate | 待验收候选版本 |
| P50 | 第 50 百分位耗时，近似典型启动时间 |
| P95 | 第 95 百分位耗时，用来观察慢启动尾部 |
| Failure Rate | 启动失败率 |
| Startup Phase | 启动阶段 |
| StartupPhaseRecorder | 启动阶段计时记录器 |
| Diagnostics | 连接诊断 |
| DiagnosticsCoordinator | 诊断协调器 |
| PASS | 该项已确认正常 |
| WARNING | 已确认存在非致命问题或需要注意 |
| FAIL | 已确认存在阻断问题 |
| UNKNOWN | 当前无法安全确认，不能当作正常 |
| Remediation | 修复建议 / 下一步操作建议 |
| Safe Evidence | 安全技术证据 |
| Runtime | 运行时进程 |
| MCP | 模型上下文协议接口 |
| Tunnel | 隧道进程 |
| Control Plane | 控制平面 |
| Proxy | 代理 |
| Credential | 凭据 |
| SecretStore | 凭据存储 |
| Workspace | 工作区 |
| Tray | 系统托盘 |
| Copy Diagnostic Summary | 复制诊断摘要 |

---

## 2. 验收前确认

请确认：

- [ ] M02 Development（开发）已完成。
- [ ] M02 Integration Test（集成测试）M02-INT-01～M02-INT-16 已有真实结果。
- [ ] M02 E2E Test（端到端测试）M02-E2E-01～M02-E2E-04 已有真实结果。
- [ ] 当前使用的 Candidate（候选版本）与 E2E 使用的是同一个生产候选。
- [ ] 已记录候选提交、包路径、SHA-256。
- [ ] B0（优化前基线）和最终候选的 Cold/Warm P50/P95、失败率已经有正式结果。
- [ ] 性能阈值是在优化前冻结的，没有为了通过验收而事后降低。
- [ ] 使用受控 Workspace（工作区）和测试配置，不修改真实重要文件。

验收信息：

~~~text
候选提交：
包类型：
包路径：
SHA-256：
Windows 版本：
B0 提交：
验收人：
日期：
~~~

---

## 3. M02-MAN-01 — 启动耗时信息是否真实、可理解

**对应：** M02-AC01、M02-AC02、M02-AC06  
**主要功能：** Startup Performance（启动性能）、Startup Phase（启动阶段）

### 操作

1. 正常启动 ChatGPT-Harness。
2. 确认 Harness 当前为 Stopped（已停止）。
3. 点击 Start Harness（启动 Harness）。
4. 等待真实 Ready（就绪状态）。
5. 查看 Overview（概览）中的最新启动耗时详情。

检查：

- [ ] 能看到本次启动总耗时。
- [ ] 能看到多个启动阶段，而不是只有一个总数字。
- [ ] 阶段名称能让我大致理解正在做什么，例如 Runtime（运行时进程）、MCP、Tunnel（隧道）、Control Plane（控制平面）等。
- [ ] 能看出哪个阶段最慢。
- [ ] 本次启动成功时显示 Ready（就绪），失败时不会伪装成成功。
- [ ] 页面不会把“单次启动很快”直接写成“性能优化已经通过”。
- [ ] 没有显示 API Key（接口密钥）、Secret（凭据）、原始错误响应或其他不必要的敏感内容。

### 判断

人工验收不要求手算 P50/P95。你只需要确认自动化/E2E 已经给出 B0 与 Candidate 的正式对比，并检查 UI 展示与实际体验没有明显矛盾。

**M02-MAN-01 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 4. M02-MAN-02 — 启动是否真的变快，同时没有“假 Ready”

**对应：** M02-AC03～M02-AC05、M02-AC15  
**主要功能：** Performance Baseline（性能基线）、Authoritative Ready（权威就绪状态）

先查看 QA/Codex 提供的性能结果。

确认：

- [ ] B0 是在优化代码之前采集的。
- [ ] B0 和 Candidate 使用相同机器、配置、网络方式和包类型。
- [ ] Cold P50 至少改善 10%。
- [ ] Warm P50 至少改善 15%。
- [ ] Cold/Warm P95 都没有退化超过 5%。
- [ ] 至少一个 P95 改善 10%。
- [ ] Failure Rate（启动失败率）没有变差。
- [ ] 已满足 SPEC 中“真正改善主要瓶颈”的要求，而不是只优化一个几乎不耗时的小阶段。

然后人工执行 2～3 次正常 Start / Stop：

- [ ] 点击 Start 后不会一瞬间显示 Ready，但实际上工具还不能用。
- [ ] Ready 出现时，Runtime、MCP、Tunnel 的状态与实际连接一致。
- [ ] 如果网络/凭据有问题，应用会显示真实 Error（错误）/Degraded（降级）/诊断结果，而不是为了缩短时间提前显示 Ready。
- [ ] Stop / Restart（停止 / 重启）行为仍然正常。
- [ ] Tray（系统托盘）的 Start / Stop 仍然正常。

如果自动化性能数字通过，但人工操作明显出现“Ready 很快出现、实际还不能使用”，本项必须 FAIL。

**M02-MAN-02 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 5. M02-MAN-03 — 一键诊断是否能直接回答“现在为什么连不上”

**对应：** M02-AC07～M02-AC09、M02-AC13  
**主要功能：** One-click Diagnostics（一键连接诊断）

### 正常状态

让 Harness 达到 Ready（就绪状态），进入 Overview（概览）。

检查：

- [ ] Overview 中有明显的“运行连接诊断”入口，不需要到高级设置里翻找。
- [ ] 点击一次即可完成诊断。
- [ ] 能看到 Overall Result（总体结果）。
- [ ] 能看到 Config（配置）、Workspace（工作区）、Runtime（运行时）、MCP、Tunnel（隧道）、Control Plane（控制平面）、Proxy（代理）、Credential（凭据）、Lifecycle（生命周期）相关结果。
- [ ] 每项使用 PASS / WARNING / FAIL / UNKNOWN 中的一种状态。
- [ ] UNKNOWN（未知）在视觉和文字上不会被当作 PASS（正常）。
- [ ] 每项说明是用户能理解的，不只是内部错误码。
- [ ] 需要处理的问题有 Remediation（修复建议）。
- [ ] Settings（设置）中的诊断结果与 Overview 使用同一套真实结果，不会一边显示 PASS、一边显示 FAIL。

### 用户体验判断

- [ ] 不阅读日志，我也能大致回答“Runtime 有没有启动”“Tunnel 有没有连接”“控制平面有没有连上”“代理有没有被使用”“凭据是否存在/是否有当前拒绝证据”。
- [ ] 对无法确认的内容，产品愿意明确写 UNKNOWN，而不是猜测。

**M02-MAN-03 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 6. M02-MAN-04 — 故障诊断、修复建议和安全证据

**对应：** M02-AC09～M02-AC12  
**主要功能：** Fault Diagnostics（故障诊断）、Safe Evidence（安全技术证据）

本项使用 QA 已准备好的受控故障，不要修改真实重要配置。

### 6.1 本地配置/工作区故障

使用一个验收专用配置制造可恢复的故障，例如测试 Workspace（工作区）不可访问。

运行诊断：

- [ ] 对应项目显示 FAIL（失败）。
- [ ] 因为前置条件不存在而无法检查的后续项目显示 UNKNOWN（未知），而不是 PASS。
- [ ] 修复建议明确告诉我应该去哪个 Settings（设置）区域处理。
- [ ] 仅仅点击“运行诊断”没有自动修改 Workspace。
- [ ] 修复测试配置后，再次诊断能够显示新的真实状态。

### 6.2 连接故障

使用 QA 提供的安全步骤，让当前候选的 Tunnel（隧道）/Control Plane（控制平面）出现真实、可恢复的故障。

运行诊断：

- [ ] 能准确指出 Tunnel / Control Plane / MCP Target（MCP 目标）中的实际故障位置。
- [ ] 如果是 stale（状态过期），不会显示 PASS。
- [ ] 如果没有足够实时证据，会显示 UNKNOWN。
- [ ] 诊断不会自动 Restart Harness（重启 Harness）。
- [ ] 恢复外部条件后，再次诊断显示当前恢复状态，而不是继续使用旧结果。

### 6.3 隐私

点击 Copy Diagnostic Summary（复制诊断摘要），粘贴到临时文本查看：

- [ ] 不包含 API Key / Secret（密钥 / 凭据）。
- [ ] 不包含完整 Workspace 物理路径。
- [ ] 不包含带用户名/密码的 Proxy URL（代理地址）。
- [ ] 不包含原始环境变量。
- [ ] 不包含整段 stdout/stderr（标准输出/错误输出）。
- [ ] 不包含大段原始控制平面错误响应。
- [ ] 仍然包含足够的状态、错误类别和修复建议，可以用于排查。

**M02-MAN-04 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 7. M02-MAN-05 — 诊断无副作用与已有功能回归

**对应：** M02-AC10、M02-AC14、M02-AC15  
**主要功能：** Read-only Diagnostics（只读诊断）、Regression（回归）

### 7.1 诊断无副作用

在 Harness Ready（就绪）时记录当前状态，然后运行连接诊断。

检查：

- [ ] Harness 没有因为诊断被停止。
- [ ] Harness 没有因为诊断被自动重启。
- [ ] Runtime / Tunnel 没有出现额外重复进程。
- [ ] 当前配置没有因为诊断被保存/修改。
- [ ] API Key / Credential（凭据）没有被替换或删除。
- [ ] 没有因为诊断创建新的 Pending Approval（待审批请求）。
- [ ] 没有执行任何文件写入/编辑工具。

### 7.2 M01 功能回归

快速检查：

- [ ] 关闭主窗口仍然进入 Tray（系统托盘）而不是退出。
- [ ] Tray 的 Show / Start / Stop / Quit 正常。
- [ ] 审批通知和连接通知设置仍存在。
- [ ] 主动 Stop/Quit 不产生异常断连通知。
- [ ] 原有 Workspace、Policy（策略）、Approval（审批）、Bash、SecretStore 行为没有明显回退。

### 7.3 真实可用性

查看 E2E 证据：

- [ ] 候选版本在显示 Ready 后，已经通过真实 ChatGPT → Tunnel → Runtime 的 Tool（工具）调用。
- [ ] 不是只用“进程存在”“本地 MCP 可以连”来证明 Ready。

**M02-MAN-05 结论：** PASS / FAIL / BLOCKED  
备注：

---

## 8. 最终人工验收汇总

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M02-MAN-01 启动耗时信息 |  |  |
| M02-MAN-02 启动性能与真实 Ready |  |  |
| M02-MAN-03 一键诊断可理解性 |  |  |
| M02-MAN-04 故障诊断与隐私 |  |  |
| M02-MAN-05 无副作用与回归 |  |  |

自动化结果：

~~~text
M02 Development：
M02 Integration：
M02 E2E：
B0 commit：
Candidate commit：
Cold P50/P95：
Warm P50/P95：
Failure Rate：
Package：
Cleanup：
~~~

### 最终结论

只能选择一个：

- [ ] M02 — ACCEPTED / FROZEN（验收通过 / 已冻结）
- [ ] M02 — FAIL（验收失败）
- [ ] M02 — BLOCKED（存在阻塞）
- [ ] M02 — INCONCLUSIVE（证据不足）

只有以下条件全部满足，才能选择 ACCEPTED / FROZEN：

- [ ] M02-AC01～M02-AC15 均有有效证据。
- [ ] M02 Integration Test 全部 PASS。
- [ ] M02 E2E Test 全部 PASS。
- [ ] M02-MAN-01～M02-MAN-05 全部 PASS。
- [ ] B0 在优化前冻结，性能阈值没有事后下调。
- [ ] Final Candidate 满足 SPEC §8 所有性能目标。
- [ ] Ready 已通过真实 ChatGPT Tool 调用证明。
- [ ] One-click Diagnostics（一键诊断）能正确处理正常和故障状态。
- [ ] Diagnostics（诊断）保持只读，没有自动修复副作用。
- [ ] Safe Diagnostic Summary（安全诊断摘要）无敏感信息泄漏。
- [ ] M01/V3 相关功能无当前回归 blocker。
- [ ] 没有未解决的 FAIL / BLOCKED / INCONCLUSIVE。

人工验收完成前，Coding Agent（编码代理）不得自行把 M02 标记为 ACCEPTED / FROZEN，也不得自行推进需要 M02 验收作为前置条件的后续里程碑。
