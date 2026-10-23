# M5 TASKS — Controlled Bash

> Inputs: `SPEC.md`, `PLAN.md`  
> Status: M5 Development Complete
> Evidence: `DEVELOPMENT_REPORT.md`
> Executor: Codex / Coding Agent  
> Baseline: M4 ACCEPTED / FROZEN  
> 每个 Task 必须包含实现 + Unit Test / code-level automated verification。  
> Integration / E2E / Manual Acceptance 不属于本文件。

## Dependency

```text
T1 Bash Config / Discovery / Registry
        ↓
T2 Command Analyzer / Bash Policy
        ↓
T3 Bash Runtime / Process Tree / Limits
        ↓
T4 Pipeline / MCP / Approval / Evidence
        ↓
T5 Tools / Policies / Approval UI
        ↓
T6 Regression / Development Completion
```

---

## T1 — Bash Config / Discovery / Registry

**Primary files:** `protocol/src/v1/config.rs`, `protocol/src/v1/tools.rs`, generated protocol artifacts, new `runtime/src/bash/discovery.rs`, `runtime/src/tools/registry.rs`, `runtime/src/tools/effective.rs`, Tauri config/runtime snapshot wiring, tests.

### Implement

#### BashConfig

增加：

```text
BashConfig
├ autoDetect: bool
└ executablePath?: string
```

加入：

```text
AppConfig.bash
RuntimeConfigSnapshot.bash
```

defaults：

```text
autoDetect=true
executablePath=null
```

保持 `schemaVersion=1`。

旧 config缺字段安全 default。

semantic changes必须继续触发 `configRevision`。

#### Discovery / validation

实现 Runtime-owned Git Bash executable resolution：

```text
manual override present
→ validate exact only

otherwise autoDetect=true
→ deterministic Git-for-Windows candidates
→ first valid

otherwise
→ unavailable
```

Runtime validation至少：

- local absolute path。
- regular file。
- canonical。
- supported Git Bash expectation。
- bounded `--version` probe。
- typed unavailable reason/remediation。

manual path invalid不得静默 fallback到其他 bash。

#### Capability / Registry

新增：

```text
ToolCapability::DeveloperShell
ToolAvailability::UnavailableByMode
```

Registry正式注册 bash，稳定顺序：

```text
read_file, glob, write, edit, bash
```

Bash descriptor：

- implemented true。
- enabled = `tools.bash.enabled`。
- readiness = Runtime validated Bash state。
- risk = High。
- capability = DeveloperShell。

#### Mode semantics

```text
Normal → UnavailableByMode
DeveloperTest + disabled → UnavailableDisabled
DeveloperTest + enabled + exe unavailable → UnavailableByRuntime
```

`security.mode` 是 authoritative mode gate；legacy `developerTestMode` 不能成为 bypass。

### Required Tests

至少覆盖：

- legacy config missing bash → default。
- save/reload BashConfig。
- configRevision semantic change / identical save。
- manual valid path。
- manual invalid path no auto fallback。
- auto-detect deterministic selection。
- arbitrary non-Git bash rejected from auto-detect。
- probe failure → unavailable。
- registry stable order includes bash last。
- bash capability = DeveloperShell。
- Normal mode = UnavailableByMode。
- disabled/readiness distinctions。
- read/glob/write/edit registry regression。

### Done

- Runtime generation可得到 immutable Bash desired config + effective executable readiness。
- 尚未添加 command execution。
- 无 M7 schema migration。

---

## T2 — Command Analyzer / Bash Policy

**Primary files:** `protocol/src/v1/policies.rs`, `protocol/src/v1/config.rs` validation, new `runtime/src/bash/analyzer.rs`, `runtime/src/policy/engine.rs`, Policies UI types only if generated, tests.

### Implement

#### Tagged matcher

将现有 matcher演进为 wire-compatible typed variants：

```text
Tool { tool }
BashCommand { executable, argvPrefix }
```

existing JSON：

```json
{"type":"tool","tool":"write"}
```

必须保持兼容。

新增：

```json
{"type":"bash_command","executable":"git","argvPrefix":["status"]}
```

允许 Tool matcher target `bash`。

enabled invalid matcher继续使 whole Policy snapshot invalid。

#### CommandAnalyzer

新增 typed model：

```text
BashCommandAnalysis
BashCommandSegment
BashOperator
BashRiskFlag
BashClassification
```

至少支持 quote/escape-aware：

- simple argv。
- `&&`。
- `||`。
- `;`。
- `|`。

至少标记为 `UnclassifiedHighRisk`：

- `$(...)`。
- backticks。
- `eval`。
- nested shell `bash/sh -c`。
- interpreter inline code。
- unsafe/unsupported redirection semantics。
- material dynamic expansion。
- malformed/unbalanced quote。
- unsupported grammar。

Analyzer不得重写执行 command。

#### Normalization

实现 deterministic：

- logical executable。
- logical argv。
- executable suffix/case normalization as frozen by SPEC。
- exact operator sequence。
- exact normalized full command scope。

BashCommand matcher按 exact executable + argv prefix匹配，不使用 regex/substring。

#### Policy Engine

Catalog-level Bash policy：

- ToolMatcher(bash)。
- Bash Tool Default。
- BashCommand rules不隐藏整个 Bash catalog。

Request-level：

每 segment匹配：

- ToolMatcher(bash)。
- BashCommandMatcher。

explicit rules：

```text
DENY > ASK > ALLOW
```

无 explicit match：

```text
Bash default
```

Analyzer floor：

```text
Classified → ALLOW
UnclassifiedHighRisk → ASK
```

最终 command：

```text
max(all segment policy decisions + floors)
```

matched IDs union/dedupe/stable sort。

### Required Tests

至少 table-driven覆盖：

- `git status`。
- quoted args。
- quoted operator不分段。
- `git status && git push`。
- `||` / `;` / pipe。
- malformed quotes。
- command substitution。
- backticks。
- eval。
- nested shell -c。
- interpreter inline code。
- redirection。
- dynamic expansion。
- ToolMatcher bash。
- BashCommand exact executable。
- argvPrefix match/non-match。
- multiple rules severity。
- compound max severity。
- no match → default ASK。
- broad ALLOW + unclassified → ASK。
- explicit DENY + unclassified → DENY。
- invalid enabled Bash matcher whole snapshot fail closed。
- old M3 Tool matcher tests unchanged。

### Done

- Policy可对 Bash command产生 deterministic evidence。
- 无 shell process spawn。
- 不以 blacklist regex作为 authorization核心。

---

## T3 — Bash Runtime / Process Tree / Limits

**Primary files:** new `runtime/src/bash/runtime.rs`, `process_tree.rs`, `output.rs`, `mod.rs`, `runtime/Cargo.toml`, tests.

### Implement

#### Frozen hard limits

```text
MAX_COMMAND_BYTES         = 8192
EXECUTION_TIMEOUT         = 30s
MAX_CAPTURED_OUTPUT_BYTES = 262144 combined
MAX_CONCURRENT_BASH       = 1
```

#### Request validation

reject：

- empty/whitespace。
- NUL。
- too long。

#### Process invocation

执行：

```text
validated bash.exe
--noprofile
--norc
-c
<original command>
```

要求：

- original command单独作为 process arg。
- cwd = canonical Workspace root。
- 不接受 MCP-supplied cwd/env。
- neutralize `BASH_ENV` / exported function startup injection或等价已识别入口。

#### Windows ProcessTreeOwner

每 invocation独立拥有 process tree。

使用 Job Object或等价 Windows primitive：

- kill-on-close。
- race-safe child assignment。
- descendants被包含。
- unrelated process不受影响。
- handles cleanup幂等。

如果需要，引入最小 target-specific `windows-sys`。

#### Async/cancellation

Runner必须 non-blocking to MCP event loop。

支持 cancellation signal：

- timeout。
- output limit。
- Runtime stop。
- request cancellation/drop。

#### Output

stdout/stderr并发 drain。

raw combined bytes超过 limit：

- terminate tree。
- return `BASH_OUTPUT_LIMIT`。

normal exit：

```text
BashOutput {
  exitCode,
  stdout,
  stderr,
  durationMs
}
```

non-zero exit仍是 structured process result。

#### Concurrency

使用 Runtime-owned semaphore/counter：

- only 1 active Bash invocation。
- second attempt immediate `BASH_CONCURRENCY_LIMIT`。
- no unbounded queue。

### Required Tests

至少覆盖：

- simple echo/output。
- stderr capture。
- nonzero exit code。
- cwd is Workspace。
- empty/NUL/too-long reject before spawn。
- timeout terminates parent。
- timeout terminates descendant/grandchild。
- output limit terminates tree。
- stdout/stderr deadlock resistance。
- concurrency limit immediate reject。
- cancellation terminates tree。
- cleanup idempotent。
- no orphan child after test。
- Runtime-level cancellation hook。
- invalid UTF-8 output stable lossy conversion。
- process spawn/probe error sanitized。

Windows process-tree test必须证明 descendant PID不再存活，不只检查 parent exit。

### Done

- BashRuntime可独立安全运行 harmless command。
- hard limits由 Runtime enforcement。
- 还未允许 MCP绕过 Pipeline调用。

---

## T4 — Pipeline / MCP / Approval / Evidence

**Primary files:** `runtime/src/execution/executor.rs`, `pipeline.rs`, `state.rs`, `runtime/src/mcp/service.rs`, `errors.rs`, `approval.rs`, `session_grant.rs`, Activity/Audit, protocol approvals/tools as needed, tests.

### Implement

#### Bash tool contract

新增：

```text
BashRequest { command }
BashOutput
BashError
```

MCP metadata：

```text
readOnly=false
destructive=true
openWorld=true
```

#### Pipeline workspace semantics

扩展：

```text
WorkspaceTarget::WorkspaceRoot
```

Bash只验证 working-directory root。

不得把 WorkspaceValidated解释为 command path sandbox。

#### Async allowed execution

扩展 Pipeline/ToolExecutor execution seam：

- Bash process wait是 async/cancellation-aware。
- file tools可继续 sync implementation。
- ExecutionPermit仍 Pipeline-only。

Bash完整链：

```text
validate
→ capability/mode
→ Workspace root
→ analyze
→ policy
→ grant/approval
→ revalidate
→ ExecutionPermit
→ BashRuntime
```

#### M4 Approval reuse

ApprovalRequest增加 Bash需要的 sanitized facts：

- command preview。
- normalized scope。
- working directory。
- risk flags。
- matched IDs/reason。

AllowOnce：

- exact original request fingerprint。

AllowForSession：

仅 Classified command eligible。

scope：

```text
same generation
+ bash
+ exact normalized full command shape
```

UnclassifiedHighRisk：

```text
allowForSessionEligible=false
```

若 Control强行 resolve AllowForSession：

- typed failure。
- request保持 Pending或按明确 fail-closed语义处理。
- 不得 silently AllowOnce。

#### Post-approval revalidation

至少重新检查：

- Runtime ready。
- mode = DeveloperTest。
- bash enabled。
- executable still generation-ready。
- Policy/System DENY。
- exact request fingerprint。
- limits/cancellation。

#### Session grant

matching grant只可降低：

```text
classified ASK → ALLOW
```

不能覆盖：

- DENY。
- unclassified ASK floor。
- mode。
- disabled。
- executable unavailable。
- Runtime unavailable。

#### MCP exposure

`tools/list`：

- catalog ALLOW → include。
- catalog ASK → include as AvailableWithApproval。
- DENY/mode/disabled/unavailable → omit。

#### Errors

新增/映射：

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

#### Lifecycle / shutdown

Runtime stop：

1. mark not-ready。
2. cancel approvals。
3. cancel active Bash trees。
4. clear grants。
5. stop MCP。

rollback/crash同样不能遗留 Bash descendants。

#### Evidence

记录：

- sanitized command/classification。
- risk flags。
- cwd。
- policy/matched IDs。
- approval。
- grant hit。
- execution result。
- exit code。
- timeout/output/concurrency/cancel。
- duration。

不记录 full output/env/secrets。

### Required Tests

至少覆盖：

- Normal mode stale bash call拒绝且不spawn。
- DeveloperTest disabled拒绝。
- executable unavailable拒绝。
- default ASK产生 same-request pending。
- AllowOnce resume Bash exact call。
- Deny no spawn。
- classified AllowForSession current call + next exact match。
- different normalized command grant miss。
- unclassified no session grant。
- broad ALLOW + unclassified still pending ASK。
- explicit DENY no spawn。
- Runtime stop during active Bash kills tree。
- Runtime stop during Bash approval cancels request。
- post-approval revalidation。
- MCP schema/annotations。
- Activity correlation。
- file-tool M4 approval regression。

### Done

- Bash只能经最终 Runtime authorization chain执行。
- M4 Approval semantics保持。
- no Tauri shell execution。

---

## T5 — Tools / Policies / Approval UI

**Primary files:** `ui/src/pages/tools/*`, `policies/*`, `approvals/*`, services/providers, Tauri file picker/convenience discovery as needed, tests.

### Implement

#### Tools

Bash row/panel展示：

- Enabled desired。
- Effective availability。
- Developer/Test restriction。
- Policy。
- readiness。
- remediation。
- Auto Detect。
- Manual override。
- Restart Required。

支持：

- toggle enable。
- autoDetect toggle。
- choose `bash.exe`。
- clear manual override。

UI path选择只是 desired config。

Runtime Ready必须来自 Runtime effective observation。

Normal Mode显示：

```text
Unavailable by mode
```

不能显示 Disabled/Denied冒充。

#### Policies

matcher editor支持：

```text
Tool
Bash Command
```

Bash Command fields：

- executable。
- argv prefix tokens。
- decision。
- enabled。
- name。
- reason。

帮助文本明确：

- prefix不是 regex。
- dynamic/unclassified expression仍至少 ASK。
- Tool-level bash ALLOW不是 sandbox。

#### Approval Center

Bash card显示：

- sanitized command preview。
- normalized segments。
- Workspace cwd。
- matched rules。
- High risk。
- risk flags。
- AllowForSession eligibility。

Classified：

- Allow once。
- Allow for session。
- Deny。
- Create rule。

Unclassified：

- Allow once。
- Deny。
- Create rule。
- Allow for session disabled + reason。

#### Create Rule

Classified Bash approval：

- prefill BashCommand matcher draft。
- 不 auto save。
- 不 auto resolve current request。

Unclassified：

- 不生成“自动永久允许”。
- 可以打开可审查 draft，但默认不得制造宽泛 silent ALLOW。

#### Restart semantics

修改：

- bash enabled。
- mode。
- executable config。
- bash policy。

均沿用：

```text
save desired
→ configRevision
→ Restart Required
→ next generation effective
```

### UI Tests

至少覆盖：

- Normal/DeveloperTest mode display。
- disabled/unavailable/policy distinctions。
- auto/manual config controls。
- save failure rollback。
- running mismatch/restart convergence。
- Bash matcher create/edit/delete。
- argv prefix editor。
- Bash Approval classified/unclassified states。
- AllowForSession disabled for unclassified。
- Create Rule prefill且不 auto-resolve。
- no false sandbox wording。
- existing file tool UI regression。

### Done

- 用户能安全配置/理解 Bash状态与 command policy。
- UI不自行推导 Runtime readiness/policy。

---

## T6 — Cross-cutting Regression / Development Completion

**Primary scope:** 全仓 automated regression，不新增产品功能。

### M4 Approval regression

- file ASK same-request resume。
- AllowOnce。
- file session grant。
- Deny/expiry/cancel。
- restart cleanup。

### M3 Policy regression

- Tool default fallback。
- explicit rule severity。
- invalid snapshot fail closed。
- System guardrails。
- tool matchers persisted。

### M2 Tool Toggle regression

- disabled stale-call rejection。
- desired/effective/restart semantics。

### M1/M0 regression

- atomic write/edit。
- read/glob。
- Workspace boundaries。
- Activity confidentiality。

### Bash cross-cutting

- bash default OFF。
- bash default ASK。
- Normal mode not exposed。
- no command executes before ALLOW/approved。
- no process orphan after any failure path。
- no raw env/full output leak。
- Runtime clean shutdown。
- registry order/schema stable。

### Development Commands

执行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests/typecheck/build per repository
git diff --check
```

全部 PASS。

### Dependency Review

对新增 parser/Windows dependency记录：

- name/version。
- purpose。
- security role。
- why required。

### Done

T1–T6完成只代表：

```text
M5 Development Complete
```

随后必须独立生成/执行：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；Manual Acceptance才需要人工介入。

---

## Development Completion Checklist

- [x] T1 Bash Config / Discovery / Registry PASS
- [x] T2 Command Analyzer / Bash Policy PASS
- [x] T3 Bash Runtime / Process Tree / Limits PASS
- [x] T4 Pipeline / MCP / Approval / Evidence PASS
- [x] T5 Tools / Policies / Approval UI PASS
- [x] T6 Regression PASS
- [x] Bash default OFF
- [x] Bash default ASK
- [x] Normal Mode = UnavailableByMode / not MCP exposed
- [x] DeveloperTest + enabled + ready works
- [x] Git Bash manual/auto detection
- [x] typed BashCommand matcher
- [x] compound severity
- [x] unclassified risk floor ASK
- [x] broad ALLOW cannot bypass floor
- [x] M4 same-request Approval reused
- [x] classified exact session grant
- [x] unclassified no reusable session grant
- [x] timeout hard kill
- [x] output-limit hard kill
- [x] concurrency limit
- [x] cancellation cleanup
- [x] descendant process-tree cleanup
- [x] Runtime shutdown cleanup
- [x] Bash MCP metadata truthful
- [x] no fake sandbox claim
- [x] M0–M4 regression PASS
- [x] cargo fmt/check/test PASS
- [x] frontend checks PASS
- [x] git diff --check PASS
- [x] TASKS未声称 Integration/E2E/Manual Acceptance PASS
