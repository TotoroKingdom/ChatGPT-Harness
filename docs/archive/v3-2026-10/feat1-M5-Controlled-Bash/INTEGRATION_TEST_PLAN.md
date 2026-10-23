# M5 Integration Test Plan — Controlled Bash

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M5 — Controlled Bash  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Scope: Protocol / ConfigStore / Bash discovery / Registry / Analyzer / Policy / Approval / SessionGrant / BashRuntime / process ownership / UI component integration. This is not Local Black-box E2E and does not require External ChatGPT.

## 1. Objective

验证 M5 各 Task 组合后形成一致的 Controlled Bash system：

```text
Desired Bash Config
→ RuntimeConfigSnapshot
→ Git Bash readiness
→ DeveloperShell capability
→ CommandAnalyzer
→ PolicyEngine
→ M4 Approval / SessionGrant
→ BashRuntime / ProcessTreeOwner
→ Activity / Audit
→ Tools / Policies / Approval UI
```

核心不变量：

- Normal Mode不能通过任何 Policy/Approval开启 Bash。
- Bash default OFF / default ASK。
- Runtime而不是 UI 决定 executable readiness。
- Command Policy基于 typed command facts，不依赖 substring blacklist。
- 无法可靠分类的高风险表达式至少 ASK。
- explicit DENY始终阻止 shell spawn。
- Approval复用 M4 same-request lifecycle。
- classified session grant仅匹配 exact normalized command shape。
- unclassified command不生成 reusable session grant。
- cwd=Workspace不等于 filesystem sandbox。
- timeout/output/concurrency/cancellation/shutdown由 Runtime enforce。
- process-tree termination必须清理 invocation-owned descendants。

## 2. Entry Criteria

执行本计划前：

- T1–T6 development complete。
- Unit/code-level tests PASS。
- `cargo fmt --all -- --check` PASS。
- `cargo check --workspace` PASS。
- `cargo test --workspace` PASS。
- frontend tests/typecheck/build PASS。
- `git diff --check` PASS。

若实现未完成：

```text
Integration = BLOCKED_BY_IMPLEMENTATION
```

不得以 fake shell / mock process runner替代需要验证的 process ownership cases。

## 3. Integration Boundary

Integration 可以：

- 使用真实 ConfigStore和 temporary config.json。
- 使用真实 Git Bash executable（process-related cases）。
- 直接实例化 BashDiscovery / CommandAnalyzer / PolicyEngine / BashRuntime / ExecutionPipeline。
- 使用 Tokio async integration tests。
- 使用 temporary Workspace。
- 使用真实 ApprovalManager / SessionGrantStore。
- 使用 React/provider/component integration tests。

Integration 不应：

- 把 analyzer unit tables重复记为完整 integration。
- 用 substring fixtures声称 command policy正确。
- 用只 kill parent的断言声称 process tree cleanup正确。
- 用 in-process RuntimeCore测试冒充真实 subprocess E2E。
- 依赖 External ChatGPT/Tunnel。

## 4. Required Integration Cases

### INT-01 — Legacy config without BashConfig defaults safely

旧 schema v1 config缺 `bash`。

期望：

```text
bash.autoDetect = true
bash.executablePath = null
tools.bash.enabled = false
policies.toolDefaults.bash = ASK
schemaVersion = 1
```

M0–M4字段保持原值。

### INT-02 — BashConfig persistence / configRevision

使用真实 ConfigStore依次：

1. autoDetect true → false。
2. set manual executablePath。
3. save identical config。
4. clear override。

期望 semantic change增加 revision；identical save不增加。

### INT-03 — Runtime snapshot carries BashConfig

AppConfig → RuntimeConfigSnapshot：

- BashConfig精确复制。
- SecurityMode精确复制。
- configRevision精确。
- 无 secret进入 snapshot。

### INT-04 — Manual executable valid

配置有效 Git Bash path：

- Runtime canonicalize。
- bounded `--version` probe成功。
- readiness=Ready。
- source=manual/equivalent。
- effective observation使用 Runtime canonical result。

### INT-05 — Invalid manual path does not auto-fallback

manual path存在但无效，同时机器另有可 auto-detect Git Bash。

期望：

```text
Bash = Unavailable
```

不得静默改用另一路径。

### INT-06 — Auto-detect is deterministic and Git-Bash-specific

验证候选顺序稳定，并且：

- supported Git-for-Windows bash可 Ready。
- arbitrary/unsupported bash executable不会仅因名为 `bash.exe` 就被接受。
- no candidate时返回 typed unavailable reason/remediation。

### INT-07 — Registry adds DeveloperShell without changing file capabilities

期望 stable order：

```text
read_file, glob, write, edit, bash
```

且：

- bash = DeveloperShell。
- risk=High。
- read/glob/write/edit capability不变。

### INT-08 — Mode / enabled / readiness states are distinct

组合验证：

```text
Normal + bash ON + ready        → UnavailableByMode
DeveloperTest + bash OFF        → UnavailableDisabled
DeveloperTest + bash ON + noexe → UnavailableByRuntime/readiness
DeveloperTest + ON + ready      → proceed to Policy
```

状态不能互相伪装。

### INT-09 — Bash catalog default ASK

DeveloperTest + enabled + ready + default config：

```text
effective decision = ASK
availability = AvailableWithApproval
```

MCP catalog应有 exposure资格。

### INT-10 — Command-specific DENY does not hide entire Bash catalog

创建：

```text
bash default ASK
BashCommand rule: git push → DENY
```

期望 Bash catalog仍 AvailableWithApproval；具体 `git push` invocation才 DENY。

### INT-11 — Existing Tool matcher wire compatibility

旧 M3 JSON：

```json
{"type":"tool","tool":"write"}
```

仍可 deserialize/serialize，并保持行为。

### INT-12 — BashCommand matcher persistence and validation

创建/保存/reload：

```text
executable=git
argvPrefix=["status"]
decision=ALLOW
```

期望 typed shape保持。

enabled invalid Bash matcher使 whole policy snapshot invalid。

### INT-13 — Simple classified command facts

`git status --short`：

- one segment。
- executable normalized to git。
- argv = status, --short。
- classification=Classified。
- normalized scope deterministic。

### INT-14 — Quote-aware analyzer

至少覆盖：

```text
printf "a && b"
git commit -m "x;y"
```

quoted operators不得被错误拆成 compound segments。

### INT-15 — Compound operator analysis

分别验证：

- `&&`
- `||`
- `;`
- `|`

segments/operators顺序稳定。

### INT-16 — Unclassifiable high-risk detection

至少覆盖：

- `$(...)`
- backticks。
- `eval`。
- `bash -c` / `sh -c`。
- interpreter inline code。
- unsafe/unsupported redirection。
- material dynamic expansion。
- malformed quotes。

全部至少为 `UnclassifiedHighRisk`。

### INT-17 — BashCommand prefix matching

rule：

```text
git + ["status"]
```

期望：

- `git status` match。
- `git status --short` match。
- `git stash` no match。
- `git statusx` no match。
- substring不可 match。

### INT-18 — Tool-level + command-level severity

同一 command同时匹配：

- Tool bash ALLOW。
- command ASK。
- command DENY。

验证 `DENY > ASK > ALLOW`，matched IDs去重稳定排序。

### INT-19 — Compound command highest severity

示例：

```text
git status → ALLOW
git push   → ASK
whole      → ASK
```

以及任一 segment DENY → whole DENY。

### INT-20 — Tool Default fallback

没有显式 match时：

- Bash default ALLOW → classified simple command ALLOW。
- default ASK → ASK。
- default DENY → DENY。

### INT-21 — Unclassified ASK floor cannot be lowered by broad ALLOW

设置：

```text
ToolMatcher bash → ALLOW
```

请求 dynamic/unclassified command。

期望：

```text
effective = ASK
```

不是 ALLOW。

### INT-22 — Explicit DENY still beats unclassified ASK floor

显式 matching DENY + unclassified command：

```text
effective = DENY
no spawn
```

### INT-23 — Bash request validation before spawn

覆盖：

- empty。
- whitespace。
- NUL。
- 8192 bytes boundary。
- >8192 bytes。

invalid cases不创建 child process。

### INT-24 — Bash cwd is canonical Workspace root

真实 BashRuntime执行 harmless command输出 cwd。

期望与 active Workspace canonical root一致。

同时测试 request schema不能传 cwd/env override。

### INT-25 — Process output / nonzero exit semantics

真实 Git Bash执行：

- stdout。
- stderr。
- nonzero exit。

期望：

```text
exitCode/stdout/stderr/durationMs
```

nonzero exit不是 transport failure。

### INT-26 — Output cap kills owned process tree

使用 deterministic high-output fixture超过 256 KiB。

期望：

- `BASH_OUTPUT_LIMIT`。
- parent结束。
- child/descendant结束。
- bounded capture。
- no orphan process。

### INT-27 — Timeout kills owned process tree

启动 parent + descendant长期运行 fixture。

达到 30s hard timeout或测试 seam的等价 production-limit path。

期望：

- `BASH_TIMEOUT`。
- entire owned tree terminated。
- descendant PID确认不再存活。
- unrelated control process不受影响。

### INT-28 — Cancellation kills owned process tree

运行中触发 invocation cancellation。

期望：

- `BASH_CANCELLED`。
- all descendants gone。
- cleanup idempotent。

### INT-29 — Concurrency limit

一个 Bash execution占用唯一 slot时发第二个执行。

期望第二个立即：

```text
BASH_CONCURRENCY_LIMIT
```

不进入无界队列。

### INT-30 — Runtime shutdown cancels active Bash

active Bash运行时停止 RuntimeCore/lifecycle。

期望：

- active tree terminated。
- no hanging task。
- counters收敛。
- existing M4 pending approvals/session grants也按原规则cleanup。

### INT-31 — Default ASK uses M4 same-request approval

DeveloperTest + ON + ready + Bash default ASK。

发起 classified command：

- same invocation进入 Pending。
- AllowOnce恢复原 invocation。
- 只 spawn一次。
- request ID贯穿。

### INT-32 — Bash Deny does not spawn

对 ASK pending点 Deny，或 Policy直接 DENY。

期望没有 Bash child/process-tree owner产生。

### INT-33 — Classified AllowForSession exact-match

批准 classified：

```text
git status --short
```

为 session。

下一次 exact normalized same command：

- 命中 grant。
- 不产生新 approval。

变化 command shape：

```text
git status
git status --branch
```

不得命中。

### INT-34 — Unclassified command cannot create reusable grant

unclassified ASK pending。

Approval DTO：

```text
allowForSessionEligible=false
```

若 control强制 AllowForSession：

- fail closed。
- 不创建 grant。
- 不 silently降级 AllowOnce。

### INT-35 — Grant cannot bypass mode/disabled/readiness/DENY

已有 Bash session grant后分别构造：

- Normal Mode。
- bash disabled。
- executable unavailable。
- explicit DENY。

全部不得执行。

### INT-36 — Post-approval Bash revalidation

Pending期间改变 Runtime readiness/capability fixture。

批准后仍必须重新检查，失败时不 spawn。

### INT-37 — MCP metadata is truthful

`bash` schema只接受 command。

annotations/equivalent metadata：

```text
readOnly=false
destructive=true
openWorld=true
```

不得添加假的 Workspace sandbox声明。

### INT-38 — Bash Activity/Audit lifecycle

至少覆盖：

- ALLOW execution。
- ASK approved。
- DENY。
- timeout。
- output limit。
- concurrency reject。

evidence包含 normalized classification / policy / approval / limit outcome / duration，但不包含 full env、unbounded output、secret。

### INT-39 — Tools UI desired/effective/restart

运行中修改：

- Bash enabled。
- Security mode。
- autoDetect/manual path。

期望 desired立即保存，effective保持 current generation，Restart Required显示；restart后收敛。

### INT-40 — Policies UI Bash matcher CRUD

创建/edit/disable/delete BashCommand rule。

验证 executable/argvPrefix typed validation，且 command rule不会被 UI展示成整个 Bash catalog policy。

### INT-41 — Approval Center classified/unclassified UX

Classified：

- Allow once/session/Deny/Create Rule。

Unclassified：

- session action不可用并有原因。
- Allow once/Deny仍可用。

### INT-42 — Create Rule handoff

classified Bash approval → BashCommand matcher draft。

期望：

- 不 auto-save。
- 不 auto-resolve current request。
- matcher facts与 Runtime analysis一致。

### INT-43 — No fake sandbox wording

Tools/Policies/Approval用户可见文本不得声称 Bash只能访问 Workspace。

允许准确表述：

```text
working directory is Workspace
Bash is not OS-sandboxed
```

### INT-44 — M0–M4 regression

默认 Bash OFF时验证：

- file tool registry/exposure不变。
- M4 file approvals / grants不变。
- M3 Policy semantics不变。
- M2 Tool Toggle不变。
- M1 atomic write/edit。
- M0 read/glob + Workspace boundary。

## 5. Evidence Format

每个 case记录：

```text
Case ID
PASS / FAIL / BLOCKED
Test name / command
SecurityMode
Bash enabled/readiness
Analyzer classification
Policy decision / matched IDs
Approval/grant result where applicable
Process result / exit code / limit category
PID/tree cleanup assertion where applicable
Activity/Audit assertion
```

不得记录 secret、完整环境或无界 output。

## 6. Exit Criteria

M5 Integration只有 INT-01 ～ INT-44全部满足才为 PASS。

特别要求：

- DeveloperShell mode gate PASS。
- Git Bash authoritative readiness PASS。
- analyzer + typed matcher PASS。
- unclassified ASK floor PASS。
- explicit DENY no-spawn PASS。
- M4 same-request approval reuse PASS。
- classified exact session grant PASS。
- unclassified no reusable grant PASS。
- timeout/output/cancel/shutdown process-tree cleanup PASS。
- concurrency PASS。
- UI desired/effective/restart PASS。
- M0–M4 regression PASS。

随后进入 `E2E_TEST_PLAN.md`。

## 7. Non-Claims

Integration PASS不代表：

```text
real release Runtime black-box Bash flow PASS
External ChatGPT Bash path PASS
Bash is OS sandboxed
M6 i18n complete
M7 migration/evidence hardening complete
M5 ACCEPTED / FROZEN
```
