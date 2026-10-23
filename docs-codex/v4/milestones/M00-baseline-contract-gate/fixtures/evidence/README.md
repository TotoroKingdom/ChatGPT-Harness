# Shared Evidence Contract v1 fixtures

这些 fixture 是 M00 的 test-only contract shape，不是生产 History/Trace schema，也不会被 Runtime/Tauri import。

## Identity boundary

- `eventId` 是单个观察事件的全局去重键。
- `requestId` 是 Runtime/MCP 请求键。
- `runtimeId` 在当前 `AuditEvent` 中尚未明确携带，因此当前正例为 `null`，并通过 `runtimeIdentityStatus=not_available_in_current_audit_event` 显式记录缺口；M03 必须在 confirmed mapping 后填充，不能按时间推断。
- `hostSessionId`、`supervisorGeneration`、`traceId` 和 `spanId` 不因字段存在而自动等同于 `runtimeId`。

## Safe projection rules

只允许 typed、bounded、sanitized 字段：logical tool/capability、logical workspace label、policy/approval enums、bounded IDs/rule IDs、scalar duration/size/count、工具专属 sanitized evidence 和 coverage。物理路径、完整文件正文、raw argv/command、environment、secret、unbounded error、hidden reasoning 和未经确认的 upstream trace ID 都不能进入输出。

`unsafe-input-cases.json` 只保存 synthetic candidate input，测试证明这些值不会出现在 emitted safe projection 中；其中的 sentinel 不是凭据。

## Coverage and authority

`COMPLETE` 只表示本地观察到了终态，不表示 Agent task 完成。`INCOMPLETE`/`UNKNOWN` 不得被补写成 success/failure。Evidence 是 observation-only data，不能创建/恢复 `Pending Approval`、`Session Grant` 或 `ExecutionPermit`。
