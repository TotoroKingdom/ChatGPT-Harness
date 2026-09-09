# M9-A — Distribution & First-Run Architecture Freeze

> 状态：M9-A 架构冻结文档（仅调查与决策）
>
> 本文冻结 Windows 本地分发、安装和首次启动边界。本文不实现 installer、auto updater，也不修改 Runtime、MCP、Policy、Tunnel 或 SecretStore。

## 1. Current repository/build audit

### 1.1 上位文档与约束

已审查：

- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- `docs/CONFIG.md`
- `docs/CONTROL_PLANE.md`
- M7/M8 相关 feature 文档
- M9-A 任务规格

上位文档与本任务目标一致，没有发现需要停止并上报的产品或架构冲突。仓库根目录当前不存在 `CONTEXT.md`，因此本次审计无法引用该文件；这不改变既有上位文档的优先级，也不构成新增产品假设。

上位文档已经确定的相关原则包括：Windows-first desktop、UI 是 control plane、Runtime 独立执行核心 MCP、Runtime 与 tunnel-client 分离、机器配置位于 OS application-data、secret 使用 OS secret store，以及 Local Runtime Ready 不等同于外部 Tunnel/MCP Ready。M7-C credential transport 仍为 `BLOCKED`，本冻结不改变该状态。

### 1.2 Cargo workspace 与 Runtime

Workspace `Cargo.toml` 的成员为：

- `protocol`
- `runtime`
- `ui/src-tauri`

`runtime/Cargo.toml` 定义独立 binary：

- package：`chatgpt-harness-runtime`
- binary：`chatgpt-harness-runtime`
- Windows 文件名：`chatgpt-harness-runtime.exe`

当前 Runtime 输出位置是：

- `D:\ChatGPT-Harness\target\debug\chatgpt-harness-runtime.exe`
- `D:\ChatGPT-Harness\target\release\chatgpt-harness-runtime.exe`

仓库内当前没有 `target-exe` 输出目录；`.gitignore` 虽然忽略了 `/target-exe/`，但该目录当前不存在。

### 1.3 UI、Tauri 与构建脚本

`ui/package.json` 当前脚本为：

- `runtime:dev`：Cargo debug build
- `runtime:release`：Cargo release build
- `tauri:dev`：预检、构建 debug Runtime、启动 Tauri
- `tauri:build`：构建 release Runtime、执行 `tauri build`

`ui/src-tauri/tauri.conf.json` 当前为：

- product name：`ChatGPT-Harness`
- identifier：`com.chatgpt.harness`
- version：`0.1.0`
- `bundle.active`：`false`
- 未配置 `bundle.externalBin`
- 未配置 Tauri resources
- 未配置 sidecar

`ui/src-tauri/build.rs` 目前只调用 `tauri_build::build()`。因此仓库当前已经具备“UI 构建前先构建 Runtime”的开发/发布构建脚本，但尚未具备一个可安装的正式分发拓扑；M9-A 只冻结目标，不在此实现打包接线。

### 1.4 RuntimeSupervisor 与可执行文件发现

`ui/src-tauri/src/runtime/mod.rs` 当前 Runtime 查找顺序为：

1. `CHATGPT_HARNESS_RUNTIME_PATH`
2. UI `current_exe()` 所在目录
3. 该目录下的 `chatgpt-harness-runtime.exe`
4. 找不到则失败

启动使用 `Command::new(executable_path)`，并通过 stdin/stdout/stderr pipe 与 Runtime 通信；Windows 启动时隐藏控制台。初始化后会校验 protocol version，并记录 PID、Runtime ID、Runtime version、实际 executable path、workspace 与 config revision。

这说明现有 RuntimeSupervisor 已有“明确启动哪个 binary”和“启动后校验身份/协议”的基础，但当前 discovery 仍是开发态布局。正式安装模式不得依赖环境变量、Git repository root、Cargo target、开发者绝对路径或偶然的 current working directory。

### 1.5 Config 与 SecretStore

`ui/src-tauri/src/config/mod.rs` 当前使用 Tauri `app.path().app_data_dir()`，文件名为 `config.json`。写入采用 atomic write，配置带有 `schemaVersion = 1`，保存时维护 `configRevision`。`tunnelClientPath` 在保存时会规范化为包含 executable 的目录；workspace、tunnel、security、ui 等字段属于 AppConfig。

`ui/src-tauri/src/secrets/mod.rs` 在 Windows 使用 `keyring` 的 `windows-native` backend，即 Windows Credential Manager。当前 service 为 `ChatGPT-Harness`，API key locator 为 `tunnel-api-key`。AppConfig 只保存 `SecretReference`，不保存 API key 明文。

仓库 fixture `protocol/fixtures/v1/app-config.example.json` 中的 `<workspace>`、示例 Tunnel ID 与 secret reference 仅是机器无关样例，不能被当作真实本机配置。

### 1.6 tunnel-client、ignore 与当前仓库布局

当前仓库存在并被跟踪的目录：

`tunnel-client-v0.0.13-windows-amd64/`

其中包括：

- `tunnel-client.exe`
- `cloudflared.exe`
- LICENSE / NOTICE
- license/SPDX 清单与 manifest

当前 tunnel-client discovery 可检查 UI executable 同目录、`PATH` 以及用户手动选择的目录；配置保存的是目录而不是 executable 文件路径。

当前 `.gitignore` 忽略：`/target/`、`/ui/dist/`、`/ui/node_modules/`、`/target-exe/` 等构建产物，但没有把本机 AppData 作为仓库文件处理。这是正确方向，因为正式配置应位于仓库外的 Windows application-data 目录，而不是依赖新增 repo-local config。

本次审计未发现真实 workspace path、开发者绝对路径、真实 Tunnel ID 或 API key 被写入 tracked AppConfig。发现的唯一当前工作区修改是用户预先修改的 M9-A 任务规格文件，状态为 `AM`；不得覆盖、重置或清理该修改。

## 2. Installed topology

V1 冻结为“一个发行包、两个进程、两类数据”的拓扑：UI 和 Runtime 同版本发布，Runtime 由 UI 按需启动为子进程；应用 binary 与用户数据完全分离。

概念安装目录如下（`<install-root>` 可为 Program Files 下的默认目录或用户选择的安装目录）：

```text
<install-root>\\
├─ chatgpt-harness-ui.exe
├─ runtime\\
│  └─ chatgpt-harness-runtime.exe
├─ distribution.json                 # V1 发行身份/兼容性元数据
└─ resources\\                       # UI 静态资源及其他非本机数据（如需要）
```

上述文件名是 V1 的逻辑布局；M9-B 接线 Tauri 时必须让实际生成的 UI executable 与该逻辑名称和 discovery 规则一致，不能再以仓库目录名推导路径。

安装目录只放不可变的应用 binary、静态资源和发行元数据。`config.json`、workspace path、Tunnel path、Runtime state、logs、approval history 和任何 secret 不得写入 `<install-root>`。

Runtime 不注册为 Windows service，不作为独立产品安装，不要求用户手动启动终端，也不被复制到用户 workspace。UI 负责生命周期控制；Runtime 仍是可独立执行的核心 Runtime，UI 退出或 stop 时必须遵循既有生命周期语义。

## 3. Runtime packaging decision

### 3.1 方案比较

| 方案 | Tauri 支持度 | Discovery | Upgrade | Lifecycle | Security | Dev/prod parity |
| --- | --- | --- | --- | --- | --- | --- |
| A. Tauri sidecar | Tauri 2 可通过 `externalBin`/sidecar 机制支持；需要配置 target-specific sidecar 命名 | 由 Tauri sidecar API 管理，路径抽象较好 | 可随 UI bundle 更新 | 适合由 UI 启停，但会绑定 Tauri sidecar 约定 | 可限制在 bundle 内，但仍须校验身份与协议 | 需要为 dev 与 bundle 建立两套接线 |
| B. bundled resource | Tauri 可把 binary/resource 放入 bundle；后端按安装目录/resource dir 的固定布局解析 | 规则直接、可诊断，可保留现有 `Command::new` 子进程模型 | UI 与 Runtime 可以作为同一发行包原子替换 | 明确是 UI 启动的本地子进程 | 可限制到受信安装目录并做版本/协议校验 | 最接近当前 Supervisor，开发态仍可显式使用 target |
| C. 独立安装 binary | 需要额外安装、注册、升级与卸载协调 | 容易落入 PATH、注册表或用户手工路径 | 容易产生 UI/Runtime 静默混搭 | 两套安装生命周期，故障面更大 | 需要额外信任和权限边界 | 与当前仓库构建不匹配 |
| D. 其他（service/运行时下载等） | 超出当前最小需求 | 引入服务注册或网络下载依赖 | 需要 updater/rollback 设计 | 复杂度和恢复边界显著增加 | 扩大供应链与权限面 | 不符合 V1 窄范围 |

### 3.2 V1 决策

**V1 采用 B：Runtime 作为 bundled application executable/resource 随 UI 同包发布，安装在 `<install-root>\\runtime\\chatgpt-harness-runtime.exe`，由 UI 作为受控子进程启动。**

这里的 “bundled resource” 是分发拓扑决策，不等同于当前 Tauri 已经配置了 resources；当前 `bundle.active` 仍为 false。M9-B 才负责将该冻结布局接入 Tauri 打包。

V1 不采用 C，因为独立 Runtime 安装会使 UI/Runtime 版本、卸载、权限和 discovery 变成两个发布系统，并增加静默混搭的风险。V1 也不把 A 作为当前必须实现的 sidecar contract：sidecar 是可行的 Tauri 实现选项，但本次冻结优先保证固定安装布局、可审计 discovery、现有 stdio 生命周期和开发/生产可诊断性。若 M9-B 选择使用 Tauri sidecar API，必须仍然产出本节冻结的逻辑布局/身份校验，不能改变版本一致性或安全边界。

## 4. Runtime discovery

### 4.1 Development discovery（开发模式）

开发模式由明确的 development flag/build identity 进入。当前代码实际只支持显式 `CHATGPT_HARNESS_RUNTIME_PATH` 和 UI executable 相邻的同名 Runtime；而当前 Cargo 脚本把 Runtime 输出到 workspace 的 `target/debug` 或 `target/release`。因此当前开发闭环依赖构建布局/接线，不能直接当作安装 discovery。

M9-A 冻结的 development discovery 允许以下有序来源：

1. 显式 `CHATGPT_HARNESS_RUNTIME_PATH`（仅开发/测试）
2. workspace 的 `target/debug/chatgpt-harness-runtime.exe`
3. workspace 的 `target/release/chatgpt-harness-runtime.exe`（若开发配置要求 release）
4. 现有开发态 UI 相邻 binary 兼容路径

开发 discovery 可以使用仓库相对路径，因为这是开发工具链的目标；它不能被安装模式复用。开发模式仍必须在启动前将路径解析为绝对路径，在启动记录中显示实际 binary，并通过 Runtime initialize 校验 protocol/version。

### 4.2 Installed discovery（安装模式）

安装模式只允许从 UI executable 的可信安装根推导固定路径：

```text
Path(ui_current_exe).parent / "runtime" / "chatgpt-harness-runtime.exe"
```

安装模式必须：

- 使用 UI `current_exe()` 的绝对路径作为起点，而不是 CWD；
- 解析并确认目标为 regular file；
- 确认目标位于 `<install-root>\\runtime\\` 的预期位置，不能被配置文件或 `PATH` 改写；
- 不读取 `CHATGPT_HARNESS_RUNTIME_PATH`；
- 不扫描 Git root、Cargo target、`D:\\ChatGPT-Harness` 或其他开发者绝对路径；
- 不以“找到的第一个同名 executable”作为成功条件；
- 启动前和 initialize 后都记录/校验实际 path、distribution identity、Runtime version 与 protocol version。

如果固定路径缺失、不是文件、无法启动、协议不兼容或身份不匹配，UI 必须给出可操作的诊断错误并保持 Runtime Not Ready；不得静默 fallback 到开发路径。

RuntimeSupervisor 最终必须接收并保存一个已经解析的、明确的 executable path；不能让 `Command::new` 通过 PATH 或当前目录隐式选择 Runtime。

## 5. Local data layout

Windows V1 将应用 binary 与用户数据分开：

```text
Application binaries
<install-root>\\
├─ chatgpt-harness-ui.exe
├─ runtime\\chatgpt-harness-runtime.exe
└─ distribution.json

User data
%APPDATA%\\ChatGPT-Harness\\
├─ config.json
├─ state\\
├─ logs\\
└─ policy\\                         # 预留；当前不新增 policy 功能

OS secret store
Windows Credential Manager
└─ service: ChatGPT-Harness
   └─ key: tunnel-api-key
```

`%APPDATA%\\ChatGPT-Harness\\` 是 V1 的用户数据根；当前代码已经通过 Tauri `app_data_dir()` 使用该类 OS application-data 位置，并在其中写入 `config.json`。目录与字段的未来扩展必须保持同一边界。

| 数据 | V1 位置 | 是否进入 Git | 说明 |
| --- | --- | --- | --- |
| UI/Runtime binary、静态资源 | `<install-root>` | 作为 release artifact，不是 repo 本机状态 | 随发行包更新 |
| `AppConfig` | `%APPDATA%\\ChatGPT-Harness\\config.json` | 否 | 包含 workspace、tunnel、security、ui 等机器配置；继续使用 atomic write 与 schema/revision |
| local state | `%APPDATA%\\ChatGPT-Harness\\state\\` | 否 | Runtime/UI 状态、恢复所需 transient state；不得成为协议或 secret 的替代存储 |
| logs | `%APPDATA%\\ChatGPT-Harness\\logs\\` | 否 | 仅保存当前产品允许的脱敏日志；不得写入 API key、token 或 secret argv |
| future policy config | `%APPDATA%\\ChatGPT-Harness\\policy\\` | 否 | 仅预留位置；不在 M9-A 新增 policy 能力 |
| transient Runtime metadata | `%APPDATA%\\ChatGPT-Harness\\state\\` 或进程内 | 否 | PID、Runtime identity、revision 等应可失效、可重建，不当作持久审计账本 |
| API key | Windows Credential Manager | 否 | 不迁移到普通文件、环境变量或安装目录 |
| secret reference | `config.json` 中的 locator | 否（真实配置）；fixture 可进入 Git | 仅 provider/key 等定位信息，不含 secret value |

`state`、`logs` 与 `policy` 是数据边界冻结，不表示本任务要新增对应功能。当前产品已有的持久化实现仍以代码和既有协议为准。

## 6. First-run flow

V1 首次启动流程冻结如下：

```text
Launch UI
  ↓
Determine development vs installed distribution mode
  ↓
Resolve and validate the Runtime binary
  ↓
Read %APPDATA%\\ChatGPT-Harness\\config.json
  ↓
If absent: create default non-secret config atomically
  ↓
Select Workspace
  ↓
Validate workspace boundary and save machine-local config
  ↓
Optionally configure Tunnel ID / tunnel-client directory
  ↓
Optionally enter API key → write only to Windows Credential Manager
  ↓
Run separate diagnostics: Runtime / Tunnel / MCP
  ↓
Start Local Runtime when Runtime is valid
  ↓
Show Local Runtime Ready independently from Tunnel/MCP readiness
```

具体规则：

1. 没有 config 时只创建 non-secret defaults；不能生成包含真实路径、真实 Tunnel ID 或 API key 的 repo 文件。
2. Workspace 必须由用户选择或明确确认；保存的是机器本地 workspace path，仍受 Workspace boundary 与 Policy boundary 约束。
3. Runtime binary 是 Local Runtime 的前置条件。缺失或版本不匹配时阻止 Runtime 启动，并报告修复动作。
4. Tunnel ID、tunnel-client path 与 API key 均不是 Local Runtime 启动的必要条件。
5. API key 配置可以继续写入 SecretStore；但 M7-C `BLOCKED` 期间，不能暗示该 key 已可用于 tunnel-client，也不能通过 environment、file、argv 或未经审计 wrapper 传输。
6. Tunnel 不可用只将 Tunnel/MCP 状态标记为 unavailable/not ready；不能阻止本地 Runtime 启动、读取 workspace 或使用本地 Runtime 诊断。
7. Diagnostics 必须分别展示 Runtime、Tunnel、MCP 三个状态，不能以 Tunnel unavailable 覆盖 Local Runtime Ready，也不能把 Local Runtime Ready 显示成外部 MCP Ready。
8. 首次启动完成后，用户可以先使用本地 Runtime；外部 Tunnel connectivity 由 M7-C 后续单独解除阻塞后再处理。

## 7. Config portability

### 7.1 两种配置模型

**Repository/release-shared model（可分享）**只包含机器无关的契约和样例：

- schema/type 定义；
- 默认值与文档；
- `protocol/fixtures/v1/app-config.example.json` 这类 example fixture；
- release manifest 的结构与版本字段；
- 不含本机路径或 secret value 的测试数据。

**Machine-local model（不可分享）**只存在于 `%APPDATA%\\ChatGPT-Harness\\` 或 Windows Credential Manager：

- workspace root path；
- runtime/tunnel executable directory；
- Tunnel ID；
- UI/local state、logs、diagnostic snapshots；
- approval history（如当前产品产生）；
- secret reference 与 Windows Credential Manager 中的 API key。

`config.json` 是本机配置，不是仓库配置。即使它的 JSON schema 可分享，实际值也不能回写到 repo tracked files。

### 7.2 当前审计结论

当前 tracked fixture 使用 `<workspace>` 和示例 Tunnel ID；未发现真实开发者绝对路径、真实 workspace path 或 API key 被写入 tracked AppConfig。当前 Runtime 输出在 `target/` 下且被 ignore；`ui/dist`、`node_modules` 也被 ignore。当前 tunnel-client binary 目录虽然被 tracked，但它是第三方/外部 executable artifact，不是机器配置；是否进入 V1 安装包另见第 9 节。

M9-B 的 clean-machine/portability 验收必须检查：

- 安装包不依赖 `D:\\ChatGPT-Harness`、Git root、Cargo target 或 developer machine absolute path；
- 安装后生成的 config、state、logs 不落入仓库；
- workspace path、Tunnel path、Tunnel ID 不被写入 release fixture、manifest 或源码常量；
- logs、diagnostics、audit 与 Runtime config 都不包含 secret value。

## 8. Version compatibility

V1 不实现复杂 updater，但必须冻结“同包、同身份、同协议”的最小检查，避免静默组合：

```text
UI distribution identity
  == Runtime distribution identity
  == expected package metadata

UI supported protocol version
  == Runtime protocol version

UI expected Runtime version
  == Runtime reported version
```

`distribution.json`（或等价的不可变发行元数据）至少表达：

- `distributionId`：与 `com.chatgpt.harness` 对应的发行身份；
- `distributionVersion`；
- `uiVersion`；
- `runtimeVersion`；
- `protocolVersion`；
- `runtimeRelativePath`：固定为 `runtime/chatgpt-harness-runtime.exe` 的逻辑相对路径。

Runtime 启动检查至少包括：

1. 由 installed discovery 得到的路径必须符合 manifest 的相对路径；
2. Runtime initialize 返回的 protocol version 必须受 UI 支持；
3. Runtime reported version 必须等于 manifest 的 `runtimeVersion`；
4. UI、Runtime 与 manifest 的 distribution identity 不匹配时拒绝启动；
5. 任何检查失败都要显示 actionable error，不能通过回退到另一个 Runtime binary“修复”。

这不是完整签名验证、差分升级或回滚协议；M9-A 不引入这些复杂度。升级边界是：UI 与 Runtime 必须作为同一发行包/同一 saved release 更新。未来 updater 若只更新 UI 而没有更新对应 Runtime，应被视为无效发布，而不是允许 UI version A + Runtime version B 静默运行。

## 9. Tunnel packaging decision

**V1 暂不把 tunnel-client 作为 Harness 安装包的必选 bundled binary；将其视为 optional external dependency。**

理由：

- M7-C credential transport 仍为 `BLOCKED`，当前安全边界不允许把 API key 通过 environment、file、argv 或未经审计 wrapper 交给 tunnel-client；
- 仓库中已有 `tunnel-client-v0.0.13-windows-amd64` binary 不等于它已经完成 V1 可分发、许可证、供应链和版本兼容性审查；
- 不随 UI 安装包打包可以避免在当前 blocked 状态下暗示 Tunnel connectivity 已受支持；
- Local Runtime 可以在没有 tunnel-client 的机器上工作，符合 Runtime / Tunnel separation。

M9-A 不删除当前 tracked tunnel-client 目录，也不修改 tunnel discovery。M9-B 只需确保正式安装模式不依赖仓库相对路径；若用户提供 optional tunnel-client，现有配置/发现能力可以继续作为后续接线基础。Tunnel 缺失、版本不兼容或 M7-C blocked 只能阻止 Tunnel/MCP readiness，不得阻止 Local Runtime。

未来是否改为 bundled tunnel-client，必须另行确认：credential transport 已解除阻塞、版本兼容性、许可证与 NOTICE、供应链来源、签名/完整性、更新与回滚策略，以及 Runtime/Tunnel 独立安全边界仍然成立。

## 10. Security boundary

本冻结不削弱以下边界：

### Workspace boundary

Workspace path 是 machine-local config，Runtime 的文件能力仍只能通过现有 Workspace/WorkspaceGuard/PathResolver 边界访问。安装目录、AppData、Git root 和 tunnel-client 目录不能因为“本地”而自动成为 workspace。

### Policy boundary

所有执行请求仍必须经过既有 validation → workspace guard → policy → executor → audit pipeline。M9-A 不增加 write、shell、Git、MCP 或其他工具能力，也不把 first-run/diagnostics 变成 policy bypass。

### SecretStore boundary

API key 只能由 Windows Credential Manager 保存和读取。禁止：

- plaintext API key；
- secret file；
- secret environment fallback；
- argv secret；
- 将 secret 写入 `config.json`、Runtime config、logs、Activity、Audit 或 diagnostics。

`config.json` 中的 `SecretReference` 只能是 locator data。API key 可以在首次启动由用户配置，但在 M7-C blocked 期间不允许进入 tunnel-client transport。

### Runtime / Tunnel separation

Runtime process、Tunnel process、Local Runtime readiness、Tunnel readiness 与 MCP readiness 必须独立建模。启动 UI 或 Runtime 不代表 tunnel-client 已启动；Local Runtime Ready 不代表外部 MCP Ready。

### Installed binary boundary

Installed discovery 只信任 UI executable 旁的固定 runtime layout 和发行身份；不信任 CWD、PATH、Git root、环境变量或任意配置注入的 Runtime path。RuntimeSupervisor 必须把实际启动 path 和版本作为诊断事实暴露，但不泄露 secret 或不必要的敏感完整路径。

## 11. M9-B implementation scope

### In scope

- 分离 development discovery 与 installed discovery；
- 将 Runtime binary 接入 Tauri release bundle 的固定 `runtime\\chatgpt-harness-runtime.exe` 布局；
- 保留并强化 RuntimeSupervisor 的明确 executable path、启动失败和 protocol/version diagnostics；
- 增加 UI/Runtime/distribution identity 与最小 compatibility check；
- 将首次启动的 non-secret default config、workspace 选择和 machine-local persistence 接入既有 config store；
- 在 clean Windows machine 验证安装目录、AppData 数据目录、Runtime 启停、缺失 binary 与版本不匹配错误；
- 验证 Tunnel unavailable 不会阻止 Local Runtime；
- 验证 release artifact/fixture 不携带 developer machine configuration。

### Explicitly out of scope

- installer 实现；
- auto updater、差分更新、签名更新或 rollback 系统；
- Runtime execution logic 改造；
- MCP、Tool Registry、Policy、安全规则修改；
- Tunnel process/credential transport 改造；
- SecretStore security model 迁移；
- 新 UI 页面；
- 解除或重定义 M7-C `BLOCKED`；
- 因分发便利而把 API key 迁移到普通文件、环境变量或 argv。

## Freeze summary

| 决策项 | V1 冻结结论 |
| --- | --- |
| UI/Runtime topology | 同一安装包；UI 控制 Runtime 子进程 |
| Runtime packaging | B：bundled application executable/resource；固定在 `<install-root>\\runtime\\` |
| Installed discovery | 仅由 UI `current_exe()` 推导固定路径；禁止 Git/Cargo/CWD/env fallback |
| Development discovery | 允许明确的仓库 debug/release 路径和开发专用 env override |
| User data | `%APPDATA%\\ChatGPT-Harness\\` |
| Secrets | Windows Credential Manager；service `ChatGPT-Harness`，key `tunnel-api-key` |
| First run | 先建立 non-secret config、选择 workspace、验证 Runtime；Tunnel/API key 可选，不阻止 Local Runtime |
| Version boundary | UI、Runtime、manifest 同 identity、同 protocol、同 package version set；不匹配拒绝启动 |
| tunnel-client | V1 optional external dependency；当前不默认 bundled |
| M7-C | 保持 `BLOCKED`；M9 不解决 external credential transport |



## M9-B implementation — Runtime packaging & installed discovery

M9-B 将冻结的分发边界接入 RuntimeSupervisor，但不实现 installer、updater、签名或 First Run UI。

### Runtime discovery

- Development build 使用仓库内确定的 Cargo target 路径：`ui/src-tauri/../../target/{debug|release}/chatgpt-harness-runtime(.exe)`；不读取 `PATH`、CWD 或 `CHATGPT_HARNESS_RUNTIME_PATH`。
- Installed build 由 `current_exe()` 的 parent 推导 install root，仅解析 `runtime/chatgpt-harness-runtime.exe`。
- Installed build 必须先通过 `<install-root>\distribution.json` 校验，再校验固定 Runtime 目录、regular file、canonical path 和目录边界；符号链接、缺失文件、错误文件名或边界逃逸均 fail closed。
- Discovery 返回 canonical executable；解析失败使用 `RUNTIME_DISTRIBUTION_INVALID`，错误摘要不包含完整内部路径。

### Manifest and compatibility

`distribution.json` schema version 1 包含：`distribution_schema_version`、`distribution_id`、`app_version`、`runtime_version`、`protocol_version` 和固定的 `runtime.relative_path`。UI 在 Runtime initialize 后同时校验 protocol version 与 `server_info.version`，任一不匹配都不会进入 Running/Ready。现有 protocol handshake 未扩展 distribution identity 字段；identity 由 manifest 与固定布局校验提供。

### Packaging staging

`ui/scripts/stage-distribution.ps1` 执行 release workspace build，并在被 `.gitignore` 忽略的 `target-exe/distribution/` 生成：

~~~text
<install-root>\
├─ chatgpt-harness-ui.exe
├─ runtime\
│  └─ chatgpt-harness-runtime.exe
└─ distribution.json
~~~

Manifest 版本从 UI/Runtime Cargo manifests 与 protocol source 读取，避免手写第二套版本源。该 staging 目录是 M9-B 的可验证 packaging boundary，不等同于 installer 或正式安装器。

### Security boundary and remaining work

M9-B 不改变 secret、tunnel、MCP 或 policy 行为，不解除 M7-C `BLOCKED`。M9-C/D 仍负责 installer/安装生命周期与 First Run/发布流程；本阶段只保证 Runtime executable resolution、manifest validation、compatibility gate 和 staging topology。

## M9-C implementation — First-run and machine-local configuration

M9-C 将 First Run 接入既有 Local Control Plane 和 ConfigStore，不改变 M9-A/M9-B 的分发边界：

- **Local config authority**：唯一机器配置仍为 `%APPDATA%\ChatGPT-Harness\config.json`。缺失时由 Control Plane 创建 schema v1 的安全默认值；不持久化 `first_run` 标记。
- **First-run derivation**：Setup Required 由真实配置状态派生。默认不猜 Workspace、Runtime 路径、Tunnel ID、Tunnel client 或 API key；Workspace 只能通过 UI Folder Picker 设置。
- **Workspace setup**：Workspace 路径在保存前由后端验证为可访问目录；配置写入使用临时文件、flush/sync 和原子替换。
- **Corruption behavior**：malformed JSON、未知 schema/version 和其他安全相关未知状态均 fail closed。损坏配置不会被当作首次启动，也不会被普通保存静默覆盖；用户必须明确执行 Reset 后重新配置。
- **Start eligibility**：Control Plane 提供 `runtime_start_eligibility`，统一返回 `SetupRequired`、`ConfigurationInvalid`、`RuntimeDistributionInvalid`、`ReadyToStart` 或 `RuntimeAlreadyRunning`。UI 不自行复制启动规则。
- **Tunnel independence**：Tunnel、tunnel-client 和 API key 属于可选 External Connection 配置，不参与 Local Runtime Start Eligibility；没有 Tunnel 或 API key 时，只要 Workspace 和 Runtime distribution 有效，Local Runtime 仍可启动。
- **Secret boundary**：API key 继续只写入既有 SecretStore，`config.json` 仅保留 opaque `SecretReference`，不保存 raw secret、PID、port、Runtime instance ID 或 resolved executable path。

M9-C 不实现 installer、updater、Tunnel credential transport、telemetry 或新的 Runtime execution capability；M7-C 继续保持 `BLOCKED`。

## M9-D1 Installer Strategy

> Status: frozen design (investigation only)
> Date: 2026-09-09
> Scope: Windows V1 release packaging. This section does **not** implement an installer, an updater, a Runtime/Tunnel/SecretStore change, or a configuration-schema change.

### 1. Current packaging audit

| Area | Observed state | M9-D1 implication |
| --- | --- | --- |
| Tauri configuration | `ui/src-tauri/tauri.conf.json` is the only Tauri config. Its `bundle.active` is `false`; there are no configured bundle targets, Windows installer settings, resources, signing settings, or updater artifacts. | There is no current installer output to preserve or modify. M9-D1 only freezes the configuration that a later packaging task must add. |
| Tauri bundler support | The checked-in local Tauri CLI v2.11.4 schema supports both `nsis` and `msi` Windows targets, NSIS `currentUser` / `perMachine` modes, Windows signing fields, WebView2 modes, and app resources. | NSIS and MSI are technically available options; choosing NSIS does not require replacing Tauri. |
| Build scripts | `ui/package.json` provides `tauri:build`, but it first builds Runtime and currently emits only executable output because Tauri bundling is disabled. `distribution:stage` runs `ui/scripts/stage-distribution.ps1`. | The existing staging script is the sole release-payload boundary; it is not an installer pipeline. It must remain unchanged in M9-D1. |
| Staged payload | `stage-distribution.ps1` builds the release workspace and stages `chatgpt-harness-ui.exe`, `runtime\\chatgpt-harness-runtime.exe`, and `distribution.json` into ignored `target-exe/distribution/`. The checked workspace contains that staged layout. | The future installer must consume this exact payload topology and must not invent a second Runtime location. |
| Current artifacts | `target/release/` contains executable build outputs; no `.msi`, NSIS setup executable, ZIP release, or Tauri `bundle/` output exists. | There is no released installer format or upgrade behavior to maintain yet. |
| Runtime discovery | Installed discovery derives `<install-root>` from the UI executable and validates `<install-root>\\distribution.json` plus the fixed `runtime/chatgpt-harness-runtime.exe` path before launch. | Any installer layout that changes these paths is incompatible and must be rejected rather than supported by fallback logic. |
| Resources / sidecars | `tauri.conf.json` currently has no `bundle.resources` or `externalBin`/sidecar configuration. | The Runtime is not a Tauri resource or sidecar today; M9-D1 keeps it as an ordinary executable in the installer payload. |
| Identity and version metadata | `tauri.conf.json`, UI Cargo manifest, Runtime Cargo manifest, and `ui/package.json` are all currently `0.1.0`. `distribution.json` is generated from UI/Runtime/protocol sources. Bundle publisher, icon, signing, and installer metadata are not configured. | A future release task must establish one release version and fill bundle metadata without adding a hand-maintained version source. |
| Icons | The only current icon asset is `ui/src-tauri/icons/icon.ico`; no `bundle.icon` is configured. | Icon assets and installer/uninstaller icon metadata require release-readiness validation before the first public package; this task does not alter them. |

### 2. Installer type decision

#### Option comparison

| Option | Installation UX | Upgrade / uninstall | Permissions and per-user fit | Enterprise compatibility | V1 assessment |
| --- | --- | --- | --- | --- | --- |
| A. Tauri NSIS bundle | Familiar single Setup `.exe`; can create Start-menu entry and uninstaller. | Supports in-place replacement and registered uninstall for the same product/install root. | Tauri supports `currentUser`; no administrator elevation is needed for the intended default. | Usable for individual developers, but less preferred than MSI for managed deployment. | **Recommended primary V1 format.** |
| B. MSI | Familiar to IT deployment systems; more formal managed-install experience. | Strong managed upgrade/uninstall conventions. | Per-machine policy and administrative deployment are natural; per-user is less aligned with the V1 data and update model. | Best option for Intune/SCCM/GPO and other enterprise software distribution. | Post-V1 enterprise deliverable, not the V1 primary path. |
| C. Portable ZIP | No installation and easy offline handoff. | No registered uninstall; users can mix versions or move/delete the directory, so reliable upgrades are weak. | No elevation, but an arbitrary extract location conflicts with the fixed-install-root requirement. | May be useful only where installers are prohibited; creates support and provenance burden. | Not a supported V1 end-user release format. The existing staging directory is a build/test artifact, not a portable product. |
| D. Combination | Can serve different audiences. | Requires clear precedence and parallel test/release support. | Mixes per-user installer guarantees with uncontrolled ZIP behavior. | Could include MSI later. | Do not add a combination in V1; use NSIS only to keep one audited lifecycle. |

**Frozen V1 decision:** ship one Windows release channel: a **Tauri NSIS bundle configured for current-user installation**. MSI is explicitly deferred for a managed-enterprise release. A ZIP must not be advertised as an installed V1 artifact and must not be accepted as a workaround for installer behavior.

### 3. Install scope and directory

**Frozen install scope: per-user (current Windows user).** The later NSIS configuration must use Tauri's `currentUser` install mode, not `perMachine` or the dual-mode `both` option.

Rationale:

- Local configuration already belongs to `%APPDATA%\\ChatGPT-Harness\\config.json`; it is intentionally per-user and must remain outside the install payload.
- The existing SecretStore uses the Windows Credential Manager entry under service `ChatGPT-Harness` and key `tunnel-api-key`, which is also user-scoped. Per-machine installation would not make those credentials machine-wide and would create misleading expectations.
- The UI starts the Runtime as its child process. That lifecycle does not require an elevated service, a shared Program Files directory, or a machine-wide Runtime registration.
- Per-user installation avoids UAC/admin requirements for initial install and future manual installer upgrades, while keeping application binaries non-writable by unrelated users.
- Enterprise policy can require a different deployment model; that is the reason to add a separately tested MSI later, rather than making V1's default installer ambiguous.

**Frozen default install root:**

```text
%LOCALAPPDATA%\\Programs\\ChatGPT-Harness\\
├─ chatgpt-harness-ui.exe
├─ runtime\\
│  └─ chatgpt-harness-runtime.exe
└─ distribution.json
```

The future NSIS task must install into this one product-owned root for a given Windows user. It must not offer a Runtime-specific directory, accept a configuration-supplied Runtime path, or present arbitrary executable-location selection as a supported V1 flow. A user may choose neither a different Runtime path nor an alternate install folder in the normal V1 UX.

**Upgrade stability:** the product identifier remains `com.chatgpt.harness`; a newer installer for the same user resolves the existing per-user product registration/install root and replaces the payload in that root. It must not create a versioned or random sibling folder. The UI continues to derive the Runtime location only from its own executable's parent, so a stable root prevents UI/Runtime path drift.

### 4. Resource layout and packaging boundary

**Frozen resource decision:** the Runtime is an **installer payload executable**, copied from the M9-B staging boundary into `<install-root>\\runtime\\chatgpt-harness-runtime.exe` during installation.

It is deliberately **not**:

- a Tauri `bundle.resources` file placed beneath Tauri's resource directory;
- a Tauri sidecar with target-triple naming/discovery semantics;
- a first-launch extracted binary in `%APPDATA%`, `%TEMP%`, or another writable data location;
- a separately downloaded component; or
- a user-configurable executable path.

The future packaging implementation must take the complete staged M9-B payload as the input set:

```text
<staging-root>\\
├─ chatgpt-harness-ui.exe
├─ runtime\\
│  └─ chatgpt-harness-runtime.exe
└─ distribution.json
```

The installed layout must be byte-for-byte equivalent in topology, including the exact Runtime filename and manifest-relative path. Installer extraction is allowed only as the normal installation action into the fixed install root; there is no post-install/self-extraction design. This keeps M9-B's `current_exe()`-anchored discovery, canonical-path validation, manifest validation, and fail-closed behavior intact.

### 5. Upgrade policy and boundary

V1 provides **installer-mediated upgrades only**. It does not provide an in-app updater, background updater service, differential updater, rollback service, updater endpoint, or update credential transport.

A later release-packaging implementation must enforce the following policy:

1. A release installer contains one matched UI binary, Runtime binary, and `distribution.json` generated for the same release version/protocol set.
2. Before replacement, the installer must require the UI to be closed; because Runtime is its child, it must also ensure the Runtime is no longer running. It must not overwrite a live Runtime executable.
3. The installer replaces only the application payload in the fixed install root: `chatgpt-harness-ui.exe`, `runtime\\chatgpt-harness-runtime.exe`, and `distribution.json` (plus installer-owned shortcuts/uninstaller metadata where applicable).
4. The installed UI remains the final compatibility gate. It validates `distribution.json`, the fixed Runtime path, the manifest application/protocol identity, and Runtime handshake compatibility. A partial/incorrect package therefore fails closed as `RUNTIME_DISTRIBUTION_INVALID` rather than selecting another binary.
5. The later Tauri bundle configuration should disallow downgrades for normal releases. Downgrade/repair behavior, if ever needed for support, requires an explicit, separately tested recovery procedure rather than silently allowing mismatched files.
6. Upgrade must never rewrite, migrate, or delete `%APPDATA%\\ChatGPT-Harness\\config.json` or Windows Credential Manager entries. Any future config migration is outside this installer decision and must preserve M9-C corruption/fail-closed rules.

### 6. Uninstall policy

**Frozen V1 uninstall behavior:** uninstall removes application-owned installation artifacts only:

- the fixed install-root application binaries, including the Runtime binary;
- `distribution.json`;
- installer-created shortcuts, uninstall registration, and other installer-owned metadata.

Uninstall **preserves by default**:

- `%APPDATA%\\ChatGPT-Harness\\config.json` and any future user-local state/log data under that product-owned AppData directory; and
- the Windows Credential Manager secret represented by `ChatGPT-Harness` / `tunnel-api-key`.

Why: uninstall/reinstall or a repair install must not silently discard workspace selection or an opaque secret reference/credential, and a general uninstaller must not unexpectedly erase a user's credential. Preserving the credential does not weaken the secret boundary: it remains in Windows Credential Manager and is never copied into the installer or AppData.

A future explicit **full local-data cleanup** UX may separately ask the signed-in Windows user to delete AppData and the Credential Manager entry. It is not part of V1 uninstall and must not be implemented by this task. The uninstaller must make preservation clear to the user rather than implying a secure wipe.

### 7. Code-signing strategy

**Frozen decision:** Authenticode code signing is **required before the first externally distributed V1 release**. Unsigned artifacts may exist only as clearly labeled internal development/clean-machine test candidates; they are not public release artifacts.

The future release process must sign and timestamp:

- the NSIS setup executable;
- `chatgpt-harness-ui.exe`; and
- `runtime\\chatgpt-harness-runtime.exe`.

Use a trusted code-signing certificate, SHA-256 digest, and RFC 3161-capable timestamp service. Keep certificate private material out of the repository, installer payload, `distribution.json`, and local config. Tauri's local schema already provides Windows certificate thumbprint, digest, timestamp, and custom signing-command configuration points; M9-D1 does not enable them.

Effects:

| Area | Signing impact |
| --- | --- |
| SmartScreen | A signed, timestamped publisher identity materially improves provenance and reputation compared with an unsigned setup or child executable, although signing does not guarantee immediate reputation or eliminate all warnings. |
| Installer trust | Users and enterprise security tools can identify the publisher and detect post-signing binary modification. The Runtime must be signed too because it is launched as a separate child executable. |
| Upgrade | The same trusted publisher identity lets users and endpoint tooling assess replacement installers consistently. It does not itself implement update delivery or rollback. |
| Future updater | If an updater is introduced later, it requires a separately designed update-metadata/artifact-signature model. Authenticode alone is not an updater protocol or authorization mechanism. |

### 8. M9-E clean-machine acceptance plan

M9-E validates the future packaged release on a clean, ordinary Windows user profile. It must use a release candidate built elsewhere; the acceptance machine/profile has:

- no prior `%APPDATA%\\ChatGPT-Harness` data;
- no pre-existing `ChatGPT-Harness` Credential Manager entry;
- no source Git repository or staged distribution directory;
- no Rust toolchain, Cargo, Node.js, pnpm/npm, or developer build scripts; and
- no preinstalled ChatGPT-Harness UI/Runtime binaries, shortcuts, or uninstall registration.

A small non-secret workspace fixture is created outside the install root and selected via the product UI. The acceptance run records installer version, installed paths, `distribution.json` contents, UI/Runtime versions, and only redacted diagnostics.

| Step | Expected result / evidence |
| --- | --- |
| 1. Install signed NSIS release candidate as the clean user | Installs without administrator elevation into `%LOCALAPPDATA%\\Programs\\ChatGPT-Harness`; the exact three-item M9-B topology exists; no repo/Cargo/Node dependency is consulted. |
| 2. Launch from installed shortcut or installed UI executable | UI derives the installed root from `current_exe()`; no path is accepted from CWD, `PATH`, Git, or an environment override. |
| 3. First run | Missing AppData config yields Setup Required and creates only the safe non-secret local config state. No Tunnel ID/API key/tunnel-client is required for Local Runtime eligibility. |
| 4. Select workspace | The UI picker persists a valid accessible fixture directory to AppData. The workspace is not copied into the install root and no Runtime path is configurable. |
| 5. Start local Runtime | Eligibility becomes ready, `distribution.json` and the fixed Runtime binary validate, and the Runtime launches as the UI child process. Capture non-secret version/protocol diagnostics. |
| 6. Issue local safe tool calls | Through the local Runtime acceptance path, verify `read_file` and `glob` against the selected fixture succeed and an outside-workspace access is denied. This does not require M7-C Tunnel credential transport, which remains blocked. |
| 7. Stop and uninstall | Runtime stops before uninstall. Uninstall removes UI/Runtime/manifest/shortcuts/uninstaller registration from the fixed root, while the AppData config and Credential Manager entry remain. Verify no application binary is left in the install root. |
| 8. Reinstall (recommended continuation) | The next install uses the same stable root; existing AppData setup is recognized without a false first-run state. This confirms the chosen preservation/upgrade boundary. |

M9-E must fail if the product needs a repository, developer toolchain, arbitrary Runtime path, temporary extraction location, plaintext secret, or Tunnel availability to complete the Local Runtime/tool-call portion of the acceptance flow.

### 9. Implementation guardrails for the follow-up packaging task

The task that implements this frozen strategy may configure Tauri bundling and signing, but it must not change the following M9-A/M9-B/M9-C contracts:

- installed Runtime discovery remains `<install-root>\\runtime\\chatgpt-harness-runtime.exe` plus validated `distribution.json`;
- `%APPDATA%\\ChatGPT-Harness\\config.json` remains the machine-local config authority;
- Windows Credential Manager remains the secret authority; and
- Tunnel/external credential transport remains outside this packaging work while M7-C is blocked.

No installer behavior is implemented by this M9-D1 documentation change.

## M9-D2 Implementation — Tauri NSIS Installer Pipeline

> Status: implemented, unsigned internal build pipeline
> Date: 2026-09-09
> Scope: Windows V1 NSIS packaging only

M9-D2 enables the frozen M9-D1 strategy without changing Runtime discovery, configuration, secrets, or distribution architecture.

### Build command

From the repository root:

```powershell
npm run bundle:windows --prefix ui
```

The command first runs the existing `distribution:stage` boundary, which builds the workspace in release mode and stages the payload from the current workspace build artifacts. It then runs Tauri with the explicit `nsis` bundle target and performs a static bundle verification. No upload, signing, updater, or release automation is run.

### Generated artifact

The internal Windows installer is generated under:

```text
target\release\bundle\nsis\ChatGPT-Harness_<version>_x64-setup.exe
```

The exact version is read from the existing package metadata; the staging guard rejects a mismatch between `ui/package.json`, the UI Cargo package, and the Runtime Cargo package before bundling.

### Payload layout

The M9-B staging payload remains the source boundary and has this topology:

```text
<staging-root>\
├─ chatgpt-harness-ui.exe
├─ runtime\
│  └─ chatgpt-harness-runtime.exe
└─ distribution.json
```

Tauri maps the staged `runtime` directory and `distribution.json` into the install root, producing the same installed discovery contract:

```text
%LOCALAPPDATA%\Programs\ChatGPT-Harness\
├─ chatgpt-harness-ui.exe
├─ runtime\
│  └─ chatgpt-harness-runtime.exe
└─ distribution.json
```

The Runtime is not a sidecar, resource extracted to AppData, downloaded component, or user-configurable executable.

### Install and uninstall boundary

The NSIS bundle uses `currentUser` mode and creates installer-owned application files plus the normal Start-menu shortcut/uninstaller metadata. It does not create `config.json` or a Credential Manager secret; First Run remains owned by M9-C.

Uninstall removes application files, shortcuts, and installer registration, while preserving `%APPDATA%\ChatGPT-Harness\` and the Windows Credential Manager secret.

### Known limitations

- The generated installer is unsigned and is for internal development/clean-machine validation only; code-signing is intentionally not part of M9-D2.
- NSIS tool acquisition is performed by the installed Tauri CLI when the local NSIS toolchain is unavailable; the binary itself is never downloaded as an application payload.
- On a clean environment without a local NSIS toolchain, the Tauri CLI may need to acquire NSIS 3.11 into its official cache before bundling; this can be affected by network or proxy policy.
- M9-D2 does not implement MSI, updater, CI release, tunnel packaging, credential transport, telemetry, or automatic upload.
- Full clean-machine install/uninstall acceptance remains a separate M9-E activity.
