# M5 SPEC — Controlled Bash

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M5 — Controlled Bash  
> Baseline: M4 — Human Approval Workflow, ACCEPTED / FROZEN  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, M0–M4 milestone docs

## 1. Objective

M5 在 Windows-first Runtime 中增加一个受控的高风险 `bash` Tool，用于开发工作，同时保持 V3 已冻结的安全链：

```text
ChatGPT
→ Tunnel
→ Runtime MCP
→ bash adapter
→ ExecutionPipeline
→ Capability / Mode Gate
→ Workspace cwd validation
→ CommandAnalyzer
→ PolicyEngine
→ SessionGrant / ApprovalManager
→ ExecutionPermit
→ BashRuntime
→ Activity / Audit
```

M5 的目标不是提供完整 terminal，也不是提供 OS sandbox。

核心安全定位：

> **Bash 是 Developer/Test Mode 下的高风险能力。Workspace cwd 不是 filesystem sandbox；Policy、Approval、hard limits 和 process-tree ownership 是 M5 的补偿控制。**

## 2. Baseline Facts

M4 后当前代码已经具备：

- `SecurityMode::Normal / DeveloperTest`。
- `ToolConfig.bash.enabled` persisted future field，默认 OFF。
- Bash Tool Default = ASK。
- Registry 当前明确不注册 bash。
- `ToolCapability` 当前只有 WorkspaceRead / WorkspaceWrite。
- `EffectiveTool` 已有 `AvailableWithApproval`。
- M3 typed User Policy仍只有 Tool matcher。
- M4 Runtime-owned `ApprovalManager`。
- M4 `SessionGrantStore`。
- same-request async approval resume。
- `ExecutionPermit` 只能由 Pipeline创建。
- current MCP handler已经 async-capable。
- current file tools仍使用 synchronous executor path。
- Runtime stop已具备 pending approval/session grant cleanup。

M5 应复用 M4 Approval 和 M3 Policy，不得新建 Bash-specific authorization path。

## 3. Scope

M5 实现：

1. `BashConfig` persistence + Runtime snapshot。
2. Git Bash auto-detect / manual override。
3. Runtime authoritative executable validation/readiness。
4. `ToolCapability::DeveloperShell`。
5. `bash` Registry / EffectiveTool / MCP exposure。
6. Bash MCP input/output schema。
7. typed Bash command analyzer。
8. Bash command-specific Policy matcher。
9. compound command deterministic severity aggregation。
10. unclassifiable high-risk → ASK floor。
11. M4 Approval for Bash。
12. Bash session grants with normalized exact command scope。
13. execution timeout。
14. max output bytes。
15. max command length。
16. max concurrent Bash executions。
17. cancellation + Runtime shutdown cleanup。
18. Windows process-tree ownership。
19. Bash Activity/Audit evidence。
20. Tools / Policies UI M5 support。
21. M0–M4 regression。

M5 不实现：

- PowerShell tool。
- CMD tool。
- generic terminal abstraction。
- interactive TTY/PTY。
- stdin streaming/session shell。
- long-lived shell sessions。
- full Windows sandbox。
- network sandbox。
- filesystem sandbox for Bash。
- Linux/macOS release support。
- arbitrary Boolean Policy language。
- arbitrary shell AST coverage。
- persistent session grants。
- M6 i18n。
- M7 formal config schema migration/evidence hardening。

## 4. Security Position

### 4.1 Developer/Test Mode is eligibility, not bypass

Bash只有同时满足：

```text
security.mode = DeveloperTest
AND bash.enabled = true
AND Git Bash executable Ready
AND Runtime Ready
AND Policy != DENY
```

才具备 MCP exposure资格。

Developer/Test Mode不得绕过：

- System Guardrails。
- User DENY。
- Approval。
- execution limits。
- process cleanup。
- Activity/Audit。

现有 `security.developerTestMode` legacy/config compatibility field不得成为第二套独立 authorization authority；M5 mode gate以 typed `SecurityMode::DeveloperTest` 为准。

### 4.2 No fake sandbox claim

Bash固定：

```text
cwd = active Workspace root
```

但 Bash child仍可能访问：

- Workspace外 filesystem。
- network。
- user environment。
- external processes。

因此 Tool description、UI、Approval、README/验收不得声明：

```text
bash is sandboxed to Workspace
```

M5只承诺 controlled cwd + Policy/Approval/limits/process ownership。

## 5. Persistent Bash Config

### 5.1 Config shape

在现有 schemaVersion = 1 上 additive 增加：

```text
AppConfig
└ bash
   ├ autoDetect: bool
   └ executablePath?: string
```

推荐：

```text
BashConfig
```

defaults：

```text
autoDetect = true
executablePath = null
```

`RuntimeConfigSnapshot` 同步增加 non-secret `bash`。

旧 config 缺 `bash` 时使用 default。

M5 不把 schemaVersion升到 2；正式 v1→v2 migration仍留 M7。

### 5.2 Desired/effective semantics

Bash config与 Tool/Policy config一样：

```text
save desired config
→ configRevision changes
→ current Runtime generation unchanged
→ Restart Required
→ next generation validates Bash executable
```

不做 Bash executable hot reload。

## 6. Git Bash Discovery / Validation

### 6.1 Selection precedence

Runtime使用确定顺序：

```text
manual executablePath present
    ↓
validate exact configured candidate
    ↓
Ready OR Unavailable

manual path absent + autoDetect=true
    ↓
deterministic Git-for-Windows discovery
    ↓
first Runtime-validated candidate

manual path absent + autoDetect=false
    ↓
Unavailable
```

若用户明确配置 manual path但该路径无效，Runtime不得静默改用另一个 auto-detected bash。

### 6.2 Auto-detect

Windows V3只支持 Git for Windows Bash。

Auto-detect可检查：

- Git for Windows standard install locations。
- Git installation derivable from a validated `git.exe` on PATH。
- other deterministic Git-for-Windows locations already used by the product。

不得因为系统里存在任意 `bash.exe` 就自动宣称 Ready，例如 WSL/Cygwin Bash不能被 M5自动当成受支持 Git Bash。

### 6.3 Runtime authoritative validation

最终 candidate必须由 Runtime验证，至少：

- local absolute path。
- existing regular file。
- canonical path安全可读。
- executable filename/layout符合 supported Git Bash expectation。
- bounded `--version` probe可成功启动并终止。
- validation failure产生明确 unavailable reason/remediation。

Tauri file picker / UI detection只能提供 desired path；不能替代 Runtime readiness。

## 7. Bash Runtime Limits

M5冻结 Runtime hard limits，不作为用户可调 policy：

```text
MAX_COMMAND_BYTES          = 8192
EXECUTION_TIMEOUT          = 30 seconds
MAX_CAPTURED_OUTPUT_BYTES  = 262144   # stdout + stderr raw bytes combined
MAX_CONCURRENT_BASH        = 1
```

实现以集中 constants / typed limits表示。

未来如需配置化可单独演进；M5 不把安全 limit暴露成任意用户可放宽配置。

### 7.1 Command length

command按 UTF-8 bytes计数。

reject：

- empty/whitespace command。
- NUL。
- > MAX_COMMAND_BYTES。

在 shell spawn前失败。

### 7.2 Timeout

达到 timeout：

```text
terminate invocation-owned process tree
→ wait cleanup
→ BASH_TIMEOUT
→ evidence
```

### 7.3 Output limit

stdout + stderr按 raw bytes累计。

超过上限：

```text
terminate process tree
→ BASH_OUTPUT_LIMIT
→ bounded evidence
```

不能只是截断后让无限输出进程继续运行。

### 7.4 Concurrency

超过 `MAX_CONCURRENT_BASH`：

```text
fail immediately
→ BASH_CONCURRENCY_LIMIT
```

M5不排队无限等待。

## 8. Bash Tool Contract

### 8.1 Input

MCP schema：

```json
{
  "command": "git status"
}
```

M5不接受：

- user-specified cwd。
- user-specified timeout override。
- arbitrary environment override。
- interactive stdin。

### 8.2 Working directory

Runtime authoritative：

```text
cwd = canonical active Workspace root
```

Bash executor不能接受客户端传入 cwd绕过。

### 8.3 Invocation

推荐：

```text
bash.exe --noprofile --norc -c <original command>
```

command必须作为单独 process argument传递，不由 Harness重新拼接成另一条 shell string。

Runtime应移除会在 command执行前隐式注入 shell代码的环境入口，例如：

- `BASH_ENV`。
- exported Bash function entries。
- equivalent startup injection state identified by implementation。

其余环境可继承以保持 Git/开发工具可用，但 M5不声称 hermetic environment。

### 8.4 Output

正常 process exit返回结构化结果：

```text
BashOutput
├ exitCode
├ stdout
├ stderr
└ durationMs
```

non-zero exit code是正常 shell process result，不等于 Runtime transport failure。

stdout/stderr：

- output cap按 raw bytes enforcement。
- successful bounded bytes转为 UTF-8 text；invalid sequences采用明确、稳定的 lossy replacement语义。
- Activity/Audit不保存完整 stdout/stderr。

### 8.5 MCP metadata

`bash`：

```text
readOnly = false
destructive = true
openWorld = true
risk = High
```

不能用虚假 annotation降低 ChatGPT确认体验。

## 9. Tool Registry / Capability

新增：

```text
ToolCapability::DeveloperShell
```

Registry开始正式注册：

```text
read_file
glob
write
edit
bash
```

稳定顺序保持 bash 在最后。

Bash descriptor：

- implemented = true。
- enabled = snapshot.tools.bash.enabled。
- risk = High。
- readiness由 Runtime Bash executable validation产生。
- capability = DeveloperShell。

### 9.1 Mode gate

Normal Mode：

```text
DeveloperShell → UnavailableByMode
```

不是：

- ToolDisabled。
- PolicyDenied。

Effective tool model新增/启用：

```text
UnavailableByMode
```

Developer/Test Mode下再继续检查 enabled/readiness/policy。

## 10. Bash Exposure Semantics

对于 Bash catalog exposure，在尚无具体 command时只评估：

- capability/mode。
- enabled。
- executable readiness。
- Runtime readiness。
- invalid Policy snapshot。
- explicit tool-level `bash` User Rules。
- Bash Tool Default。

command-specific rules不应因为某一命令 DENY而隐藏整个 Bash tool。

结果：

```text
catalog policy ALLOW → Available
catalog policy ASK   → AvailableWithApproval
catalog policy DENY  → UnavailableByPolicy
```

MCP `tools/list`包含：

- Available。
- AvailableWithApproval。

## 11. Typed Bash Policy Matcher

M5扩展 M3 matcher为真正 tagged model，同时保持现有 Tool matcher JSON兼容：

```text
PolicyMatcher
├ Tool
│  └ tool
└ BashCommand
   ├ executable
   └ argvPrefix[]
```

wire概念：

```json
{ "type": "tool", "tool": "write" }

{ "type": "bash_command", "executable": "git", "argvPrefix": ["status"] }
```

BashCommand matcher隐含：

```text
tool = bash
```

### 11.1 Matcher validation

enabled BashCommand rule要求：

- executable non-empty。
- executable是一个 normalized command token，不包含 shell operator。
- argvPrefix是 bounded token vector。
- matcher只能作用于 bash。
- duplicate rule IDs仍使 snapshot invalid。
- malformed enabled matcher继续 whole-snapshot fail closed。

M5同时允许 persistent Tool matcher：

```text
tool = bash
```

作为全 Bash显式 User Rule。

## 12. CommandAnalyzer

M5不能用简单 substring/regex blacklist作为主要授权机制。

Analyzer输入：

> 原始 command string。

输出 internal typed facts：

```text
BashCommandAnalysis
├ segments[]
│  ├ executable
│  ├ argv[]
│  └ riskFlags[]
├ operators[]
└ classification
   ├ Classified
   └ UnclassifiedHighRisk
```

Policy facts不包含 shell output/secrets。

### 12.1 Supported structural analysis

Analyzer必须 quote/escape aware，至少可靠识别 top-level：

- simple executable + argv。
- `&&`。
- `||`。
- `;`。
- `|`。

quoted operator text不能误切 segment。

### 12.2 Conservative high-risk classification

以下形态至少产生 `UnclassifiedHighRisk` / ASK floor：

- command substitution `$(...)`。
- backticks。
- `eval`。
- nested shell execution such as `bash -c`, `sh -c`。
- interpreter inline code such as `python -c`, `node -e` and equivalent detected forms。
- redirection when analyzer不能可靠 model target/semantics。
- dynamic expansion that can materially change executable/argv。
- malformed/unbalanced quoting。
- unsupported shell grammar。
- any expression analyzer无法证明与生成 facts一致。

Analyzer parse uncertainty：

```text
must not → ALLOW
must → at least ASK
```

System Guardrail明确 DENY仍保持 DENY。

### 12.3 Analyzer does not rewrite execution

真正执行：

> 原始、已批准 command。

不得执行由 analyzer重新序列化的 command。

Analyzer只产生 Policy/Approval facts。

## 13. Command Normalization

用于 matcher/grant的 normalized facts必须 deterministic。

最低要求：

- quote decoding由 analyzer产生 logical argv。
- executable identity使用确定 normalization。
- Windows常见 `.exe` suffix处理一致。
- command matcher使用 exact executable + argv prefix。
- rule匹配不使用 substring。

示例：

```text
rule: executable=git, argvPrefix=["status"]

git status
git status --short
→ match

git stash
git statusx
→ no match
```

## 14. Bash Policy Evaluation

### 14.1 Per-segment explicit rules

对于每个 classified segment，匹配集合包括：

- enabled `ToolMatcher { tool=bash }`。
- enabled matching `BashCommandMatcher`。

存在显式匹配时：

```text
DENY > ASK > ALLOW
```

无显式匹配时：

```text
Bash Tool Default
```

默认 ASK。

### 14.2 Analyzer risk floor

每个 segment/command还有 analyzer risk floor：

```text
Classified            → ALLOW floor
UnclassifiedHighRisk  → ASK floor
```

最终：

```text
segment decision = max(policy decision, analyzer floor)
```

因此：

```text
broad bash ALLOW
+ unclassified dynamic command
→ ASK
```

这保证无法可靠分类的高风险表达式不会因宽泛 ALLOW rule变成自动执行。

### 14.3 Compound aggregation

每个 segment独立评估。

整条 command：

```text
max(segment decisions)
```

例如：

```text
git status = ALLOW
git push   = ASK

git status && git push
→ ASK
```

任一 DENY：

```text
whole command → DENY
no shell spawn
```

matched rule IDs合并、去重、稳定排序。

## 15. Effective Policy Projection

Policies UI / Runtime effective snapshot必须能展示 Bash规则。

对于无 command的 Bash tool row：

- Tool-level bash rules + Tool Default用于 catalog decision。
- command-specific rules单独列在 User Rules中。
- UI不得假装一个 command-specific rule代表整个 Bash effective decision。

Bash command实际 request的 Activity/Approval evidence才展示 command-specific matched IDs。

## 16. Approval Integration

M5复用 M4：

```text
Policy ASK
→ ApprovalManager Pending
→ same request waits
→ AllowOnce / AllowForSession / Deny
```

不得创建 Bash-specific approval manager。

Approval至少展示：

- tool = bash。
- sanitized command preview。
- normalized command/segment scope。
- working directory = Workspace logical identity。
- matched policy IDs。
- risk = High。
- reason。
- analyzer high-risk flags。
- expiry。

### 16.1 AllowOnce

绑定 original raw command request fingerprint。

批准后仍需要重新检查：

- Runtime Ready。
- Developer/Test mode。
- bash enabled。
- executable readiness。
- exact command fingerprint。
- Policy/System DENY。
- process limits。

### 16.2 AllowForSession — classified commands

对于 `Classified` command，session grant scope冻结为：

```text
same Runtime generation
+ tool=bash
+ exact normalized full command shape
  (segments + operators + logical argv)
```

它不是 Tool-wide bash allow，也不是 persistent Policy rule。

### 16.3 Unclassified commands

对于 `UnclassifiedHighRisk` command：

```text
AllowForSession = not eligible
```

仅允许：

- AllowOnce。
- Deny。
- Create Rule draft（仍受 matcher能力限制）。

原因：动态/无法可靠分类的同一 shell文本可能在不同环境下产生不同实际行为，不能安全生成 normalized reusable grant。

Approval DTO/UI需要能表达：

```text
allowForSessionEligible = false
reason
```

## 17. Bash Session Grant Matching

Classified grant必须 exact-match normalized full command shape。

例如 grant：

```text
git status --short
```

不自动覆盖：

```text
git status
git status --branch
git push
```

除非实现的 displayed scope本身明确且 deterministic地表示更宽范围；M5默认采用 exact shape，避免过宽。

Session grant永远不能覆盖：

- explicit DENY。
- System Guardrail。
- Normal Mode。
- disabled bash。
- executable unavailable。
- Runtime not ready。
- analyzer ASK floor for unclassified command。

## 18. Workspace Pipeline Semantics

Bash仍必须经过 ExecutionPipeline，但 Bash没有单个 filesystem target可供 WorkspaceGuard限制。

M5扩展 Workspace validation concept，例如：

```text
WorkspaceTarget::WorkspaceRoot
```

Bash阶段只验证：

- active Workspace root仍有效。
- working directory使用该 canonical root。

这项 validation **不是** 对 command内所有 path的 sandbox authorization。

Pipeline/Activity必须避免把它描述成“command只能访问 Workspace”。

## 19. Async Execution Boundary

Bash可能运行数十秒，因此不能在 MCP Tokio event loop上执行 blocking wait。

M5要求 ExecutionPipeline allowed path支持 tool-specific async/cancellation-aware execution。

可采用：

- async ToolExecutor seam。
- dedicated Bash execution task + async completion。
- equivalent non-blocking architecture。

要求：

- file tools无需被全面重写。
- Bash execution wait不能阻塞 MCP control/approval work。
- MCP request cancellation/drop必须触发 process cleanup。
- Runtime stop必须能主动取消所有 active Bash invocations。

## 20. BashRuntime

Runtime新增：

```text
BashRuntime
├ BashDiscovery
├ CommandAnalyzer
├ BashPolicyFacts
├ ProcessRunner
├ OutputLimiter
└ ProcessTreeOwner
```

BashRuntime由 Runtime generation拥有。

不得由 Tauri执行 shell。

## 21. Windows Process-tree Ownership

每个 Bash invocation拥有独立 process-tree owner。

目标：

```text
timeout
cancellation
MCP request drop
Runtime shutdown
output limit
→ terminate full invocation-owned tree
```

不能只 kill `bash.exe` parent留下 descendants。

Windows实现优先使用 Job Object语义，例如：

```text
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
```

实现必须避免明显的：

```text
spawn running process
→ child already forks descendants
→ later assign job
```

escape race。

可以使用 minimal Windows platform wrapper / `windows-sys` 等依赖实现 create/assign/resume 或等价 race-safe ownership。

不得让 Tauri Tunnel Job Object代替 Runtime Bash ownership。

## 22. Cancellation Model

每个 invocation有 Runtime-owned cancellation signal。

触发来源至少：

- timeout。
- output limit。
- Runtime stop。
- MCP request cancellation/drop（transport能观察时）。
- internal execution cancellation。

cancel流程：

```text
signal cancel
→ terminate Job/process tree
→ drain/join bounded readers
→ close handles
→ return typed error
→ evidence
```

cleanup必须幂等。

## 23. Bash Error Contract

新增稳定错误分类，例如：

```text
BASH_UNAVAILABLE
BASH_INVALID_COMMAND
BASH_COMMAND_TOO_LONG
BASH_TIMEOUT
BASH_OUTPUT_LIMIT
BASH_CONCURRENCY_LIMIT
BASH_SPAWN_FAILED
BASH_CANCELLED
BASH_PROCESS_FAILED
```

Policy/Approval已有错误保持：

```text
POLICY_DENIED
APPROVAL_DENIED
APPROVAL_EXPIRED
APPROVAL_CANCELLED
TOOL_DISABLED
RUNTIME_NOT_READY
```

错误不得泄露完整 environment/secret-bearing command details。

## 24. Activity / Audit

每个 Bash request至少关联：

- request ID。
- tool = bash。
- mode/capability result。
- sanitized command evidence。
- normalized command classification。
- working directory logical identity。
- policy decision。
- matched rule IDs。
- analyzer risk flags。
- approval ID/decision when applicable。
- execution started/completed/failed。
- exit code when process normally exits。
- timeout/output/concurrency/cancel limit category。
- duration。

Activity/Audit不得存：

- raw environment。
- Tunnel/API secrets。
- unbounded stdout/stderr。
- unbounded raw command。

M7会继续 harden final evidence schema，但 M5必须提供足够的 Bash执行证据。

## 25. Tools UI

Tools页面正式显示 Bash。

至少展示：

- Desired Enabled。
- Effective Availability。
- Developer/Test mode restriction。
- Policy decision。
- Git Bash readiness。
- unavailable reason/remediation。
- Restart Required。

Bash config UI至少支持：

- Auto Detect toggle。
- Manual executable override。
- file picker。
- clear override。

运行中修改：

```text
save AppConfig
→ configRevision
→ Restart Required
```

UI不能因为选择了一个 `bash.exe` 就显示 Runtime Ready；必须等待新 Runtime generation的 effective result。

## 26. Policies UI

M5 Policy editor增加：

- Tool rule target = bash。
- Bash Command matcher editor。

Command rule fields：

```text
Executable
Argv Prefix
Decision
Enabled
Name
Reason
```

UI保存前做 typed validation，但 Runtime compile仍 authoritative。

System/runtime risk floor不可由 UI关闭。

## 27. Approval Center M5 Extension

Bash approval card显示：

- sanitized command preview。
- normalized classified command / segments。
- working directory。
- matched policies。
- High risk。
- unclassified risk warning when applicable。
- AllowForSession eligibility。

对 unclassified：

- Allow for session disabled/hidden并说明原因。
- Allow once / Deny仍可用。

Create Rule：

- classified command可生成 BashCommand policy draft。
- unclassified request只生成用户可审查的 draft起点，不得自动形成宽泛 ALLOW。
- Create Rule仍不自动批准当前 request。

## 28. Compatibility Requirements

M5必须保持：

- M4 same-request approval resume。
- M4 file-tool session grant exact scope。
- M3 Tool Default fallback。
- M3 DENY > ASK > ALLOW。
- M3 invalid enabled rule whole-snapshot fail closed。
- M2 Tool Toggle。
- M1 atomic write/edit。
- WorkspaceGuard semantics。
- ExecutionPermit pipeline-only。
- Runtime config immutable per generation。
- no protocol major bump。
- old schema v1 configs load with default BashConfig。
- Normal Mode file tools行为不受 Bash实现影响。

## 29. Expected Code Touchpoints

```text
protocol/src/v1/config.rs
protocol/src/v1/tools.rs
protocol/src/v1/policies.rs
protocol/src/v1/approvals.rs
protocol generated TS/schema

runtime/src/bash/*                  # new
runtime/src/tools/registry.rs
runtime/src/tools/effective.rs
runtime/src/policy/engine.rs
runtime/src/execution/executor.rs
runtime/src/execution/pipeline.rs
runtime/src/state.rs
runtime/src/mcp/service.rs
runtime/src/mcp/errors.rs
runtime/src/session_grant.rs
runtime/src/approval.rs             # Bash facts/eligibility only as needed
runtime/src/activity/* / audit/*
runtime/Cargo.toml                  # Windows process ownership dependency as needed

ui/src-tauri/src/config/*
ui/src-tauri/src/runtime/*
ui/src-tauri/src/commands/*         # file picker / convenience discovery only

ui/src/pages/tools/*
ui/src/pages/policies/*
ui/src/pages/approvals/*
ui/src/services/harness/*
related tests
```

Tunnel Client不应因 Bash执行而成为 shell owner。

## 30. Development Verification Requirements

每个 Task必须有 unit/code-level tests。

开发完成至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests / typecheck / build per repository
git diff --check
```

必须自动化覆盖：

```text
legacy config missing bash → defaults
manual/auto executable resolution
invalid executable → unavailable
Normal mode → UnavailableByMode
DeveloperTest + disabled → disabled
DeveloperTest + enabled + unavailable executable → unavailable
DeveloperTest + ready + default ASK → AvailableWithApproval
bash MCP metadata
command validation / length
simple command analysis
quoted operators
compound severity
BashCommand matcher exact prefix semantics
tool-level + command-level rule aggregation
unclassified dynamic → ASK floor
DENY prevents spawn
ASK uses M4 same-request approval
classified AllowForSession exact normalized scope
unclassified session grant unavailable
timeout kills process tree
output limit kills process tree
concurrency limit
cancellation cleanup
Runtime shutdown cleanup
nonzero exit structured output
M0–M4 regression
```

## 31. Post-TASK Verification Documents

SPEC / PLAN / TASKS冻结后独立生成：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；Manual Acceptance才需要人工介入。

M5 Local Black-box E2E必须使用真实 Runtime subprocess +真实 Git Bash executable，并验证真实 process execution、ASK approval、timeout/tree termination；不得用 mock shell替代核心 golden path。

## 32. Final Acceptance Criteria

M5可 ACCEPTED / FROZEN 前必须证明：

- Normal Mode不暴露 Bash。
- Developer/Test Mode下只有 enabled + executable ready + Runtime ready + Policy非DENY时具备 exposure资格。
- Git Bash auto-detect/manual override/unavailable remediation工作。
- bash默认 OFF / default ASK保持。
- Bash MCP annotations真实为 high-risk/open-world/destructive。
- typed command analyzer deterministic。
- command-specific matcher deterministic。
- compound命令按最高 severity。
- unclassifiable high-risk最低 ASK。
- broad ALLOW不能把 unclassifiable风险降为自动 ALLOW。
- explicit DENY阻止 shell spawn。
- ASK复用 M4 same-request Approval。
- classified session grant不宽于 exact normalized command scope。
- unclassified command不创建 reusable session grant。
- cwd固定 Workspace但不虚假声明 sandbox。
- command length/timeout/output/concurrency均 Runtime enforcement。
- timeout/cancel/output-limit/shutdown终止完整 invocation process tree。
- normal nonzero exit有结构化结果。
- Activity/Audit有 Bash policy/approval/execution/limit evidence。
- M0–M4 regression PASS。
- Integration / Local Black-box E2E / Manual Acceptance完成。

## 33. Open Questions

```text
0
```

局部 parser crate、Windows wrapper和 module命名可在实现中选择，但不得改变 DeveloperShell mode gate、unclassified ASK floor、M4 approval复用、exact classified session grant、hard limits、race-safe process-tree ownership和 no-sandbox boundary。
