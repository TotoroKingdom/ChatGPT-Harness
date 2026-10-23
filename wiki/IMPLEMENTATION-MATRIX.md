# IMPLEMENTATION MATRIX

> 当前 main / HEAD（0.1.3）能力矩阵。这里描述“代码当前具备什么”，不使用旧 V2/V3 milestone 状态替代事实，也不把 V4 Roadmap 目标写成已实现。

## 1. Runtime / Tool Capability

| Capability | 当前实现 | 默认状态 | Runtime authority |
| --- | --- | --- | --- |
| Tool Registry | 已实现 | 固定 5 个内置工具 | `runtime/src/tools/registry.rs` |
| `read_file` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy |
| `glob` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy |
| `write` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy + atomic mutation |
| `edit` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy + atomic mutation |
| `bash` | 已实现 | disabled + ASK | Developer/Test mode + readiness + Policy/Approval |
| Effective tools | 已实现 | Runtime 计算 | `runtime/src/tools/effective.rs` |
| MCP filtering | 已实现 | 仅 Available / AvailableWithApproval | `runtime/src/mcp/service.rs` |

## 2. Security / Authorization

| Capability | 当前实现 | 关键语义 |
| --- | --- | --- |
| Workspace confinement | 已实现 | Runtime 内 canonicalization / escape rejection |
| System Policy guardrail | 已实现 | 固定、只读、用户不可降级 |
| Tool default policy | 已实现 | ALLOW / ASK / DENY |
| User Tool rule | 已实现 | typed tool matcher |
| User Bash rule | 已实现 | executable + argvPrefix |
| Invalid policy fail-closed | 已实现 | invalid snapshot/context 不放行 |
| Approval | 已实现 | exact invocation suspend/resume |
| Approval TTL | 已实现 | 默认 5 分钟 |
| Allow once | 已实现 | 只放行该次调用 |
| Allow for session | 已实现 | generation-local exact-scope grant |
| Deny | 已实现 | 不执行副作用 |
| Approval expiry/cancel | 已实现 | waiter 收敛 |
| Session Grant | 已实现 | memory-only，非 tool-wide permission |

## 3. File Mutation

| Capability | 当前实现 | 行为 |
| --- | --- | --- |
| Full-file write | 已实现 | create / overwrite UTF-8 file |
| Atomic write path | 已实现 | shared atomic mutation |
| Target revalidation | 已实现 | mutation 前后保护 |
| Edit | 已实现 | exact single-match replacement |
| Multi-match edit rejection | 已实现 | 不自动选择 first match |
| Outside Workspace rejection | 已实现 | no side effect |
| Target changed detection | 已实现 | fail closed |

## 4. Bash

| Capability | 当前实现 | 当前硬限制/语义 |
| --- | --- | --- |
| Git Bash discovery | 已实现 | Runtime 每 generation 验证 |
| Developer/Test mode gate | 已实现 | Normal mode 不可用 |
| Analyzer | 已实现 | quote-aware + risk flags |
| High-risk ASK floor | 已实现 | unclassified high-risk 至少 ASK |
| Command-specific Policy | 已实现 | executable + argvPrefix |
| Async execution | 已实现 | Runtime-owned |
| Command size | 已实现 | 8192 bytes |
| Timeout | 已实现 | 30s |
| Output cap | 已实现 | stdout + stderr 256 KiB |
| Concurrency cap | 已实现 | 1 |
| Cancellation | 已实现 | request cancellation |
| Process-tree cleanup | 已实现 | owned tree cleanup |
| Bash session grant | 已实现 | 仅 classified exact scope |

## 5. Observability

| Capability | 当前实现 | 边界 |
| --- | --- | --- |
| Audit events | 已实现 | bounded, non-secret metadata |
| Activity projection | 已实现 | UI-safe projection |
| Activity capacity | 已实现 | 256 / Runtime lifetime |
| Approval evidence | 已实现 | approval id / decision / lifecycle |
| Result evidence | 已实现 | bounded result metadata |
| Bash evidence | 已实现 | sanitized preview / flags / exit/duration |
| Runtime diagnostics | 已实现 | local control read |
| Effective tools read | 已实现 | Runtime authoritative |
| Policy effective snapshot | 已实现 | Runtime authoritative |

Activity 不是 durable audit database；Runtime restart 后不要把旧 Activity 当成持久历史。

## 6. Desktop / Lifecycle

| Capability | 当前实现 |
| --- | --- |
| React + Tauri desktop app | 已实现 |
| Config persistence | 已实现 |
| Config migration | 已实现 |
| Current config schema | 2 |
| SecretStore | 已实现 |
| Runtime child lifecycle | 已实现 |
| Runtime MCP endpoint handoff | 已实现 |
| Tunnel managed launch | 已实现 |
| Tunnel readiness | 已实现 |
| Harness Start / Stop / Restart | 已实现 |
| Crash/readiness convergence | 已实现 |
| Session/generation stale callback protection | 已实现 |
| Portable build | 已实现 |
| NSIS build | 已实现 |

## 7. UI

| Capability | 当前实现 |
| --- | --- |
| Overview | 已实现 |
| Tools | 已实现 |
| Policies | 已实现 |
| Approvals | 已实现 |
| Activity | 已实现 |
| Settings | 已实现 |
| First-run flow | 已实现 |
| English | 已实现 |
| 简体中文 | 已实现 |
| dark | 已实现 |
| light | 已实现 |
| system theme | 已实现 |

## 8. 当前不应误写成“已实现”的 V4 目标

V4 正在开发。以下能力是否完成必须看对应 milestone 的源码与验收状态，不能仅因出现在 PRD/Roadmap 就更新为 Implemented：

- 系统托盘 / 关闭窗口后后台运行；
- 应用启动后自动启动 Harness；
- 桌面通知；
- 一键连接诊断；
- 更高级的结构化调用链追踪；
- Eval / Agent 工程评测能力；
- 权限模拟器；
- 其他 V4 新增能力。

当这些能力真正合入并通过其定义的验证/验收后，再更新本页。

## 9. 默认安全状态

新配置的关键默认值：

```text
tools:
  read_file: enabled
  glob: enabled
  write: enabled
  edit: enabled
  bash: disabled

policy defaults:
  read_file: ALLOW
  glob: ALLOW
  write: ALLOW
  edit: ALLOW
  bash: ASK

security:
  mode: Normal
```

因此 Bash 即使实现存在，也不会在默认 Normal 模式 + disabled 配置下暴露。

## 10. 判定规则

本矩阵只回答当前实现，不直接宣称“某版本测试 PASS”。

实现证据优先级：

```text
source
→ tests
→ manifest/build scripts
→ current wiki
```

历史人工验收记录可说明过去某个发布点发生过什么，但不能覆盖当前 HEAD 的源码事实。
