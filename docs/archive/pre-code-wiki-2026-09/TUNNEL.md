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
## M7-B — Tunnel Process Lifecycle (Lifecycle Only)

M7-B 在 Tauri Launcher / Control Plane 内新增 `TunnelProcessManager`，只管理受管 `tunnel-client` 子进程的生命周期；不建立或验证任何外部连接。

### State Model

进程状态仅保存在内存中：

- `Stopped`：没有受管子进程。
- `Starting`：已接受启动请求，尚未完成子进程登记。
- `Running`：受管子进程已 spawn，持有 `Child` handle；这只表示进程存活，不表示已连接。
- `Stopping`：已请求受管子进程退出，等待 graceful termination 或超时后的强制终止。
- `Failed`：受管子进程意外退出；记录非敏感 exit code 和时间戳，不自动重启。

每次启动创建新的 `TunnelInstance`，其中包含 `instance_id`、PID、可选 `tunnel_id`、Runtime endpoint reference、启动时间和 canonical executable path。停止操作必须同时匹配当前受管 `instance_id` 并使用 manager 持有的 `Child` handle；不会按进程名或 PID 搜索、终止外部进程，因此旧 instance ID 不能终止 PID 复用后的新进程。

### Lifecycle Semantics

- **Start**：先验证可执行文件存在并 canonicalize，再创建内存中的 instance identity、spawn 子进程并保存 PID/启动时间。spawn 成功后状态为 `Running`；它不是 `Connected`，不会合成产品级 `Ready`。
- **Stop**：仅对当前 owned instance 发出 Windows graceful termination 请求，等待固定超时；未退出时才使用 `Child::kill()` 强制终止。exit watcher 将该 managed stop 收敛为 `Stopped`。
- **Restart**：必须先成功停止当前 owned instance；停止失败或状态仍在转换时 fail closed，绝不启动第二个 tunnel-client。
- **Crash detection**：exit watcher 观察到 `Running` 实例退出时转为 `Failed`，保存 exit code/时间及非敏感错误分类 `TUNNEL_PROCESS_EXITED`；没有自动或无限重启。

### Boundary

本阶段不读取或传递 API Key/plaintext secret，不创建 provider authentication、network connection、health check、MCP verification 或 `Ready` synthesis；不修改 Runtime、MCP schema、PolicyEngine、ExecutionPipeline、UI 或 M0 Control Protocol。动态 PID、启动时间与退出码不写入 `config.json` 或任何持久配置。

## M7-C0 Credential Capability Audit

**审计日期：2026-09-08**

本节只记录 tunnel-client 的能力审计和 V1 设计决策；不实现 CredentialInjector，不修改 TunnelProcessManager、Runtime、UI、config schema 或 SecretStore integration。

### 1. Tunnel client information

| 项目 | 结果 |
| --- | --- |
| Executable | `tunnel-client.exe` |
| Repository location | `tunnel-client-v0.0.13-windows-amd64/tunnel-client.exe` |
| Version | `0.0.13+4b5267f823be0b046bb883aacb51603cfde3a0ea` |
| Build information | Git SHA `4b5267f823be0b046bb883aacb51603cfde3a0ea`; Windows amd64 package |
| Provider / product | OpenAI tunnel client；help 输出为 “Tunnel client for the OpenAI MCP control plane.”；随包包含 Cloudflare Tunnel companion `cloudflared.exe` |
| Repository artifacts inspected | `tunnel-client.exe`、`cloudflared.exe`、`cloudflared-manifest.json`、SPDX 清单、licenses、NOTICE |
| Local discovery result | 仓库已包含固定版本 Windows amd64 binary；当前项目的预期方式是由 Launcher/Control Plane 启动外部 `tunnel-client.exe`，把本机 Runtime MCP endpoint 作为非敏感输入传入 |
| Official documentation | `https://github.com/openai/tunnel-client/blob/master/docs/configuration.md`；`https://github.com/openai/tunnel-client/blob/master/docs/troubleshooting.md`；本地 binary 的 `run --help` 是当前随包能力的直接证据 |

本地 binary 的 `run --help` 明确显示：配置优先级为 flags > environment > YAML > defaults；`control-plane.api-key` 支持 `env:VARNAME` 或 `file:/path/to/secret` 引用；没有显示 stdin、pipe 或 OS credential-store 参数。官方 configuration 文档同时说明直接 literal API key 虽兼容但不推荐，secret-bearing fields 应使用 `env:` 或 `file:` 引用。

当前项目预期的 tunnel-client 启动输入应保持最小化：

- 非敏感：executable path、Tunnel ID、loopback MCP endpoint、health endpoint/health URL file path。
- 敏感：control-plane API Key；不得作为 literal value 进入 argv、YAML/profile、Runtime 或普通日志。

### 2. Capability matrix

| 方式 | 支持情况 | 安全评价 |
| --- | --- | --- |
| command line argument | **部分支持**：`--control-plane.api-key` 接受 `env:VARNAME` 或 `file:/path/to/secret`；也接受 literal，但不能采用 literal | **不允许 secret 直传**。`env:`/`file:` 只是引用，argv 中仍会出现引用名或路径；不得把 API key 本身放入 argv。文件引用还引入磁盘明文和 ACL/清理问题 |
| environment variable | **支持**：`CONTROL_PLANE_API_KEY`，并可由 `--control-plane.api-key=env:VARNAME` 选择；`OPENAI_API_KEY` 是 fallback | **唯一可作为 V1 候选的注入通道，但不是无风险通道**。secret 会存在 tunnel-client 进程环境中，可能被同权限进程检查、进程诊断、crash dump 或错误的子进程继承；必须限制生命周期、继承范围和诊断/转储面 |
| stdin / pipe | **未发现支持**：`run --help`、相关官方 configuration 文档和 package artifacts 未提供 API key stdin/pipe 输入方式 | **不能推荐**。不能把 stdin 当作隐含的 credential API；在没有 upstream contract 和实测证据前视为不支持 |
| config file | **支持**：YAML/profile 中可配置 `control_plane.api_key`；支持 literal、`env:VARNAME`、`file:/path/to/secret` | **literal 明文不允许**。`env:` 可作为引用，但 profile/config 仍可能出现在诊断或导出范围；`file:` 需要明文 secret 文件、Windows ACL、原子创建和可靠擦除，当前 V1 没有已审计的安全文件注入设计 |
| OS credential lookup | **未发现支持**：没有 Windows Credential Manager/keychain/DPAPI/SecretStore 参数或文档契约 | **不可直接采用**。Launcher 的 SecretStore 不能假设 tunnel-client 会读取它；需要 upstream credential-helper 或 wrapper 契约后才能评估 |
| interactive login | **存在相关命令/帮助主题**：binary 提供 `init`、`profiles`、`admin-profiles`、`help oauth` 等交互/配置面；但未形成适合 Launcher 非交互启动的 API-key injection contract | **不适合 V1**。依赖用户交互、profile 持久化和生命周期不可控，不能作为 Start Harness 的确定性凭据路径 |

### 3. Security assessment

本审计对 API Key 采用以下不可违反的边界：API Key 不得进入 Git、`config.json`、`RuntimeConfigSnapshot`、`RuntimeState`、literal command line、URL、logs、stderr、stdout、`AuditEvent` 或 Activity event。

审计结论：

1. **literal command-line 和 literal YAML/profile 明文不合格**，即使 tunnel-client 为兼容性接受，也不进入 V1。
2. **stdin/pipe 和 OS credential lookup 没有证据支持**，不能设计成“推测存在”的安全方案。
3. `file:/path/to/secret` 能避免 secret 进入 argv，但会把 secret 落到磁盘；在当前任务禁止新增安全文件注入/SecretStore integration 的边界下，不能宣称其满足 V1 安全约束。
4. environment injection 不会把 API Key 放入 argv、config 或 Runtime 状态，但会使 API Key 出现在 tunnel-client 进程环境中。Windows 上应将其视为可被同权限进程/诊断机制观察的短生命周期 secret，而不是等同于 SecretStore 隔离。
5. tunnel-client 自身提供 redaction/diagnostic 设计的官方说明，但本项目不能把它当作 Launcher 日志安全的充分证明；Launcher 仍必须禁止记录 argv、环境值、配置内容和完整 tunnel-client 输出中的 secret-bearing data。

### 4. Recommended V1 approach

#### V1 decision: Option B, conditional environment-reference launch

当前版本没有被证实的 stdin/pipe、OS credential lookup 或 credential-helper 接口。因此，若 M7-C1 必须在现有 `0.0.13` binary 上继续，唯一可落地候选是：

- Launcher 从 SecretStore 取得 API Key，仅在启动 tunnel-client 的短生命周期内设置一个专用 child environment variable。
- argv 只包含 `--control-plane.api-key=env:<专用变量名>`，不包含 API Key value。
- Tunnel ID 和 loopback MCP endpoint 作为非敏感参数传入。
- 不写 secret file，不写 YAML/profile literal secret，不设置全局/Launcher 长生命周期环境变量。
- 子进程启动后，Launcher 不把环境值、argv 快照、stdout/stderr 原文、诊断导出或 crash artifact 送入日志、Activity 或 AuditEvent。
- 停止/失败路径必须尽快终止 tunnel-client 并释放该 child environment；M7-C1 还必须先验证进程树、同权限进程可见性、crash dump/诊断策略和日志 redaction。

这不是“无风险注入”；它是**在当前 binary 能力下的条件性 V1 方案**。若安全要求将“API Key 不得存在任何进程环境”也视为硬约束，则 Option B 也不合格，V1 必须阻塞，直到 upstream 提供 stdin/pipe、OS credential lookup 或受审计的 credential-helper/wrapper 接口。

#### 不采用的方案

- Option A（stdin/pipe）：当前未证实支持，不进入设计。
- Option C（literal command line / plaintext config）：明确不采用。
- `file:/path/to/secret`：当前不作为默认 V1 方案；除非另行批准 Windows ACL、临时文件生命周期、清理失败处理和 crash/dump 风险设计。
- interactive login/profile：不作为 Launcher 自动 Start Harness 的 V1 依赖。

### 5. Blockers / Open Questions

#### Blockers

1. **Credential transport 仍无零明文落盘/零 argv secret 的 upstream 专用通道**。如果环境变量的 process-inspection 风险不被接受，则 M7-C1 blocked。
2. 当前审计没有动态验证真实 control-plane 请求；没有 API Key、组织权限、可用 Tunnel ID 和外网条件，因此不能声称认证或 tunnel readiness 已验证。
3. `0.0.13` binary 的 Windows 进程环境、crash dump、embedded admin UI/log export 和子进程继承行为尚未做安全验收；该项必须在 M7-C1 之前明确为通过或 blocker。

#### Open Questions

- tunnel-client 是否由本项目长期控制，还是只消费外部发布的 binary？
- 项目是否可以增加并维护 wrapper？wrapper 是否允许作为正式的 credential-helper 边界？
- 是否可以升级 tunnel-client；升级后 capability contract 和 redaction 行为如何回归验证？
- upstream 是否可以提供 stdin/pipe、Windows Credential Manager/DPAPI、或 credential-helper 回调？
- Launcher 是否允许创建受 ACL 保护的临时 secret file；如果允许，谁负责清理失败和 crash recovery？
- 生产环境是否启用了用户态 crash dump、endpoint inspection 或诊断导出策略？

**M7-C0 结论：** 当前随仓库的 tunnel-client 明确支持 environment reference 和 file reference；没有证据支持 stdin/pipe 或 OS credential lookup。V1 可进入 M7-C1 的前提是接受并审计短生命周期 child-environment 方案；若“API Key 不得进入任何进程环境”是硬约束，则当前能力不足，V1 blocker。

## M7-C Credential Transport Decision

**决策日期：2026-09-08**

本节是 M7-C 的架构决策，覆盖并取代 M7-C0 中“Option B 可作为条件性 V1 候选”的建议；M7-C0 的 capability audit（能力审计）仍是当前 `tunnel-client` `v0.0.13` 能力边界的证据。本决策不授权任何代码、Runtime、UI、`TunnelProcessManager` 或 `CredentialInjector` 变更。

### 1. Problem

现有 `tunnel-client v0.0.13` 支持 environment reference（环境变量引用）、file reference（文件引用）及 YAML/profile 配置，也兼容 literal command-line/config secret（命令行或配置明文 secret）；没有已证实的 stdin/pipe 注入、Windows Credential Manager、DPAPI lookup 或 credential-helper interface（凭据辅助程序接口）。

M7-C1 曾考虑由 Launcher 从 `SecretStore` 读取 API Key，并只把它放入目标 `tunnel-client` 的 child environment（子进程环境）。这能避免 API Key 出现在 argv、持久化配置和 Runtime config，但不能证明“API Key 仅目标 `tunnel-client` 可见”：同一用户/同等权限主体的 process inspection（进程检查）、崩溃转储、诊断采集和后代进程继承都可能扩大可见范围。

因此问题不是能否降低暴露概率，而是当前传输机制能否建立并证明 V1 所需的 credential visibility boundary（凭据可见性边界）。答案是否定的。

### 2. Security requirements

M7-C 将以下约束冻结为 V1 的硬安全要求：

1. API Key 不得进入 argv、URL、plaintext config、YAML/profile literal、普通文件或任何进程环境（包括 Launcher、`tunnel-client` 及其子进程）。
2. API Key 的长期持久化仅允许在当前用户边界内受操作系统保护的 `SecretStore` / OS credential store（操作系统凭据存储）；普通配置只能保存 opaque `apiKeyRef`。
3. Runtime、`RuntimeConfigSnapshot`、`RuntimeState`、UI state、Activity、`AuditEvent`、diagnostics、日志、stdout/stderr、错误文本、截图和导出的 crash artifact（崩溃工件）不得包含 API Key 或可还原的 secret-bearing data（含有 secret 的数据）。
4. 安全相关事实无法验证时 fail closed（失败关闭）：不得以“生命周期短”“仅 child scope”或“未观察到泄露”替代边界证明。
5. 任何未来通道都必须能以文档化接口契约和针对 Windows 的实测证明：secret 不进入禁止载体，且启动失败、重试、诊断、导出、crash recovery（崩溃恢复）和停止路径不会把 secret 扩散到这些载体。

这些要求明确收紧了此前“API Key 可在 tunnel-client 进程环境中短暂存在”的条件性假设；该假设不再是可接受的 V1 security boundary（安全边界）。

### 3. Options considered

#### Option A — Strict Security Boundary

**定义：** 保持上述硬要求：API Key 不允许出现在 argv、environment、file 或 plaintext config。当前 `tunnel-client` 不具备已证实的合规输入通道，因此 M7-C 保持 blocked（阻塞）。

**优点：**

- 与现有 SecretStore isolation（SecretStore 隔离）、secret redaction 和 security-relevant unknowns fail-closed 原则一致。
- 不把“目标子进程环境”误称为 secret isolation，也不把同权限进程检查、crash dump 或诊断导出风险转嫁给实现细节。
- 不新增未经审计的凭据持有进程、临时文件、IPC 协议或 Windows ACL/清理恢复责任。
- 保持 Launcher 负责编排、Runtime 不接触 tunnel credential 的既有 M7-A 职责边界。

**缺点：**

- 在当前 `v0.0.13` capability contract（能力契约）下无法完成自动 tunnel authentication，M7-C1 和依赖它的外部连接验收不能推进。
- 需要等待 upstream capability（上游能力）或另行批准、设计并验证一个真正合规的替代方案。

**适用条件：**

- API Key 仅目标 `tunnel-client` 可见是不可放宽的产品安全要求。
- 不能针对 Windows process inspection、crash dump、diagnostic export（诊断导出）和 child inheritance（子进程继承）给出可重复的边界证明。
- V1 可以接受 tunnel authentication 延后，而不能接受降低 credential transport 安全要求。

#### Option B — Scoped Environment Acceptance

**定义：** Launcher 从 `SecretStore` 读取 API Key，以专用 child environment variable（子进程环境变量）启动 `tunnel-client`，argv 仅传 `env:<variable-name>` 引用。

**优点：**

- 是当前 binary 已证实可用的自动化候选；API Key 不会作为 literal value 进入 argv、URL 或持久化 config。
- 改动范围表面上较小，不需要等待 client 升级。

**缺点和逐项风险/缓解：**

| 主题 | 风险 | 可做的缓解 | 为什么仍不合格 |
| --- | --- | --- | --- |
| Process inspection（进程检查） | 同用户或同等权限的工具、EDR、调试器、管理员或诊断代理可能读取目标进程 environment。 | 仅在 `Command`/spawn 时设置专用变量；不写入父进程/全局环境；最小权限运行；不记录 environment snapshot。 | 缓解只能缩短或减少观察面，不能证明 secret 仅目标进程可见。 |
| Crash dump（崩溃转储） | `tunnel-client`、Launcher 或后代进程的内存/环境可能进入 user-mode dump、WER、调试器或支持包。 | 默认关闭或限制应用级 dump；禁止自动上传/导出原始 dump；制定本地访问控制与人工取证流程。 | 当前项目没有覆盖 OS/EDR/管理员采集面的端到端控制；关闭应用级转储不等于没有可访问的内存工件。 |
| Child inheritance（子进程继承） | `tunnel-client` 或其 companion/diagnostic/reconnect helper 可能继承 environment，扩大 secret 可见的进程集合。 | 只允许已审计的进程树；对子进程启动接口做最小环境白名单；停止时清理所有 owned descendants。 | 当前 client 及其 companion 的完整进程树和继承行为未被证明；即使受管，也已违反“仅目标 tunnel-client 可见”。 |
| Diagnostics/log export（诊断/日志导出） | 进程诊断、support bundle、command snapshot、stderr/stdout 或第三方日志可能收集变量值或相关内存。 | 只保存分类化结果；禁止 raw stdout/stderr、argv/config/environment、dump 和 support bundle 自动进入 Activity/Audit/export。 | Launcher 侧 redaction 无法约束外部 client、OS 或安全产品的采集和导出。 |
| Secret lifecycle（secret 生命周期） | spawn 失败、重试、崩溃、僵尸/孤儿后代或 stop 超时会延长 secret 在内存/环境中的存在时间。 | 单次读取、单次 spawn、无自动重试；失败立即终止 owned process tree；不持久化 secret；释放本地 buffer。 | 生命周期最小化不是可见性隔离；而且 Windows 清零/进程退出/转储可见性仍不能由本方案证明。 |

**结论：** 不选择。Option B 可以作为未来风险接受（risk acceptance）决策的明确对象，但在当前 V1 硬要求下不是合规 credential transport architecture（凭据传输架构），不得实现为 M7-C1。

#### Option C — Credential Helper / Wrapper

**定义：** 引入由 Harness 维护的 wrapper process（包装进程）或 credential helper，例如：

```text
Launcher → credential helper → tunnel-client
```

wrapper 可以从 Windows Credential Manager / DPAPI-backed SecretStore 获取 secret，也可以通过 named pipe（命名管道）等 IPC 取得一次性凭据；它可以避免 API Key 进入 argv，并将 secret lookup 责任从 `tunnel-client` 移回受控组件。

**优点：**

- 可把 OS credential store lookup 和凭据访问策略收敛到受控的 helper，而不是扩展 Runtime。
- 若 client 支持已认证的 helper protocol、one-shot secure handle 或 pipe input，未来可提供比 environment 更明确的 transport boundary。
- 允许保留 M7-A 的 Launcher ownership（Launcher 所有权）与 Runtime secret exclusion（Runtime 排除 secret）原则。

**缺点：**

- wrapper 不能凭空消除“helper 到 `tunnel-client`”的最后一跳：当前 binary 没有已证实的 stdin/pipe、credential-helper protocol 或 OS credential lookup。因此 helper 最终仍会退化为 environment、file 或 literal config，仍不合规。
- 新增长期维护的 privileged credential-bearing process、IPC authentication/authorization、peer identity、replay、timeout、cleanup、crash recovery、diagnostics redaction、Windows ACL 和升级兼容性责任。
- wrapper 自身扩大 trusted computing base（可信计算基），并增加 V1 scope；当前没有批准的协议、威胁模型或验收计划。

**实现成本：** 高，且在未获得 client-side receiver（客户端接收端）契约前没有可验证的完成路径。单独引入 wrapper 不解除 blocker；本任务不创建或批准 `CredentialInjector`/wrapper 实现。

#### Option D — Upgrade / Replace tunnel-client capability

**定义：** 要求或替换为具有下列至少一种经验证能力的 `tunnel-client`：

- API Key 的 stdin/pipe secret input（标准输入/管道凭据输入）；
- Windows Credential Manager / DPAPI lookup；
- documented credential-helper protocol（有文档的凭据辅助程序协议）；或
- 等效的、可证明不把 API Key 放入 argv/environment/file/plaintext config 的 secure IPC（安全 IPC）机制。

**优点：**

- 是解除 Option A blocker 的直接技术路径；正确的 client-side contract 可让 SecretStore/credential helper 成为事实上的 receiver，而不是让 Launcher 通过不安全载体转运明文。
- 可将 upstream 的 redaction、crash/diagnostics 和 child-process 行为纳入版本化 capability contract 与回归测试。

**缺点：**

- 依赖上游版本、协议设计、供应链审查、Windows 行为验证和发布节奏；没有完成日期保证。
- 升级/替换本身需要重新验证参数、配置优先级、认证、process tree、redaction、日志、诊断和兼容性，不能仅因新版本声称支持而自动批准。

**成本与依赖：** 中到高。依赖上游功能或一个经过单独架构审查的替换客户端；在 capability contract、Windows 实测和安全验收完成前，不是当前 V1 的实现任务。

### 4. Final decision

**选择：Option A — Strict Security Boundary。**

原因：当前 blocker 已明确表明 scoped child environment injection（受作用域的子进程环境注入）无法证明 API Key 仅目标 `tunnel-client` 可见。Option B 的缓解措施只能降低风险，不能满足 V1 的可证明隔离要求；Option C 在当前 client 没有安全接收接口时只会把问题向后移动并扩大实现范围；Option D 是可行的 future migration path（未来迁移路径），但目前仍是外部依赖而非已具备能力。

因此，M7-C1 **保持 blocked**。任何人不得以 environment reference、file reference、literal config、argv 或未审计 wrapper 绕过本决定。`TunnelProcessManager` 只能继续其 M7-B lifecycle-only（仅生命周期）职责，不得读取、传递或注入 API Key。

### 5. Consequences

- **M7-C1：BLOCKED。** 不实现 scoped child environment injection，也不创建 `CredentialInjector`、credential helper、wrapper、临时 secret file 或新的 SecretStore integration。
- **M7-MCP external readiness：NOT ESTABLISHED。** 由于没有合规自动 credential transport，不得声称真实 tunnel authentication、provider connection 或 end-to-end MCP reachability 已验证。
- **既有边界保持不变。** API Key 仍只可长期存放在 OS-protected `SecretStore`；Runtime、UI、`TunnelProcessManager`、普通 config、Activity、AuditEvent 和 diagnostics 不获得 secret value。
- **不采用 file-reference 规避。** 受 ACL 保护的临时文件可能是未来单独评审的方案，但不满足本决策所冻结的“不得进入 file”要求；在要求被显式修订前不得实现。
- **M7-C0 文档状态。** C0 的 capability audit 仍有效；其 Option B 只说明“当前 binary 能启动的候选”，不再构成 V1 接受结论。

### 6. Future migration path

解除 blocker 的最小顺序如下；每一步均需单独证据，不可跳过：

1. **获取合规能力。** 上游 `tunnel-client` 或替换客户端提供上述 Option D 的正式、版本化 secret-input contract；若提出 helper/wrapper，则必须同时定义 client-side receiver 和 secure IPC，不接受仅在 helper 端读取 secret 的设计。
2. **冻结新接口设计。** 记录 secret source、receiver、进程/IPC identity、最小权限、生命周期、failure/stop/crash cleanup、redaction 和 diagnostics/export boundary；明确 Runtime 不接触 secret。
3. **Windows security verification（Windows 安全验证）。** 对 process inspection、child inheritance、crash dump/WER、diagnostic/support export、spawn failure、retry、orphan process 和 forced termination 执行可重复测试；验证 API Key 不出现在禁止载体。
4. **Capability and regression verification（能力与回归验证）。** 针对精确 client version 验证启动参数、认证、process tree、日志/redaction、诊断和 MCP readiness；无真实凭据/外网证据时必须标记 `NOT RUN` 或 `NOT VERIFIED`，不得作成功声明。
5. **新架构决策后才实施。** 只有步骤 1–4 的证据满足本节 Security requirements，才可创建后续实现任务并明确允许的最小代码变更；在此之前 M7-C1 继续 blocked。
