# M7 Development Report

Status: **M7 Development Complete**

执行依据仅为本目录 `SPEC.md`、`PLAN.md`、`TASKS.md`；基线为 M6 ACCEPTED / FROZEN。完成顺序为 T1 → T6，各阶段对应测试通过后才进入下一阶段。

## Task gates

| Task | 完成情况 | 测试证据 |
| --- | --- | --- |
| T1 | PASS — schema2、显式双来源迁移、完整验证、原子持久化、安全错误 | 首个门槛 ConfigStore 44 项、protocol 17 项通过；最终迁移模块增强至 47 项。 |
| T2 | PASS — typed sanitized arguments/result、通用审批关联、单调请求时长、事件枚举对齐 | Runtime 338 项、前端 evidence parser 14 项、protocol 18 项通过。 |
| T3 | PASS — 真实 active/running/pending counters、RAII 清理、typed ToolStatusReason | Runtime 342 项、前端 parser 22 项、protocol 18 项和 typecheck 通过。 |
| T4 | PASS — Activity detail、remediation、recovery、shared revision、diagnostics、双语 | 前端 235 项、typecheck、宿主 reset recovery 专项测试通过。 |
| T5 | PASS — 六份历史 fixture、安全哨兵、生命周期矩阵 | ConfigStore 47 项、Runtime 344 项通过；前端安全渲染哨兵由 T4 专项测试覆盖。 |
| T6 | PASS — M0–M6 全量开发回归 | 最终 workspace 458 项通过、4 项 ignored；前端 235 项通过；全部规定命令 PASS。 |

Task 门槛计数记录阶段当时的测试规模；最终计数包含后续增强和跨模块回归。

## 主要改动

- `CURRENT_SCHEMA_VERSION = 2`，wire protocol 仍为 v1。ConfigStore 先读 raw JSON，再按版本分派；独立 `LegacyAppConfigV1` 支持纯 V2 v1 和 M2–M6 transitional v1。只在字段缺失时补默认值，拒绝 malformed/partial/unknown owned data。
- 迁移保留 workspace、tunnel/proxy/client、opaque apiKeyRef、SecurityMode、UI preference、tools/policies/bash 及 configRevision。compatibility boolean 从 authoritative mode 规范化；UI-only 保存仍不增加 Runtime revision。
- 完整 v2 验证和序列化后才 atomic replace。失败保留原始字节并清理本次创建的临时文件；第二次加载不重写、不改 revision。ConfigStore 不访问 SecretStore 原始 secret。
- Runtime 从实际 Registry 记录工具启用和 capability，typed executor 提供安全逻辑路径、bounded pattern、Bash sanitized preview 和标量结果。审批 ID/decision 可贯穿文件与 Bash 请求；终端记录 monotonic duration，不记录 write/edit 内容或 stdout/stderr/environment。
- activeRequests 包含 validation/policy/approval wait/execution，runningExecutions 只计 executor，pendingApprovals 来自 ApprovalManager。RAII 覆盖成功、失败、拒绝、过期、取消、future drop、panic unwind、并发和 stop 后清理。
- EffectiveTool/ToolDiagnostic 提供 machine reason；UI 区分 Available、ASK、Disabled、Denied、Unavailable，并给出本地化下一步。无效 Policy 显示 fail-closed 状态，不把其他 compiled ALLOW rule 显示为执行依据。
- Activity 可展开 Runtime evidence；缺失字段明确显示未记录，不补造事实。Overview 展示真实计数、revision、safe last error。Tools/Policies/Settings/Overview 复用 revision presentation，停止状态不制造 active identity 或 restart mismatch。
- Config metadata 与内容加载解耦；迁移/unsupported schema 可 Retry、复制/检查路径及确认后 Reset。没有自动 reset。修复后的配置可重试恢复。
- 六份历史配置 fixture 位于 `fixtures/config/`，只含 fake IDs/paths/references；en/zh-CN 新资源通过 exact key/placeholder parity 检查。

## 最终 Development Commands

| Command | 工作目录 | 结果 | 日志 |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | repository root | PASS | `target/m7-final-fmt.log` |
| `cargo check --workspace` | repository root | PASS | `target/m7-final-check.log` |
| `cargo test --workspace` | repository root | PASS — 458 passed, 0 failed, 4 ignored | `target/m7-final-cargo-test.log` |
| `npm test` | `ui/` | PASS — 235 tests, 21 files | `target/m7-final-ui-test.log` |
| `npm run typecheck` | `ui/` | PASS | `target/m7-final-typecheck.log` |
| `npm run build` | `ui/` | PASS | `target/m7-final-build.log` |
| `git diff --check` | repository root | PASS | `target/m7-final-diff-check.log` |

Rust 最终细分：Runtime 345、默认 transport regression 1、Tauri library 94、protocol 18；合计 458。四个 ignored 用例为既有 M2/M3/M4/M6 release black-box E2E，没有使用 `--ignored` 执行它们。日志为本工作区 ignored 开发产物。

## 安全 / 兼容判断与规格冲突处理

没有发现需要改变 M7 规格或架构的实质冲突。

- 旧测试曾 blanket 禁止任何文件名证据，与 M7 明确要求记录 validated logical path 冲突。按 M7 更新断言，继续禁止物理 Workspace root、越界路径及内容/secret 哨兵泄露；没有改变授权结果或文件 side effects。
- Shared Workspace/Security wire DTO 保持 M6 兼容解析边界；host migration/current-config raw shape validation 独立拒绝 unknown owned fields。没有为了迁移收紧 Runtime snapshot wire parsing。
- 完整 workspace 构建暴露生成 Schema 的对象键顺序漂移，已通过 recursive canonical key sorting 修复，保证 standalone/workspace 生成产物字节一致；Schema 语义没有改变。
- 最终交叉检查修正了取消证据时钟：general duration 包含审批等待，Bash execution duration 只从 executor 开始，不为审批等待伪造进程时长。Runtime status readiness 与实际 readiness gate/diagnostics 一致。
- 无法信任的 broken/unsupported config 在显式 reset 时只清理已知 app-owned 默认 credential key，不从坏文件猜测任意 secret reference。未知自定义 credential 可能保留，未自动读取或删除它们。
- 本次失败的临时文件会被清理；预先存在且非本次创建的临时路径目录不会被递归删除。

## 遗留问题 / 观察

无阻塞开发项。已有 `ts-rs` 忽略 `deny_unknown_fields` 的生成警告，以及既有 unused/dead-code 警告仍存在；Rust serde / migration validation 实际执行该安全边界。没有使用 `cargo fix` 扩大范围。

## 阶段边界

未生成或开展 M7 独立 Integration、Local Black-box E2E、Manual Acceptance 流程，未构建 Portable 验收包。规定的 `cargo test --workspace` / `npm test` 默认选择包含仓库既有、命名为 Integration 的代码级用例和默认 transport regression；这些结果仅作为全量开发回归，不升级为 M7 独立 QA 或验收结论。

没有改动 Bash production limits、Policy precedence、Approval scope、Tool schemas、Workspace boundary，也未增加 DB、protocol/v2 或新工具。M7/V3 没有被标记 ACCEPTED / FROZEN；本轮结论仅为 **M7 Development Complete**。
