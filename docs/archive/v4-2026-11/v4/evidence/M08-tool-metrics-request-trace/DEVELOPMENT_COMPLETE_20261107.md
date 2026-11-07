# M08 Development Complete — 2026-11-07

本轮按用户明确授权跳过前置里程碑验收和文档冻结门禁，完成 T1–T6 开发。SPEC / PLAN / TASKS、验收门禁脚本均未修改。本记录不是 M08 ACCEPTED / FROZEN，也不代表真实 External ChatGPT 或人工验收通过。

## 开发交付

| 任务 | 状态 | 实现与开发证据 |
| --- | --- | --- |
| T1 | COMPLETE | Protocol v1 additive DTO、TS/JSON Schema/样例；SQLite v2、v1 校验/备份/事务迁移、兼容读取及隔离回滚 fixture。 |
| T2 | COMPLETE | canonical AuditEvent → bounded TraceProjector → 同一队列/事务持久化；单调计时、解析/Workspace/Policy/Approval/Execution/terminal；容量 UNKNOWN、open span、重试幂等。 |
| T3 | COMPLETE | 30 天与精确 Tool 查询、独立终态分类、成功率分母、独立耗时样本、nearest-rank P95、20 样本门槛、覆盖质量与 2 秒查询限制。 |
| T4 | COMPLETE | Tauri 只读 metrics_query / trace_get；Activity/History Metrics 与精确请求 Trace；en/zh-CN、DOM 隐私和接口测试。 |
| T5 | COMPLETE | populated v1 多会话/审批/未完成数据迁移与恢复；保留/清理、存储降级既有回归；真实 Runtime 流水线成功、DENY、执行前失败、审批等待中停止；隐私和无授权恢复回归。 |
| T6 | COMPLETE | 最终 Rust/前端开发检查通过；50k 查询性能通过；干净提交的全新 Portable、复制散列校验及隔离 Runtime 冒烟通过。 |

实现提交 / candidate SHA：`45fb232dc58a5e3d743ee414931d68500e744315`。

产品版本：`0.1.3`。AppConfig schema：`3`；Control Protocol：`1`；EvidenceEvent logical schema：`1`；Evidence Store physical schema：`2`。

Evidence Store v2 checksum：`5035bdcaafa3659e1cc7b6eec0c6b7dfa00ee0df415971baa9288c274f1e93d8`。

v1 SQL 与 checksum 保留；迁移不生成旧请求的 trace_id/spans，不把旧 wall duration 当执行耗时。迁移前的 sibling backup 保留。回滚开发测试恢复 v1 备份并用 v1 兼容读取层核对 History；没有在真实用户数据上执行迁移/回滚，没有重启用户的活动实例。

## 最终验证

| 命令 / 验证 | 结果 |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo check --workspace | PASS |
| cargo test --workspace | PASS：738 passed / 0 failed / 27 ignored；ignored 不视为通过。 |
| cargo build -p chatgpt-harness-runtime | PASS；M03 子进程回归前构建了当前源码的 debug Runtime。 |
| cargo test -p chatgpt-harness-runtime --bin chatgpt-harness-runtime m03_qa_integration -- --ignored --test-threads=1 | PASS：8/8（int_04/06/07/08/09/10/16/17），作为 M08 的受影响开发回归。 |
| Protocol generated_artifacts_are_current / M08 fixtures | PASS，已包含于工作区测试。 |
| M08 实际 Runtime 流水线、unknown Tool dispatch、trace 生命周期/容量测试 | PASS，已包含于工作区测试。 |
| npm run typecheck | PASS |
| npm test | PASS：41 files / 385 tests。 |
| npm run build | PASS |
| 50k-request M08 performance（release） | PASS：P95 466.470 ms，0 timeout。 |
| check-repository-layout.ps1 | PASS |
| check-milestone-layout.ps1 | FAIL / 已授权跳过：M08 尚缺 INTEGRATION_TEST_PLAN.md、E2E_TEST_PLAN.md、MANUAL_ACCEPTANCE_CHECKLIST.md；没有补造文档或修改脚本。 |
| Portable 文件/版本/图标/散列 + 临时工作区 Runtime smoke | PASS，构建目录和交付副本均验证。 |
| git diff --check | PASS |

指标不信任旧汇总中对 executorStarted 的宽泛推断，只认明确 ExecutionStarted 事件；DENY、cancel、timeout、审批终态、执行前失败、incomplete/unknown 不进入 successful / (successful + failed) 的分母。React 只展示 DAL 统计结果。History/Trace 不恢复 Pending/Grant/Permit，查询不调用 Policy/执行工具。

## 性能环境与记录

Windows 10 专业工作站版 `10.0.19045`，x86_64，Intel Core i7-12700，系统报告物理内存 `68497039360` bytes；Rust `1.98.0`，Node `v24.18.0`。

复现：

```powershell
cargo test -p chatgpt-harness-evidence-store --release --test m08_performance -- --ignored --nocapture
```

临时数据库包含 50,000 request_summary / 250,000 evidence_events / 200,000 request_spans。6 个固定的 24h/7d/30d × All/精确 Tool 组合预热后，执行 20 次查询；benchmark P95 同样使用 nearest-rank。只读连接已断言，查询计划使用 summary/span 索引及新增的 evidence_events(runtime_id, request_id, stage) covering index。

Fixture SHA-256：`bd38c7ba49cf03e0f351d730df465ecb35a1ac1e27ecfae63f8ceaa6e33c69b5`。逐查询耗时、查询计划见同目录 `PERFORMANCE_20261107.txt`。

## Portable 候选

路径：`artifacts/V4-M08-tool-metrics-request-trace/20261107-002118-d3820197/ChatGPT-Harness_0.1.3_x64-portable/`。

入口：`chatgpt-harness-ui.exe`。构建清单 source_dirty：`false`；source_state_sha256：`AAB02FF59BD73D8047D521EB0B47EF9264C36AD8281F52147FEE4E61D80C454C`。

| 文件 | SHA-256 |
| --- | --- |
| UI | `89BC1E82280297DA1F0FAFF2D02F82058609055018203A24D3F46FB5A9780A4C` |
| Runtime | `D5E76A61FA8287289D0783B3B058FCD727869131B97EC1ABBE39B8308A821D60` |
| Tunnel client | `83F08FB39B1C154747DEBD31B81B65DD4EE834CACF5A073B6301B2688699BC76` |
| cloudflared | `CD0BC49AEC24D0D62A37B6E842DAD46000F7B401F6167B4CBACE4E170819C9A2` |
| 候选根目录 delivery.json（目录级文件散列清单） | `413EE2AF4A80B0C4F94E83A92FEC9682C05890A8FEC6FBBB6446C2269B9AB686` |

按本轮授权，本次在内存中调用既有 build-portable.ps1 的构建体，仅跳过其 milestone-layout 前置调用并绑定原脚本目录；没有修改仓库脚本。其 stage-distribution、Tauri custom-protocol 构建、来源清单及 verify-portable 流程保留。交付复制后逐文件对比散列，并再次执行 verify-portable.ps1。后续本记录的独立文档提交不改变候选来源 SHA。

## 中间失败与遗留边界

- 在严格改用 ExecutionStarted 事件后，性能初测 P95 923.207 ms（FAIL）；新增 covering index 后同一 fixture/query 组合复测为 466.470 ms（PASS），没有调整阈值、样本数或查询组合。
- 初次前端全测遇到既有 m8Tools.availability 翻译缺项及 App restart 等待超时；补齐翻译，保留原 App 测试断言和超时，随后全量 385/385 通过。
- v1 OfflineMaintenance 打开测试曾检测到 journal_mode 改写文件头；改为校验及 connection-local FK/busy 配置，保留字节/数据/schema 断言，最终通过。App 预检查保持只读，正常迁移只归 Runtime writer。
- Rust 仍有既有 unused/dead-code、ts-rs serde attribute 警告；Vite 有大于 500 KiB chunk 提示。构建通过，未把这些警告写成失败或验收通过。
- M08 Formal Integration：NOT EXECUTED。
- M08 Formal packaged E2E：NOT EXECUTED；Portable smoke 不替代该验证。
- Real External ChatGPT：NOT EXECUTED。
- Manual Acceptance：NOT EXECUTED。
- M08 ACCEPTED / FROZEN：未执行、未宣称。
