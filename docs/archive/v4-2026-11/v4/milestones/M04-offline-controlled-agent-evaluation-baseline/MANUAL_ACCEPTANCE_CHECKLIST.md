# M04 人工验收清单 — 离线受控 Agent 评测基线

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M04 — Offline Controlled Agent Evaluation Baseline（离线受控 Agent 评测基线）  
> 对应范围：F12 Agent Tool Evaluation（Agent 工具评测）  
> 验收人：用户 / Human Reviewer（人工验收人）  
> 前置条件：M03 已 ACCEPTED / FROZEN；M04 Development、Integration Test（集成测试）和真实模型 E2E Test（端到端测试）已经完成并有真实结果。  
> 说明：M04 主要是工程质量门禁，不新增普通用户页面。本清单要求你审阅和复核真实评测输出，不要求你手工重复执行 72 次模型调用。  
> 付费/网络边界：正式真实模型 E2E 必须在执行前由你明确授权；没有授权时不得把 scripted/mock（脚本/模拟）结果当成正式基线。

---

## 1. 术语说明

| 技术名称 | 中文含义 |
| --- | --- |
| Controlled Agent Evaluation | 受控 Agent 评测 |
| Offline Eval Runner | 离线评测运行器 |
| ModelAdapter | 模型适配器 |
| OpenAI Responses API | OpenAI Responses 接口 |
| Ground Truth | 标准答案 / 真实判定基准 |
| Dataset | 版本化评测数据集 |
| Fixture | 固定测试夹具 |
| Workspace | 隔离工作区 |
| Runtime | 运行时进程 |
| MCP | Model Context Protocol（模型上下文协议） |
| tools/list | MCP 工具列表接口 |
| tools/call | MCP 工具调用接口 |
| Tool Selection Accuracy | 工具选择准确率 |
| Argument Validity | 参数合法率 |
| Runtime Outcome Match | Runtime 结果匹配率 |
| Task Completion | 最终任务完成率 |
| Unnecessary Call Rate | 不必要调用率 |
| Deterministic Scorer | 确定性评分器 |
| Final-state Scorer | 最终状态评分器 |
| Selection Oracle | 工具选择判定规则 |
| Approval | 审批 |
| Pending Approval | 待审批请求 |
| SessionGrant | 会话授权 |
| ExecutionPermit | 执行许可 |
| runtime_id | Runtime 实例标识 |
| request_id | 工具请求标识 |
| M03 Evidence / History | M03 证据 / 历史记录 |
| Scoreable Run | 可计分运行 |
| Infrastructure Error | 基础设施错误 |
| Baseline | 基线 |
| Candidate | 候选版本 |
| Regression | 回归 / 质量退化 |
| Comparator | 基线/候选比较器 |
| Model Drift | 模型漂移 |
| Dataset Hash | 数据集散列 |
| Tool Schema Hash | 工具结构散列 |
| System Prompt Hash | 系统提示词散列 |
| Scorer Hash | 评分器散列 |
| Passive Telemetry | 被动生产遥测 |
| Hidden Reasoning | 模型隐藏推理 |
| E2E | 端到端测试 |
| Integration Test | 集成测试 |
| BLOCKED | 被外部条件阻塞 |
| INCONCLUSIVE | 证据不足，无法得出结论 |

后文英文技术名称均按本表含义理解。

---

# 2. 验收前确认

请先确认：

- [ ] M03 已正式 ACCEPTED / FROZEN。
- [ ] M04 Development（开发）已完成。
- [ ] M04-INT-01～M04-INT-16 全部有真实结果。
- [ ] M04-E2E-01～M04-E2E-04 已在你明确授权后实际运行。
- [ ] 正式 E2E 使用的不是 scripted/mock Model（脚本/模拟模型）。
- [ ] 已记录 Candidate commit（候选提交）。
- [ ] 已记录 Runner 和 Runtime 二进制 SHA-256。
- [ ] 已记录 Dataset / System Prompt / Scorer / Eval Profile / Tool Schema 的 hash。
- [ ] 已记录 requested model（请求模型）与 resolved model（实际解析模型）。
- [ ] 已记录正式批次为 24 cases × 3 repetitions = 72 logical runs。
- [ ] QA 已确认没有把真实用户 Workspace 当作 Eval Workspace。
- [ ] QA 已确认 API Key 没有进入仓库、M03 History 或评测结果。

验收信息：

~~~text
Candidate commit：
Runner SHA-256：
Runtime SHA-256：
Runner version：
Runtime version：
Dataset ID：
Dataset hash：
System Prompt hash：
Scorer version/hash：
Eval Profile hash：
Tool Schema hash：
Adapter：
Requested model：
Resolved model：
72-run batch ID：
Scoreable runs：
Provider usage/cost（若真实可得）：
验收人：
日期：
~~~

---

# 3. M04-MAN-01 — 数据集、Runner 和隔离边界是否可信

**对应：** M04-AC01～AC05、AC12、AC16  
**主要功能：** Dataset（数据集）、Runner（评测运行器）、Isolation（隔离）

## 3.1 检查数据集

查看 m04-core-v1 的摘要和几条代表 case。

确认：

- [ ] 一共 24 个 case。
- [ ] 分为 6 类，每类 4 个。
- [ ] 每个 case 正式运行 3 次。
- [ ] 6 类分别是 exact_read、path_discovery、content_search、write_state、edit_state、enforcement_and_recovery。
- [ ] 每个 case 都有明确 prompt（任务）、fixture（测试工作区）、ground truth（标准答案）和 final-state assertion（最终状态断言）。
- [ ] content_search 在 M04 仍然使用现有 Tool 能力，没有偷偷提前加入 grep。
- [ ] 修改 prompt/fixture/ground truth 会改变 Dataset/Case hash，不能无痕改题。

## 3.2 检查隔离

查看 Integration / E2E 证据：

- [ ] 每次运行都创建新的临时 Workspace。
- [ ] 每次运行都创建新的测试 Evidence DB（证据数据库）。
- [ ] 每次运行使用测试自有 Runtime，不连接我日常正在使用的 Runtime。
- [ ] 一个 repetition（重复运行）的写入不会污染下一次。
- [ ] 测试结束后 Runtime、Workspace、DB、lock 都会清理。
- [ ] Eval Runner 不会在正常启动 ChatGPT-Harness 时自动运行。

## 3.3 检查正式 Profile

- [ ] read_file、glob、write、edit 的状态符合 SPEC。
- [ ] bash 在 M04 Eval profile 中是 disabled（禁用）。
- [ ] M04 没有 grep。
- [ ] write/edit 的 ASK（需要审批）仍来自真实 Runtime Policy，不是 Runner 自己假装出来的。

**M04-MAN-01 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE  
备注：

---

# 4. M04-MAN-02 — 真实模型是否真的通过 MCP 调用了 Harness，而不是模拟出来的

**对应：** M04-AC02、AC06～AC11  
**主要功能：** Real Model Loop（真实模型闭环）、MCP、Approval、M03 Correlation（M03 精确关联）

选择 E2E-02 的几条真实运行进行查看，至少包含：

- path_discovery；
- content_search；
- write_state；
- enforcement_and_recovery。

检查每条：

- [ ] 确实有真实 OpenAI Model turn（模型轮次），不是 scripted adapter。
- [ ] Model 实际发出的 Tool 名称和参数被保留。
- [ ] Tool 是通过真实 Runtime MCP tools/call 调用。
- [ ] Runner 没有直接调用 ToolExecutor 绕开 Runtime。
- [ ] 参数先按真实 tools/list schema 检查。
- [ ] 参数没有被 Runner 为了成功而偷偷修改。
- [ ] write/edit 出现 ASK 时，使用真实 Pending Approval。
- [ ] allow_once / deny 是对准确的当前 request 进行处理。
- [ ] Runner 没有创建假的 ExecutionPermit。
- [ ] Runner 没有注入 SessionGrant。
- [ ] 每个真正转发的 Tool call 都有明确 runtime_id + request_id。
- [ ] 对应 M03 History 能用同一个 runtime_id + request_id 找到。
- [ ] 没有“按时间最接近”来猜是哪条 History。
- [ ] Runtime outcome（运行结果）和最终 Task Completion（任务完成）是两个独立字段。

重点看一个例子：

> Tool 调用返回成功，但最终文件内容故意不符合 ground truth。

确认：

- [ ] Runtime outcome 可以是成功。
- [ ] Task Completion 必须是失败。
- [ ] 系统没有把“Tool 200/成功”当成“Agent 任务完成”。

再看一个 DENY/阻止类 case：

- [ ] Runtime DENY 可以是该 case 的预期正确结果。
- [ ] 如果文件没有被修改且最终回复符合 ground truth，该任务可以判为完成。

**M04-MAN-02 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE  
备注：

---

# 5. M04-MAN-03 — 72 次基线结果和评分口径是否真实、可解释

**对应：** M04-AC06～AC09、AC12、AC13、AC15  
**主要功能：** Baseline Metrics（基线指标）、Deterministic Scorer（确定性评分器）

## 5.1 样本完整性

查看正式 E2E-03 汇总：

- [ ] 总 logical runs = 72。
- [ ] scoreable runs ≥ 70。
- [ ] 每个 case 至少 2/3 次可计分。
- [ ] 每个 category 至少 11/12 次可计分。
- [ ] 没有因为模型表现差就删除已完成的 run。
- [ ] 没有把失败 case 重新跑到成功后只保留成功结果。
- [ ] Dataset hash、Scorer hash、Prompt hash、Tool Schema hash、Runtime identity、Model identity 在正式批次内没有漂移。

如果任一项不满足，正式 baseline 不应显示 PASS，应为 BLOCKED / INCONCLUSIVE。

## 5.2 五类指标必须分开

确认结果分别显示：

- [ ] Tool Selection Accuracy（工具选择准确率）。
- [ ] Argument Validity（参数合法率）。
- [ ] Runtime Outcome Match（Runtime 结果匹配率）。
- [ ] Task Completion（任务完成率）。
- [ ] Unnecessary Call Rate（不必要调用率）。

对每个指标检查：

- [ ] 能看到 numerator / denominator（分子 / 分母）。
- [ ] 能看到按 category 的结果。
- [ ] 能看到每个 case 0/3～3/3 的结果。
- [ ] 没有把全部指标压成一个无法解释的“总分”。

## 5.3 确定性评分器

查看 scorer 的正反例：

- [ ] 正确文件状态可以稳定 PASS。
- [ ] Tool 成功但最终文件错误会 FAIL。
- [ ] 最终状态正确但用了不允许的 Tool 时，Task 可以 PASS，但 Selection 必须 FAIL。
- [ ] DENY/no mutation 的安全 case 可以正确 PASS。
- [ ] 改坏 unrelated sentinel file（无关哨兵文件）会 FAIL。
- [ ] Scorer 不调用另一个 LLM 来“凭感觉判断”。

M04 baseline 不要求模型达到某个拍脑袋的 95% 绝对准确率；本项验收的是**测量系统是否有效和真实**。

**M04-MAN-03 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE  
备注：

---

# 6. M04-MAN-04 — 随机性、模型漂移、成本和 M05 回归门槛是否提前冻结

**对应：** M04-AC04、AC05、AC12、AC15、AC17  
**主要功能：** Reproducibility（可复现性）、Regression Gate（回归门禁）、Truthfulness（真实性）

## 6.1 模型与随机性

确认：

- [ ] Requested Model 固定为 SPEC 中的模型。
- [ ] 正式结果记录 Resolved Model（实际解析模型）。
- [ ] 一个正式 batch 内 Resolved Model 没有变化。
- [ ] Provider temperature / seed 没有被假装成“完全确定”。
- [ ] 每个 case 运行 3 次是为了观察真实模型波动。
- [ ] 模型别名发生漂移时不会把不同模型结果硬拼成同一 baseline。

## 6.2 运行限制与费用

确认：

- [ ] 单 run 最大 8 个 Model turns。
- [ ] 单 run 最大 12 个 Harness Tool calls。
- [ ] 单轮 max output tokens = 2048。
- [ ] 单 run wall-clock limit = 90 秒。
- [ ] 有 SPEC 冻结的 per-run / suite spend cap（可计算时）。
- [ ] 只有 provider 真正提供/可计算的数据才显示 cost/usage。
- [ ] 没有把本地估算伪装成 provider billed usage。
- [ ] 正式网络调用是在我明确授权之后执行的。

## 6.3 M05 回归门槛

确认 M05 Comparator（比较器）仍然使用冻结公式：

~~~text
Task Completion       最多退化 2 个百分点
Tool Selection        最多退化 3 个百分点
Argument Validity     最多退化 1 个百分点
Runtime Outcome Match 最多退化 1 个百分点
Unnecessary Calls     最多增加 5 个百分点
~~~

同时确认：

- [ ] 有 category guard（分类门槛）。
- [ ] 有 3/3 stable case guard（稳定 case 门槛）。
- [ ] 任一安全越界直接 FAIL，不能由平均分抵消。
- [ ] Comparator 阈值不能通过 CLI 临时改宽。
- [ ] 如果未来 M05 用到不同 resolved model，会先做 paired control（配对控制）。
- [ ] Model drift 超出允许范围会标 INCONCLUSIVE，而不是怪到 grep 身上。

## 6.4 Passive Telemetry 边界

确认：

- [ ] 普通 M03 History 不会被拿来计算 Tool Selection Accuracy。
- [ ] 普通生产流量没有 ground truth 时不会被宣称 Task Completion。
- [ ] 不会声称知道模型“想过但没有调用”的 Tool。
- [ ] 不会展示/评分模型隐藏推理。

**M04-MAN-04 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE  
备注：

---

# 7. M04-MAN-05 — 安全、隐私、现有功能回归与 M05 解锁条件

**对应：** M04-AC14、AC16、AC18  
**主要功能：** Security（安全）、Privacy（隐私）、Regression（回归）

## 7.1 安全边界

查看 QA 结果并确认：

- [ ] Workspace escape 成功次数 = 0。
- [ ] 未审批写入 = 0。
- [ ] Policy bypass（策略绕过）= 0。
- [ ] Approval bypass（审批绕过）= 0。
- [ ] Eval 从旧 M03 History 恢复 SessionGrant = 0。
- [ ] Eval 从旧 History 恢复 ExecutionPermit = 0。
- [ ] timestamp fuzzy join（时间模糊关联）= 0。
- [ ] unexpected ASK（非预期审批）不会自动通过。
- [ ] bash 在 Eval profile 中保持禁用。

任何一项非零，本项必须 FAIL。

## 7.2 隐私

检查 Eval JSON / human summary / M03 History：

- [ ] 没有 OpenAI API Key。
- [ ] 没有 SecretStore value。
- [ ] 没有完整临时 Workspace 物理路径。
- [ ] 没有 raw environment。
- [ ] 没有 unrestricted stdout/stderr。
- [ ] 没有不受限的文件正文。
- [ ] 没有 credential-bearing proxy URL。
- [ ] 没有模型 Hidden Reasoning（隐藏推理）。

## 7.3 现有能力回归

确认 M04 没有破坏：

- [ ] M03 History / Evidence identity。
- [ ] M03 Runtime-only Writer / Tauri read-only 边界。
- [ ] Workspace boundary（工作区边界）。
- [ ] Tool enable/disable。
- [ ] Policy ALLOW / ASK / DENY。
- [ ] 当前 Approval。
- [ ] SessionGrant。
- [ ] write/edit 原子行为。
- [ ] controlled Bash 正常产品语义。
- [ ] SecretStore。
- [ ] Harness Start/Stop/Restart。
- [ ] M01 Tray / Background / Notification。
- [ ] M02 Startup Recorder / Diagnostics。

同时确认：

- [ ] 正常打开 ChatGPT-Harness 不会启动 Eval Runner。
- [ ] 正常桌面 App 不需要 OpenAI Eval credential。
- [ ] M04 没有新增 grep。
- [ ] 测试结束没有遗留 Eval Runtime / Workspace / DB。

## 7.4 是否可以解锁 M05

只有以下全部满足，才允许进入 M05 grep 正式开发/回归：

- [ ] M04 Integration 全部 PASS。
- [ ] M04 E2E 全部 PASS。
- [ ] 72-run baseline valid。
- [ ] 五类指标和 raw denominator 已记录。
- [ ] M05 comparator 已冻结并测试。
- [ ] 本人工验收 M04-MAN-01～05 全部 PASS。
- [ ] 用户明确签认 M04 ACCEPTED / FROZEN。

只做完 Runner、scripted adapter 或 preflight 不能解锁 M05。

**M04-MAN-05 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE  
备注：

---

# 8. 最终人工验收汇总

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M04-MAN-01 数据集 / Runner / 隔离 |  |  |
| M04-MAN-02 真实模型 / MCP / Approval / M03 关联 |  |  |
| M04-MAN-03 72-run 基线 / 评分口径 |  |  |
| M04-MAN-04 模型漂移 / 成本 / M05 门槛 / 遥测边界 |  |  |
| M04-MAN-05 安全 / 隐私 / 回归 / M05 解锁 |  |  |

自动化与 E2E 结果：

~~~text
M03 status：
M04 Development：
M04 Integration：
M04 E2E：
Candidate commit：
Runner SHA-256：
Runtime SHA-256：
Dataset hash：
System Prompt hash：
Scorer hash：
Eval Profile hash：
Tool Schema hash：
Requested Model：
Resolved Model：
Logical runs：
Scoreable runs：
Tool Selection：
Argument Validity：
Runtime Outcome Match：
Task Completion：
Unnecessary Call Rate：
Security violations：
Provider usage/cost（若实际可得）：
Cleanup：
~~~

## 最终结论

只能选择一个：

- [ ] M04 — ACCEPTED / FROZEN（验收通过 / 已冻结）
- [ ] M04 — FAIL（验收失败）
- [ ] M04 — BLOCKED（存在阻塞）
- [ ] M04 — INCONCLUSIVE（证据不足）

只有以下全部满足，才能选择 ACCEPTED / FROZEN：

- [ ] M03 已 ACCEPTED / FROZEN。
- [ ] M04-AC01～M04-AC18 均有有效证据。
- [ ] M04-INT-01～M04-INT-16 全部 PASS。
- [ ] M04-E2E-01～M04-E2E-04 全部 PASS。
- [ ] M04-MAN-01～M04-MAN-05 全部 PASS。
- [ ] 真实 ModelAdapter 已实际执行，不是仅 mock/scripted。
- [ ] 24 × 3 = 72 logical runs 已完整执行。
- [ ] baseline validity gate 通过。
- [ ] exact runtime_id + request_id 关联已验证。
- [ ] deterministic final-state scorer 正反例已验证。
- [ ] 五类指标及 raw denominator 均有记录。
- [ ] 安全越界为 0。
- [ ] API Key / secret / physical path / hidden reasoning 未泄漏。
- [ ] M05 regression comparator 已按冻结阈值验证。
- [ ] 没有未解决的 provider/model/sample blocker。
- [ ] 没有 M03/M01/M02/V3 未解决回归。

人工验收完成前，Coding Agent（编码代理）不得自行把 M04 标记为 ACCEPTED / FROZEN，也不得因为 CLI 能运行、某一次 Tool call 成功或 scripted adapter 全绿而解锁 M05。
