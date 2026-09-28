# M0 E2E Test Plan — External ChatGPT Write/Edit

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M0 — Write/Edit Capability Acceptance Gate  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`  
> Scope: real External ChatGPT → Tunnel → Runtime MCP → Workspace filesystem.

## 1. Objective

验证 M0 的真正 blocking requirement：真实 ChatGPT 产品通过 Tunnel 发现并调用 Harness 的 `write` / `edit`，且所有请求仍经过 Runtime security / policy / execution path。

唯一有效 E2E 链路：

```text
External ChatGPT
→ Tunnel Client
→ Runtime MCP
→ MCP Adapter
→ ExecutionPipeline
→ WorkspaceGuard
→ PolicyEngine
→ ExecutionPermit
→ write/edit executor
→ Workspace filesystem
→ Activity/Audit
```

任何 local HTTP、MCP Inspector、mock、unit/integration test 均不属于本计划的 PASS 证据。

## 2. Entry Criteria

执行前必须满足：

- M0 T1–T5 开发完成。
- `INTEGRATION_TEST_PLAN.md` 已 PASS。
- Harness Runtime/Tunnel 能按 V2 已验收生命周期正常启动。
- 当前测试 Workspace 是专用临时/验收目录，不使用重要用户文件。
- Tunnel credential / API Key 有效，且 secret 不写入测试记录。
- External ChatGPT 会话已连接该 Harness MCP。

如果 ChatGPT 产品侧当前无法调用 write/modify MCP action，则停止测试并记录：

```text
M0 = BLOCKED_BY_EXTERNAL_CAPABILITY
```

不得用 local PASS 替代。

## 3. Test Workspace

使用单独验收目录，例如逻辑结构：

```text
m0-e2e-workspace/
└ baseline.txt
```

初始：

```text
baseline.txt = baseline
```

执行前记录 Workspace 文件清单与关键文件内容，执行后用于核对 mutation。

## 4. E2E-01 — ChatGPT discovers `write` and `edit`

在 External ChatGPT 会话中确认 MCP tool exposure。

期望：

- ChatGPT 可发现 `write`。
- ChatGPT 可发现 `edit`。
- `read_file` / `glob` 仍可用。
- 不要求通过 ChatGPT UI 的视觉表现推导 Runtime truth；如 UI 有缓存，以实际 tool invocation 能力和 Runtime evidence 为准。

若 ChatGPT 完全不提供 write/modify 调用能力，标记 `BLOCKED_BY_EXTERNAL_CAPABILITY`。

## 5. E2E-02 — Golden Path: external write

向 ChatGPT 发出明确请求：

```text
请在当前 Workspace 创建文件 v3-write-test.txt，内容必须完全是：
hello from ChatGPT
```

不得人工预先创建该文件。

期望：

1. ChatGPT 实际发起 `write` tool call。
2. Runtime 接收并执行同一 request lifecycle。
3. `v3-write-test.txt` 在 Workspace 中真实创建。
4. 文件内容完全等于：

```text
hello from ChatGPT
```

5. 没有额外前后缀、Markdown fence 或解释文字写入文件。
6. Activity/Audit 能看到对应 `write` 请求与成功 outcome。

PASS 证据至少包括：

- ChatGPT tool invocation 可见证据或会话记录。
- Workspace 实际文件内容核对。
- Runtime Activity/Audit request evidence。

## 6. E2E-03 — Golden Path: external exact edit

在 E2E-02 创建的文件上向 ChatGPT 发出：

```text
请把 v3-write-test.txt 中唯一的文本：
hello from ChatGPT

精确替换为：
hello from V3
```

期望：

1. ChatGPT 实际发起 `edit` tool call，而不是用其它旁路模拟。
2. Runtime exact-match count = 1。
3. 原请求完成后文件内容完全等于：

```text
hello from V3
```

4. Activity/Audit 对应 `edit` request 为成功。

## 7. E2E-04 — External boundary rejection

要求 ChatGPT 尝试写 Workspace 外路径。使用专门的安全测试目标，不指向任何重要文件。

优先使用相对 escape 示例：

```text
../m0-outside-test.txt
```

期望：

- ChatGPT 可以发起 tool call（如客户端允许）。
- Runtime 返回 Workspace rejection。
- Workspace 外目标未创建/未修改。
- Activity/Audit 记录 Workspace validation failure。
- 不出现 ExecutionCompleted success。
- 返回给 ChatGPT 的 error 不泄露不必要的绝对路径。

如果 ChatGPT 客户端在发送前自行拦截该请求，则记录 `CLIENT_BLOCKED_BEFORE_RUNTIME`，并额外执行一个能真实到达 Runtime 的外部 boundary case；只有 Runtime rejection 被观察到才满足 M0 boundary E2E。

## 8. E2E-05 — Existing read path regression smoke

让 External ChatGPT 读取 `v3-write-test.txt`。

期望：

- ChatGPT 通过 `read_file` 读取到 `hello from V3`。
- 证明同一 External MCP connection 下，原有 read capability 与新增 mutation capability 可共同工作。

此 case 是 smoke，不替代 V2 已完成验收。

## 9. Failure Classification

测试失败必须分类，不得笼统记为“E2E failed”：

```text
HARNESS_IMPLEMENTATION_FAILURE
TUNNEL_CONNECTIVITY_FAILURE
CREDENTIAL_FAILURE
CHATGPT_TOOL_DISCOVERY_FAILURE
CHATGPT_PRODUCT_CAPABILITY_BLOCK
CLIENT_BLOCKED_BEFORE_RUNTIME
RUNTIME_POLICY_OR_WORKSPACE_REJECTION_EXPECTED
UNKNOWN_EXTERNAL_FAILURE
```

其中仅 `CHATGPT_PRODUCT_CAPABILITY_BLOCK` 可映射：

```text
BLOCKED_BY_EXTERNAL_CAPABILITY
```

Harness 自身实现错误不得使用该状态隐藏。

## 10. Evidence Required

每个 E2E case 记录：

```text
Case ID
Date/time
Harness build/commit
Runtime generation / request ID when available
Result
ChatGPT action observed
Filesystem before/after
Activity/Audit outcome
Failure classification if not PASS
```

不得记录 Tunnel API Key、secret value 或不必要的本机敏感路径。

## 11. E2E Exit Criteria

E2E Plan 为 PASS 必须同时满足：

- E2E-01：External ChatGPT 能发现/调用 write/edit。
- E2E-02：真实 `write` 创建正确文件。
- E2E-03：真实 `edit` 精确修改正确。
- E2E-04：真实请求到达 Runtime 后，Workspace 外 mutation 被拒绝。
- E2E-05：External read smoke PASS。
- 所有成功/拒绝请求都能找到对应 Runtime evidence。

如果产品侧 capability block：

```text
E2E = BLOCKED_BY_EXTERNAL_CAPABILITY
M0 = BLOCKED_BY_EXTERNAL_CAPABILITY
```

不得标记 `PASS`。

## 12. Non-Claims

E2E PASS 仍不自动等于 Milestone 已冻结；最终状态由 `MANUAL_ACCEPTANCE_CHECKLIST.md` 汇总 Development + Integration + E2E evidence 后决定。
