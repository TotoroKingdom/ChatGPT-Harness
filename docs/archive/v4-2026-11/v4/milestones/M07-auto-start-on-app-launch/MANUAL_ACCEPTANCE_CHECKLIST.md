# M07 人工验收清单 — Auto Start Harness on App Launch

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M07 — Auto Start on App Launch  
> 目标：验证 Auto Start 的真实用户体验、设置语义、App 重开行为、失败恢复，以及真实 External ChatGPT 最终链路  
> 前置条件：M00/M01/M02 已 ACCEPTED / FROZEN；M07 Development、Integration、E2E 已有真实结果  
> 人工边界：不要求你手工复现所有 config migration、race、fault injection、双进程竞争或单元测试。人工重点是最终候选是否符合用户直觉且不引入危险的自动行为。  
> 记录规则：本文件是冻结验收合同，不为填写执行结果而修改；真实 PASS/FAIL 在验收对话和候选证据中记录。  
> 重要：F02 不是 Windows 开机/登录自启动，只是“用户打开 ChatGPT-Harness 后，自动 Start Harness”。

---

# 1. 验收前确认

必须先具备：

- [ ] M00 已 ACCEPTED / FROZEN。
- [ ] M01 已 ACCEPTED / FROZEN。
- [ ] M02 已 ACCEPTED / FROZEN。
- [ ] M07 Development Complete。
- [ ] M07-INT-01～M07-INT-12 全部 PASS。
- [ ] M07-E2E-01～M07-E2E-04 全部 PASS。
- [ ] 最终 candidate SHA 已固定。
- [ ] Portable / UI / Runtime / Tunnel hash 已记录。
- [ ] Config schema 仍为 v3。
- [ ] Auto Start 默认值证据为 false。
- [ ] configRevision / Runtime restart debt 测试 PASS。
- [ ] Auto/Manual/Tray race 没有重复 Runtime/Tunnel generation。
- [ ] Quit cleanup 无 owned orphan。
- [ ] 没有 Windows Run key / Startup folder / Task Scheduler / Service 等登录启动机制。
- [ ] 自动 E2E 使用的 external MCP probe 没有被冒充成 External ChatGPT。

验收信息：

~~~text
Candidate SHA：
Product version：
Portable SHA-256：
UI SHA-256：
Runtime SHA-256：
Tunnel SHA-256：
Config schema：
Integration verdict：
E2E verdict：
验收人：
日期：
~~~

---

# 2. M07-MAN-01 — 设置开关、默认值和用户语义

**对应：** F02-AC01，M07-AC01～AC04

打开最终候选 Settings。

确认：

- [ ] 能看到独立的 “Auto Start Harness on app launch” 设置。
- [ ] 中文和英文说明都容易理解。
- [ ] 文案明确表示“打开 ChatGPT-Harness 后自动 Start Harness”。
- [ ] 文案明确表示“不是 Windows 登录/开机自启动 App”。
- [ ] 新配置/重置后的默认状态为关闭。
- [ ] 开关当前显示值与实际保存值一致。

## 开启

在 Harness 当前为 Stopped 的情况下把开关打开。

确认：

- [ ] 保存成功。
- [ ] 当前这一次 App 会话不会因为刚打开开关而立即自己 Start。
- [ ] 不出现 Restart Required。
- [ ] 不要求重启 Runtime。
- [ ] 其他通知、Workspace、Tunnel、Policy 设置没有被覆盖。

## 关闭

在 Harness 当前为 Running/Ready 的情况下关闭开关。

确认：

- [ ] 保存成功。
- [ ] 当前 Harness 不会因为关闭开关而自动 Stop。
- [ ] 不出现 Restart Required。
- [ ] 只影响下一次 App 启动。

## Windows 登录边界

从产品设置和 Windows 常见启动位置确认：

- [ ] 没有“Start with Windows / 登录时启动”等功能被偷偷加入。
- [ ] 没有额外 Windows Service / Startup 项需要用户管理。

**M07-MAN-01 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 3. M07-MAN-02 — 开启后的真实 Quit→重开→一次 Auto Start

**对应：** F02-AC02，M07-AC05、AC06、AC12、AC14

前置：

- Auto Start = ON；
- Workspace / Tunnel / credential 已正确配置；
- Harness 当前可正常手动 Start。

执行：

1. 确认 Auto Start 已保存为 ON。
2. 从正式 Quit 入口完全退出 App。
3. 确认受管 Runtime/Tunnel 已退出。
4. 重新启动 ChatGPT-Harness。
5. 这一次不要点 Start。

确认：

- [ ] App 打开后 Harness 自动开始启动。
- [ ] 只发生一次自动 Start。
- [ ] 不需要再点 Start。
- [ ] UI 的 Starting / Ready 状态与真实生命周期一致。
- [ ] 最终达到 authoritative Ready。
- [ ] 没有第二个 Runtime。
- [ ] 没有第二个 Tunnel。
- [ ] Startup/Diagnostics 页面看到的是正常标准 Start 证据，而不是一套单独的 Auto Start 假状态。

在 Ready 后：

- [ ] Overview / Tray 显示一致。
- [ ] Tray 状态正确。
- [ ] 现有 Tool 列表/配置没有因 Auto Start 改变。

**M07-MAN-02 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 4. M07-MAN-03 — Hide/Show、Stop、Restart 和 Quit 的用户控制权

**对应：** F02-AC02/03，M07-AC09～AC12

从已经 Auto Start 成功、Harness Ready 的状态开始。

## Close → Hide → Show

关闭主窗口，让 App 隐藏到 Tray。

确认：

- [ ] Harness 继续运行。
- [ ] 再从 Tray Show 主窗口不会触发第二次 Auto Start。
- [ ] 状态连续，没有突然出现第二套 Runtime/Tunnel。

## Stop

从 Tray 或主界面执行 Stop。

确认：

- [ ] Harness 正常停止。
- [ ] 等待一段正常状态刷新时间后，仍保持 Stopped。
- [ ] Auto Start 不会因为“发现 Stopped”又自动启动。
- [ ] Hide/Show 之后仍然是 Stopped。

随后手工 Start：

- [ ] 手工 Start 正常工作。
- [ ] 不会与一个迟到的 Auto Start 竞争出第二个实例。

## Restart

执行显式 Restart。

确认：

- [ ] Restart 是一次正常用户生命周期操作。
- [ ] 不会被误显示成“第二次 Auto Start”。
- [ ] 最终生命周期真实。

## Quit

最后正式 Quit。

确认：

- [ ] App 退出。
- [ ] Runtime 退出。
- [ ] Tunnel 退出。
- [ ] 没有 orphan。
- [ ] 没有退出后又被 Auto Start 拉起。

**M07-MAN-03 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 5. M07-MAN-04 — 失败、Diagnostics、无自动重试和手工恢复

**对应：** F02-AC03，M07-AC07、AC08、AC11、AC14

使用 QA 准备的安全、可恢复测试条件。优先使用隔离配置，不修改真实生产 credential。

例如：

- 缺失测试 credential；
- 无效测试 Workspace；
- 其他测试计划已验证安全的 eligibility failure。

让 Auto Start = ON，完全 Quit，再重新打开 App。

确认：

- [ ] Auto Start 尝试失败。
- [ ] UI 没有错误显示 Ready。
- [ ] 错误信息可理解。
- [ ] Diagnostics 能看到真实原因。
- [ ] Diagnostics 不会偷偷帮你 Start。
- [ ] 产品不会自动改 Workspace。
- [ ] 产品不会自动生成/替换 credential。
- [ ] 产品不会关闭 Policy/安全检查。
- [ ] 等待、切页、Hide/Show 后不会自动再次 Start。
- [ ] 没有不断重试/刷错误。

修好该测试条件，但不重启 App。

确认：

- [ ] 修好之后仍不会突然自动重试。
- [ ] 你明确点 Manual Start 后可以正常启动。
- [ ] 最终 Ready 后状态正确。

查看 QA 的 race 证据：

- [ ] Auto Start vs Manual Start 只产生一代 Runtime/Tunnel。
- [ ] Auto Start vs Tray Start 只产生一代 Runtime/Tunnel。
- [ ] Quit 与 Auto Start 冲突时，Quit 最终获胜且无 orphan。

**M07-MAN-04 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 6. M07-MAN-05 — 真实 External ChatGPT 最终用户链路

**对应：** F02-AC02/03，M07-AC14  
**这是 M07 最终真实用户链路，不能用内部 MCP client 替代。**

前置：

- 最终 M07 candidate；
- Auto Start = ON；
- 安全测试 Workspace；
- 真实 Tunnel；
- 真实 External ChatGPT；
- 当前 App 完全退出。

## 6.1 App launch

启动 ChatGPT-Harness，但不要点击 Start。

确认：

- [ ] Harness 自动 Start。
- [ ] 最终达到 Ready。
- [ ] 只启动一次。

## 6.2 External ChatGPT Tool call

在真实 ChatGPT 中执行一个安全自然任务，例如：

> 读取当前 Workspace 中指定测试文件，并告诉我 TEST_M07_AUTOSTART 标记内容。

确认：

- [ ] ChatGPT 能看到 Harness Tools。
- [ ] Tool 调用成功。
- [ ] 结果正确。
- [ ] 这次调用发生在 Auto Start 后，而不是你手动 Start 后。
- [ ] Activity/History（如果当前候选显示）对应真实请求。
- [ ] 没有因为 Auto Start 绕过 Workspace/Policy/Approval。

## 6.3 Hidden-window continuity

保持 Harness Ready，关闭主窗口隐藏到 Tray。

再在真实 ChatGPT 中执行一个安全 read 或 grep 任务。

确认：

- [ ] Tool 仍然可用。
- [ ] Show 主窗口不会再次触发 Auto Start。
- [ ] 状态仍然连续。

## 6.4 Stop user control

手工 Stop Harness。

再尝试一次 ChatGPT Tool 调用。

确认：

- [ ] Harness 不会因为 ChatGPT 请求或后台状态刷新自动重新启动。
- [ ] 产品不会违反用户 Stop 意图。

随后手工 Start 可恢复正常。

## 6.5 Final Quit

正式 Quit。

确认：

- [ ] App / Runtime / Tunnel 全部退出。
- [ ] 无 orphan。
- [ ] 下次启动时，如果 Auto Start 仍为 ON，才会进行新 App process 的一次新 Auto Start。

**M07-MAN-05 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 7. 人工验收汇总

| 验收项 | 结论 | 备注 |
| --- | --- | --- |
| M07-MAN-01 设置 / 默认 / 语义 |  |  |
| M07-MAN-02 Quit→重开→一次 Auto Start |  |  |
| M07-MAN-03 Hide / Stop / Restart / Quit |  |  |
| M07-MAN-04 Failure / Diagnostics / No Retry |  |  |
| M07-MAN-05 External ChatGPT Final Chain |  |  |

候选摘要：

~~~text
Candidate SHA：
Portable SHA-256：
UI SHA-256：
Runtime SHA-256：
Tunnel SHA-256：
Config schema：
Default Auto Start：
Enabled relaunch：
Disabled relaunch：
Exactly-once：
Auto vs Manual race：
Auto vs Tray race：
Stop/no-restart：
Quit/orphan：
Failure/no-retry：
Diagnostics：
Windows login startup mechanism：
External ChatGPT：
Integration：
E2E：
~~~

---

# 8. 最终结论

只能选择一个：

- [ ] M07 — ACCEPTED / FROZEN
- [ ] M07 — FAIL
- [ ] M07 — BLOCKED
- [ ] M07 — INCONCLUSIVE

只有以下全部满足才能选择 M07 — ACCEPTED / FROZEN：

- [ ] M00/M01/M02 全部 ACCEPTED / FROZEN。
- [ ] F02-AC01～F02-AC03 全部 PASS。
- [ ] M07-AC01～M07-AC14 全部 PASS。
- [ ] M07-INT-01～M07-INT-12 全部 PASS。
- [ ] M07-E2E-01～M07-E2E-04 全部 PASS。
- [ ] M07-MAN-01～M07-MAN-05 全部 PASS。
- [ ] schema 仍为 v3。
- [ ] 新安装/重置默认 OFF。
- [ ] 显式 ON/OFF 可持久保存。
- [ ] preference save 不产生 Runtime restart debt。
- [ ] ON/OFF 不立即 Start/Stop 当前 Harness。
- [ ] 新 App process 中 enabled 只 Auto Start 一次。
- [ ] disabled 新 App process 不 Auto Start。
- [ ] Manual/Tray/Auto race 不产生重复 generation。
- [ ] Auto failure 不自动 retry。
- [ ] Stop 后不自动 re-Start。
- [ ] Quit 后无 owned orphan。
- [ ] 无 Windows 登录自启动机制。
- [ ] 缺 credential/eligibility failure 不伪报 Ready。
- [ ] Diagnostics 保持真实、只读。
- [ ] M01/M02/V3 受影响回归 PASS。
- [ ] 真实 External ChatGPT 在 Auto Start 后成功调用 Harness Tool。
- [ ] 隐藏窗口期间 External ChatGPT Tool 仍可用。
- [ ] 无当前范围 blocker。

M07 成功只表示 F02 里程碑验收完成。

它不会单独产生：

~~~text
V4 — ACCEPTED / FROZEN
~~~

V4 Full 仍必须等待 M08/M09/M10 及 M11 Full Release Gate。
