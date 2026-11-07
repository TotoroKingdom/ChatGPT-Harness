# M05 人工验收清单 — grep Tool 与 Agent 质量回归

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M05 — grep Tool and Quality Regression（grep 工具与质量回归）  
> 对应范围：F10 grep + F12 Agent Evaluation Regression（Agent 评测回归）  
> 验收人：用户 / Human Reviewer  
> 前置条件：M03、M04 已 ACCEPTED / FROZEN；M05 Development、Integration、E2E 均已有真实结果。  
> 人工边界：自动化 Integration/E2E 不需要你逐项手工重复。你主要检查用户可见行为、关键安全/隐私证据、真实 External ChatGPT 链路，以及是否可以正式接受 M05。  
> 付费/网络边界：E2E-03 的真实受控模型运行必须在执行前得到你的明确授权；External ChatGPT 的真实用户操作在本清单 M05-MAN-05 中执行。  
> 记录规则：本文件是冻结的验收基线。实际结论在验收对话中记录，不要求 Coding Agent 为填写结果修改本文件。

---

# 1. 术语说明

| 技术名称 | 中文含义 |
| --- | --- |
| grep Tool | 工作区文本搜索工具 |
| Literal Search | 字面文本搜索 |
| Regex Search | 正则表达式搜索 |
| Scope | 相对 Workspace 的搜索范围 |
| Include / Exclude | 包含 / 排除路径过滤器 |
| Preview | 匹配行的有界预览 |
| Deterministic Ordering | 确定性排序 |
| Deterministic Truncation | 确定性截断 |
| WorkspaceRead | 工作区只读能力 |
| WorkspaceGuard | 工作区边界检查 |
| Policy | 策略 |
| Approval | 审批 |
| Pending Approval | 待审批请求 |
| ExecutionPermit | 执行许可 |
| Reparse Point | Windows 重解析点 |
| Junction | Windows 目录联接 |
| TOCTOU | 检查后到使用前的竞态 |
| Opened-handle Final Path | 已打开文件句柄的最终真实路径 |
| Activity | 实时活动记录 |
| History / M03 Evidence | M03 持久化历史 / 证据 |
| runtime_id | Runtime 实例标识 |
| request_id | 工具请求标识 |
| Controlled Agent Evaluation | 受控 Agent 评测 |
| M04 Baseline | M04 正式评测基线 |
| Candidate | M05 候选版本 |
| Paired Control | 同期配对控制 |
| Model Drift | 模型 / Provider 漂移 |
| Comparator | 基线与候选比较器 |
| Tool Selection | 工具选择准确率 |
| Argument Validity | 参数合法率 |
| Runtime Outcome Match | Runtime 结果匹配率 |
| Task Completion | 最终任务完成率 |
| Unnecessary Call Rate | 不必要调用率 |
| Supplemental Admission Set | grep 补充准入评测集 |
| External ChatGPT | 实际 ChatGPT 产品客户端 |
| Tunnel | ChatGPT 与 Harness 之间的真实传输链路 |
| BLOCKED | 被前置条件阻塞 |
| INCONCLUSIVE | 证据不足，无法得出结论 |

---

# 2. 验收前确认

请先确认：

- [ ] M03 已 ACCEPTED / FROZEN。
- [ ] M04 已 ACCEPTED / FROZEN。
- [ ] M04 正式 accepted baseline 的身份、hash 和五类指标可查。
- [ ] M05 Development Complete。
- [ ] M05-INT-01～M05-INT-18 已有真实自动化结果。
- [ ] M05-E2E-01～M05-E2E-04 已有真实结果。
- [ ] E2E-03 真实受控模型不是 scripted/mock。
- [ ] E2E-03 是在你明确授权网络/费用后执行。
- [ ] Candidate commit、Portable SHA-256、Runtime SHA-256、grep Tool schema hash 已记录。
- [ ] Windows reparse / junction / TOCTOU 必测项没有被跳过。
- [ ] 没有用真实用户 Workspace 做自动化破坏性测试。
- [ ] External ChatGPT 实际用户链路尚未被内部 MCP client 冒充。

验收信息：

~~~text
Candidate commit：
Portable SHA-256：
Runtime SHA-256：
Tunnel version/hash：
grep Tool schema hash：
M03 schema/checksum：
M04 baseline ID/hash：
M04 requested model：
M04 resolved model：
M05 candidate requested/resolved model：
M05 candidate profile hash：
M05 supplemental dataset hash：
Strict candidate scoreable runs：
Security violations：
验收人：
日期：
~~~

---

# 3. M05-MAN-01 — grep 配置、升级行为和 Tools UI 是否清楚可信

**对应：** M05-AC01、AC05、AC06、AC13  
**主要功能：** Tool Config（工具配置）、Upgrade（升级）、Restart Required（需要重启）

## 3.1 新安装

用一个全新的测试配置启动 M05。

确认：

- [ ] Tools 页面有 grep。
- [ ] grep 默认是开启状态。
- [ ] 默认 Policy 是 ALLOW。
- [ ] 描述明确表示它只搜索当前 Workspace 内的文本。
- [ ] 中文界面和英文界面都有完整文字。
- [ ] 不需要单独配置 grep 可执行文件，因为它不是外部命令。

## 3.2 升级已有配置

用一份真实格式的 pre-M05 配置升级。

确认：

- [ ] schemaVersion 仍然是 3。
- [ ] 升级后 grep 显式显示为关闭，而不是突然自动开启。
- [ ] grep 默认 Policy 显示 ALLOW，但因为 Tool 关闭所以当前不可调用。
- [ ] 旧的 Workspace / Tunnel / Proxy / Tool / Policy / SecretReference 没有丢失。
- [ ] 单纯补上 grep 字段没有制造新的 configRevision。
- [ ] 用户手工开启 grep 后才出现新的 configRevision / Restart Required。
- [ ] 重启前 Running 状态仍显示旧 Runtime 的真实有效值。
- [ ] 重启后 Running 状态与 Saved 配置收敛。

## 3.3 基本交互

- [ ] grep Enable 开关好理解。
- [ ] ALLOW / ASK / DENY 切换好理解。
- [ ] Saved 和 Running 不一致时 UI 不会假装已经生效。
- [ ] Inspector 没有暴露 raw query 或匹配文本。

**M05-MAN-01 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 4. M05-MAN-02 — grep 搜索结果是否好用、稳定、符合直觉

**对应：** M05-AC01～AC04、AC11  
**主要功能：** Literal / Regex / Scope / Filters / Line Number / Truncation

使用一个你能直观看懂的小型测试 Workspace。

建议准备：

- 多个嵌套目录；
- 同一个关键词出现在不同文件/行；
- 一份 UTF-8 中文文件；
- 一份 CRLF 文件；
- 一个很长的匹配行；
- 一份应该被 exclude 的文件。

## 4.1 Literal

执行字面搜索。

确认：

- [ ] 只返回真正包含目标文本的行。
- [ ] path 是 Workspace 相对路径。
- [ ] lineNumber 从 1 开始且肉眼核对正确。
- [ ] Preview 能看到匹配文本。
- [ ] 同一行有多个命中时只返回一条行结果。
- [ ] 返回顺序稳定。

## 4.2 Regex

执行一个简单 regex。

确认：

- [ ] regex 结果与预期一致。
- [ ] 错误 regex 给出清晰、有限的错误，而不是崩溃。
- [ ] regex 不会跨行“神奇匹配”。

## 4.3 Scope / Include / Exclude

确认：

- [ ] 相对 scope 只搜索指定子目录。
- [ ] include 能缩小文件集合。
- [ ] exclude 能排除文件。
- [ ] include 与 exclude 冲突时 exclude 生效。
- [ ] 不能输入绝对路径或 parent escape 搜到 Workspace 外。

## 4.4 截断

用小 maxResults / maxOutputBytes 触发截断。

确认：

- [ ] 明确显示结果被截断。
- [ ] 能看出截断原因。
- [ ] 同样的 Workspace 和参数重复执行，返回同样的前缀。
- [ ] 没有因为超时而把不完整随机结果伪装成正常截断。

**M05-MAN-02 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 5. M05-MAN-03 — Policy / Approval / Workspace / History 安全边界是否可信

**对应：** M05-AC07～AC12  
**主要功能：** Security（安全）、Authority（权限）、Evidence（证据）、Privacy（隐私）

## 5.1 Policy 和 Approval

把 grep 依次设置成 ALLOW / ASK / DENY。

确认：

### ALLOW

- [ ] 正常搜索成功。

### ASK

- [ ] 调用后先出现真实 Pending Approval。
- [ ] 审批之前没有先偷偷完成扫描。
- [ ] allow_once 后才得到结果。
- [ ] deny 后没有搜索结果。
- [ ] Approval 界面没有展示 raw query / matching line 这类不必要敏感内容。

### DENY

- [ ] 直接被 Runtime Policy 拒绝。
- [ ] 没有通过 Bash 或其他 Tool 偷偷执行搜索。

### Disabled

- [ ] Tool 关闭后模型无法正常调用 grep。
- [ ] 不会 fallback 到一个隐藏 shell grep。

## 5.2 Workspace 安全

查看 E2E/Integration 的 Windows 安全证据。

确认：

- [ ] ../ escape 被阻止。
- [ ] 绝对路径 escape 被阻止。
- [ ] symlink/junction/reparse 指向 Workspace 外不会被跟随。
- [ ] TOCTOU 测试把文件在检查后换到 Workspace 外时，外部 sentinel 内容没有被读到。
- [ ] outside sentinel observed = 0。
- [ ] 测试没有把“Windows 无法创建 fixture”当成 PASS。

## 5.3 Activity / History

做一次正常 grep 后查看 Activity 和 History。

确认：

- [ ] 能看到这是 grep 请求。
- [ ] runtime_id + request_id 能和真实调用对应。
- [ ] Activity/History 里没有原始搜索 query。
- [ ] 没有 include/exclude 原始 pattern。
- [ ] 没有匹配行/preview 全文。
- [ ] 没有完整物理 Workspace 路径。
- [ ] 能看到有用的安全 scalar 信息，例如 matchCount、scannedFiles、truncated。
- [ ] 重启后旧 History 仍然只是历史，不会变成新的 Approval/授权。

**M05-MAN-03 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 6. M05-MAN-04 — M04 基线没有被改题，M05 质量回归是否可信

**对应：** M05-AC14～AC18  
**主要功能：** Agent Evaluation Regression（Agent 评测回归）

## 6.1 M04 基线完整性

查看 QA 的 hash 对比。

确认：

- [ ] m04-core-v1 dataset 没改。
- [ ] 24 个 M04 case 没改。
- [ ] M04 fixture 没改。
- [ ] M04 system prompt 没改。
- [ ] M04 scorer 没改。
- [ ] M04 content_search 没为了 grep 改写 prompt。
- [ ] M04 baseline mode 仍然只有 read_file / glob / write / edit。
- [ ] baseline mode 遇到多出来的 grep 会报 catalog drift，而不是偷偷接受。

## 6.2 Strict 72-run candidate

确认真实模型批次：

- [ ] 使用 m05-grep-candidate-v1。
- [ ] 使用原样 m04-core-v1。
- [ ] Tool catalog 是 read_file / glob / grep / write / edit。
- [ ] 一共 24 cases ×3 = 72 logical runs。
- [ ] scoreable runs >=70。
- [ ] 每个 case >=2/3 可计分。
- [ ] 每个 category >=11/12 可计分。
- [ ] security violations = 0。
- [ ] 没有因为某次模型选错 Tool 就删掉那次 run。
- [ ] 没有“重跑到成功为止只保留好结果”。

## 6.3 五类回归门槛

确认 Comparator 仍是 M04 冻结值：

~~~text
Task Completion       最多退化 2 个百分点
Tool Selection        最多退化 3 个百分点
Argument Validity     最多退化 1 个百分点
Runtime Outcome Match 最多退化 1 个百分点
Unnecessary Calls     最多增加 5 个百分点
~~~

同时确认：

- [ ] Category guard 仍然存在。
- [ ] 3/3 stable-case guard 仍然存在。
- [ ] 任一安全越界不能被平均分抵消。
- [ ] 阈值不是在看完 M05 结果之后改宽。

## 6.4 Model Drift

如果 M05 resolved model 与 M04 不同：

- [ ] 先跑了完整 paired control。
- [ ] paired control 没有暴露 grep。
- [ ] paired control Tool schema 与 M04 一致。
- [ ] 使用原 compare_provider_drift。
- [ ] 超出 drift guard 时结果是 INCONCLUSIVE，而不是强行判 grep PASS/FAIL。

## 6.5 grep-admission 补充集

确认：

- [ ] 单独是 4 cases ×3 = 12 runs。
- [ ] prompt 没有直接告诉模型“必须用 grep”。
- [ ] literal / regex / scope+filter / truncation 都覆盖。
- [ ] 结果单独报告。
- [ ] 没有拿这 12 次去替换或稀释 72-run strict regression。

**M05-MAN-04 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 7. M05-MAN-05 — 真实 External ChatGPT → Tunnel → Harness → grep 最终用户链路

**对应：** M05-AC19、AC20  
**主要功能：** Final User Acceptance（最终用户验收）

这是 M05 唯一要求你亲自使用 External ChatGPT 的关键链路。

前置：

- 自动化 M05-E2E-04 已证明真实 Tunnel 外部传输链可用；
- 使用最终候选 Portable；
- 使用一个安全、可丢弃的测试 Workspace；
- 不在真实敏感工作目录做验收。

## 7.1 Tool 是否真实出现在 ChatGPT

在 External ChatGPT 中连接当前 Harness。

确认：

- [ ] ChatGPT 能看到 grep Tool。
- [ ] Tool 描述能让模型理解它是“按内容搜索 Workspace”，而 glob 是“按路径找文件”。
- [ ] read_file / glob 等现有 Tools 仍正常存在。

## 7.2 Literal 实际调用

给 ChatGPT 一个自然任务，例如：

> 在当前 Workspace 里找到包含唯一标记 TEST_ORCHID_71 的文件和行，并告诉我结果。

不要在提示词里要求“必须调用 grep”。

确认：

- [ ] ChatGPT 实际选择了 grep，或者给出的 Tool 选择有合理解释。
- [ ] 如果调用 grep，返回的文件和行号正确。
- [ ] Activity 能看到同一调用。
- [ ] History 能找到同一个 runtime_id + request_id。
- [ ] 没有 Workspace 外文件出现在结果里。

## 7.3 Regex / Filter 实际调用

再给一个需要 regex 或限定目录/文件类型才能高效完成的自然任务。

确认：

- [ ] ChatGPT 能生成合法 grep 参数。
- [ ] scope/include/exclude 的结果符合预期。
- [ ] 无需通过 Bash 才能完成。
- [ ] 错误参数时 Runtime 返回可理解的 typed error，而不是崩溃。

## 7.4 ASK / DENY 用户体验

把 grep Policy 改为 ASK。

确认：

- [ ] External ChatGPT 调用后出现正常审批。
- [ ] 你拒绝后 ChatGPT 收到真实拒绝结果。
- [ ] 你允许一次后只授权这一条真实请求。
- [ ] 不会从旧 History 自动恢复授权。

再改为 DENY：

- [ ] External ChatGPT 不能绕过 Policy。

## 7.5 隐私和 History

在搜索 query 和匹配行里放明显 sentinel。

确认：

- [ ] ChatGPT 当前 Tool result 可以看到必要的有界匹配 preview。
- [ ] Activity/History 不持久化 raw query。
- [ ] History 不持久化匹配行/preview。
- [ ] History 不显示完整物理 Workspace 路径。

## 7.6 现有产品回归

最后确认：

- [ ] read_file 仍正常。
- [ ] glob 仍正常。
- [ ] write/edit 审批语义正常。
- [ ] bash 原有安全语义正常。
- [ ] Tray / Background / Notification 没被破坏。
- [ ] Start / Stop / Restart 正常。
- [ ] Diagnostics 正常。
- [ ] History 正常。
- [ ] 没有遗留测试 Runtime / Workspace / Approval。

**M05-MAN-05 结论：** PASS / FAIL / BLOCKED / INCONCLUSIVE

---

# 8. 最终人工验收汇总

| 人工验收项 | 结论 | 备注 |
| --- | --- | --- |
| M05-MAN-01 配置 / 升级 / Tools UI |  |  |
| M05-MAN-02 搜索行为 / 结果 / 截断 |  |  |
| M05-MAN-03 Policy / Approval / Workspace / History |  |  |
| M05-MAN-04 M04 基线 / 72-run 回归 / Model Drift |  |  |
| M05-MAN-05 External ChatGPT 最终用户链路 |  |  |

自动化证据摘要：

~~~text
M03 status：
M04 status：
M05 Development：
M05 Integration：
M05 E2E：
Candidate commit：
Portable SHA-256：
Runtime SHA-256：
grep Tool schema hash：
M04 baseline ID/hash：
M05 strict logical runs：
M05 strict scoreable runs：
Tool Selection：
Argument Validity：
Runtime Outcome Match：
Task Completion：
Unnecessary Call Rate：
Security violations：
Paired-control verdict（若执行）：
Supplemental scoreable runs：
Windows outside-sentinel observed：
Cleanup：
~~~

## 最终结论

只能选择一个：

- [ ] M05 — ACCEPTED / FROZEN（验收通过 / 已冻结）
- [ ] M05 — FAIL（验收失败）
- [ ] M05 — BLOCKED（存在阻塞）
- [ ] M05 — INCONCLUSIVE（证据不足）

只有以下全部满足，才能选择 ACCEPTED / FROZEN：

- [ ] M03 已 ACCEPTED / FROZEN。
- [ ] M04 已 ACCEPTED / FROZEN。
- [ ] M05-AC01～M05-AC20 均有有效证据。
- [ ] M05-INT-01～M05-INT-18 全部 PASS。
- [ ] M05-E2E-01～M05-E2E-04 全部 PASS。
- [ ] M05-MAN-01～M05-MAN-05 全部 PASS。
- [ ] Windows reparse/junction/TOCTOU 实测 PASS。
- [ ] outside sentinel read = 0。
- [ ] 真实 controlled Model strict 72-run 已执行，不是 scripted/mock。
- [ ] compare_m05 使用冻结阈值并 PASS。
- [ ] 如有 Model drift，paired control 在 guard 内。
- [ ] grep-admission 12-run 补充集有效且单独报告。
- [ ] security violations = 0。
- [ ] exact runtime_id + request_id 关联已验证。
- [ ] raw query/filter/match preview 没有进入 durable History。
- [ ] External ChatGPT → Tunnel → Harness → Runtime → grep 真实用户链路已亲自验收。
- [ ] M03/M04/V3/M01/M02 没有未解决回归。
- [ ] 没有未解决 blocker。

人工验收完成前，Coding Agent 不得自行把 M05 标记为 ACCEPTED / FROZEN，也不得因为本地 grep 成功、scripted Eval 全绿、内部 MCP client 成功或某一次 External ChatGPT 调用成功就提前解锁 M06。
