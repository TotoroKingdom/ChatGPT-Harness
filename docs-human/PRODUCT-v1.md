# ChatGPT-Harness v1需求
ChatGPT-Harness 是一个通过 MCP 向 ChatGPT 提供受控本地执行能力的安全 Runtime，并通过 Tauri UI 提供配置、授权和审计控制。

### 1-连通runtime--tunnel--ChatGPT
ChatGPT
│
│ OpenAI Tunnel
↓
tunnel-client
│
│ MCP
↓
Harness Runtime
│
├── read_file
├── write_file
├── edit_file
├── glob
└── bash

### 2-工具系统
bash

read_file(
path,
offset?,
limit?
)

write_file
write_file = 我知道整个文件应该长什么样
edit_file = 我只修改局部

edit_file(
path,
patch
)

glob
找文件
列目录
按模式搜索
### 3-权限模型
DENY -> 无条件拒绝，不允许用户临时批准

ASK  -> 暂停执行;   UI 请求用户批准;  允许一次 / 拒绝一次;

ALLOW -> 直接执行;  无需用户确认

#### 3-1默认规则
| 操作                    | 默认         |
| --------------------- | ---------- |
| read_file workspace 内 | ALLOW      |
| glob workspace 内      | ALLOW      |
| write_file            | ASK        |
| edit_file             | ASK        |
| bash                  | ASK        |
| workspace 外读取         | ASK / DENY |
| workspace 外写入         | DENY       |
| 明确危险命令                | DENY       |

#### 3-2
Tool Request
↓
Validation
↓
Policy Engine      ← 不能绕过
↓
Approval Manager   ← ASK 时进入
↓
Executor
↓
Result

#### 3-3 Approval Manager
至少管理：
request_id
tool
arguments
created_at
status
decision

状态：
pending
approved
denied
expired
cancelled

### 4-UI
┌──────────────────────────┐
│       Tauri UI           │
│                          │
│ config / approval / log  │
└─────────────┬────────────┘
│ IPC
↓
┌──────────────────────────┐
│      Harness Runtime     │
│                          │
│ MCP                      │
│ Tool Registry            │
│ Policy Engine            │
│ Executor                 │
│ Audit                    │
└─────────────┬────────────┘
│
↓
Local Machine

Runtime 是核心产品，UI是ChatGPT-Harness 的本地 Control Plane。
负责4件事
配置、生命周期、权限、安全可见性

布局
| 页面           | 核心职责                        |
| ------------ | --------------------------- |
| **Overview** | 运行状态、启动停止、当前 Workspace、安全模式 |
| **Tools**    | 当前暴露工具、工具能力、权限状态            |
| **Policies** | DENY / ASK / ALLOW 规则及命中范围  |
| **Activity** | Tool Call、审批、拒绝、错误、连接日志     |
| **Settings** | Tunnel、Workspace、本机路径、配置管理  |
┌──────────────────────────────────────────────┐
│ ChatGPT Harness                  ● Connected │
├────────────┬─────────────────────────────────┤
│ Overview   │                                 │
│ Tools      │          Main Content           │
│ Policies   │                                 │
│ Activity   │                                 │
│ Settings   │                                 │
│            │                                 │
│            │                                 │
│ v0.1.0     │                                 │
└────────────┴─────────────────────────────────┘