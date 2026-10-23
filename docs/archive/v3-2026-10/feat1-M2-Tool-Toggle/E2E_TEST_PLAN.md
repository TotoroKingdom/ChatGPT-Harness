# M2 E2E Test Plan — Tool Enable / Disable

> Status: ACCEPTED / FROZEN
> Milestone: M2 — Tool Enable / Disable  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`  
> Scope: Coding Agent-executable Local Black-box E2E using real compiled Runtime processes, real Control RPC, real loopback MCP HTTP, real filesystem and Activity. No External ChatGPT, browser, Harness UI clicking, mocks, or in-process executor substitution.

## 1. Objective

验证 M2 在接近最终构建的真实本地运行链路中满足：

```text
persisted desired tool config
→ Runtime generation A
→ real Control initialize / acknowledge
→ real MCP endpoint
→ real tools/list / tools/call
→ Activity / filesystem
→ saved config revision change
→ Runtime generation B
→ new effective tool state
```

E2E 重点证明：

- Tool OFF 在新的 Runtime generation 中真实生效。
- OFF tool 不出现在真实 `tools/list`。
- cached/stale `tools/call` 即使绕过 list，也被真实 Runtime 拒绝。
- current Runtime generation 不受 later desired config save影响。
- restart/new generation 后 desired 与 effective 收敛。
- M1 的真实 write/edit path没有回归。

## 2. E2E Boundary

每个 E2E case 必须由 Coding Agent 在本机自主执行。

有效链路必须包含真实组件：

```text
compiled chatgpt-harness-runtime process
→ stdin/stdout JSON-RPC Control Protocol
→ initialize / initialized
→ mcp.endpoint.get
→ loopback HTTP MCP
→ Runtime ToolRegistry / ExecutionPipeline
→ Workspace filesystem / Activity
```

涉及 config persistence/revision 的 case 还应使用真实 ConfigStore 或同一 production config construction path生成 RuntimeConfigSnapshot；不得手工伪造与 production 不一致的 snapshot builder。

以下不能替代 E2E：

- unit test。
- Integration Test。
- mock Runtime。
- in-process `RuntimeCore` only。
- internal executor call。
- mocked MCP service。
- External ChatGPT人工操作。
- 浏览器/Harness UI点击。

## 3. Entry Criteria

开始 E2E 前必须满足：

```text
Development Tasks = PASS
Unit/Regression = PASS
Integration = PASS
```

并具备：

- 可执行的当前 Runtime build。
- 独立临时 Workspace。
- 独立临时 config directory/file fixture。
- 能启动/停止多个 Runtime generation。
- 能通过真实 Control RPC获取 MCP endpoint和 Activity。

## 4. Fixture

建议使用：

```text
M2-E2E/
├ config/
│  └ config.json
├ workspace/
│  ├ baseline.txt
│  └ mutable.txt
└ outside/
   └ outside.txt
```

初始 desired state：

```text
read_file = ON
glob      = ON
write     = ON
edit      = ON
bash      = OFF
```

初始 Workspace：

```text
baseline.txt = baseline
mutable.txt  = old
```

## 5. Runner Requirements

推荐新增/扩展真实子进程 E2E runner，例如：

```text
runtime/tests/m2_e2e.rs
```

runner 必须：

- spawn release/near-final Runtime executable。
- 驱动真实 stdin/stdout Control RPC。
- 调用真实 loopback MCP HTTP。
- 读取真实 filesystem bytes。
- 调用 `activity.list` 或正式 control observation接口。
- 启动至少两个独立 Runtime generation。
- 清理子进程和临时目录。

如果 config save/restart UX需要 Tauri-side production code参与，可增加一个 Tauri/core integration helper或测试 harness，但 E2E 对 Runtime/MCP链路仍必须是真实子进程，不能退回 mock。

## 6. Required E2E Cases

### E2E-01 — Default generation exposes all M1 tools

使用 default desired config启动 Generation A。

通过真实 Control initialize/ack + MCP `tools/list` 验证：

```text
read_file
glob
write
edit
```

期望：

- 四个工具存在且顺序稳定。
- bash 不存在。
- Runtime active revision与启动 snapshot一致。

### E2E-02 — Default write/edit regression through real MCP

在 Generation A：

1. `write` 创建 `m2-e2e.txt = hello from M2`。
2. `edit` 精确替换为 `hello from M2 enabled`。
3. `read_file` 读回。

期望：

- filesystem最终 bytes精确正确。
- Activity有真实 tool invocation evidence。
- M1 atomic semantics没有可观察回归。

### E2E-03 — Desired change does not mutate active generation

在 Generation A运行期间，通过 production ConfigStore path把：

```text
write: ON → OFF
```

并使 persisted `configRevision`从 N 变 N+1。

不要 restart Generation A。

然后再次调用真实 MCP `tools/list` 和 `write`。

期望：

- Generation A `tools/list` 仍包含 write。
- Generation A 的 write仍按其 immutable snapshot可执行。
- active Runtime revision仍为 N。
- latest persisted revision为 N+1。
- 可通过 production read model确定 Restart Required。

这不是 bug；这是 M2 冻结的 generation semantics。

### E2E-04 — Restart/new generation applies disabled tool

停止 Generation A，使用最新 persisted config启动 Generation B。

期望：

- Generation B active revision = N+1。
- `tools/list` 不包含 write。
- `read_file/glob/edit`仍按 saved state暴露。
- effective tools中 write = `UnavailableDisabled` 或正式等价 observation。
- restart-required mismatch消失。

### E2E-05 — Cached stale `write` call is rejected

在 Generation B 中，不依赖 `tools/list`，直接构造真实 MCP `tools/call` 请求 `write`。

期望：

- Runtime返回 `TOOL_DISABLED` 或冻结等价 code。
- 目标文件不创建/不修改。
- Activity有 disabled rejection evidence。
- 无 ExecutionCompleted success。

### E2E-06 — Re-enable write in next generation

运行 Generation B时，将 persisted desired：

```text
write: OFF → ON
```

得到 revision N+2。

先确认 Generation B仍保持 OFF；随后停止 B并启动 Generation C。

期望：

- B 不 live mutate。
- C active revision = N+2。
- C `tools/list`重新包含 write。
- 真实 `write` 调用成功。

### E2E-07 — Disable read_file while sibling tools remain usable

保存：

```text
read_file OFF
write ON
edit ON
glob ON
```

启动下一 generation。

期望：

- `tools/list` 不含 read_file。
- stale `read_file` call返回 disabled。
- glob/write/edit仍能正常工作。
- 不出现“关闭一个工具导致整个 registry unavailable”。

### E2E-08 — Multiple independent disabled tools

配置：

```text
read_file OFF
write OFF
edit ON
glob ON
```

新 generation：

期望：

- list精确只含 glob/edit（在现有四工具集合内）。
- stale read/write均被拒绝。
- glob/edit仍成功。
- effective observation逐工具准确。

### E2E-09 — Disabled vs unknown tool are distinct

在一个 disabled `write` generation：

1. direct call `write`。
2. direct call不存在的 `does_not_exist`。

期望：

- write = disabled semantics。
- unknown = existing unknown-tool semantics。
- 两者 code/reason可区分。

### E2E-10 — Disabled tool produces no mutation side effect

在 write OFF generation：

- 尝试 create new file。
- 尝试 overwrite existing `mutable.txt`。

期望：

- 两次均被拒绝。
- new target不存在。
- existing target仍为调用前精确 bytes。

### E2E-11 — Config compatibility from pre-M2 schema v1

准备一份真实 pre-M2 schema v1 config JSON，不包含 `tools`。

通过 production config load/start construction启动 Runtime generation。

期望：

- 不需要手工升级 JSON。
- Runtime正常启动。
- read/glob/write/edit全部按 defaults ON。
- schemaVersion没有被 M2擅自升级为2。

### E2E-12 — Shutdown and cleanup

每个 generation结束时：

- 通过正式 stop/shutdown path停止 Runtime。
- MCP endpoint不再提供 active服务。
- 子进程正常退出。
- 临时测试目录可清理。

不得遗留 orphan Runtime process。

## 7. E2E Evidence

每次 runner输出至少包括：

```text
Case ID
PASS / FAIL
Runtime executable identity
Runtime PID / generation identity
active configRevision
relevant desired tool state
MCP tools/list observation
MCP stale-call result/error code
filesystem assertion where applicable
Activity assertion where applicable
shutdown result
```

不要把 raw secret写入 E2E log。

## 8. Failure Classification

使用：

```text
IMPLEMENTATION_FAILURE
CONFIG_PERSISTENCE_FAILURE
REVISION_SEMANTICS_FAILURE
SNAPSHOT_WIRING_FAILURE
MCP_EXPOSURE_FAILURE
STALE_CALL_ENFORCEMENT_FAILURE
RUNTIME_LIFECYCLE_FAILURE
M1_REGRESSION
TEST_ENVIRONMENT_ERROR
```

若 case 依赖的 local environment缺失，应记录环境错误并停止该次验证，不能用 mock替代真实链路后宣称 E2E PASS。

## 9. E2E Exit Criteria

M2 Local Black-box E2E 只有在以下全部满足时 PASS：

- E2E-01 ～ E2E-12 全部 PASS。
- 至少真实启动多个 Runtime generations。
- desired change期间当前 generation保持 immutable。
- restart/new generation应用 saved tool state。
- disabled tool不在真实 `tools/list`。
- direct stale disabled `tools/call`被真实 Runtime拒绝。
- disabled operation无 filesystem/read side effect。
- re-enable在下一 generation恢复。
- legacy schema v1 without tools可真实启动。
- M1 write/edit/read/glob真实链路无回归。
- 所有 Runtime子进程正常清理。

通过后进入 `MANUAL_ACCEPTANCE_CHECKLIST.md`。

## 10. Final Result / Evidence Retention

```text
Local Black-box E2E result: PASS
Cases: E2E-01–E2E-12 all PASS
Evidence: retained from the prior M2 Local Black-box E2E run; no E2E was re-executed during final closeout.
Implementation commit: ceb91cc36ef71e38a489357580af3cd8bcd9a495
Test/evidence commit: aafc63fe86d857245b483e4b1613daa8d92edd13
```

## 11. Non-Claims

M2 E2E PASS 不代表：

```text
External ChatGPT current-session tool cache instantly refreshes
same-generation tools/list_changed implemented
UI visual/manual UX accepted
M3 configurable Policy implemented
M5 bash implemented
M7 schema migration implemented
```

The Local Black-box E2E PASS result did not by itself establish the human UI gate; that gate subsequently passed and the milestone is now recorded as `M2 — ACCEPTED / FROZEN`.
