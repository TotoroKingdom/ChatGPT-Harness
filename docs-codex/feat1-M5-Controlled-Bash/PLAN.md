# M5 PLAN — Controlled Bash

> Input: `SPEC.md`  
> Baseline: M4 ACCEPTED / FROZEN  
> Goal: 将 Developer/Test Mode Bash 接入现有 Runtime Pipeline / Policy / Approval，并提供真实 Windows process-tree limits。  
> Rule: PLAN 只定义实现路径；Integration/E2E/Manual Acceptance另行生成。

## 1. Strategy

沿现有 V3链路增量扩展：

```text
Config / Runtime Snapshot
→ BashDiscovery
→ ToolRegistry DeveloperShell
→ MCP bash adapter
→ CommandAnalyzer
→ PolicyEngine
→ M4 SessionGrant / ApprovalManager
→ ExecutionPermit
→ BashRuntime
→ ProcessTreeOwner
→ Activity / Audit
```

M5不建立：

- Bash专用 control plane。
- Bash专用 approval manager。
- Tauri shell executor。
- string blacklist authorization。
- fake Workspace sandbox。

关键实现顺序：

1. 先建立 config/readiness/capability。
2. 再冻结 command facts与 policy matcher。
3. 再实现真实 process runner/limits。
4. 最后接 Pipeline/MCP/Approval。
5. UI只消费 Runtime truth。

## 2. Phase A — Bash Config / Protocol

扩展 `protocol/src/v1/config.rs`：

```text
BashConfig
├ autoDetect
└ executablePath?
```

加入：

```text
AppConfig.bash
RuntimeConfigSnapshot.bash
```

default：

```text
autoDetect=true
executablePath=null
```

保持：

```text
schemaVersion=1
```

同步：

- schemars。
- ts-rs。
- generated TypeScript。
- fixtures/default configs。
- ConfigStore semantic revision tests。

旧 config缺 bash时安全 default。

Exit：Tauri能持久化 Bash desired config，新 Runtime generation能收到 immutable snapshot。

## 3. Phase B — Bash Discovery / Runtime Readiness

新增 Runtime模块：

```text
runtime/src/bash/discovery.rs
runtime/src/bash/config.rs or status.rs
```

实现 selection precedence：

```text
manual path
→ validate exact
OR
auto-detect
→ deterministic candidates
```

Runtime validation：

- absolute/local。
- existing regular file。
- canonical。
- supported Git Bash identity/layout。
- bounded `--version` probe。
- failure reason/remediation。

输出内部：

```text
BashExecutableState
├ Ready { canonicalPath, source }
└ Unavailable { reason }
```

不要因为配置文件/Picker写入路径就直接 Ready。

Exit：Runtime initialization可以得到 generation-specific immutable Bash executable state。

## 4. Phase C — Tool Capability / Registry / Effective State

扩展：

```text
ToolCapability::DeveloperShell
ToolAvailability::UnavailableByMode
```

Registry开始注册 bash，顺序：

```text
read_file
glob
write
edit
bash
```

Bash descriptor：

- implemented true。
- enabled from ToolConfig。
- readiness from BashExecutableState。
- risk High。
- capability DeveloperShell。

调整 EffectiveToolCatalog：

```text
Normal + DeveloperShell
→ UnavailableByMode

DeveloperTest + disabled
→ UnavailableDisabled

DeveloperTest + enabled + Bash unavailable
→ UnavailableByRuntime / readiness reason

DeveloperTest + ready
→ evaluate catalog-level Policy
```

Catalog-level Bash Policy：

- ToolMatcher(bash)。
- Bash Tool Default。
- invalid snapshot。
- command-specific rules不决定整个 Tool exposure。

Exit：Tools UI/MCP能够区分 Mode / Disabled / Runtime unavailable / Policy。

## 5. Phase D — Matcher Protocol Evolution

重构现有 `PolicyMatcherConfig` 为 wire-compatible tagged matcher。

目标：

```text
Tool { tool }
BashCommand { executable, argvPrefix }
```

确保原 JSON：

```json
{"type":"tool","tool":"write"}
```

继续可读/可写。

新增：

```json
{"type":"bash_command","executable":"git","argvPrefix":["status"]}
```

更新：

- ConfigStore validation。
- Runtime compile validation。
- Policies UI generated types。
- existing M3 tests。

允许：

```text
ToolMatcher { tool=bash }
```

以前 M3为了未实现 bash而拒绝的配置约束在 M5解除。

Exit：Policy config可表达 Bash全局规则和 command-specific规则，不破坏 M3 rule persistence。

## 6. Phase E — CommandAnalyzer Core

新增：

```text
runtime/src/bash/analyzer.rs
```

设计 typed facts：

```text
BashCommandAnalysis
BashCommandSegment
BashOperator
BashRiskFlag
BashClassification
```

Analyzer职责：

- quote-aware lexical scanning。
- escape处理。
- simple executable/argv。
- top-level `&& || ; |`。
- dynamic/high-risk detection。
- normalized executable/argv。
- deterministic command scope serialization。
- sanitized display projection。

禁止：

- regex/substring blacklist作为主模型。
- analyzer重写要执行的 command。

### Conservative flags

至少识别：

- command substitution。
- backticks。
- eval。
- bash/sh -c。
- interpreter inline code。
- unsupported/malformed quote/grammar。
- risky redirection/dynamic expansion when not safely modeled。

结果：

```text
uncertain → UnclassifiedHighRisk
```

Exit：Analyzer单独可被大量 deterministic unit tests覆盖，无 process spawn。

## 7. Phase F — Bash Policy Evaluation

扩展 M3 `PolicyContext` / compiled rule model，使 Runtime请求可携带 optional typed Bash facts。

不要让 UI计算 policy。

实现：

### Catalog evaluation

无 concrete command：

```text
ToolMatcher(bash) + Bash default
```

### Request evaluation

对每个 Bash segment：

```text
matching ToolMatcher(bash)
+
matching BashCommandMatcher
→ DENY > ASK > ALLOW
```

若无显式 rule：

```text
Bash default
```

apply analyzer floor：

```text
Classified → ALLOW floor
UnclassifiedHighRisk → ASK floor
```

compound：

```text
max(all segments)
```

matched IDs：

- union。
- dedupe。
- stable sort。

invalid enabled Bash matcher：

```text
whole snapshot invalid
→ fail closed
```

Exit：PolicyEngine可在不启动任何 process的情况下确定 Bash command decision。

## 8. Phase G — Bash Tool Request / Output

新增 Runtime tool model和 MCP DTO：

```text
BashRequest { command }
BashOutput { exitCode, stdout, stderr, durationMs }
BashError
```

validate：

- non-empty。
- no NUL。
- <= 8192 UTF-8 bytes。

MCP tool annotations：

```text
readOnly=false
destructive=true
openWorld=true
```

Tool description明确高风险 Developer/Test shell。

Exit：adapter schema/metadata冻结，但尚不运行真实 Bash。

## 9. Phase H — Windows ProcessTreeOwner

这是 M5独立安全子系统。

新增：

```text
runtime/src/bash/process_tree.rs
```

目标：

- each invocation own Job Object/process group。
- kill-on-close semantics。
- timeout/cancel/output-limit/shutdown均终止 descendants。
- unrelated processes不受影响。
- handle cleanup幂等。

Windows优先：

```text
Job Object
+ race-safe process assignment
```

允许引入 target-specific minimal dependency：

```text
windows-sys
```

或等价 wrapper。

实现必须避免明显的先运行再 assign race；必要时使用 suspended creation / startup attributes / equivalent safe technique。

为 process ownership提供低层测试 seam，不依赖真实危险命令。

Exit：可以可靠创建、终止并等待一个 owned process tree。

## 10. Phase I — Bash ProcessRunner / Limits

新增：

```text
runtime/src/bash/runtime.rs
runtime/src/bash/output.rs
```

Runner输入：

- validated Bash executable。
- original raw command。
- canonical Workspace cwd。
- cancellation token。

spawn：

```text
bash.exe --noprofile --norc -c <original command>
```

删除/neutralize Bash startup injection env，如 `BASH_ENV` / exported functions。

执行 limits：

```text
timeout = 30s
output = 256 KiB combined
concurrency = 1
command bytes = 8192
```

### Output capture

- stdout/stderr并行 drain，避免 pipe deadlock。
- raw bytes先计限。
- overflow立即 cancel whole process tree。
- successful output decode稳定。
- process exit code结构化返回。

### Cancellation

共享 Runtime cancellation + invocation cancellation。

Runtime stop必须可以 enumerate/cancel active invocation owners。

Exit：BashRuntime在没有 Policy/MCP的独立测试中通过正常、timeout、output、concurrency、tree cleanup。

## 11. Phase J — Async ExecutionPipeline Integration

当前 M4 pipeline已经 async-capable用于 Approval。

M5扩展 allowed execution seam以支持 Bash长时间异步运行。

要求：

- Bash不能在 MCP Tokio event loop做 blocking wait。
- file executors可保留当前同步实现。
- ExecutionPermit仍只由 Pipeline创建。
- BashRuntime只在 ALLOW/approved后收到 permit/execution authority。

Workspace validation：

新增概念：

```text
WorkspaceTarget::WorkspaceRoot
```

只验证 cwd root，不声称 command path sandbox。

Bash flow：

```text
validate request
→ mode/capability
→ Workspace root
→ analyze
→ Policy
→ session grant / Approval
→ post-approval revalidate
→ async BashRuntime execute
```

Exit：一个 Bash MCP future能在 Approval wait与process execution阶段都保持 non-blocking。

## 12. Phase K — Bash Approval / Session Grant

复用 M4 ApprovalManager。

`ToolApprovalFacts` 对 Bash增加：

- sanitized command preview。
- normalized scope。
- internal fingerprint = exact original command + generation relevant identity。
- analyzer flags。
- working directory projection。

### Classified

AllowForSession eligible。

grant scope：

```text
exact normalized full command shape
```

### UnclassifiedHighRisk

```text
AllowForSession not eligible
```

需要 additive Approval DTO capability，例如：

```text
allowForSessionEligible
allowForSessionUnavailableReason?
```

Control/UI resolve如果强行提交 AllowForSession到不eligible request：

```text
fail closed
```

不能偷偷降级为 AllowOnce。

Exit：Bash Approval保持 same-request semantics且 reusable grant不扩大动态命令风险。

## 13. Phase L — MCP / Runtime Lifecycle Wiring

`McpService`：

- 注册 bash adapter。
- tools/list exposure按 effective state。
- call_tool async执行。

`RuntimeCore`：

- own BashRuntime/Bash executable state。
- initialize generation resources。
- stop时：
  1. mark not ready。
  2. cancel pending approvals。
  3. cancel active Bash invocation trees。
  4. clear grants。
  5. stop MCP。

rollback/crash路径也清理 Bash owned resources。

Exit：Bash lifecycle归 Runtime ownership，不归 Tauri/Tunnel。

## 14. Phase M — Activity / Audit Evidence

扩展 sanitized evidence：

- normalized command classification。
- risk flags。
- logical cwd。
- Policy/matched IDs。
- Approval。
- process exit code。
- duration。
- timeout/output/concurrency/cancel categories。

不存：

- full env。
- unbounded output。
- unbounded raw command。
- Harness secrets。

Activity错误分类能区分：

```text
policy denied
approval denied
bash unavailable
timeout
output limit
concurrency
spawn/process failure
cancelled
```

Exit：用户能诊断 Bash为什么没有执行/如何结束。

## 15. Phase N — Tools UI / Bash Configuration

Tools页面增加 Bash row/config panel：

- Desired enabled。
- mode restriction。
- effective availability。
- policy。
- Runtime readiness。
- Auto Detect。
- manual override。
- file picker / clear。
- remediation。
- Restart Required。

Runtime stopped时：

- desired config仍可编辑。
- effective不伪造 Ready。

Runtime running config修改：

- save config。
- revision mismatch。
- restart required。
- current Bash effective保持旧 generation。

Exit：UI符合 M2 desired/effective semantics。

## 16. Phase O — Policies UI

扩展 matcher editor：

```text
Tool
Bash Command
```

Bash Command fields：

- executable。
- argv prefix。
- decision。
- enabled/name/reason。

提供简洁说明：

- prefix是 token prefix，不是 substring/regex。
- dynamic shell expressions仍可能强制 ASK。
- Tool-level Bash ALLOW不保证 dynamic command自动 ALLOW。

System guardrails read-only。

Exit：用户能创建 `git status ALLOW` / `git push ASK` / `shutdown DENY` 一类规则。

## 17. Phase P — Approval Center Bash UX

Approval card增加：

- command preview。
- normalized segments。
- workspace cwd。
- policy IDs。
- high-risk flags。
- session grant eligibility。

Classified：

- Allow once。
- Allow for session。
- Deny。
- Create rule。

Unclassified：

- Allow once。
- Deny。
- Create rule。
- Allow for session disabled。

Create Rule：

- classified command prefill BashCommand matcher。
- 不 auto-save。
- 不 auto-resolve current request。

Exit：M4 UX语义保持，Bash风险信息充分。

## 18. Test Architecture

### Analyzer tests

纯 unit table-driven，覆盖大量 quote/operator/dynamic cases。

### Process tests

使用 harmless child commands验证：

- normal exit。
- nonzero exit。
- timeout。
- large output。
- child/grandchild termination。
- cancellation。
- concurrency。

Windows-specific tree tests必须验证 descendants确实退出。

### Policy tests

不 spawn process即可验证：

- command matcher。
- severity。
- floor ASK。
- invalid snapshot。

### Approval tests

使用真实 analyzer facts + M4 Manager，验证 exact command session grant。

不要让同一个 giant test承担 analyzer+policy+spawn+UI全部责任。

## 19. Dependency Discipline

优先最小依赖。

允许：

- Windows process API wrapper dependency（如 `windows-sys`）。
- 经审查的 shell lexical/parser dependency，如果它能严格满足 frozen grammar。

如果 parser dependency无法提供保守 classification，宁可实现小型 typed scanner，也不要使用黑名单 regex。

任何新增 dependency必须说明：

- purpose。
- security role。
- why std/tokio insufficient。

## 20. Change Discipline

M5不得：

- 提前做 M6翻译。
- 提前把 schemaVersion升 2。
- 增加 PowerShell/CMD tool。
- 将 Bash设为 Normal可用。
- 将 bash默认打开。
- 将 Bash default policy改为 ALLOW。
- 允许 UI生成 ExecutionPermit。
- 让 Approval绕过 DENY。
- 让 session grant覆盖 unclassified dynamic command。
- 声称 OS sandbox。
- 让 Tauri代执行 shell。

## 21. Development Completion Check

至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests/typecheck/build per repository
git diff --check
```

核心开发验证：

```text
config/default/revision
Git Bash discovery/readiness
DeveloperShell mode gate
registry/effective exposure
matcher compatibility
analyzer
policy severity/floor
MCP schema/annotations
process tree
timeout/output/concurrency
async cancellation
Approval/session grant
Runtime shutdown
UI desired/effective
M0–M4 regression
```

完成后再独立生成 M5 Integration / Local Black-box E2E / Manual Acceptance。
