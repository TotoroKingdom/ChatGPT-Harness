# M02 人工验收清单 — 启动性能与一键连接诊断

> 状态：验收基线已冻结 / 尚未执行  
> 里程碑：M02 — Startup Performance and One-click Diagnostics  
> 对应范围：F05 启动性能、F06 一键连接诊断  
> 验收人：用户 / Human Reviewer（人工验收人）  
> 前置条件：M02 Development Complete；Integration 全部 PASS；E2E 全部 PASS；使用同一 packaged candidate。  
> 本文只用于人工观察与最终签认，不替代自动化测试，不把单次启动尝试当作性能 PASS。

## 1. 验收信息

~~~text
Candidate commit:
Package/hash:
Package type:
Windows build:
Power mode:
App/Runtime/Tunnel versions:
B0 SHA and statistics:
Candidate statistics:
Workspace fixture:
Proxy/route mode:
Execution time:
Reviewer:
~~~

## 2. 性能与 Ready 正确性

### M02-MAN-01 — 启动阶段可解释且 Ready 真实

- [ ] Overview/Settings 可看到最近一次 startup attempt。
- [ ] cold/warm cohort、outcome、total duration、阶段耗时和 slowest phase 清晰可读。
- [ ] Runtime/Tunnel 进程存在但 authoritative readiness 未满足时，不显示 Ready。
- [ ] Ready 后真实 ChatGPT Tool call 可用。
- [ ] 单次 attempt 没有被显示为 benchmark PASS。

结论：PASS / FAIL / BLOCKED / INCONCLUSIVE

### M02-MAN-02 — 冻结阈值与候选比较

- [ ] B0 2 primer + 30 cold + 30 warm 已记录，失败样本进入 denominator。
- [ ] Candidate 2 primer + 30 cold + 30 warm 已记录，失败样本进入 denominator。
- [ ] 每个 cohort 至少 29 个 authoritative Ready 样本。
- [ ] nearest-rank P50/P95、failure rate、phase stats 已按 SPEC §8 计算。
- [ ] cold/warm P50、P95 tail guard、tail improvement、failure-rate guard、material bottleneck 全部满足。
- [ ] 没有为通过而修改阈值或删除有效慢样本。

结论：PASS / FAIL / BLOCKED / INCONCLUSIVE

## 3. 一键诊断

### M02-MAN-03 — 健康连接诊断

- [ ] Overview 有明显的“运行连接诊断”入口。
- [ ] 结果显示 overall、16 项必需 scope/check、四种状态、安全 evidence 和 remediation。
- [ ] Settings 显示与 Overview 相同的 aggregate snapshot。
- [ ] UNKNOWN 没有被当作 PASS。
- [ ] latest startup detail 与 diagnostics result 可区分。

结论：PASS / FAIL / BLOCKED / INCONCLUSIVE

### M02-MAN-04 — 故障与只读隐私

- [ ] 配置、Workspace、Runtime、MCP、Tunnel、control-plane、proxy、credential 故障状态分类符合 SPEC。
- [ ] 诊断过程中没有保存/重置配置、选择 Workspace、启动/停止/重启、写文件、触碰 secret value 或改变授权。
- [ ] copied summary 只含 allowlisted safe fields。
- [ ] 不含 API key、SecretStore value、物理路径、credential-bearing proxy URL、raw env/stdout/stderr 或 raw error body。
- [ ] 修复外部条件后重新运行诊断能反映当前状态，而不是旧 History。

结论：PASS / FAIL / BLOCKED / INCONCLUSIVE

## 4. M01/V3 回归与交付

### M02-MAN-05 — 桌面连续性与候选交付

- [ ] Tray/Close/Show/Stop/Restart/Quit 行为保持 M01 语义。
- [ ] 连接通知、AppConfig v3 desktop preferences、Workspace、Policy/Approval、Bash、SecretStore 回归通过。
- [ ] Portable/NSIS 差异已如实记录。
- [ ] 所有测试拥有的 Runtime/Tunnel 进程和 system temp 资源已清理。
- [ ] 候选 manifest/hash 与验收记录绑定。
- [ ] Integration/E2E/Manual 尚未通过前，没有把 M02 标记为 ACCEPTED/FROZEN。

结论：PASS / FAIL / BLOCKED / INCONCLUSIVE

## 5. 最终结论

只能选择一个：

- [ ] M02 — ACCEPTED / FROZEN
- [ ] M02 — FAIL
- [ ] M02 — BLOCKED
- [ ] M02 — INCONCLUSIVE

只有在以下条件全部满足时，才能勾选 ACCEPTED / FROZEN：

- [ ] M02-AC01～M02-AC15 均有有效证据。
- [ ] B0 先于 T2，并且 candidate 满足所有冻结性能公式。
- [ ] M02 Integration 全部 PASS。
- [ ] M02 E2E 全部 PASS。
- [ ] M02-MAN-01～M02-MAN-05 全部 PASS。
- [ ] 没有 raw sensitive payload 或未清理的测试资源。
- [ ] 没有新增第二套 lifecycle、Ready、Policy、Approval 或 credential authority。

人工验收完成前，不得由 Coding Agent 自行把 M02 标记为 ACCEPTED / FROZEN。

