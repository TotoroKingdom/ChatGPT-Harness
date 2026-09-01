# ChatGPT-Harness v1需求

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

Runtime 是核心产品，Tauri 是 Runtime 的控制台。