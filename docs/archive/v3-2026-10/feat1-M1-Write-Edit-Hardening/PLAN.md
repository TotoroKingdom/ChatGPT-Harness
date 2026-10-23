# M1 PLAN — Write/Edit Production Hardening

> Input: `SPEC.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Goal: 将 write/edit mutation 从 M0 非原子实现升级为共享、可验证的 production atomic text mutation path。  
> Rule: PLAN 只决定如何实现 SPEC；Integration/E2E/Manual Acceptance 另行生成独立文档。

## 1. Strategy

M1 不改变 MCP/authorization architecture，只替换 mutation internals：

```text
M0 execution path
        ↓
shared atomic file primitive
        ↓
write create / overwrite
        ↓
edit exact replacement
        ↓
failure / concurrency / cleanup hardening
```

关键原则：

- atomic primitive 只负责文件 mutation，不成为新的 security authority。
- WorkspaceGuard / Policy / ExecutionPermit ownership 不变。
- write/edit 共享一套 temp/write/flush/commit/cleanup implementation。
- fault-path 可注入测试能力优先于为了测试复制 production code。

## 2. Phase A — Freeze Atomic Primitive Contract

先新增 Runtime-internal atomic mutation abstraction，例如：

```text
atomic_file.rs
```

职责冻结为：

```text
approved target
+ prepared bytes
+ commit mode
+ revalidation callback/context
→ atomic mutation result
```

至少支持两种语义：

```text
CreateNew
ReplaceExisting
```

实现要求：

- same-directory temp。
- exclusive temp creation。
- write_all。
- flush。
- pre-commit revalidation。
- Windows existing-target atomic replace。
- success/failure cleanup。
- phase-aware internal errors。

为 unit tests 提供最小 fault injection seam，以稳定模拟：

```text
TempCreate failure
TempWrite failure
TempFlush failure
Commit failure
```

不要用真实磁盘耗尽等不稳定环境条件作为主要 fault test 方法。

Exit：atomic primitive 有独立 unit tests，证明 existing target 在 pre-commit/commit failure 下保持原 bytes。

## 3. Phase B — Harden `write`

将 `execute_write` 从当前：

```text
OpenOptions(create + truncate)
→ write_all(target)
```

迁移到：

```text
approve/revalidate
→ UTF-8 text boundary check when existing
→ atomic primitive
```

### Create path

- missing target + existing approved parent。
- temp complete 后 commit create。
- race 中 target 已出现时不静默 clobber；返回确定失败或重新进入明确 overwrite decision path。

### Overwrite path

- existing regular UTF-8 file。
- 不 truncate live target。
- atomic replace。

保留 M0 output contract。

Exit：write create/overwrite 正常路径和所有 atomic fault path 自动化 PASS。

## 4. Phase C — Harden `edit`

保留当前 exact-match semantics，只更换 mutation commit path并加入 concurrent-change detection：

```text
approve existing target
→ read original bytes
→ validate UTF-8
→ count old_text
→ build replacement
→ stage temp
→ re-read/revalidate target
→ if bytes changed: TARGET_CHANGED
→ atomic replace
```

重点验证：

- 0 match unchanged。
- >1 match unchanged。
- invalid UTF-8 unchanged。
- concurrent source change unchanged by this invocation。
- commit failure keeps current target intact。

Exit：edit 不再通过 `fs::write(target, ...)` 修改 live target。

## 5. Phase D — Error / Evidence Hardening

统一内部 atomic errors，并映射到现有 Runtime/MCP error model。

新增外部稳定语义：

```text
TARGET_CHANGED
```

其它 OS-level staging/flush/commit details 默认映射到 sanitized `IO_FAILURE` / `EXECUTION_FAILED`，除非现有 error taxonomy 已有更精确且稳定的 code。

Activity/Audit：

- 保留 request/tool/policy/outcome。
- 可区分 target changed 与 I/O commit failure。
- 不记录 raw mutation text、temp path、unsafe absolute path。

Exit：所有失败对 MCP 可诊断但不泄密，Audit 无新敏感数据。

## 6. Phase E — Cross-cutting Regression

运行和补充自动化回归：

- M0 write create golden path。
- M0 write overwrite golden path。
- M0 edit exact replacement。
- outside workspace write/edit reject。
- missing parent reject。
- read_file / glob regression。
- ToolRegistry / MCP annotations / routing unchanged。
- ExecutionPipeline / ExecutionPermit boundary unchanged。

Windows-specific：

- atomic replacement implementation 必须有 Windows test coverage。
- 如果 platform API wrapper 有 `cfg(windows)`，非 Windows test/build path 必须明确、可编译，不能让 workspace CI 静默缺失核心代码。

## 7. Dependency Discipline

如果标准库不能提供满足 SPEC 的 Windows atomic replace semantics：

- 优先选择小型、边界明确、维护活跃的依赖；或
- 写最小 Windows API wrapper。

不得为了 M1 引入大型 filesystem framework。

新增 dependency 前检查：

- license。
- Windows API semantics。
- overwrite existing target behavior。
- temp/rename cleanup behavior。
- 是否隐含跨-volume fallback copy（M1 不接受这种 fallback 冒充 atomic）。

## 8. Development Completion Check

完成所有开发 Task 后运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

并确认至少存在稳定自动化测试覆盖：

```text
write create success
write overwrite success
write existing binary rejected
edit success
edit target-changed rejection
0/multi match unchanged
temp write failure preserves target
flush failure preserves target
commit failure preserves target
failed create leaves no named target
temp cleanup
outside boundary regression
read/glob regression
```

随后单独生成并执行 `Integration Test Plan`、必要的 E2E regression smoke、`Manual Acceptance Checklist`。

## 9. Change Discipline

- 不实现 M2 Tool Toggle。
- 不实现 M3 configurable Policy。
- 不实现 M4 Approval。
- 不修改 Tunnel/Tauri/UI ownership。
- 不引入 protocol major。
- 不把 power-loss durability 宣称为已解决。
- 不把 local atomic rename 当作 filesystem sandbox。
- 不修改 M0 frozen evidence 文档来“适配”M1。
