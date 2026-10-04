# M7 E2E Test Plan — Local Black-box Migration, Evidence & UX Hardening

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M7 — Migration, Evidence & UX Hardening  
> Owner: Coding Agent / QA Agent  
> Requirement: fully automated local black-box validation against production config/runtime surfaces.  
> M8 External ChatGPT manual acceptance is explicitly out of scope.

## 1. Objective

从真实产品边界证明 M7 的最终收口链：

```text
historical v1 config on disk
→ production ConfigStore load/migration
→ persisted AppConfig v2
→ production Runtime snapshot
→ release Runtime subprocess
→ real Control + loopback MCP
→ real request / approval / execution evidence
→ real counters / diagnostics
```

Local Black-box E2E 必须证明两个核心事实：

1. **升级不会丢配置、不会覆盖失败源文件、不会泄露 secret。**
2. **真实 Runtime 请求能够留下可信、sanitized、可关联的 evidence，并且 UI/Control 所观察的状态来自真实 Runtime。**

## 2. Entry Criteria

- M7 T1–T6 development complete。
- Unit PASS。
- Integration PASS。
- release/final-ish Runtime build PASS。
- production Tauri/ConfigStore test surface可调用。
- disposable config directory + Workspace。

若 migration E2E只能直接调用内部 `migrate_v1_to_v2()`：

```text
M7 E2E = BLOCKED_BY_TEST_SURFACE
```

因为那只是 Integration，不是 production persistence path。

## 3. Black-box Boundary

### 3.1 Migration boundary

必须把真实 `config.json` fixture放入 disposable config directory，然后通过 production ConfigStore/Tauri-host config load path触发 migration。

允许实现专用自动化 host runner，但 runner必须调用 production ConfigStore boundary，不能复制一份 test-only migrator。

### 3.2 Runtime boundary

启动真实：

```text
target/release/chatgpt-harness-runtime.exe
```

通过真实 stdin/stdout JSONL：

```text
initialize
initialized
runtime.status
control.diagnostics.get
control.activity.snapshot
approvals.list
approvals.resolve
mcp.endpoint.get
runtime.stop
```

通过真实 loopback MCP：

```text
tools/list
tools/call
```

### 3.3 No GUI requirement

M7 Local E2E 不要求浏览器/桌面点击。

Migration recovery视觉质量、Activity详情可读性、中英文 remediation体验留给 Manual Acceptance。

## 4. Fixtures

至少准备：

```text
M7-E2E/config-v2-era-v1.json
M7-E2E/config-transitional-v1.json
M7-E2E/config-invalid-v1.json
M7-E2E/config-future-schema.json
M7-E2E/workspace/
```

Transitional fixture必须包含非默认：

- Tool disabled state。
- User Policy。
- Bash config。
- ui.language。
- Tunnel/proxy/apiKeyRef。

fixtures不得包含真实 credential。

## 5. Required E2E Cases

### E2E-01 — Pure V2-era v1 migrates through production ConfigStore

把 historical V2 v1文件复制到 disposable production config location。

调用真实 config load/get。

验证：

- load成功。
- disk变为 schemaVersion 2。
- configRevision preserved。
- Workspace/Tunnel/Security preserved。
- V3 missing fields使用 frozen defaults。
- no temp residue。

### E2E-02 — Transitional M2–M6 v1 preserves user settings

使用含非默认 V3 additive fields的 v1 fixture。

迁移后逐项验证：

- Tool states。
- Tool defaults。
- User rules。
- Bash autoDetect/manual path。
- language。
- Workspace/Tunnel/proxy/apiKeyRef/security。
- configRevision。

不得被 defaults覆盖。

### E2E-03 — Migration second load is stable

对 E2E-01/02生成的 v2再次走 production load。

验证：

- no migration again。
- schemaVersion仍 2。
- configRevision unchanged。
- no unnecessary restart debt。

### E2E-04 — Invalid v1 migration preserves original bytes

使用 malformed known-field fixture。

记录 migration前 SHA-256/bytes。

生产 load后：

- typed migration failure。
- post-failure bytes/SHA exact same。
- no partial v2。
- no temp residue。
- config metadata path仍可读取。

### E2E-05 — Future schema is not downgraded or overwritten

schemaVersion > 2 fixture：

- load fails typed unsupported schema。
- source bytes unchanged。
- no reset。

### E2E-06 — Secret-like migration failure does not leak value

fixture放置 synthetic sentinel：

```text
M7_SECRET_SENTINEL_DO_NOT_LEAK
```

于不受支持 raw-secret field。

验证：

- migration fails。
- original unchanged。
- returned error / logs / diagnostics / Activity不得出现 sentinel。

### E2E-07 — Migrated v2 starts real Runtime

将成功迁移的 v2通过 production snapshot construction启动 release Runtime。

验证：

- initialize/initialized成功。
- Runtime configRevision等于迁移保留值。
- Workspace/Security/Tools/Policy/Bash effective state与迁移 config一致。

### E2E-08 — Migrated Tool state reaches real MCP catalog

Transitional fixture中设置一个 file tool OFF。

启动 Runtime后：

- `tools/list`不含该 tool。
- stale/direct call被 TOOL_DISABLED。
- 证明 migration没有只保存 JSON而未进入 Runtime semantics。

### E2E-09 — Migrated Policy reaches real Runtime

fixture保存显式 DENY/ASK rule。

启动后真实 MCP call：

- DENY request不执行。
- ASK进入 Pending。
- effective/matched rules与 fixture一致。

### E2E-10 — Real read_file hardened evidence

通过 MCP读取 harmless file。

Activity snapshot验证：

- request ID。
- tool=read_file。
- sanitized logical path。
- enabled/capability/workspace result。
- policy decision/reason/matched IDs。
- terminal outcome。
- duration。
- bytesRead scalar。

不得包含 file content。

### E2E-11 — Real write hardened evidence

通过 MCP write sentinel content。

Activity：

- logical target path存在。
- bytesWritten / created-or-overwritten scalar存在。
- raw sentinel content不得出现在 Activity/Diagnostics。

filesystem结果正确。

### E2E-12 — Real edit hardened evidence

通过 MCP edit。

Activity保留 path + safe result scalar，不保留 old/new text。

最终文件正确。

### E2E-13 — Real Policy DENY evidence

调用 explicit DENY tool/request。

验证：

- no execution side effect。
- Activity有 policy DENY / matched ID / reason。
- terminal outcome=Denied。
- duration存在。

### E2E-14 — Real ASK approval evidence correlation

发起真实 ASK MCP request保持 Pending。

在 Pending时：

- `activeRequests = 1`。
- `pendingApprovals = 1`。
- `runningExecutions = 0`。

Activity有 approval ID。

通过真实 Control AllowOnce。

原 call恢复。

完成后：

```text
activeRequests=0
pendingApprovals=0
runningExecutions=0
```

Activity可用同 request ID关联 Policy→Approval→Execution。

### E2E-15 — Denied approval correlation

ASK request → Deny。

验证：

- same original call终止。
- no side effect。
- approval ID / Deny decision / terminal outcome可关联。
- counters全部归零。

### E2E-16 — Real runningExecutions observation

使用 real Bash或确定性长运行 production executor。

在 executor已经开始而未完成时读取 Runtime status/diagnostics：

```text
activeRequests >= 1
runningExecutions = 1
```

若不是 Approval wait：

```text
pendingApprovals = 0
```

结束后全部归零。

### E2E-17 — Bash timeout/cancel keeps hardened evidence and counter cleanup

复用 M5真实 Bash fixture触发 timeout或 cancellation。

验证：

- process tree cleanup仍 PASS。
- Activity failure category正确。
- duration存在。
- Bash preview bounded。
- full output/env不出现。
- counters归零。

### E2E-18 — Disabled / Denied / Ask / Unavailable are machine-distinct

构造四种真实状态：

- Tool disabled。
- Policy DENY。
- Policy ASK。
- Bash unavailable or Runtime unavailable。

Control/effective diagnostics必须返回不同 availability/reason code，而不是只给不同自由文本。

### E2E-19 — Remediation reason agrees with actual state

从 production read DTO获取 reason/remediation identity。

至少验证：

- disabled → enable/restart类 remediation。
- policy denied → review policy类 remediation。
- ASK → Approval Center类 remediation。
- Bash unavailable → configure Bash/restart类 remediation。

不得出现与状态矛盾的 action。

执行澄清（2026-10-04）：当前 production read DTO 的机器身份为
`statusReason`；remediation key 由 production UI 的 `toolRemediationKey`
映射。此 case 必须将真实 release Runtime Control DTO 交给实际 production
presentation code，断言 disabled/policy denied/ASK/Bash unavailable 的
reason、remediation key 和 en/zh-CN 资源一致。允许无 GUI 的 Node 探针直接
执行该生产代码；不得在 runner 中复制映射表，或用 mock DTO 替代真实读取。
澄清原因是实现把本地化展示放在 UI，未在 Runtime DTO 增加重复展示字段；
真实机器状态和修复指引的验证要求保持不变。

### E2E-20 — Desired/effective revision mismatch is truthful

Runtime使用 revision N启动。

通过 production ConfigStore修改 runtime-affecting config → revision N+1。

验证：

- current Runtime仍 N。
- saved config N+1。
- current effective状态未伪装变化。
- restartRequired condition成立。

restart Runtime后：

- active revision=N+1。
- desired/effective收敛。

### E2E-21 — UI-only language remains revision-neutral on v2

v2 config运行时切换 language preference production path。

验证：

- config persisted。
- configRevision不变。
- Runtime PID/runtimeId无需改变。
- no restartRequired debt。

### E2E-22 — Activity sanitization sentinel scan

在一组真实 calls中使用唯一 synthetic sentinels作为：

- write content。
- edit old/new text。
- Bash output/environment fixture。
- migration secret-like value。

聚合检查：

- Activity snapshot。
- diagnostics JSON。
- control errors。
- generated evidence file/report。

sentinel不得出现于禁止表面。

### E2E-23 — Activity remains bounded

生成超过 Activity capacity的安全 requests。

验证：

- snapshot bounded。
- latest entries可读取。
- Runtime仍可工作。
- bounded store不影响 authorization/execution。

### E2E-24 — Real M0–M6 regression

在最终 M7 build上通过真实 MCP/Control至少验证：

- read_file。
- glob。
- write。
- edit。
- Tool OFF stale rejection。
- Policy DENY。
- ASK AllowOnce same-request resume。
- Bash representative safe path（environment具备时）。

### E2E-25 — Clean shutdown

最终：

- no active requests。
- no pending approvals。
- no running executions。
- Runtime clean exit。
- MCP endpoint释放。
- no orphan Bash child。
- migrated config remains valid v2。

## 6. Migration Black-box Evidence

每个 migration case至少记录：

```text
fixture identity
pre-migration SHA-256
post-operation SHA-256
source schemaVersion
target schemaVersion
configRevision before/after
production ConfigStore command/path
result code
preserved-field assertions
temp residue assertion
```

不要把 API key/secret内容写进报告。

## 7. Runtime Evidence Requirements

至少记录：

```text
Runtime binary identity / SHA-256
Runtime PID/runtimeId
active configRevision
MCP endpoint
request ID
approval ID where applicable
counters before/during/after
Activity snapshot assertions
filesystem/process assertions
```

## 8. Timing / Concurrency Rules

- ASK Pending使用 bounded polling确认，不靠随机 sleep。
- runningExecutions使用明确 long-running fixture/barrier。
- counter cleanup必须最终 bounded wait收敛为 0。
- 不引入 flaky race来证明 instrumentation。

## 9. Exit Criteria

E2E-01 ～ E2E-25全部满足才可输出：

```text
LOCAL_M7_BLACK_BOX_E2E=PASS
```

特别要求：

- real v1 files经 production persistence path迁移。
- migration failure original bytes unchanged。
- successful migrated v2真实驱动 release Runtime behavior。
- real MCP evidence完整且 sanitized。
- approval correlation真实。
- active/pending/running counters真实变化并归零。
- typed reason/remediation与真实状态一致。
- revision mismatch/restart语义真实。
- M0–M6 regression PASS。

## 10. Explicit Non-Claims

M7 Local E2E PASS不代表：

```text
External ChatGPT full V3 acceptance PASS
real user production config has been modified
Activity is durable audit storage
M8 complete
V3 ACCEPTED / FROZEN
```

External ChatGPT golden paths留给 M8。
