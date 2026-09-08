## Architecture Decision

### 1. V1 总体拓扑

```text
ChatGPT
   │
   │ external MCP connection
   ▼
Tunnel Client
   │
   │ authenticated tunnel transport
   ▼
127.0.0.1:<runtime-port>/mcp
   │
   ▼
Harness Runtime MCP Service
   │
   └─ existing M6 execution boundary
      Validation → WorkspaceGuard → PolicyEngine → Executor → AuditEvent
```

V1 冻结以下原则：

- Runtime 与 tunnel-client 是两个独立进程、两个独立状态域、两个独立故障边界。
- Runtime MCP endpoint 只监听本机 loopback 地址；Tunnel Client 是唯一允许被 Harness 编排的外部传输组件。
- Tunnel 只负责把外部连接转发到 Runtime 的 MCP endpoint，不获得工具目录、WorkspaceGuard、PolicyEngine 或 Executor 的控制权。
- 所有来自 ChatGPT 的 MCP tool request 仍然进入现有 Runtime 安全执行链；Tunnel 不构成安全策略层，也不能绕过 M6。
- `Ready` 是 Launcher/Control Plane 根据 Runtime、Tunnel、MCP 三个独立检查合成的产品状态，不是 Runtime 单独声明的状态。
- 本设计不修改 Runtime execution pipeline、MCP schema、Policy semantics 或现有 UI/Control Protocol contract。

### 2. 生命周期模型候选分析

#### A. Launcher/Control Plane 启动 tunnel-client

优点：

- 与现有架构一致：Tauri backend 是 Config Authority、SecretStore Authority、Runtime Process Supervisor 和生命周期编排者。
- Runtime 保持独立核心进程，不依赖 UI 才能执行本地 MCP 工作。
- Tunnel 与 Runtime 可以分别检测、启动、停止和观察，故障状态不会被合并。
- API Key 读取和进程凭据注入可以限制在 Launcher/SecretStore 边界内。
- Shutdown 顺序可由一个编排者统一保证。

缺点：

- Launcher 需要维护 Tunnel Process Manager、进程身份和健康检查。
- Launcher 必须处理 Runtime endpoint 的动态端口和 tunnel-client 参数适配。
- UI 退出或 Launcher 崩溃时，需要定义已启动子进程的清理策略。

#### B. Runtime spawn tunnel-client

优点：

- Runtime 看起来可以提供一个单一的启动入口。
- Runtime 可以直接获得 MCP endpoint 信息。

缺点：

- 混合 Runtime 的安全/执行职责与外部网络传输职责。
- Runtime 必须读取或间接处理 API Key，扩大 Runtime secret exposure 面。
- Runtime 会依赖 tunnel-client 的进程、参数和重连行为，破坏 Runtime 独立性。
- Runtime crash 时容易留下 tunnel-client，形成外部连接仍存活而本地安全核心已失效的风险。
- 需要把 tunnel-client 状态塞入 Runtime lifecycle，容易改变现有 RuntimeState 和 control protocol 语义。

#### C. 独立 service manager

优点：

- 可以脱离 UI 持续管理 tunnel-client。
- 适合未来系统级后台服务、跨用户会话或多实例场景。

缺点：

- 引入新的常驻权限主体、安装/升级/卸载和进程发现问题。
- V1 需要额外定义 service manager 与 UI、Runtime、SecretStore 的信任边界。
- 会增加安装脚本、服务注册、权限提升和故障恢复范围，与本阶段非目标冲突。
- 不适合当前 Windows-first、单用户、单 Harness 实例的 V1 模型。

### 3. V1 决策

**采用 A：由 Tauri backend 内的 Launcher/Control Plane 负责编排 tunnel-client。**

这里的 Launcher 指本地控制平面的后端进程监督/编排职责，不是 React frontend 自己启动进程。React UI 只发起产品级 Start/Stop/Diagnostics 操作并展示观察到的状态。

V1 不创建独立 service manager，也不允许 Runtime 自行 spawn tunnel-client。

### 4. 启动事务边界

`Start Harness` 是一个产品级编排动作，但每一步仍保留独立状态：

```text
Validate desired config and secret reference
        ↓
Locate and validate Runtime executable
        ↓
Start Runtime
        ↓
Wait Runtime Ready and obtain its loopback MCP endpoint
        ↓
Locate and validate tunnel-client
        ↓
Resolve API Key through SecretStore
        ↓
Start tunnel-client against the Runtime endpoint
        ↓
Wait Tunnel Healthy
        ↓
Verify MCP reachability and expected EffectiveToolCatalog
        ↓
Composite Harness Ready
```

任一阶段失败都不能显示 `Ready`。在尚未进入 `Ready` 前发生的失败，Launcher 应回滚本次已启动的子进程，避免留下半启动的 Harness。

## Component Boundary

### 1. Runtime 负责什么

Runtime 是本地执行和安全权威，负责：

- MCP server/service 及其 loopback endpoint。
- MCP request 的协议处理、参数验证和错误映射。
- `ToolRegistry` 与 `EffectiveToolCatalog`。
- `WorkspaceGuard` 和 Workspace 边界强制执行。
- `PolicyEngine`、现有 M6 `ExecutionPipeline` 和 hard deny 语义。
- Executor、结构化 ToolResult 与既有审计事件发射。
- Runtime 自身 lifecycle、Runtime identity、配置快照和 Runtime readiness。
- MCP endpoint 的绑定、监听、readiness gate 和 graceful shutdown。
- 只接受由现有 MCP boundary 定义的请求；不因为请求来自 tunnel 而降低校验或授权要求。

Runtime 不负责：

- 发现、安装、升级或重启 tunnel-client。
- 读取、保存或删除 API Key。
- 建立外部网络连接、处理 tunnel provider authentication 或实现 reconnect policy。
- 判断 ChatGPT 用户身份或替代 tunnel 的传输认证。
- 把 tunnel 状态写入自身安全决策或修改工具目录。

### 2. Launcher/Control Plane 负责什么

Launcher/Control Plane 负责：

- 读取已保存的非敏感 AppConfig。
- 通过 SecretStore reference 获取 API Key，不把 secret 交给 Runtime。
- tunnel-client discovery、版本/可执行文件检查和进程身份管理。
- 启动、观察、停止 tunnel-client。
- 将 Runtime 启动后产生的实际 loopback MCP endpoint 传给 tunnel-client。
- 观察 Runtime、Tunnel、MCP 三个状态域并合成 `Harness Ready`。
- 实施启动回滚、停止顺序、有限重试和 stale process 清理。
- 向 UI 提供安全的状态摘要和可操作的诊断结果。

Launcher 不负责：

- 复制或实现 Runtime 的 PolicyEngine、WorkspaceGuard、ToolRegistry 或 EffectiveToolCatalog。
- 在 tunnel-client 前后执行另一套工具授权。
- 解析 MCP tool arguments 来代替 Runtime policy。
- 将 API Key 写入命令行、普通配置、Activity、审计事件或常规日志。

### 3. tunnel-client 负责什么

tunnel-client 是外部传输适配器，负责：

- 与 tunnel provider 建立外部连接。
- tunnel provider 侧的认证传输。
- 将外部 MCP 流量转发到 Launcher 指定的本机 Runtime MCP endpoint。
- 按其自身协议进行连接保活和 reconnect。
- 返回进程退出码、健康结果和 provider/network 错误。

`tunnel-client` 不负责：

- 工具注册、工具过滤或 Tool Catalog 决策。
- Workspace 路径解析或本地文件访问。
- PolicyEngine、Approval Manager、Executor 或 AuditEvent。
- 修改 Runtime 配置、Workspace 或安全模式。
- 暴露 Runtime 的任意非 MCP control endpoint。

### 4. 三个边界的信任关系

- Launcher 信任 Runtime 提供的已验证 loopback endpoint 信息，但不把 Runtime 的 PID 单独当作安全身份；必须结合 Runtime instance/config identity。
- Runtime 不信任 tunnel-client 作为授权主体；来自 tunnel 的请求与其他 MCP 请求使用同一 Runtime 安全执行链。
- tunnel-client 不应获得 Runtime source config 或 secret store 数据；Launcher 只向它提供完成连接所需的最小运行时输入。

## Lifecycle

### 1. 状态归属

#### Runtime 状态

RuntimeState 继续只表示 Runtime 自身的 ephemeral observed state，包括：

- Runtime instance identity 和 process lifecycle。
- 当前初始化的 `RuntimeConfigSnapshot` revision。
- active Workspace/security mode 的运行时快照。
- Runtime MCP service 是否已启动及其 readiness gate。
- 当前 EffectiveToolCatalog、活动请求和既有 Runtime error summary。

`RuntimeState` **不新增 Tunnel 状态字段**。这样可以保持 M0 RuntimeState/Control Protocol 的边界，并避免把外部传输故障误认为 Runtime 安全状态。

#### Launcher 状态

Launcher/Control Plane 负责保存和观察：

- Runtime process state：未启动、启动中、运行中、崩溃、停止中、已停止。
- Tunnel process state：`Disconnected`、`Starting`、`Connected`、`Stopping`、`Failed`。
- MCP verification state：未验证、验证中、可达、不可达或 Tool Catalog 不匹配。
- `Harness` composite state：只有 Runtime Ready、Tunnel Connected、MCP verified 三者同时满足时才是 `Ready`。

因此：Tunnel `Connected` 不等于 Runtime `Ready`，Runtime `Ready` 也不等于 Harness `Ready`。

### 2. Start Harness

启动时严格采用以下顺序：

1. 验证 Workspace、Tunnel ID、tunnel-client 路径、API Key reference 和其他必需配置。
2. 验证 Runtime executable 和 process identity。
3. 启动 Runtime，发送已有 `initialize`，等待 Runtime MCP endpoint ready。
4. 从 Runtime/Launcher 内部控制面取得本次运行的实际 loopback endpoint；不通过 stderr、临时文件或日志传递。
5. 由 Launcher 启动唯一受管的 tunnel-client，并将 endpoint 作为非 secret 运行参数传入。
6. 等待 tunnel-client 的健康/连接结果。
7. 通过 tunnel 路径验证 MCP reachability，并核对 Runtime 的 `EffectiveToolCatalog`。
8. 只有全部成功，Launcher 才报告产品级 `Ready`。

MCP endpoint 使用当前 M5-D2/M5-D3 已冻结的 loopback 安全边界；M7 不改变 `/mcp`、Host/Origin 校验或请求大小限制。

### 3. Stop Harness

V1 冻结以下关闭顺序：

```text
停止接受新的产品级启动/连接操作
        ↓
请求 tunnel-client graceful stop
        ↓
等待 tunnel-client 退出；超时则执行受控终止和身份确认
        ↓
发送 Runtime stop
        ↓
Runtime 先关闭 MCP readiness / endpoint，再完成自身 graceful shutdown
        ↓
等待 Runtime 进程退出并清理 process metadata
        ↓
Harness Stopped
```

ChatGPT 的外部连接不由 Runtime 主动管理；关闭 tunnel-client 后，外部连接自然断开。MCP tools 不是独立进程，属于 Runtime MCP service 的一部分；Runtime 关闭 readiness 和 endpoint 即表示工具入口已关闭。

如果 tunnel-client 已经崩溃，Launcher 仍按“Tunnel 已断开 → Runtime stop”的逻辑继续清理，不能因为找不到 tunnel PID 就跳过 Runtime shutdown。

### 4. Workspace 切换

V1 不支持运行中 in-place Workspace 切换。

Workspace 是 Runtime 初始化快照的一部分，并由 `WorkspaceGuard` 绑定。切换 Workspace 时：

1. 标记 desired config 与 active Runtime snapshot 不一致，显示 `Restart Required`。
2. 按 Stop Harness 顺序停止 tunnel-client 和 Runtime。
3. 用新 Workspace 重新初始化 Runtime，创建新的 WorkspaceGuard、ExecutionPipeline 和 EffectiveToolCatalog。
4. 获取新的 Runtime MCP endpoint。
5. 重启 tunnel-client 并重新验证 MCP/Tool Catalog。
6. 全部成功后才恢复 `Ready`。

因此：

- **需要 restart Runtime：是。**
- **需要 restart tunnel-client：是，作为新 Runtime endpoint 的绑定更新和旧连接清理的一部分。**
- **需要 refresh catalog：是，在新 Runtime 成功初始化后重新读取；不是由 Tunnel 完成。**
- 不允许只刷新 UI、只刷新 Tunnel 或复用旧 Runtime 的 WorkspaceGuard。

### 5. 多实例限制

V1 冻结为单 Harness 会话模型：

- 一个 App 安装/Launcher 同时只允许一个 active Runtime 与一个 managed tunnel-client。
- 一个 Runtime 只允许一个 MCP endpoint 和一个受管 tunnel binding。
- 多个 Runtime **不得绑定同一个 `tunnel-id`**；否则 provider 侧路由和请求归属不确定，可能导致请求进入错误 Workspace。
- 多个 tunnel-client **不得连接同一个 Runtime MCP endpoint**；V1 不支持多连接汇聚、重复外部身份或连接级流量仲裁。
- 不支持 Launcher 之外的 unmanaged tunnel-client 作为 V1 受支持配置。
- 若未来跨机器运行多个 Harness 实例，每个实例必须使用不同的 tunnel identity，并保持各自独立的 Workspace、secret reference 和 Runtime endpoint。

## Config Model

### 1. 配置原则

`TunnelConfig` 是 persistent desired state，不是 RuntimeState，也不是 process metadata。它只能描述“希望如何连接”，不能记录本次运行的 secret 或动态 endpoint。

建议的概念模型：

```text
TunnelConfig
├─ tunnelId: string
├─ apiKeyRef: SecretRef
├─ tunnelClientPath: optional local path
├─ endpoint: optional provider/control endpoint
└─ runtimeBinding
   ├─ host: 127.0.0.1
   ├─ path: /mcp
   └─ portPolicy: ephemeral
```

说明：

- `tunnelId`：非 secret 的 tunnel identity；必须按 provider 约束校验。
- `apiKeyRef`：指向 SecretStore 的 opaque reference，例如已有配置模型中的 `{ provider, key }`；不包含 API Key value。
- `tunnelClientPath`：本机 tunnel-client 安装/程序目录或已验证路径，属于机器配置，不进入 Git。
- `endpoint`：可选的 provider/control endpoint 或环境选择；不得默认为把 API Key 写入 URL。若 provider 没有可配置 endpoint，可省略。
- `runtimeBinding`：只允许本机 MCP binding。V1 默认固定 loopback host、`/mcp` 路径和 ephemeral port policy。

### 2. 动态运行信息

以下信息属于 Launcher process state 或 Runtime observed state，不写入 `TunnelConfig`：

- 当前 tunnel-client PID、process start time、executable identity 和 exit code。
- 本次 Runtime instance ID。
- Runtime 实际分配的 ephemeral port。
- 本次解析出的完整 loopback endpoint。
- Tunnel provider session ID、连接时间、重连次数和瞬时网络状态。
- 最后一次健康检查结果。

动态 endpoint 必须通过受信任的 Launcher ↔ Runtime 内部协调获得。M7 不通过修改 M0 Control Protocol schema、stderr、临时文件或日志暴露它。

### 3. API Key 生命周期

#### 输入

- 用户通过现有 UI 设置流程输入 API Key。
- Tauri backend 接收输入并直接交给 SecretStore adapter。
- React frontend、Runtime 和普通配置序列化模型不持有长期 secret value。

#### 保存

- V1 使用 **OS credential store（操作系统凭据存储）**，Windows 优先使用当前用户可用的 OS-protected credential facility。
- AppConfig 只保存 `apiKeyRef`。
- 保存、替换和删除都以 SecretStore 操作的成功结果为准；配置不能伪造一个不存在的 reference。
- Secret value 不写入 Git、repository、普通 config、crash dump、截图或 process metadata。

#### 使用

- Start Harness 时，Launcher 根据 `apiKeyRef` 进行一次受控读取。
- API Key 只在 tunnel-client 启动/认证所需的最短生命周期内存在于 Launcher 的内存或 provider 支持的安全输入通道。
- 禁止将 API Key 放入命令行参数、URL、普通环境诊断输出、stdout/stderr、Activity、AuditEvent 或 Runtime source config。
- tunnel-client 所需的具体 secret injection channel 必须使用其官方支持的非持久化方式；若该客户端只支持明文命令行或明文文件，则 V1 不启动该连接。

#### 删除

- 用户删除连接配置时，先删除 SecretStore 中对应 secret，再删除/清理 `apiKeyRef`；失败时必须保留可诊断的非 secret 错误，不报告 secret value。
- 用户只替换 API Key 时，旧 secret 必须被覆盖或删除，旧 reference 不得继续被启动流程使用。
- 停止 Harness 不删除 API Key；Stop 只停止运行中的连接。显式删除/清除凭据才触发 SecretStore 删除。

### 4. 为什么不使用其他方案

- **Plain local config（明文本地配置）**：拒绝。容易被 Git、备份、诊断、同步工具或普通文件读取意外暴露，不满足现有产品的 secret isolation 要求。
- **Local encrypted config（本地加密配置）**：V1 不选作主方案。它需要额外的密钥封装、密钥轮换、恢复和权限模型；若加密密钥仍由本机普通配置保护，实际安全边界不如 OS credential store 清晰。
- **OS credential store**：V1 推荐。使用操作系统用户边界和已有安全存储能力，配置文件只保存 opaque reference，且与 Runtime source/config schema 分离。

## Security Model

### 1. 外部访问不改变 Runtime 安全边界

Tunnel 只是连接路径：

```text
external MCP request
        ↓
Tunnel transport
        ↓
Runtime MCP endpoint
        ↓
MCP validation
        ↓
WorkspaceGuard
        ↓
PolicyEngine
        ↓
Executor
        ↓
ToolResult + AuditEvent
```

因此：

- Tunnel authentication 不能替代 Runtime authorization。
- tunnel-client 连接成功不能证明某个 tool request 被允许。
- Runtime 仍必须执行现有 M6 的 validation、WorkspaceGuard、PolicyEngine、Executor 和审计边界。
- MCP endpoint 继续保持 loopback-only 绑定和既有 Host/Origin/request-limit 防护。
- 不因为流量来自受管 tunnel-client 就允许新的工具、写操作、shell 或跨 Workspace 访问。

### 2. Secret boundary

API Key 只存在于：

- 用户输入的短暂 UI/backend 处理阶段；
- OS credential store；
- Launcher 为 tunnel-client 创建认证会话所需的最短运行时范围。

API Key 禁止出现在：

- Git、提交、仓库文件和配置示例以外的 repository data；
- Runtime source config、`RuntimeConfigSnapshot`、`RuntimeState`；
- tunnel config 的序列化内容；
- 命令行、URL、普通环境快照、stdout/stderr 和进程日志；
- `Activity`、`AuditEvent`、结构化错误、崩溃文本和截图；
- UI 状态、诊断摘要或 MCP tool result。

错误信息只能使用稳定的非敏感分类，例如 `API_KEY_MISSING`、`API_KEY_INVALID`、`TUNNEL_AUTH_FAILED`，并带可操作 remediation；不得回显 provider 原始 secret-bearing message。

### 3. Endpoint 与进程身份

- Runtime MCP endpoint 只对 loopback 开放，不能绑定 `0.0.0.0` 或未经授权的 LAN 地址。
- Launcher 必须同时验证 process liveness、Runtime instance identity、executable identity/version、启动时间和当前配置 revision；不能只依赖 PID。
- Tunnel-client 启动参数中的 endpoint 必须来自当前 Runtime instance，不能复用旧 session、旧 port 或历史日志中的地址。
- 一个 tunnel-id 与一个 Runtime/Workspace identity 建立一对一绑定；发现不一致时 fail closed。
- tunnel-client crash、Runtime crash、MCP verification failure 或配置不一致都使产品级 `Ready` 失效。

### 4. 可观测性边界

V1 可以记录以下非敏感 operational metadata：

- Runtime/Tunnel/MCP 生命周期状态变化；
- 组件 instance/process identity；
- 连接是否成功、失败分类、重试次数和时间戳；
- diagnostics check ID、结果和 remediation；
- 工具请求继续使用 M6 已定义的 data-minimized AuditEvent。

连接事件与 M6 工具请求 AuditEvent 是不同事件域。M7 不把 tunnel 原始日志、请求 payload、API Key、完整 endpoint secret 参数或 provider token 写进 AuditEvent。

### 5. MCP readiness 与产品 Ready

必须区分：

- `Runtime Ready`：Runtime 已初始化，MCP service 已启动，Runtime 自身 readiness gate 已打开。
- `Tunnel Connected`：tunnel-client 已建立 provider 连接。
- `MCP Verified`：通过实际 tunnel 路径到达 Runtime，且预期 EffectiveToolCatalog 与 Runtime 观察一致。
- `Harness Ready`：以上三项同时成立。

任何未知或不一致状态都不能自动升级为 `Ready`。Security-relevant unknowns 按 fail-closed 处理。

## Open Questions

以下问题保留为 M7 后续实现前必须明确的接口/验证项；它们不改变本设计冻结的职责边界：

1. **Tunnel Client 的确切 secret injection 方式**：需要根据最终支持的 tunnel-client 版本确认其官方支持的非命令行、非持久化 API Key 输入通道。如果不存在安全通道，必须在实现前决定更换客户端适配方式或将该配置标记为不可用。
2. **Runtime endpoint 的现有内部交付方式**：需要在不修改 M0 wire schema、stderr、临时文件和日志的前提下，确定 Launcher 从 Runtime Supervisor 获取当前 instance 的实际 endpoint 的内部接口。
3. **Tunnel 健康检查的权威信号**：需要确定采用 tunnel-client 的退出/状态协议、provider health endpoint、还是受限的 MCP probe；实现必须避免把“进程仍存活”单独当成 Connected。
4. **Runtime 与 tunnel-client 的进程树清理机制**：需要验证 Windows 下 graceful stop、超时终止、子进程树清理和 stale process identity 检查的具体实现能力。
5. **Provider endpoint 与版本兼容矩阵**：需要冻结 tunnel-client 版本、可接受的 provider endpoint 配置格式、版本检测规则及不兼容时的诊断分类。
6. **有限重试参数**：需要在实现阶段确定启动超时、健康检查超时、Tunnel crash 后重启次数和 backoff；不得实现无限后台重试。
7. **连接事件的落点**：需要确定 Runtime/Tunnel/MCP 生命周期事件是否进入现有结构化 Activity event bus，或由 Launcher 保持独立的 bounded diagnostics stream；无论选择哪一种，都不得把 secret 或原始 tunnel 日志带入 AuditEvent。
8. **跨机器/多账户部署**：V1 只冻结单安装、单 active Runtime、单 managed tunnel-client；多账户 Windows service、系统级常驻 tunnel 和跨机器共享 tunnel identity 留待后续版本。
9. **第三方/手工启动 tunnel-client**：V1 不承诺支持 unmanaged client。若未来开放，必须先增加 endpoint ownership、连接身份和冲突检测设计，不能仅通过 PID 或端口判断归属。
