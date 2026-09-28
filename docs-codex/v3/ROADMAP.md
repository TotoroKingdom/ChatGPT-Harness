# V3 — Writable Agent Runtime

```yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN

V3 — Writable Agent Runtime
Status: CURRENT DEVELOPMENT
```

本文件是 V3 的当前开发入口。以下所有阶段均为：

```text
PLANNED / NOT IMPLEMENTED
```

它们不构成当前实现事实；当前系统能力请以源码、Tests、Manifests 和 `wiki/**` 为准。

## V3 boundary

V2 保持只读执行边界。V3 逐阶段增加可配置能力、写入能力、审批恢复和 shell 执行，并在最后阶段完成外部 E2E 与安全加固。

## Phase 1 — Tool Configuration + Policy V2

**PLANNED / NOT IMPLEMENTED**

目标：

- 支持工具 `enable / disable`。
- 建立 Tool capability 模型。
- 支持 Policy 按 capability / tool 匹配。
- 提供可配置的 Policies UI。

## Phase 2 — `write_file` + `edit_file`

**PLANNED / NOT IMPLEMENTED**

增加受 Workspace confinement、Policy 和 Audit 约束的文件写入与编辑能力。

## Phase 3 — Approval Runtime

**PLANNED / NOT IMPLEMENTED**

实现完整审批恢复流程：

```text
ASK → pending approval → Allow once / Deny → resume
```

## Phase 4 — `bash` / shell execution

**PLANNED / NOT IMPLEMENTED**

增加受 Policy、Workspace / process 边界和 Audit 约束的 `bash` / shell execution 能力。

## Phase 5 — External E2E + Security Hardening

**PLANNED / NOT IMPLEMENTED**

在前述能力具备后，补充真实外部 E2E、安装版验证、权限边界审计、secret handling、进程清理和安全回归。

## Non-goals for this roadmap entry

- 本 Roadmap 不修改 Runtime、Tauri、React 或 Protocol 源码。
- 本 Roadmap 不改变 V2 的 `ACCEPTED / FROZEN` 状态。
- 本 Roadmap 的任何条目都不能被 Wiki 当作当前功能证据。
