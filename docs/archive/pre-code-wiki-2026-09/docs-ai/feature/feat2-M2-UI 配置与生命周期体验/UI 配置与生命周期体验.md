你现在要一次性完成 ChatGPT-Harness Roadmap **M2 — UI 配置与生命周期体验**。

这是实现任务。直接检查现有 M0/M1 代码并完成 M2，不要先输出 spec / plan / task，不要重新设计已经冻结的架构。

# 1. 最高约束

开始前读取：

- `AGENTS.md`
- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- `docs/IPC.md`
- `docs/CONFIG.md`
- `protocol/generated/typescript/v1.ts`
- 当前 `ui/` 全部 M1 实现

优先级：

```text
PRODUCT.md
> ARCHITECTURE.md
> Roadmap.md
> M0 Contract
> 当前 M1 architecture
> 本任务
```

不要扩大 V1 scope。

---

# 2. M2 最终目标

用户必须可以真实走通：

```text
首次启动
→ First Run Setup
→ 配置 Workspace / Tunnel ID / API Key / tunnel-client
→ Run Diagnostics
→ 保存本地配置
→ Start Harness
→ 查看 Runtime / Tunnel / MCP 独立状态
→ 修改配置后看到 Restart Required / Config Mismatch
→ Stop Harness
```

此阶段：

```text
Runtime / Tunnel / MCP execution backend
```

仍然允许使用 Mock。

但以下必须是真实实现：

```text
本地 AppConfig 持久化
SecretStore abstraction
Workspace folder picker
tunnel-client file picker / detection
First Run 判断
Settings 编辑保存
配置校验
```

不要实现真实 Runtime executable、真实 MCP 或真实 tunnel-client 启停。

---

# 3. 保留 M1 架构

M1 已建立：

```text
Pages
 ↓
HarnessDataProvider / UI State
 ↓
HarnessControlClient
 ↓
MockHarnessControlClient
```

不要推翻。

M2 后仍应保持页面只依赖统一 frontend control-plane abstraction。

推荐内部结构：

```text
Pages
 ↓
HarnessDataProvider
 ↓
HarnessControlClient
 ├─ Runtime Adapter
 │    └─ Mock Runtime       ← M2
 │
 └─ Local Control Adapter
      └─ Tauri Backend
           ├─ ConfigStore
           ├─ SecretStore
           ├─ Workspace Picker
           └─ Tunnel Client Locator
```

未来 M3：

```text
Mock Runtime Adapter
        ↓ replace
Real Runtime Adapter
```

页面不得重构。

不要让页面直接散落调用：

```ts
invoke(...)
```

所有 Tauri command 调用集中在 service/adapter layer。

---

# 4. 不修改 M0 Runtime wire contract

M2 原则上不得修改：

```text
protocol/
docs/IPC.md
M0 Runtime RPC models
```

M2 新增的是：

```text
Frontend ↔ Tauri local control-plane operations
```

不是：

```text
Frontend ↔ Runtime wire protocol
```

因此配置保存、folder picker、SecretStore 等不要硬塞进 M0 Runtime RPC。

如果确实发现 M0 客观 defect，再做最小修复并重新生成全部 contract artifacts/tests，并在最终报告说明。

---

# 5. AppConfig 真实本地持久化

实现 Tauri Backend `ConfigStore`。

唯一 authority：

```text
Tauri Backend
```

Frontend 不直接读写 config file。

Runtime Mock 也不能自己持久化 AppConfig。

使用 OS application config/data location，不允许仓库内保存机器配置。

Windows V1 逻辑位置：

```text
%APPDATA%\ChatGPT-Harness\
```

通过 Tauri/OS path API 获取，不要硬编码具体用户路径。

至少保存：

```text
config.json
```

需要确保：

- 自动创建目录；
- UTF-8；
- pretty JSON；
- atomic write，避免半写文件；
- schemaVersion 校验；
- 缺失文件视为 First Run；
- 无效/损坏配置返回结构化错误，不 panic；
- 不把配置保存到 Git repo；
- 不出现开发机绝对路径默认值。

---

# 6. AppConfig 必须直接使用 M0 model

结构必须服从：

```text
AppConfig
├─ schemaVersion
├─ configRevision
├─ workspace
├─ tunnel
├─ security
└─ ui
```

不要新增第二套 domain config model。

如果 Tauri Rust Backend 可以直接依赖：

```text
harness-control-protocol
```

则直接复用 canonical Rust types。

不要复制一套：

```rust
struct UiConfigFile ...
```

来表达同一领域模型。

---

# 7. Config Revision

实现稳定、可预测的 `configRevision`。

规则保持简单：

- 首次创建配置时建立 revision；
- 每次成功保存**语义发生变化**的 AppConfig 时 revision 单调递增；
- 保存完全相同配置不得无意义增加 revision；
- revision 由 Tauri Backend 管理；
- Frontend 不自行计算 revision；
- Runtime Mock 在 Start 时记录当前 configRevision 作为 active snapshot revision。

运行过程中：

```text
Saved AppConfig revision = 8
Active Runtime revision = 7
```

必须显示：

```text
Config Mismatch
Restart Required
```

Stop 后重新 Start：

```text
Active Runtime revision = 8
```

Mismatch 清除。

M2 使用 coarse-grained Harness restart semantics 即可；不要在本阶段设计复杂 per-component hot reload。

---

# 8. SecretStore abstraction

实现：

```rust
trait SecretStore
```

或等价清晰 abstraction。

至少支持：

```text
set
exists/status
delete
```

API Key：

- raw API Key 只能交给 SecretStore；
- `config.json` 只能保存 `apiKeyRef`；
- 前端读取设置页时不得读取并回显完整 API Key；
- UI 只需要知道 `configured / not configured`；
- 更新 API Key 时覆盖 SecretStore 中对应 secret；
- 删除/重置配置时能够删除对应 secret；
- API Key 不允许进入 logs、Activity、error details、fixtures、screenshots-friendly debug state。

Windows production adapter 应使用操作系统安全凭据存储，例如 Windows Credential Manager，或当前维护良好的等价 OS credential backend。

不要使用：

```text
plaintext file
localStorage
config.json
.env
Git ignored plaintext secrets
```

作为 production SecretStore。

测试中使用 in-memory/mock SecretStore，不要依赖真实用户凭据库。

抽象层保持未来 macOS/Linux 可以增加 adapter，不要把调用方写死成 Windows API。

---

# 9. First Run detection

启动应用时：

```text
ConfigStore.load()
```

如果：

```text
config.json 不存在
```

进入 First Run。

如果 config 存在但关键配置不完整，则进入：

```text
Setup Required
```

如果配置完整：

```text
进入正常 Overview
```

不要每次启动都显示 wizard。

---

# 10. First Run Setup

实现一个清晰的桌面向导。

建议步骤：

```text
1. Welcome / Prerequisites
2. Workspace
3. Connection
4. Security
5. Diagnostics
6. Ready
```

不要做成十几个页面。

## Workspace

用户选择：

```text
Workspace root
```

必须使用真实 Tauri folder picker。

最低验证：

- 路径存在；
- 是目录；
- 当前用户可访问。

M2 不实现 WorkspaceGuard、安全边界解析、symlink/junction 防逃逸。

那是 M4。

---

# 11. Tunnel configuration

用户可以配置：

```text
Tunnel ID
tunnel-client path
Auto Detect
```

Tunnel ID：

- trim；
- 非空；
- 不记录到 Activity；
- 属于 machine-local config。

不要验证远程 tunnel 是否真正可用。

真实 tunnel startup 属于 M7。

---

# 12. tunnel-client detection

M2 实现**定位**，不实现启动。

按 PRODUCT 的优先思想检查：

```text
1. bundled / managed location if available
2. application directory
3. PATH
4. known local locations
5. manual selection
```

保持实现简单。

找到后：

- 展示 path；
- 可接受并保存；
- 用户仍可手动选择 executable。

必须验证：

```text
path exists
is file
```

如可低成本判断 executable/version，可做本地检查；不要连接远程服务。

不要修改/替换仓库里的 tunnel-client binary。

---

# 13. API Key UX

First Run / Settings 提供密码型输入。

行为：

```text
未配置：
[ Enter API Key ]

已配置：
API Key    Configured
[ Replace ]
[ Remove ]
```

不要显示：

```text
sk-abc...xyz
```

即使部分 masking 也没必要。

保存时：

```text
Frontend
→ Tauri Backend
→ SecretStore
→ 返回 SecretReference / configured state
```

Frontend 不持久化 raw secret。

保存完成后清空输入框内 raw API Key。

---

# 14. Security Mode / Developer Test Mode

Settings / First Run 提供：

```text
Normal Mode
Developer/Test Mode
```

M0 schema 里同时有：

```text
security.mode
security.developerTestMode
```

M2 必须保持二者一致：

Normal：

```text
mode = normal
developerTestMode = false
```

Developer/Test：

```text
mode = developer_test
developerTestMode = true
```

不要允许保存矛盾状态。

启用 Developer/Test Mode 时显示明确 warning，例如：

```text
Developer/Test Mode may expose higher-risk capabilities in later milestones.
```

M2 不实现 bash，不实现 Approval Manager。

---

# 15. Settings Page 从 skeleton 变成真实配置页

保留当前页面结构，但启用：

```text
Workspace
Connection
Security
Local Data
```

至少支持：

### Workspace
- current path
- Choose Folder

### Connection
- Tunnel ID
- auto detect tunnel-client
- manual tunnel-client picker
- API Key configured state
- replace/remove API Key

### Security
- Normal
- Developer/Test Mode

### Local Data
展示：

```text
Config directory
Config file path
Secret provider name
```

不要显示 raw secret。

提供：

```text
Save Changes
Discard Changes
```

需要：

- dirty state；
- field validation；
- saving state；
- save error；
- save success feedback。

不要 input 每输入一个字符就写磁盘。

---

# 16. First Run 保存行为

完成配置后：

1. 校验 Workspace；
2. 校验 Tunnel ID；
3. 确认 tunnel-client path 或 detection result；
4. API Key 写入 SecretStore；
5. 生成/更新 `apiKeyRef`；
6. 保存 AppConfig；
7. Run Diagnostics；
8. 进入 Ready step。

任何一步失败：

```text
不要写出“看似成功”的半完成状态。
```

尤其 SecretStore 保存失败时，不得假装 API Key configured。

---

# 17. Diagnostics

M2 必须有真实可操作：

```text
Run Diagnostics
```

但不要伪造真实 Runtime/MCP/tunnel 网络健康。

Diagnostics 分成两类：

### M2 可以真实检查

- AppConfig 是否可读/合法；
- Workspace 是否存在/可读；
- Tunnel ID 是否配置；
- API Key Secret 是否存在；
- tunnel-client 是否找到；
- tunnel-client path 是否是文件；
- Security config 是否一致。

### Runtime/MCP/tunnel execution checks

因为 M3/M7 尚未实现：

使用 Mock Runtime diagnostics / 明确的 development result。

不得向用户声称：

```text
MCP reachable
Tunnel connected
Runtime binary healthy
```

如果实际上没有执行这些检查。

可以显示：

```text
Not available until Runtime integration
```

或 Warning。

Diagnostics result 仍必须服从 M0：

```text
DiagnosticRun
DiagnosticCheck
pass / warning / fail
```

---

# 18. Lifecycle model

M2 必须实现最终用户可见的生命周期体验，但 backend 仍使用 deterministic Mock orchestration。

Start Harness：

```text
Validate saved config
→ ensure setup complete
→ run/confirm diagnostics
→ Runtime: starting
→ Runtime: ready
→ Tunnel: starting
→ Tunnel: ready
→ MCP: starting
→ MCP: ready
→ Overall: Ready
```

Stop Harness：

```text
Overall: Stopping
→ MCP: stopping/stopped
→ Tunnel: stopping/stopped
→ Runtime: stopping/stopped
→ Overall: Stopped
```

使用短、deterministic 的 mock delays 即可。

不要使用复杂随机失败。

---

# 19. Runtime / Tunnel / MCP 状态必须独立

Overview 必须明确展示三块独立 observed state：

```text
Runtime
Tunnel
MCP
```

不要合成一个布尔：

```text
isRunning
```

每个组件至少能表达：

```text
stopped
starting
ready
degraded
error
stopping
```

这是 **Tauri/Harness orchestration state**，不是重新定义 M0 `RuntimeState`。

Runtime 真正 domain data 继续使用 M0：

```text
RuntimeState
```

不要创建第二套 RuntimeState。

---

# 20. Overall Harness State

Overview 同时可以有一个 composed overall state：

```text
Stopped
Starting
Ready
Degraded
Error
Stopping
```

它由 control-plane orchestration state组合得到。

规则保持简单、集中在一个 selector/function 中。

禁止每个页面自己推导。

至少：

```text
全部 stopped      → Stopped
启动过程中        → Starting
三个都 ready      → Ready
任一 error        → Error
部分 ready/异常    → Degraded
停止过程中        → Stopping
```

不要把安全策略塞进这个 selector。

---

# 21. Start Harness

Overview 提供真实可点击：

```text
Start Harness
```

M2 行为：

- 必须有完整 saved config；
- API Key configured；
- Workspace valid；
- tunnel-client location valid；
- 如果配置不完整，阻止 Start 并链接到 Settings/Setup；
- 防止重复 Start；
- starting 时按钮 disabled；
- success 后 Overview 更新为 Ready；
- active Runtime Mock state 记录启动时 configRevision。

不要 spawn `runtime.exe`。

那是 M3。

不要 spawn tunnel-client。

那是 M7。

---

# 22. Stop Harness

提供：

```text
Stop Harness
```

要求：

- stopped 时 disabled；
- stopping 时 disabled；
- mock lifecycle 顺序稳定；
- 完成后 Runtime/Tunnel/MCP 均为 stopped；
- active runtime snapshot 清除或标记 inactive；
- Activity 中产生对应结构化 mock lifecycle events。

不要 kill 真实进程。

---

# 23. Restart Required / Config Mismatch

当 Harness 正在运行：

```text
active configRevision = N
```

用户保存新配置：

```text
saved configRevision = N + 1
```

Overview / Settings 都必须明确显示：

```text
Restart Required
Configuration has changed since the current Harness session started.
```

提供：

```text
Restart Harness
```

可以作为：

```text
Stop
→ Start
```

的 M2 mock orchestration convenience。

不要热修改 Mock Runtime active config 来“消除 mismatch”。

只有重新 Start 后 mismatch 才清除。

---

# 24. 未运行时不显示假 mismatch

如果 Harness stopped：

即使：

```text
saved configRevision
```

变化，也不需要显示 Restart Required。

因为没有 active Runtime snapshot 可以 mismatch。

---

# 25. First Run → Diagnostics → Start → Stop 必须完整走通

这是 M2 最核心验收路径。

必须手工验证：

```text
删除/移动本机 M2 config
→ 启动 app
→ 自动进入 First Run
→ 选择 Workspace
→ 填 Tunnel ID
→ detect/select tunnel-client
→ 输入 API Key
→ 选择 security mode
→ Save
→ Run Diagnostics
→ Finish Setup
→ Start Harness
→ Runtime/Tunnel/MCP 依次 Ready
→ Stop Harness
→ 全部 Stopped
→ 关闭应用
→ 再次打开
→ 不再进入 First Run
→ 配置仍存在
→ API Key 只显示 Configured
```

不要为了测试删除用户真实凭据；使用专用 app secret key/reference。

---

# 26. Mock scenarios 保留

M1：

```text
?mock=ready
?mock=disconnected
?mock=empty
?mock=error
```

不要破坏。

可以扩展用于 M2 deterministic testing，但不要让 URL mock scenario 成为 production 核心状态来源。

M2 正常 desktop dev flow 应能通过真实本地 ConfigStore + Mock Runtime 工作。

---

# 27. Activity

M2 lifecycle 和 config operations 增加结构化 UI/mock events。

可以包含：

```text
runtime_started
runtime_stopped
```

以及 M0 已允许的相关 event。

不要为了表示：

```text
config_saved
tunnel_started
```

擅自扩展 M0 `ActivityEventType`，除非确实有架构必要。

非 M0 event 可留在 control-plane UI feedback，不要污染 Runtime AuditEvent contract。

禁止 raw logs。

---

# 28. Error handling

所有本地控制面错误统一结构化处理，例如：

```text
CONFIG_NOT_FOUND
CONFIG_INVALID
CONFIG_WRITE_FAILED
SECRET_STORE_FAILED
WORKSPACE_INVALID
TUNNEL_CLIENT_NOT_FOUND
SETUP_INCOMPLETE
LIFECYCLE_CONFLICT
```

这些属于 Frontend/Tauri local control-plane errors，不需要修改 Runtime M0 error enum。

用户看到 human-readable message。

不要默认显示：

- raw Rust errors；
- stack traces；
- secret values；
- full invoke payload。

---

# 29. Config corruption handling

如果 config.json 存在但 JSON/schema 无效：

不要覆盖原文件。

UI 显示：

```text
Configuration needs attention.
```

提供：

```text
Open Settings / Reset Configuration
```

Reset 必须有确认。

重置 AppConfig 不得意外泄漏或展示旧 API Key。

如果删除关联 secret，也需要明确执行。

---

# 30. Tauri commands

具体命令名由实现决定，但集中且语义清晰。

类似：

```text
config_get
config_save
config_reset

secret_api_key_status
secret_api_key_set
secret_api_key_delete

workspace_choose

tunnel_client_detect
tunnel_client_choose

local_diagnostics_run
```

不要在 React 页面直接 invoke。

统一经过 adapter/client。

---

# 31. Rust Backend layering

保持简单：

```text
src-tauri/
├─ commands/
├─ config/
├─ secrets/
├─ platform/
└─ state/
```

或等价结构。

避免把所有代码塞进：

```text
lib.rs / main.rs
```

但也不要过度拆成大量空 module。

---

# 32. Tests — Rust

至少测试：

### ConfigStore
- first run / missing config；
- save/load roundtrip；
- revision increases on semantic changes；
- identical save does not increase；
- invalid JSON does not panic；
- config remains outside repo in production path abstraction；
- atomic write helper。

测试使用 temp directory。

### SecretStore
使用 fake/in-memory implementation：

- set；
- exists；
- replace；
- delete；
- raw secret 不进入 AppConfig serialization。

### Validation
- invalid workspace；
- empty Tunnel ID；
- inconsistent security fields；
- setup incomplete。

不要让 unit test 修改真实 `%APPDATA%` 或 Windows Credential Manager。

---

# 33. Tests — Frontend

至少覆盖：

### First Run
无配置时显示 Setup。

### Existing config
已有完整配置时进入正常 shell。

### Settings
编辑 → dirty → save → clean。

### API Key
configured 状态不显示 raw secret。

### Start / Stop
完整 mock lifecycle。

### Independent status
Runtime / Tunnel / MCP 独立显示。

### Restart required
运行时保存新 config → 显示 restart required；
Restart → mismatch 清除。

### Diagnostics
pass/warning/fail 正确展示。

### Existing M1 tests
继续通过。

---

# 34. UI/UX

保持 M1 视觉语言，不重新设计整个应用。

First Run 应该像 desktop setup flow，不是后台管理表单。

Overview 应该一眼看到：

```text
Harness status
Runtime
Tunnel
MCP
Workspace
Security Mode
Restart Required
```

Settings 避免信息过载。

Danger/warning 只用于真正需要注意的状态。

Developer/Test Mode 要明显区别于 Normal Mode。

---

# 35. README

更新：

```text
ui/README.md
```

补充：

- First Run dev flow；
- AppConfig 存储位置；
- SecretStore backend；
- 如何清理开发配置重新测试 First Run；
- Mock Runtime 仍存在；
- M3 如何替换 Mock Runtime。

不要写成长篇架构文档。

---

# 36. 不要提前实现 M3+

禁止实现：

```text
真实 Runtime executable
stdio Runtime process supervisor
Runtime initialize handshake implementation
真实 Runtime status RPC

WorkspaceGuard
PathResolver

MCP server
read_file
glob

Policy Engine

真实 tunnel-client process startup
真实 MCP reachability

bash
Approval Manager execution

Git/DB/Redis/ES/MinIO tools
Agent loop
Skills
Memory
Background tasks
```

特别注意：

M2 `Start Harness` 是 **Mock lifecycle experience**。

M3 才是：

```text
real Runtime process
```

M7 才是：

```text
real tunnel-client
```

不要越界。

---

# 37. 验证命令

完成后必须实际执行。

Frontend：

```text
cd ui
npm install
npm run typecheck
npm run test
npm run build
```

Rust：

```text
cd repo-root
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Protocol：

```text
cargo run -p harness-control-protocol --bin protocol-export -- generate-all
git diff
```

如果没有修改 M0 canonical models，protocol generation 不应产生意外 diff。

Desktop：

```text
cd ui
npm run tauri dev
```

实际手工验证：

```text
First Run
Settings
Diagnostics
Start
Runtime/Tunnel/MCP status
Config change → Restart Required
Restart
Stop
App restart → persisted config
```

验证后关闭 dev process。

---

# 38. Definition of Done

只有全部满足才关闭 M2：

- First Run Setup 可用；
- Workspace folder picker 可用；
- Tunnel ID 可配置；
- tunnel-client 可 auto detect / manual select；
- API Key 使用真实 SecretStore abstraction 保存；
- raw API Key 不进入 AppConfig；
- Developer/Test Mode 可配置；
- Settings 可真实保存和重新加载；
- AppConfig 位于 OS app-data，而非仓库；
- configRevision 正常工作；
- Diagnostics 可运行；
- Runtime/Tunnel/MCP 独立状态可见；
- Start Harness 可完整运行 Mock lifecycle；
- Stop Harness 可完整运行 Mock lifecycle；
- 运行时改配置会出现 Config Mismatch / Restart Required；
- Restart 后 mismatch 清除；
- 第二次启动不再进入 First Run；
- M1 五个页面继续工作；
- 所有页面仍通过统一 control-plane/service abstraction；
- 没有复制 M0 Runtime domain models；
- frontend tests/typecheck/build 全通过；
- Cargo workspace tests/check 全通过；
- Tauri app 实际启动并完成核心用户流程；
- 未实现 M3+ 真实 Runtime/MCP/tunnel execution。

完成后只向我汇报：

1. 主要新增/修改文件；
2. ConfigStore 和实际配置路径；
3. SecretStore abstraction 和 Windows backend；
4. First Run 流程；
5. Start/Stop/Restart mock lifecycle；
6. Runtime/Tunnel/MCP 状态模型如何落地；
7. Diagnostics 做了哪些真实检查、哪些仍是 Mock/Unavailable；
8. 是否修改 M0 contract；
9. 执行的验证命令和结果；
10. 是否有未完成项或约束冲突。

不要再输出新的架构 proposal。