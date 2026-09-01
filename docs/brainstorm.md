"# ChatGPT-Harness" 
# 架构设计
### 1-ChatGPT--mcp工具连接本地电脑

### 2-Tools调用：包含五个工具bash，read_file，write_file，edit_file，glob

### 3-权限检查：
1：拒绝列表 deny 永远禁止的操作（rm -rf /、sudo）
2：询问 ask      命中之后询问用户
3：运行 allow    用户审批

### 4-hook-V2实现
使用hook机制注册权限检查和日志记录

### 5-todo-暂时没想到用途

### 6-subAgent-控制codex claude dsh hermes pi workbuddy，后续再考虑

### 7-skills-后续考虑

### 8-上下文压缩--暂时没想到用途（chatGPT自带？）

### 9-记忆--暂时没想到用途（chatGPT自带？）

### 10-任务系统--暂时没想到用途（chatGPT自带？）

### 11-Background Tasks--异步执行任务（需要引入线程和任务编排，先不考虑）

### 12-cron 定时任务（先不考虑）

### 13-agentTeam--和6对应，后续在考虑

## V1 
#### 工具调用：包含五个工具bash，read_file，write_file，edit_file，glob
#### 权限检查：
DENY
↓
无条件拒绝，不允许用户临时批准

ASK
↓
暂停执行
UI 请求用户批准
允许一次 / 拒绝一次

ALLOW
↓
直接执行
无需用户确认

DENY > ASK > ALLOW
#### UI层
技术架构核心运行层次使用rust，UI层选Tauri