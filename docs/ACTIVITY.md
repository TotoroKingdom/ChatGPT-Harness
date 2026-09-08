# Activity Read Model — M8-B

## 定位与边界

Activity 是 Runtime 内部的只读观察模型，用于未来 Activity 页面、Diagnostics
（诊断）页面和 Debug（调试）。不是 Security decision engine（安全决策引擎）、
Policy engine（策略引擎）或 Execution pipeline（执行流水线）。

M7-C tunnel credential（隧道凭据传输）继续保持 **BLOCKED**。
本阶段不增加 UI、IPC/RPC 方法、MCP/Tool schema（模式）、数据库、文件持久化、
远程日志或后台订阅任务，不修改 M8-A Diagnostics 契约。

## AuditEvent 与 ActivityEntry

- **AuditEvent**：现有 M6 结构化审计事件，记录单次请求已有的生命周期、策略评估
  和结果证据；仍保留在原有 ExecutionReport 中。
- **ActivityEntry**：单个 AuditEvent 的数据最小化展示投影，不重新生成事件 ID，
  不重新推断状态、不重新评估策略，也不是完整或持久审计记录。
- **RuntimeDiagnosticsSnapshot**：M8-A 的当前状态观察；Activity 是有界历史观察。
  两者各自读取已有事实，不互相驱动，不保证两次独立读取处于同一时刻。
- **协议 ActivityEvent / EventBus**：已有控制协议事件及缓冲，与这里的内部模型
  不同。本阶段不替换、合并或修改它们。

### 字段契约

Rust 字段使用 snake_case；序列化字段使用 camelCase，事件/分类/状态/结果枚举
使用其现有 snake_case 表达；PolicyDecision 复用已有序列化定义。

| 字段 | 类型 / 语义 |
| --- | --- |
| event_id | 原 AuditEvent UUID；用于事件标识 |
| request_id | 原请求 UUID；用于关联同一请求的多个事件 |
| timestamp | 可选 UTC RFC3339 毫秒时间；有效值规范化，非法值为 null |
| category | request / workspace / policy / execution / approval |
| event_type | 原 AuditEventType，保留事件的精确含义 |
| lifecycle_state | 原 AuditLifecycleState，不根据分类补造状态 |
| tool_name | 已有 ToolRegistry 中的工具标识；未知名称为 null |
| decision_summary | 原可选 PolicyDecision；null 表示未评估，绝不等于 ALLOW |
| outcome_summary | 原 AuditOutcome；不含工具返回值或错误消息 |

分类映射：request_received → request；workspace_validated → workspace；
policy_evaluated / request_denied → policy；execution_started / completed /
failed → execution；approval_required → approval。

摘要采用结构化枚举而不是任意字符串。未来 UI 可将枚举本地化为文案，但不得
将观察摘要用于授权。例如 workspace_validated 的 success 仅代表该阶段成功，
不是整个请求执行成功；approval_required 也不代表批准或已执行。

## 数据归属与接入

RuntimeCore 创建并拥有一个 ActivityStore，通过 Arc 共享给其初始化的唯一
ExecutionPipeline。Typed Runtime 调用和 MCP 上下文使用同一流水线、同一存储。

现有 ExecutionPipeline::emit() 生成 AuditEvent 后，向内部 AuditSink 借用传递
**同一个事件**，然后照常加入 ExecutionReport。接收器只投影已有事件；不通过
ExecutionHook 重建事件，也不在各个执行阶段重新埋点。AuditSink 不返回决策或
执行错误，Activity 不反向调用执行器、WorkspaceGuard 或 PolicyEngine。

## 内存存储与查询

内部 API：RuntimeCore::activity_snapshot(limit: usize) → Vec<ActivityEntry>。

- 固定容量 **256 条**，在 RuntimeCore 生命周期内不变。
- 与既有 IPC EventBus 的 max_activity_events 配置无关，不改变该配置语义。
- Mutex + VecDeque；插入与快照复制均在锁内完成，满容量时先淘汰最旧条目。
- 返回最近 min(limit, 当前数量) 条，**按接收顺序由旧到新**排列。
- limit = 0 返回空；超大 limit 仍最多复制 256 条。
- 顺序以取得存储锁并插入为准，不按墙钟时间或随机 UUID 排序。
  相同时间戳、时钟回拨不重排已有事件。并发请求可交错；同一请求保留发射顺序。
  没有新写入时重复查询一致；不承诺不同并发运行之间有相同调度顺序。
- 返回脱离存储的副本，调用者修改副本不会修改 Runtime 数据。
- 观察锁中毒时恢复内存访问，不将其转成授权/执行错误；这不是安全状态恢复规则。
- 初始为空；stop、初始化回滚和同一 Core 重新初始化不清除已有条目。
  新 RuntimeCore 从空存储开始；对象释放/进程退出后不保留历史。
- 只保存 ActivityEntry，不累积原始 AuditEvent、请求参数或执行报告。

这不是持久日志、无损流或安全审计账本。FIFO 淘汰可能截断一个请求的历史，
没有游标、分页、去重、缺口检测或持久恢复承诺。

## 脱敏与安全边界

采用封闭字段投影，而非将任意 JSON 做关键字替换后透传：

- 禁止 raw arguments（原始参数）、tool result（工具结果）、API key、token、
  secret、credential、environment（环境变量）及完整敏感路径。
- 不复制 workspace_id、matched_rule_ids、策略原因、错误文本或自由文本 metadata。
- 工具名称必须精确匹配已有注册表标识；这只是元数据校验，不查询工具是否获准。
- timestamp 只解析最多 64 字节的 RFC3339 输入，重新输出 UTC；异常输入为 null，
  不回显输入或解析错误。UUID 和封闭枚举直接复用。
- 不访问 SecretStore、环境变量或文件系统来构造投影。
- 新增可展示字段必须重新审查分类和脱敏，不能自动序列化整个审计事件。

## 当前覆盖与未来 UI 使用

当前仅消费 M6 已发射的八类 AuditEvent。不额外产生 runtime/tunnel 生命周期、
配置变更或 Diagnostics 事件。既有发射缺口仍保留：例如参数校验失败可能只有
request_received，workspace 拒绝记录 workspace_validated + failed，而不会补造
policy DENY。Activity 不能用来推断活跃请求计数或完整终态。

未来受信控制平面可增加专门只读适配器，读取安全快照并按 event_id 标识行、
按 request_id 关联请求。空列表只表示无保留观察，不代表 Ready、断连、没有请求，
或没有失败；当前状态仍由 Diagnostics/Runtime 权威状态提供。
本阶段没有把这些数据暴露给 React、MCP 或已有 IPC activity 方法。

## 验证覆盖

- 所有八类事件投影、原 ID/生命周期/决策/结果保持。
- 自由文本污染、敏感路径、非法/过长时间戳不泄漏，序列化字段集合固定。
- FIFO 淘汰、0/超大查询限制、相同/回拨时间戳排序和快照隔离。
- 并发读写、容量与生产者内顺序、观察锁中毒恢复。
- Runtime 与 MCP 共用接收器；ALLOW/DENY/ASK 分支与原审计事件完全对应。
- 真实本地文件执行结果和 workspace 拒绝路径不进入快照；Diagnostics 保持不变。

不以这些离线/本地测试宣称 UI、远程 MCP 或 tunnel credential 集成已验证。
