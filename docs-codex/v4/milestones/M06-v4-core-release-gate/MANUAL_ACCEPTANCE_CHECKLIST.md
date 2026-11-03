# M06 人工验收清单 — V4 Core Release Gate

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M06 — V4 Core Release Gate  
> 目标：对同一个最终 Core Release Candidate 完成 Windows 产品体验、关键证据复核和真实 External ChatGPT 最终用户链路验收  
> 前置条件：M01–M05 均 ACCEPTED / FROZEN；M06 Development、Integration、E2E 已有真实结果  
> 人工边界：不要求你手工重跑自动化压力、TOCTOU、SQLite fault injection、完整 72-run Eval 或所有单元/集成测试。人工重点是“最终候选是否真的可用、可理解、可发布，以及真实 ChatGPT 链路是否成立”。  
> 记录规则：本文件是冻结的验收合同，不为填写结果而修改。实际 PASS/FAIL 在验收对话和最终候选证据中记录。  
> 发布语义：M06 成功只能标 V4 Core — ACCEPTED；不能标 V4 — ACCEPTED / FROZEN。

---

# 1. 术语

| 名称 | 含义 |
| --- | --- |
| Core Release Candidate / Core RC | 本次 V4 Core 的固定候选版本 |
| Same-candidate | 所有正式证据指向同一个产品候选身份 |
| Core Product Acceptance Matrix | 28 条 Core AC 与跨版本/安全/发布证据的汇总矩阵 |
| Portable | 便携包 |
| NSIS Installer | Windows 安装包 |
| Upgrade | 从旧配置/数据升级到当前候选 |
| Rollback | 回退到上一个已接受版本 |
| Authoritative Ready | 通过真实 readiness 条件后的就绪，不是“进程存在” |
| Tray | Windows 系统托盘 |
| Approval Notification | 待审批通知 |
| Disconnect Notification | 断连/降级通知 |
| Diagnostics | 一键连接诊断 |
| Shared Evidence | 统一结构化 Tool 执行证据 |
| History | SQLite 持久化历史 |
| runtime_id + request_id | 单个真实 Tool request 的稳定关联键 |
| F05 | 启动性能门禁 |
| F12 | Agent Tool Evaluation 质量门禁 |
| Equivalence | 最终 RC 与已接受证据的身份完全匹配，因此可复用 |
| Requalification | 最终 RC 身份变化，需要重新正式验证 |
| External MCP Client | QA 自动测试使用的外部 MCP 客户端，不是 ChatGPT |
| External ChatGPT | 真实 ChatGPT 产品客户端 |
| OUT_OF_CORE | 本次 Core 不宣称已验收 |
| BLOCKED | 有前置条件未满足，不能发布 |
| INCONCLUSIVE | 证据不足或漂移，不能得出 PASS |
| V4 Core — ACCEPTED | M06 唯一允许的成功发布结论 |

---

# 2. 验收前确认

必须先具备：

- [ ] M01 已 ACCEPTED / FROZEN。
- [ ] M02 已 ACCEPTED / FROZEN。
- [ ] M03 已 ACCEPTED / FROZEN。
- [ ] M04 已 ACCEPTED / FROZEN。
- [ ] M05 已 ACCEPTED / FROZEN。
- [ ] M06 Development Complete。
- [ ] M06-INT-01～M06-INT-16 全部 PASS。
- [ ] M06-E2E-01～M06-E2E-04 全部 PASS。
- [ ] 所有 M06 自动化证据绑定同一个最终 candidate。
- [ ] Portable 和 Installer SHA-256 已记录。
- [ ] UI / Runtime / Tunnel SHA-256 已记录。
- [ ] Config schema、History schema/checksum、Tool catalog hash 已记录。
- [ ] M02 F05 最终候选结果或合法 equivalence 已记录。
- [ ] M04/M05 F12 最终候选结果或合法 equivalence 已记录。
- [ ] security violations = 0。
- [ ] 自动 E2E 没有把 QA external MCP client 冒充成 External ChatGPT。
- [ ] 自动 E2E 结束时，最终 Core Gate 因尚缺 Manual/External ChatGPT 而仍然正确 BLOCKED。

验收信息：

~~~text
Candidate ID：
Candidate commit：
Product version：
Source-state SHA-256：

Portable SHA-256：
Installer SHA-256：
UI SHA-256：
Runtime SHA-256：
Tunnel SHA-256：

Protocol/schema hash：
Config schema/profile hash：
Evidence schema/checksum：
Tool catalog hash：

M02 F05 evidence identity/verdict：
M04 baseline identity：
M05 formal regression identity/verdict：
Requested/resolved Model：
Security violations：

Integration verdict：
E2E verdict：
验收人：
日期：
~~~

---

# 3. M06-MAN-01 — 候选身份、安装/升级/回滚和 Core 发布范围

**对应：** M06-AC01～AC06、AC19、AC20

本项主要回答：

> “我们到底在验收哪个版本？这个版本能否安全安装/升级/回退？发布说明有没有夸大范围？”

## 3.1 Candidate 身份

核对最终证据：

- [ ] Git commit 唯一且明确。
- [ ] source_dirty=false。
- [ ] Portable、Installer、UI、Runtime、Tunnel hash 完整。
- [ ] Portable 与 Installer 的核心 payload 属于同一个 candidate。
- [ ] Config schema、History schema、Tool schema 与候选绑定。
- [ ] M02 / M04 / M05 被复用的证据都有明确 equivalence key；不是“看起来差不多”。

如果自动化测试中途修过产品代码：

- [ ] 已生成新 candidate。
- [ ] 受影响的正式测试已重跑。
- [ ] 没有把旧 candidate 的最好结果拼到新 candidate 上。

## 3.2 安装与 Portable

查看自动化结果并实际打开最终候选之一。

确认：

- [ ] Portable 能正常启动。
- [ ] Installer 安装后的 App 能正常启动。
- [ ] 没有测试用 API Key、临时 DB、测试 Workspace 被打包。
- [ ] 版本信息与候选记录一致。

## 3.3 Upgrade

查看/抽查升级后的设置：

- [ ] 原 Workspace 没被重置。
- [ ] Tunnel 配置没被无故丢失。
- [ ] Secret 仍通过 SecretReference 使用，没有变成明文。
- [ ] Tool / Policy 设置符合升级语义。
- [ ] grep 的旧用户迁移语义符合 M05：不会因升级突然扩大 Tool surface。
- [ ] History 仍可查看。
- [ ] 旧 Pending Approval / SessionGrant / ExecutionPermit 没有恢复。

## 3.4 Rollback

核对自动化 rollback 结果：

- [ ] 回退前正常 Quit。
- [ ] 需要旧 config shape 时使用的是预先保存的备份，不是人工乱改。
- [ ] 没有为了回退成功直接删除 History。
- [ ] 没有要求把 Secret 手工写进明文配置。
- [ ] 回退后没有 Runtime/Tunnel orphan。

## 3.5 Release Notes / Scope

检查最终 Release Notes 草稿。

必须看到：

- [ ] 明确写 V4 Core，而不是 V4 Full。
- [ ] F01/F03/F04/F05/F06/F07/F10/F12 和 Shared Evidence 的实际状态。
- [ ] P1 功能标为 OUT_OF_CORE / NOT_CORE_ACCEPTED，除非其自身里程碑后来已独立验收且说明清晰。
- [ ] P2 标为 DEFERRED 或独立验收状态。
- [ ] 没有宣称官方 MCP conformance。
- [ ] 没有宣称完整 Agent reasoning trace。
- [ ] M02 性能结果如实写。
- [ ] M04/M05 Eval 结果如实写。
- [ ] External ChatGPT 状态在本次人工验证完成前不能提前写 PASS。

**M06-MAN-01 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 4. M06-MAN-02 — Windows 后台、Tray、审批通知和断连通知

**对应：** M06-AC07、AC08、AC09

使用最终候选 Windows App。

## 4.1 Background / Tray

让 Harness 达到 Ready，然后关闭主窗口。

确认：

- [ ] App 没有退出。
- [ ] Harness 没有 Stop。
- [ ] Runtime/Tunnel 仍在正常工作。
- [ ] Tray 图标存在。
- [ ] Tray 能 Show 主窗口。
- [ ] Tray 能 Start / Stop。
- [ ] Tray 显示的连接状态与主界面真实状态一致。
- [ ] Pending Approval 数量与 Approval Center 一致。
- [ ] Stop 后状态及时更新。
- [ ] 再 Start 能正常恢复。
- [ ] 最终 Quit 后 App 真正退出，不遗留受管 Runtime/Tunnel。

## 4.2 Approval Notification

设置一个安全测试 Tool 为 ASK，产生新的 Pending Approval。

确认：

- [ ] Windows 出现一次审批通知。
- [ ] 同一个 Pending 不持续重复刷通知。
- [ ] 通知内容不包含 Secret 或不必要的 raw 内容。
- [ ] 点击通知能把 App 带回正确的 Approval 位置/当前状态。
- [ ] 如果你先处理掉 Approval 再点击旧通知，不会把旧请求恢复成 Pending。
- [ ] 通知失败或被系统隐藏时，Approval Center 里的真实 Pending 仍然存在。

## 4.3 Disconnect / Recovery Notification

使用 QA 给出的安全故障方式制造一次真实 Tunnel/连接故障。

确认：

- [ ] 出现一次断连/降级通知。
- [ ] 同一个持续故障不会不断刷屏。
- [ ] 恢复后可以看到一次恢复提示（如果产品当前启用该提示）。
- [ ] 恢复后再制造一个新的独立故障，可以再次通知。
- [ ] 手工 Stop/Quit 不被错误当成异常断连。
- [ ] 点击入口后看到的是当前 Overview/Diagnostics，而不是旧状态。

**M06-MAN-02 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 5. M06-MAN-03 — F05 启动性能、Authoritative Ready、Diagnostics 与 History 可理解性

**对应：** M06-AC10、AC11、AC12、AC13

## 5.1 F05 结果审阅

查看最终候选 F05 证据。

确认：

- [ ] 使用的是 M02 原冻结 threshold，没有在 M06 放宽。
- [ ] 如果复用了 M02 结果，App/Runtime/Tunnel/配置/环境等 equivalence key 全部匹配。
- [ ] 如果不匹配，已经完整重跑 M02 冻结 benchmark。
- [ ] cold / warm 分开。
- [ ] P50 / P95 / failure rate 都有实际样本支持。
- [ ] 没有把多个 P95 再平均。
- [ ] 没有删掉“太慢但合法”的样本来让结果好看。
- [ ] Ready 仍然代表真实可用状态，不是 spawn success。

在最终 App 中实际 Start 一次：

- [ ] UI 的阶段/耗时信息合理。
- [ ] Ready 后可以实际使用工具。
- [ ] 不出现“显示 Ready，但 Tunnel/MCP 实际不可用”的明显窗口。

## 5.2 Diagnostics

在正常状态运行一次 Diagnostics。

确认：

- [ ] Config / Workspace / Runtime / MCP / Tunnel / Control Plane / Proxy / Credential / Lifecycle 结果可理解。
- [ ] UNKNOWN 不会伪装成 PASS。
- [ ] Remediation 能告诉你下一步去哪里处理。

制造一个安全、可恢复的故障后再运行：

- [ ] 对应组件显示 FAIL/UNKNOWN。
- [ ] 诊断不会自动重启 Harness。
- [ ] 不会自动修改 Workspace/config/Secret。
- [ ] 恢复故障后重新诊断，显示当前真实恢复状态。

复制 Diagnostic Summary：

- [ ] 没有 API Key/Secret。
- [ ] 没有完整敏感物理路径。
- [ ] 没有带凭据 Proxy URL。
- [ ] 没有 raw environment。
- [ ] 没有无界 stdout/stderr。
- [ ] 仍足够排查问题。

## 5.3 History

做几次 read/grep/ASK/DENY 后查看 History。

确认：

- [ ] 能按时间/Tool/Outcome/Policy/Approval 找到对应请求。
- [ ] runtime_id + request_id 与真实请求一致。
- [ ] 重启 App/Runtime 后终态 History 还在。
- [ ] 旧 Pending/SessionGrant/ExecutionPermit 不会恢复。
- [ ] grep History 不保存 raw query。
- [ ] 不保存完整 match preview。
- [ ] 不泄露 Secret/完整物理路径。
- [ ] History 有故障/缺失时会明确显示 degraded/unknown，而不是装作完整。

**M06-MAN-03 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 6. M06-MAN-04 — Core 安全边界与 F12 质量门禁复核

**对应：** M06-AC12～AC17

本项不要求你重跑全部自动化攻击/评测，而是核对“这些证据是否可信，最终候选有没有偷偷绕过合同”。

## 6.1 Runtime Authority / Workspace

查看 Integration/E2E 证据并抽查真实行为：

- [ ] read_file/grep 都经过 Workspace/Policy/Approval/ExecutionPipeline。
- [ ] grep 没有隐藏调用 shell grep/ripgrep/Bash。
- [ ] DENY 不执行。
- [ ] ASK deny 不执行。
- [ ] ASK allow_once 只授权对应真实请求。
- [ ] Tool disabled 无法执行。
- [ ] Workspace escape / symlink / junction / reparse / TOCTOU 自动化证据 PASS。
- [ ] outside sentinel observed = 0。

## 6.2 Approval / SessionGrant / History

确认：

- [ ] 当前 Approval 才能授权当前请求。
- [ ] 旧 History 不能生成 Pending。
- [ ] 旧 History 不能恢复 SessionGrant。
- [ ] 旧 History 不能恢复 ExecutionPermit。
- [ ] 重启后新的请求重新按当前 Policy 判断。

## 6.3 Bash / SecretStore / lifecycle

查看回归证据：

- [ ] Bash timeout/output/cancel/concurrency 没退化。
- [ ] Bash 风险分类/审批仍有效。
- [ ] SecretStore 没把密钥写进 config/History/log。
- [ ] Start/Stop/Restart 不出现重复进程。
- [ ] Crash/restart 后状态真实。
- [ ] 最终 Quit 无 orphan。

## 6.4 F12

查看最终候选 F12 证据。

如果是 equivalence reuse：

- [ ] Runtime hash 匹配。
- [ ] Tool catalog/schema 匹配。
- [ ] M04 dataset/prompt/scorer 匹配。
- [ ] M05 profile 匹配。
- [ ] Model/adapter identity 满足冻结的复用条件。

如果重新跑过：

- [ ] 使用真实受控 Model，不是 scripted/mock。
- [ ] 没有改变 M04 dataset。
- [ ] 没有改变 M04 scorer。
- [ ] 没有改变 compare_m05 threshold。
- [ ] validity/sample 条件满足。
- [ ] security violations = 0。
- [ ] 如发生 provider/model drift，paired control 在 guard 内；否则应是 INCONCLUSIVE。
- [ ] 失败 run 没被“重跑到成功再删掉旧结果”。
- [ ] passive production telemetry 没冒充 Tool Selection accuracy。

**M06-MAN-04 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 7. M06-MAN-05 — 真实 External ChatGPT 最终 Core 用户链路与发布签认

**对应：** M06-AC18、AC19、AC20  
**这是 M06 最关键的最终人工链路。**

使用：

- 最终 M06 candidate；
- 真实 External ChatGPT；
- 真实 Tunnel；
- 安全、可丢弃的测试 Workspace。

不要用内部 MCP client 替代本项。

## 7.1 Hidden-window ChatGPT Tool call

让 Harness Ready，然后关闭主窗口隐藏到 Tray。

在真实 External ChatGPT 中给一个自然任务，例如：

> 读取当前 Workspace 中指定的小型测试文件，告诉我其中的 TEST_CORE_READ 标记。

确认：

- [ ] ChatGPT 能看到 Harness Tools。
- [ ] 主窗口隐藏时 Tool 仍能成功调用。
- [ ] Harness 没有因为窗口关闭而 Stop。
- [ ] Activity 能看到真实请求。
- [ ] History 能用同一个 runtime_id + request_id 找到请求。

## 7.2 Real grep

准备含唯一标记的多个测试文件，给 ChatGPT 一个自然搜索任务，不要强制说“必须用 grep”。

确认：

- [ ] ChatGPT 能发现并合理使用 grep，或其选择有合理解释。
- [ ] 如果调用 grep，结果文件/行号正确。
- [ ] 没有 Workspace 外结果。
- [ ] History 不持久化 raw query/match preview。

再做一个 regex 或 scope/filter 任务：

- [ ] 参数合法。
- [ ] 结果正确。
- [ ] 无需绕到 Bash 才能完成。

## 7.3 ASK

把一个安全 Tool/grep Policy 调成 ASK，让 ChatGPT 发起请求。

确认：

- [ ] 出现真实 Pending Approval。
- [ ] Windows 出现审批通知。
- [ ] 你点击/打开后看到的是同一个当前请求。
- [ ] 在批准前没有先执行。
- [ ] allow_once 后只执行这一条请求。
- [ ] Activity/History 能对应同一请求。

## 7.4 DENY

把同一类 Tool 改成 DENY，再让 ChatGPT 调用。

确认：

- [ ] Runtime 明确拒绝。
- [ ] ChatGPT 收到可理解的失败。
- [ ] 没有通过别的隐藏路径执行。
- [ ] History 显示真实 denied 状态。

## 7.5 Disconnect / Diagnostics / Recovery

在 ChatGPT/Harness 连通时制造一个 QA 指定的可恢复断连。

确认：

- [ ] 收到断连/降级通知。
- [ ] Diagnostics 能指出实际故障位置。
- [ ] 恢复后 Harness 回到真实 Ready。
- [ ] ChatGPT 再次调用 Tool 成功。
- [ ] History 中断前后的记录保持可理解。

## 7.6 Restart / stale authorization

产生一条有 Approval 历史的请求后重启 Runtime/App。

确认：

- [ ] History 仍能看到过去记录。
- [ ] 旧 Approval 不会重新变 Pending。
- [ ] 旧 SessionGrant/Permit 不会自动授权新请求。
- [ ] 新请求重新按当前 Policy 处理。

## 7.7 Final Quit

从 Tray 或产品正式 Quit 入口退出。

确认：

- [ ] 主 App 退出。
- [ ] 不再保留受管 Runtime。
- [ ] 不再保留受管 Tunnel。
- [ ] 没有遗留测试 Pending/进程/临时安全配置。

**M06-MAN-05 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 8. 最终 Core Product Acceptance Matrix 审阅

在做最终签名前，查看自动生成/汇总的 Core Product Acceptance Matrix。

确认：

- [ ] 28 个 canonical Core AC 全部出现。
- [ ] 每个都有 owner milestone。
- [ ] 每个都有 evidence reference。
- [ ] 每个 mandatory Core final_status 都是 PASS。
- [ ] 没有 FAIL。
- [ ] 没有 BLOCKED。
- [ ] 没有 INCONCLUSIVE。
- [ ] 没有 NOT_VERIFIED。
- [ ] P1/P2 没有混进 Core PASS。
- [ ] M02 F05 是最终候选合法证据。
- [ ] F12 是最终候选合法证据。
- [ ] 本次 MAN-05 的 External ChatGPT 证据已填入。
- [ ] Windows Tray/notification 人工观察已填入。
- [ ] Matrix 绑定最终 candidate，不混旧 build。

---

# 9. 人工验收汇总

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M06-MAN-01 Candidate / Package / Upgrade / Rollback / Scope |  |  |
| M06-MAN-02 Tray / Background / Notifications |  |  |
| M06-MAN-03 F05 / Diagnostics / History |  |  |
| M06-MAN-04 Security / Authority / F12 |  |  |
| M06-MAN-05 External ChatGPT Final Core Chain |  |  |

自动化与候选摘要：

~~~text
M01 status：
M02 status：
M03 status：
M04 status：
M05 status：

M06 Development：
M06 Integration：
M06 E2E：

Candidate ID：
Candidate SHA：
Product version：
Portable SHA-256：
Installer SHA-256：
UI SHA-256：
Runtime SHA-256：
Tunnel SHA-256：

F05 final verdict：
F12 final verdict：
Security violations：
Outside-sentinel observed：
Owned orphan count after Quit：
Core matrix mandatory PASS count：
Core matrix FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED count：
External ChatGPT final chain：
Windows Tray/notification observation：
Upgrade：
Rollback：
~~~

---

# 10. 最终结论

只能选择一个：

- [ ] V4 Core — ACCEPTED
- [ ] M06 / V4 Core — FAIL
- [ ] M06 / V4 Core — BLOCKED
- [ ] M06 / V4 Core — INCONCLUSIVE

只有以下全部满足才能选择 V4 Core — ACCEPTED：

- [ ] M01–M05 全部 ACCEPTED / FROZEN。
- [ ] M06-AC01～M06-AC20 全部 PASS。
- [ ] 28 条 canonical Core AC 全部 PASS。
- [ ] M06-INT-01～M06-INT-16 全部 PASS。
- [ ] M06-E2E-01～M06-E2E-04 全部 PASS。
- [ ] M06-MAN-01～M06-MAN-05 全部 PASS。
- [ ] 最终 Candidate identity 唯一且一致。
- [ ] Portable 和 Installer integrity PASS。
- [ ] Upgrade PASS。
- [ ] Rollback PASS。
- [ ] M02 冻结 F05 门槛 PASS。
- [ ] F12 最终候选 evidence 合法且 PASS。
- [ ] security violations = 0。
- [ ] Workspace outside sentinel observed = 0。
- [ ] Shared Evidence / History 没有恢复授权。
- [ ] Runtime/Tunnel crash/recovery 无当前 blocker。
- [ ] 最终 Quit 无 owned orphan。
- [ ] Windows Tray / Approval notification / Disconnect notification 已在最终包上实际观察。
- [ ] 真实 External ChatGPT → Tunnel → Harness → Runtime → read/grep/ASK/DENY → History 链路已亲自完成。
- [ ] Core Product Acceptance Matrix 无 mandatory FAIL/BLOCKED/INCONCLUSIVE/NOT_VERIFIED。
- [ ] Release Notes 明确是 Core-only scope。
- [ ] P1/P2 没被冒充成 Core accepted。

M06 成功后只允许写：

~~~text
V4 Core — ACCEPTED
~~~

不得写：

~~~text
V4 — ACCEPTED
V4 — ACCEPTED / FROZEN
V4 Full — ACCEPTED
~~~

这些必须等待后续 P1 里程碑及 M11 Full Release Gate。
