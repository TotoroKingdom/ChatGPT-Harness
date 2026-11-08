# IMPLEMENTATION MATRIX

> 当前 `main` / HEAD（0.1.3）的能力矩阵。这里描述“代码当前具备什么”，不使用旧 V2/V3 milestone 状态替代事实，也不把已归档的 V4 规划当成当前验收结论。

## 1. Runtime / Tool Capability

| Capability | 当前实现 | 默认状态 | Runtime authority |
| --- | --- | --- | --- |
| Tool Registry | 已实现 | 固定 6 个内置工具 | `runtime/src/tools/registry.rs` |
| `read_file` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy |
| `glob` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy |
| `grep` | 已实现 | 新配置 enabled + ALLOW；升级配置默认 disabled | WorkspaceGuard + Policy + bounded scan |
| `write` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy + atomic mutation |
| `edit` | 已实现 | enabled + ALLOW | WorkspaceGuard + Policy + atomic mutation |
| `bash` | 已实现 | disabled + ASK | Developer/Test mode + readiness + Policy/Approval |
| Effective tools | 已实现 | Runtime 计算 | `runtime/src/tools/effective.rs` |
| MCP filtering | 已实现 | 仅 Available / AvailableWithApproval | `runtime/src/mcp/service.rs` |

稳定工具顺序：

```text
read_file
glob
grep
write
edit
bash
```

`grep` 是 M05 新增的只读工具。新安装的 v3 配置默认启用；由 v1/v2 升级或缺少 `grep` 字段的旧 v3 配置会显式补为 disabled，避免静默扩大工具面。

## 2. Security / Authorization

| Capability | 当前实现 | 关键语义 |
| --- | --- | --- |
| Workspace confinement | 已实现 | Runtime 内 canonicalization / escape rejection |
| System Policy guardrail | 已实现 | 固定、只读、用户不可降级 |
| Tool default policy | 已实现 | ALLOW / ASK / DENY |
| User Tool rule | 已实现 | typed tool matcher，支持 6 个内置工具 |
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

## 4. grep

| Capability | 当前实现 | 边界 |
| --- | --- | --- |
| Workspace text search | 已实现 | 只扫描 active Workspace |
| Literal / regex mode | 已实现 | regex 使用 Rust regex 语义 |
| Scope | 已实现 | Workspace-relative directory，默认 `.` |
| Include / exclude | 已实现 | glob 过滤，exclude 优先 |
| Result limit | 已实现 | 默认 100，最大 1000 条匹配行 |
| Output limit | 已实现 | 默认 64 KiB，最大 256 KiB |
| Preview safety | 已实现 | 有界 preview，不返回完整文件内容 |
| Cancellation | 已实现 | MCP cancellation 可中止扫描 |
| Evidence | 已实现 | 只持久化 query 字节数、scope、过滤数量等标量 |

## 5. Bash

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

Bash cwd 固定在 active Workspace，但它不是 OS 级 filesystem sandbox。

## 6. Observability / History

| Capability | 当前实现 | 边界 |
| --- | --- | --- |
| Audit events | 已实现 | bounded、non-secret |
| Activity projection | 已实现 | UI-safe projection |
| Activity capacity | 已实现 | 256 / Runtime lifetime |
| Approval evidence | 已实现 | approval id / decision / lifecycle |
| Result evidence | 已实现 | bounded result metadata |
| Bash evidence | 已实现 | sanitized preview / flags / exit/duration |
| Runtime diagnostics | 已实现 | local control read |
| Effective tools read | 已实现 | Runtime authoritative |
| Policy effective snapshot | 已实现 | Runtime authoritative |
| SQLite History | 已实现 | 默认 enabled；跨重启持久化 |
| History evidence schema | 已实现 | `EVIDENCE_SCHEMA_VERSION = 1` |
| History query | 已实现 | 默认 limit 50，最大 200 |
| History retention | 已实现 | 默认 30 天 / 256 MiB |
| Metrics | 已实现 | 只读聚合，依赖 History enabled |
| Request Trace | 已实现 | 按 `(runtime_id, request_id)` 精确关联，无时间戳回退 |

重要区分：

- Activity 是 Runtime 生命周期内的有界实时视图，不是跨重启数据库。
- History、Metrics、Trace 使用 `<app data>/history/history.sqlite3`；History 停用时 Metrics/Trace 不可用。
- History 保存脱敏后的生命周期证据，不保存原始 API Key、原始命令输出或完整敏感参数。

## 7. Desktop / Lifecycle

| Capability | 当前实现 |
| --- | --- |
| React + Tauri desktop app | 已实现 |
| Config persistence | 已实现 |
| Config migration | 已实现 |
| Current config schema | 3 |
| SecretStore | 已实现，Windows Credential Manager |
| Runtime child lifecycle | 已实现 |
| Runtime MCP endpoint handoff | 已实现 |
| Tunnel managed launch | 已实现 |
| Tunnel readiness | 已实现 |
| Harness Start / Stop / Restart | 已实现 |
| Crash/readiness convergence | 已实现 |
| Session/generation stale callback protection | 已实现 |
| System tray | 已实现，Show / Status / Pending / Start / Stop / Quit |
| Close window to tray | 已实现，首次关闭先展示后台运行说明 |
| Single instance | 已实现（Windows） |
| Approval notifications | 已实现，默认启用 |
| Connection notifications | 已实现，默认启用 |
| Auto-start Harness on app launch | 已实现，默认关闭；不是 Windows 登录自启 |
| Diagnostics | 已实现，本机检查 + 连接检查 + 启动阶段耗时 |
| Startup benchmark capture | 已实现，受隔离 benchmark 环境控制 |
| Portable build | 已实现 |
| NSIS build | 已实现 |

## 8. MCP Compatibility

| Capability | 当前实现 |
| --- | --- |
| MCP transport | Streamable HTTP，loopback endpoint |
| MCP protocol revision | `2025-03-26`（冻结）/ `2025-06-18`、`2025-11-25`（互操作）/ `2026-07-28`（modern） |
| Runtime MCP SDK | `rmcp 3.2.0` |
| Compatibility manifest | `fixtures/v4/m10-mcp/compatibility-v1.json` |
| Official runner identity | `modelcontextprotocol/conformance`，commit 与包版本固定在 manifest/test |
| Tool schema conformance | 已实现，含 `grep` / 参数校验 / cancellation |

兼容性清单和测试固定的是实现身份，不代表 M11 已完成正式验收或已产生发布结论。

## 9. Evaluation / Developer Tooling

| Capability | 当前实现 | 边界 |
| --- | --- | --- |
| Offline evaluation framework | 已实现 | `runtime/src/evaluation/**` |
| M04 eval binary | 已实现 | `runtime/src/bin/m04_eval.rs` |
| M09 admission mechanics | 已实现 | `runtime/src/bin/m09_eval.rs` |
| M09 production tool admission | BLOCKED | 归档文档记录缺少 accepted M05 handoff / model authorization；不得当作已准入 |
| M11 release gate scripts | 已实现 | `ui/scripts/m11-development.ps1` 等；Development 不等于 ACCEPTED / FROZEN |

评测代码是开发/质量工具，不是普通用户界面功能。

## 10. UI

| Capability | 当前实现 |
| --- | --- |
| Overview | 已实现，含状态、诊断与启动耗时 |
| Tools | 已实现 |
| Policies | 已实现 |
| Approvals | 已实现 |
| Activity | 已实现，含 Live / History / Metrics 三个视图 |
| Settings | 已实现，含通知、自动启动、History、诊断与重置 |
| First-run flow | 已实现 |
| English | 已实现 |
| 简体中文 | 已实现 |
| dark | 已实现 |
| light | 已实现 |
| system theme | 已实现 |

## 11. 默认安全状态

新配置的关键默认值：

```text
tools:
  read_file: enabled
  glob: enabled
  grep: enabled
  write: enabled
  edit: enabled
  bash: disabled

policy defaults:
  read_file: ALLOW
  glob: ALLOW
  grep: ALLOW
  write: ALLOW
  edit: ALLOW
  bash: ASK

security:
  mode: Normal

desktop:
  approvalNotifications: true
  connectionNotifications: true
  autoStartHarnessOnAppLaunch: false

history:
  enabled: true
  retentionDays: 30
  maxStorageMiB: 256
```

因此 Bash 即使实现存在，也不会在默认 Normal 模式 + disabled 配置下暴露。升级配置中的 `grep` 默认关闭，只有新配置默认启用。

## 12. 判定规则

本矩阵只回答当前实现，不直接宣称“某版本测试 PASS”或“V4 已验收”。

实现证据优先级：

```text
source
→ tests
→ manifest/build scripts
→ current wiki
```

历史人工验收记录可说明过去某个发布点发生过什么，但不能覆盖当前 HEAD 的源码事实。
