你现在要一次性完成 ChatGPT-Harness Roadmap **M1 — Tauri UI Skeleton**。

这是一个实现任务。直接修改代码、运行验证、完成 M1；不要先输出 spec / plan / task 分解，也不要重新讨论 M0 或 V1 架构。

# 1. 最高约束

开始前先读取：

- `AGENTS.md`
- `docs-ai/PRODUCT.md`
- `docs-ai/ARCHITECTURE.md`
- `docs-ai/Roadmap.md`
- `docs/IPC.md`
- `docs/CONFIG.md`
- `protocol/generated/typescript/v1.ts`
- `protocol/fixtures/v1/*`

优先级：

```text
PRODUCT.md
> ARCHITECTURE.md
> Roadmap.md
> M0 IPC/CONFIG contract
> 本任务
```

M0 已冻结。

除非发现一个**客观导致 generated contract 无法被 UI 消费的缺陷**，否则不要修改 M0 protocol。

如果确实发现 M0 contract defect：

1. 不要在 UI 手写 workaround 类型；
2. 在 canonical Rust protocol source 做最小修复；
3. 重新生成 TypeScript / JSON Schema / fixtures；
4. 运行 M0 contract tests；
5. 在最终报告明确说明。

---

# 2. M1 目标

完成一个**真正可运行的 Tauri 桌面应用骨架**：

```text
ChatGPT-Harness
├─ Overview
├─ Tools
├─ Policies
├─ Activity
└─ Settings
```

同时建立：

```text
UI
 ↓
UI State / Query Layer
 ↓
HarnessControlClient
 ↓
MockHarnessControlClient   ← M1
```

未来：

```text
UI
 ↓
UI State / Query Layer
 ↓
HarnessControlClient
 ↓
TauriHarnessControlClient  ← M3+
 ↓
Tauri Backend
 ↓
Runtime
```

M3 替换 Mock 时：

**页面组件、页面状态结构和导航架构不得重构。**

---

# 3. 技术栈

如果仓库当前没有其它已经冻结的前端技术栈，则使用：

```text
Tauri 2.x
React
TypeScript
Vite
```

要求：

- TypeScript strict mode。
- 使用当前稳定版本。
- 使用 npm 管理前端依赖，除非仓库已经存在其它 package manager。
- 不引入大型 UI framework。
- 可以使用轻量 icon library。
- 样式优先使用普通 CSS / CSS Modules。
- 不要为了 M1 引入 Redux 等重型状态框架。
- 需要全局状态时使用轻量、清晰的 React state/store abstraction。

目标是：

**简单、可维护、容易让 Codex 后续继续迭代。**

---

# 4. 目录

创建：

```text
ui/
├─ package.json
├─ vite.config.*
├─ tsconfig*.json
├─ index.html
│
├─ src/
│  ├─ main.tsx
│  ├─ App.tsx
│  │
│  ├─ app/
│  │  ├─ router/
│  │  ├─ providers/
│  │  └─ store/
│  │
│  ├─ components/
│  │  ├─ layout/
│  │  ├─ status/
│  │  ├─ feedback/
│  │  └─ common/
│  │
│  ├─ pages/
│  │  ├─ overview/
│  │  ├─ tools/
│  │  ├─ policies/
│  │  ├─ activity/
│  │  └─ settings/
│  │
│  ├─ services/
│  │  └─ harness/
│  │     ├─ client.ts
│  │     ├─ mock-client.ts
│  │     ├─ factory.ts
│  │     └─ errors.ts
│  │
│  └─ styles/
│
└─ src-tauri/
```

可以根据 Tauri scaffold 做小幅调整。

不要制造过度抽象。

---

# 5. Cargo workspace

将：

```text
ui/src-tauri
```

正确加入现有根 Cargo workspace。

不得破坏：

```text
protocol/
```

现有 M0 crate。

最终：

```text
cargo check --workspace
cargo test --workspace
```

仍然必须通过。

---

# 6. M0 generated types 是唯一协议类型来源

UI 必须直接消费：

```text
protocol/generated/typescript/v1.ts
```

禁止重新手写：

```text
RuntimeState
ToolDescriptor
PolicyRule
ActivityEvent
ApprovalRequest
DiagnosticRun
SecurityMode
PolicyDecision
```

等 M0 已定义类型。

禁止出现：

```ts
interface UiRuntimeState { ... }
interface ToolRow { ...业务字段... }
interface PolicyModel { ... }
```

这种第二套 domain model。

允许定义纯 presentation state，例如：

```text
selectedTab
searchQuery
sortOrder
isSidebarCollapsed
```

但 presentation state 不能重新定义 Runtime domain state。

---

# 7. HarnessControlClient

建立 UI 唯一数据访问边界：

```ts
HarnessControlClient
```

必须覆盖 M0 已冻结能力：

```text
runtime.start       // facade
runtime.status
runtime.stop

tools.list

policies.list

activity.list
activity.subscribe

approvals.list
approvals.resolve

diagnostics.run
```

TypeScript 方法名可以采用正常 camelCase，但必须清楚映射 M0 method，例如：

```text
runtimeStatus()   → runtime.status
toolsList()       → tools.list
policiesList()    → policies.list
```

不要让页面组件知道 RPC envelope。

页面不能：

```text
construct RpcRequest
inspect JSON-RPC errors
know stdio
know JSONL
know Tauri invoke details
```

页面只调用 typed client。

---

# 8. Client factory

建立统一入口：

```text
createHarnessControlClient()
```

M1：

```text
createHarnessControlClient()
        ↓
MockHarnessControlClient
```

以后只需要替换这里或通过 provider 注入：

```text
TauriHarnessControlClient
```

不得在各页面：

```ts
new MockHarnessControlClient()
```

---

# 9. MockHarnessControlClient

实现一个真正遵守 `HarnessControlClient` 的 Mock。

Mock 数据必须符合 M0 generated types。

优先参考：

```text
protocol/fixtures/v1/
```

Mock 不允许定义第二套简化模型。

建议提供正常默认场景：

```text
Runtime:
ready

Workspace:
D:\example-project
```

注意：

这只能是**明显的 mock/example path**，不得使用开发者真实机器路径。

Normal Mode：

```text
read_file → exposed
glob      → exposed
bash      → not exposed
```

Policies：

包含：

```text
ALLOW
ASK
DENY
```

用于展示完整 UI。

Activity：

提供若干结构化事件。

Approval：

提供至少：

```text
0 或 1 个 pending example
```

用于组件骨架展示。

---

# 10. Mock 必须模拟异步

不要写成全部同步返回常量。

例如模拟：

```text
50–200ms
```

的异步响应。

这样 UI 的 Loading state 从 M1 就是真实存在的，而不是以后再重构。

不要做复杂随机行为。

测试必须 deterministic。

---

# 11. 页面不能直接 import mock data

这是硬要求。

禁止：

```text
ToolsPage
  import mockTools
```

正确：

```text
ToolsPage
   ↓
UI data hook/store
   ↓
HarnessControlClient.toolsList()
```

所有 Runtime/domain 数据必须经过统一 client。

---

# 12. UI 数据状态

所有异步数据至少能够表达：

```text
idle
loading
success
empty
error
disconnected
```

可以设计一个简单统一模型：

```text
AsyncState<T>
```

或等价方案。

不要让五个页面各自发明不同 loading/error 模式。

---

# 13. Runtime disconnected state

M1 必须明确支持：

```text
Runtime not connected
```

这不是 error stack。

应该是正常产品状态。

例如：

```text
Runtime
Not connected

The local Runtime is not running.
Runtime controls will be available in the next milestone.
```

不要假装 Runtime 永远在线。

---

# 14. Application Shell

实现现代 desktop control-plane shell。

建议：

```text
┌─────────────────────────────────────────────────┐
│ Sidebar       │ Header                          │
│               ├─────────────────────────────────┤
│ Overview      │                                 │
│ Tools         │         Page Content            │
│ Policies      │                                 │
│ Activity      │                                 │
│ Settings      │                                 │
│               │                                 │
└─────────────────────────────────────────────────┘
```

左侧导航至少：

```text
Overview
Tools
Policies
Activity
Settings
```

当前页面有明确 active state。

窗口尺寸变化时布局不能明显破坏。

优先 desktop 使用体验，不需要为手机设计。

---

# 15. Visual Direction

按照 PRODUCT.md：

做成：

```text
modern developer desktop control plane
```

不要做成：

```text
CMD launcher
日志查看器
传统 WinForms 配置工具
网页后台管理系统
```

视觉重点：

```text
状态清晰
安全状态清晰
低噪音
层级明确
桌面感
```

使用：

- 简洁 sidebar
- restrained cards
- clear typography
- status badges
- subtle borders
- consistent spacing
- hover/focus states
- semantic state colors

颜色只承担语义：

```text
success
warning
danger
neutral
```

不要大面积高饱和色。

---

# 16. Overview

M1 Overview 只展示当前 M0 能可靠表达的内容。

至少：

### Runtime status

```text
lifecycle
readiness
runtimeId
startedAt
```

### Active workspace

```text
activeWorkspace
```

### Security Mode

```text
normal / developer_test
```

### Capability summary

```text
exposed tools count
pending approvals
active requests
running executions
```

### Last error

有则显示。

无则不显示错误区域。

---

不要在 M1 自行定义：

```text
TunnelState
McpState
HarnessOverallState
```

这些属于后续 lifecycle/orchestration milestone。

可以留视觉 placeholder，但不要创造新的业务状态模型。

---

# 17. Overview actions

M2 才负责真实：

```text
Start Harness
Stop Harness
Run Diagnostics
```

所以 M1：

可以展示 primary action 的**视觉位置/disabled skeleton**，但不要实现假生命周期逻辑。

例如：

```text
Start Harness
disabled
```

并明确：

```text
Available in M2
```

不要为了“按钮能点”偷偷实现 M2。

---

# 18. Tools page

目标：

> What can ChatGPT do right now?

通过：

```text
client.toolsList()
```

加载。

每个 tool 展示：

```text
name
description
exposed
enabled
implemented
effectivePolicy
scope
readiness
riskLevel
```

视觉上必须能清晰区分：

```text
Exposed
Disabled
Unavailable
```

Normal Mode 要明显看出：

```text
read_file
glob
```

是 exposed。

`bash`：

```text
not exposed
```

如果 Mock fixture 中包含它。

允许加入 presentation-only：

```text
search
filter
```

但不要实现工具开关。

M1 Tools 是 read-only。

---

# 19. Policies page

通过：

```text
client.policiesList()
```

获取。

按：

```text
DENY
ASK
ALLOW
```

分组或筛选。

每条至少展示：

```text
name
decision
scope
reason
source
editable
enabled
```

SYSTEM DENY 不允许出现编辑按钮。

事实上 M1 整个 Policies page 都是 read-only。

不要实现 Policy Engine。

不要实现规则编辑。

---

# 20. Activity page

通过：

```text
client.activityList()
```

加载历史事件。

然后通过：

```text
client.activitySubscribe()
```

接收后续 Mock event。

页面展示结构化 Activity，不显示 raw log console。

至少展示：

```text
timestamp
eventType
summary
requestId?
```

metadata 可在 secondary/detail disclosure 中展示。

不要让 metadata 成为主要内容。

---

# 21. Activity subscription lifecycle

React 页面 mounted：

```text
subscribe
```

unmounted：

```text
unsubscribe/cleanup
```

不能产生重复 listener。

不能因为导航 Activity 页面多次导致事件重复出现。

需要测试这一点或至少验证 cleanup。

---

# 22. Settings page

M1 只做 Settings UI skeleton。

M2 才实现：

```text
Workspace selection
Tunnel ID
API Key
tunnel-client path
Developer/Test Mode
config persistence
SecretStore
```

因此 M1 Settings 可以展示这些 section：

```text
Workspace
Connection
Security
Local data
```

但输入控件必须：

```text
disabled / read-only
```

并明确：

```text
Configuration becomes available in M2.
```

禁止：

- 写 JSON config
- 保存 API key
- 使用 localStorage 冒充 AppConfig
- 实现 SecretStore
- 实现 workspace picker

---

# 23. Approval UI

Roadmap M1 没有要求完整 Approval page。

不要增加第六个主页面。

但 Overview 可以展示：

```text
pendingApprovals count
```

Activity 可以展示 approval events。

真正 approval interaction 属于 M8。

不要提前实现 Allow Once / Deny dialog。

---

# 24. Diagnostics

M1 不实现真实 Diagnostics。

`HarnessControlClient` 要包含：

```text
diagnosticsRun()
```

以保证 abstraction 完整。

但页面不需要实现完整诊断体验。

属于 M2。

---

# 25. Error handling

建立统一 presentation。

至少：

```text
ErrorState
EmptyState
LoadingState
DisconnectedState
```

不要每页复制一套 CSS。

RPC/domain error 在 client/service layer 转换成适合 UI 的错误对象。

UI 不允许：

```text
if (error.code === -32602)
```

这种解析 RPC transport 的逻辑。

---

# 26. Error messages

用户默认看到：

```text
human-readable message
```

开发信息可以次级展示。

不要默认显示：

```text
stack trace
raw JSON
Rust panic
RPC envelope
```

---

# 27. 可测试 Mock scenarios

为了快速验证所有 M1 state，可以让 Mock client 支持 deterministic scenarios，例如：

```text
ready
disconnected
loading
empty
error
```

实现方式保持简单。

推荐开发环境通过：

```text
?mock=ready
?mock=disconnected
?mock=empty
?mock=error
```

切换。

不要把 Mock scenario selector 做成正式产品 UI。

Production build 中不要出现显眼的 Mock 控制面板。

---

# 28. State store

建立一个轻量 UI state/data layer。

它负责：

```text
RuntimeState
Tools
Policies
Activity
```

的数据获取状态。

页面负责展示。

禁止页面负责：

```text
policy decision
runtime readiness inference
security policy
tool exposure calculation
```

例如：

禁止：

```ts
if (tool.name === "bash" && mode === "normal") {
  exposed = false
}
```

UI 必须相信 Runtime 返回的：

```text
tool.exposed
tool.effectivePolicy
```

Mock client 负责提供符合 contract 的状态。

---

# 29. 不要 duplicate source of truth

例如 exposed tool count：

可以：

```text
runtimeState.exposedToolCatalog.length
```

或基于 Runtime 提供的 catalog 做 presentation aggregation。

但不要维护一个：

```ts
const NORMAL_MODE_TOOLS = [...]
```

然后拿它作为业务 source of truth。

同样不要在 UI 写：

```ts
DENY > ASK > ALLOW
```

作为实际 policy evaluation。

UI 只展示。

---

# 30. Testing

加入合理的 frontend tests。

不需要追求高覆盖率。

至少测试：

### Navigation

五个主页面都可以进入。

### Client abstraction

页面通过 HarnessControlClient 获取数据。

### Tools

Mock tools 正确渲染。

### Policies

ALLOW / ASK / DENY 正确展示。

### Async states

至少验证：

```text
loading
error
empty
disconnected
```

### Activity

订阅 event 可以追加。

cleanup 后不会继续更新。

### M0 types

UI 可以直接编译消费：

```text
protocol/generated/typescript/v1.ts
```

---

# 31. M0 contract consumability check

实现 UI 前实际验证 generated TypeScript 和 canonical fixtures 能否自然用于 TypeScript/JSON boundary。

特别注意：

M0 某些 `u64` 字段在 generated TypeScript 中可能表现为：

```ts
bigint
```

而标准 JSON parse/stringify 对 bigint 有限制。

不要忽略这个问题，也不要在 UI 复制第二套类型掩盖问题。

如果它确实导致：

```text
fixtures 无法类型安全消费
或
未来 RPC JSON boundary 无法 round-trip
```

则：

- 把它视为 M0 contract bug；
- 在 canonical Rust protocol 中做最小、明确修复；
- 推荐选择稳定且 JSON-safe 的 wire representation；
- regenerate TypeScript / JSON Schema / fixtures；
- 重跑 M0 tests；
- 然后继续 M1。

不要因为潜在问题主动大改 M0；先验证，再决定。

---

# 32. Build scripts

至少提供：

```text
npm run dev
npm run build
npm run typecheck
npm run test
npm run tauri
```

具体 script 名可按 Tauri 标准调整，但 README/package.json 要清晰。

---

# 33. 实际运行验证

完成后必须实际执行：

```text
npm install
npm run typecheck
npm run test
npm run build
```

然后根目录：

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

还必须实际启动：

```text
npm run tauri dev
```

确认桌面窗口能够打开。

实际检查：

```text
Overview
Tools
Policies
Activity
Settings
```

五页都能导航且没有 runtime crash / frontend fatal error。

验证完成后结束 dev process。

如果环境无法启动 GUI，需要明确报告原因，但仍完成其它验证。

---

# 34. 不要执行 Tauri release packaging

M1 只要求可运行 desktop skeleton。

不需要：

```text
installer
MSI
NSIS
signed build
release artifact
```

这些属于 M9。

---

# 35. README

在：

```text
ui/README.md
```

只写开发者必要信息：

```text
prerequisites
install
dev
test
build
Mock client architecture
how real client will replace Mock
```

不要写大篇架构文档。

M0 架构已经在 docs 中。

---

# 36. M1 非目标

绝对不要顺手实现：

```text
真实 Runtime
Runtime process supervisor
stdio RPC transport
MCP
tunnel-client integration

First Run Setup
Workspace picker
API Key storage
Tunnel ID persistence
SecretStore
config persistence

real Start Harness
real Stop Harness
real Diagnostics

WorkspaceGuard
read_file
glob
bash

Policy Engine
Approval Manager

Git tools
Database tools
Skills
Agent Loop
Memory
Tasks
```

这些全部属于后续 milestone。

---

# 37. UX 验收

最终应用必须达到：

### Overview

打开就能理解 Runtime mock 当前状态。

### Tools

能快速看出：

```text
ChatGPT 当前暴露哪些 tools。
```

### Policies

能快速看出：

```text
ALLOW / ASK / DENY。
```

### Activity

能看到结构化事件，不是 raw log。

### Settings

页面结构完整，但 M2 controls 明确尚未启用。

### Navigation

五个页面切换流畅。

### States

Loading / Error / Empty / Disconnected 都有正常视觉表现。

---

# 38. Architecture 验收

必须满足：

```text
Page
  ↓
UI state/data layer
  ↓
HarnessControlClient
  ↓
MockHarnessControlClient
```

禁止：

```text
Page
  ↓
mock JSON
```

禁止：

```text
Page
  ↓
Tauri invoke
```

禁止：

```text
Page
  ↓
business/security logic
```

未来把：

```text
MockHarnessControlClient
```

替换为：

```text
TauriHarnessControlClient
```

时：

**页面不需要重构。**

这是 M1 最重要的技术验收标准。

---

# 39. Definition of Done

只有以下全部满足才算完成 M1：

- Tauri 2 desktop app 可以启动。
- React + TypeScript UI 可以运行。
- 有 Overview / Tools / Policies / Activity / Settings 五页。
- 有统一导航 shell。
- 有统一 HarnessControlClient abstraction。
- 有 MockHarnessControlClient。
- Mock 完全服从 M0 contract。
- UI 直接消费 generated TypeScript。
- 没有复制 Runtime/Tool/Policy/Activity 等 domain model。
- Runtime disconnected state 存在。
- Loading state 存在。
- Error state 存在。
- Empty state 存在。
- Activity subscription 有正确 cleanup。
- Tools 页面没有安全逻辑。
- Policies 页面没有 Policy Engine 逻辑。
- Settings 没有提前实现 M2 persistence。
- npm typecheck 通过。
- frontend tests 通过。
- frontend production build 通过。
- Cargo workspace check/test 通过。
- Tauri desktop window 实际成功启动。
- 没有实现任何 M2+ 功能。

完成后只向我汇报：

1. 新增/修改了哪些主要文件；
2. 五个页面分别实现了什么；
3. `HarnessControlClient` / Mock 架构怎么落地；
4. 是否修改了 M0 contract；如果有，为什么；
5. 执行的验证命令及结果；
6. 是否有未完成项或阻塞。

不要再输出新的架构 proposal。