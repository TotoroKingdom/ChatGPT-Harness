# M6-B Request State Machine + Enforcement Pipeline

## 实现与边界

M6-B 将 Runtime 的已验证请求统一接入内部 enforcement（强制执行）管线，顺序固定为：

`Validation → Workspace Boundary Validation → Policy Evaluation → Executor`

MCP 入口和 `RuntimeCore` typed（类型化）入口共享同一个 `ExecutionPipeline`、同一个
`WorkspaceGuard` 和同一个 `ToolRegistry`。本阶段不改变协议、MCP 方法、参数 schema（模式）、
工具目录或 UI。

`WorkspaceGuard` 仍是独立的 filesystem boundary（文件系统边界）安全组件；它不并入
`PolicyEngine`。两者都是 enforcement layer，但职责不同：

- `WorkspaceGuard` 只验证目标是否位于已初始化工作区边界内，并拒绝不安全路径形式。
- `PolicyEngine` 只产生 authorization decision（授权决策）：`DENY`、`ASK` 或 `ALLOW`。
- `Executor` 只在前两层成功后运行；既有 executor 的 guard 检查仍保留，因此前置验证不是
  TOCTOU（检查与使用之间竞态）修复的声明。

M6-B 不实现 approval manager（批准管理器）、自动批准、Allow Once、可恢复等待、audit storage
（审计存储）、新工具或有效工具目录推导。`ASK` 在本次调用中立即结束为
`AwaitingApproval`，返回 `APPROVAL_REQUIRED`，没有可继续执行的 token 或后台等待请求。

## Request State Machine（请求状态机）

生命周期是 Runtime 内部类型，不是 wire（线协议）字段：

`Received → Validating → WorkspaceValidating → PolicyEvaluating`

之后只能进入：

- `Denied →` 终止
- `AwaitingApproval →` 终止（M6-B 不提供恢复）
- `Executing → Completed` 或 `Failed`
- 任一验证/执行失败直接进入 `Failed`

所有非法迁移都会被拒绝且不改变当前状态；终态不能恢复。每次请求拥有新的内部
`request_id`，hook 事件使用同一关联 ID。

## Enforcement Pipeline（强制执行管线）

1. `Received`：创建只含安全元数据的请求上下文。
2. `Validating`：解析并校验 typed request；无效参数或 glob pattern 在文件系统和策略前失败。
3. `WorkspaceValidating`：调用共享 `WorkspaceGuard`；成功后才把 `active_workspace` 写入策略上下文。
4. `PolicyEvaluating`：对固定的不可变 `PolicyEngine` 快照求值；DENY/ASK 不调用 executor。
5. `Executing`：重新检查 Runtime readiness（就绪状态），创建内部执行许可并调用既有 executor。
6. `Completed` / `Failed`：返回现有结果 envelope（结果封装）或集中映射的错误。

Runtime 未初始化、未收到成功 ACK、停止或回滚后的请求会在执行管线入口被拒绝。`ASK` 和
`DENY` 都不会触发执行。

## PolicyEngine（策略引擎）

策略引擎只消费 `PolicyContext`（工具名、安全模式和已验证逻辑 scope），不读取路径、原始
arguments、秘密或输出。决策优先级为 `DENY > ASK > ALLOW`；无效/空快照 fail closed（默认拒绝）。
当前生产管线使用固定 system baseline（系统基线）：工作区内 WorkspaceRead 为 ALLOW，工作区
外为 DENY；M6-B 不增加公开的运行时策略修改接口。

策略引擎复用 ToolRegistry 的注册项，不维护第二份工具清单。`policies.list`、`tools.list` 和
工具描述符中的 `effective_policy` 仍不是实时策略求值结果。

## Error / Hook 边界

`DENY` 映射为 `POLICY_DENIED`，`ASK` 映射为 `APPROVAL_REQUIRED`，执行阶段未分类错误映射为
`EXECUTION_FAILED`；既有 read/glob 错误码和 `{code,message}` 结构保持不变。

M6-B 提供内部 hook seam（钩子接缝）用于后续 M6-C/测试。hook 只收到 request ID、工具名、安全
模式、生命周期状态和决策等安全元数据，不收到路径、原始参数、秘密、文件内容或完整输出；本阶段
不持久化审计记录。

## 非目标

不修改 `docs-ai/ARCHITECTURE.md`、协议定义、schema、客户端/UI、PolicyEngine 的职责、
WorkspaceGuard 的独立性或现有 M5 工具算法。外部集成、HTTP E2E、tunnel（隧道）和动态
Windows symlink/junction（符号链接/联接）能力不在本阶段的完成声明内。
