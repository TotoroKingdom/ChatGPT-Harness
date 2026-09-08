# Control Plane Read APIs — M8-C

## Runtime vs Control Plane（运行时与控制平面）

Runtime 拥有执行、安全策略、工作区边界及原始事件；Launcher 是本地控制平面。
M8-C 只向 Launcher/未来 UI 暴露安全观察，不改变 Runtime 的授权、执行或 MCP 契约。
**M7-C credential transport 保持 BLOCKED；外部 MCP readiness 仍为 NOT ESTABLISHED。**

读取链路：

```text
UI invoke → Tauri command → RuntimeSupervisor → 既有本地 stdio IPC
→ Runtime control_read → Diagnostics / ActivityStore / EffectiveToolCatalog
```

没有新 HTTP 服务、Tunnel 连接、凭据读取、文件探测或后台订阅任务。
Runtime 所有权、PolicyEngine、ToolRegistry、WorkspaceGuard 与 ExecutionPipeline 均不变。
读取不会启动/停止 Runtime、执行工具、产生/消费 Activity 或修改配置。

## Read API boundary（只读 API 边界）

| Tauri command | 本地 IPC method | 请求 | 成功响应 |
| --- | --- | --- | --- |
| runtime_get_diagnostics | control.diagnostics.get | 无 | RuntimeDiagnosticsSnapshot |
| runtime_get_activity_snapshot | control.activity.snapshot | limit | ActivityEntry[] |
| runtime_get_effective_tools | control.effective_tools.get | 无 | ToolDiagnostic[] |

命令沿用既有同步 RuntimeSupervisor 请求链路；这是本地受信 Launcher 接口，不是
ChatGPT/MCP 工具。M8-C 不连接 React，也不生成前端页面或新增 MCP schema。

### Diagnostics（诊断）

直接读取 M8-A RuntimeCore::diagnostics()；JSON 字段及 schemaVersion=1 保持不变。
协议 DTO（数据传输对象）只描述这份冻结的安全投影，不复制 RuntimeState 或重算状态。
Launcher 解码时拒绝未知字段/枚举和不支持的 diagnostics schemaVersion。

三个接口保持既有 initialize → initialized ACK 门禁。未初始化、等待 ACK 或正在停止
时拒绝读取，Launcher 统一返回 RUNTIME_NOT_READY，不例外放宽诊断接口。
内部 M8-A 仍支持这些阶段的安全快照，但本阶段不将它们绕过 IPC 握手导出。
进程未启动、已断开或请求超时返回 RUNTIME_UNAVAILABLE，不伪造诊断快照。
同步 IPC 正在处理其他请求时不能保证立即采样；这是读取能力，不是实时性承诺。

### Activity（活动记录）

limit 必填，只接受 JSON 整数 **1..=256**。
0、负数、小数、字符串、null、缺失、超上限以及 IPC 额外参数均为 INVALID_REQUEST；
不静默截断、不将无效值转成默认值。Launcher 与 Runtime 都校验 limit。
内部 M8-B 的 limit=0/超大值容错语义不变；公开边界有更严格的输入约束。

返回最近 min(limit, 保留数量) 条，按接收顺序由旧到新排列，直接复用
RuntimeCore::activity_snapshot()，不使用旧 activity.list 的协议事件缓冲。
就绪 Runtime 无保留条目时返回 []；未就绪返回 RUNTIME_NOT_READY；断开返回
RUNTIME_UNAVAILABLE。**Empty 不等于 Ready、断连或没有发生过请求。**

### Effective Tools（有效工具）

直接读取 RuntimeCore::effective_tools() 并复用 M8-A ToolDiagnostic::from 投影。
只返回 name、declaredCapability、availability、decision；不返回 description、reason、
scope、输入 schema 或任何执行参数。availability 与 decision 沿用 Runtime 权威，
不新增工具注册表或策略推导。ASK 保持 ASK，不等于直接可执行。
非 Ready 返回 RUNTIME_NOT_READY，而不是返回空列表冒充无工具。

三个读取是独立采样，不提供跨调用事务一致性。Activity/effective-tools 为未带版本
封装的数组，其字段由当前控制协议 v1 固定；不得静默改变字段或枚举语义。

## Serialization and errors（序列化与统一错误）

共享 wire DTO 定义在 protocol/src/v1/control_read.rs；所有成功响应均有 serde
Serialize / Deserialize，Launcher 使用类型化解码而非任意 JSON 透传。
DTO 是冻结内部安全投影的传输镜像，不是新的领域权威；跨边界 round-trip 测试防止漂移。

三个 Tauri command 都返回既有 ControlError 形状：

```json
{ "code": "RUNTIME_UNAVAILABLE", "message": "Runtime is unavailable." }
```

| code | 含义 |
| --- | --- |
| RUNTIME_UNAVAILABLE | 无 Runtime 进程、通道断开、崩溃或请求超时 |
| RUNTIME_NOT_READY | 尚未初始化、等待 ACK、停止中或已有就绪权威未 Ready |
| INVALID_REQUEST | 参数类型/范围/形状无效 |
| INTERNAL_ERROR | 序列化、响应解码、版本不支持或其他内部错误 |

message 全部为固定文案，不透传 SupervisorError、serde 解析器或系统错误文本。
本地 IPC 保留既有 RpcError 信封和 MachineErrorCode：NotInitialized、RuntimeNotReady、
InvalidArgument、InternalError，Launcher 将其规范化为上表；传输不可达由 Launcher 产生。
Launcher 的 limit 校验先于进程可用性判断，无效请求在未启动时仍为 INVALID_REQUEST。
直接 IPC 调用则遵守既有握手优先顺序，未初始化时先返回 NotInitialized。
旧命令的错误行为不变，不将这一安全保证扩大到历史接口。

## Security（安全）

禁止返回 API key、secret、token、credential、raw arguments、raw tool result、
环境变量、Workspace 根目录/规范化路径、敏感完整路径、动态 metadata、自由文本错误。

Runtime 只序列化既有白名单投影，绝不序列化整个 Core、配置、AuditEvent 或执行报告。
Activity 复用 M8-B 的工具名验证、时间戳规范化和封闭枚举；Diagnostics 复用 M8-A
的身份/时间规范化及 MachineErrorCode 白名单。Effective Tools 复用同一诊断投影。
工具名仍仅来自源码控制的注册表；未来外部工具注册需要单独复审。
Launcher 拒绝额外字段，不能因未来 Runtime 误加 metadata 就自动传给 UI。

## Future UI consumption（未来 UI 消费）

- 可分别 invoke 上表三个命令；Activity 传入例如 { "limit": 50 }。
- 将错误 code 映射为本地化文案，不依赖自由文本 message 判断状态。
- 用 Diagnostics 的 runtimeId、capturedAt 识别身份和新鲜度；失败/断连/重启时
  不继续把旧快照当作当前 Ready。多次读取之间可能重启，不能假设同一采样时刻。
- null、not_observed、not_instrumented、not_established 与 false/0/[] 必须区别展示。
- 本地 Runtime Ready 不建立 Tunnel 或外部 MCP readiness；UI 不得清除 m7_c_blocked。
- Activity 是容量 256 的易失观察，既不是持久审计账本，也不是无损事件流。

## Validation（验证边界）

测试覆盖 diagnostics Ready 读取与初始化前/等待 ACK/Stopping 的拒绝、非空与空 Activity、
有效工具、上下限与畸形参数、Runtime unavailable/not ready、固定错误映射、
序列化失败、wire DTO round-trip、敏感内容不泄漏、未知字段拒绝，以及读取无副作用。
本地 stdio dispatch 测试验证协议路由；Launcher 测试验证读取适配和错误边界。
这不等于真实 WebView/UI 调用、Tunnel authentication 或外部 MCP E2E 验证。

验证命令：cargo fmt --all -- --check、cargo check --workspace、
cargo test --workspace、git diff --check。不 commit。
