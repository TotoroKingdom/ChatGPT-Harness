# M4 SPEC — Human Approval Workflow

> Status: DRAFT / READY FOR REVIEW  
> Milestone: M4 — Human Approval Workflow  
> Baseline: M3 — Configurable Policy, ACCEPTED / FROZEN  
> Sources: `../ChatGPT-Harness_V3_PRD.md`, `../ChatGPT-Harness_V3_ARCHITECTURE.md`, `../ChatGPT-Harness_V3_ROADMAP.md`, M0–M3 milestone docs

## 1. Objective

M4 将 M3 的：

```text
Policy = ASK
→ AwaitingApproval evidence
→ APPROVAL_REQUIRED
→ request ends
```

升级为真正的 Runtime-owned Human Approval lifecycle：

```text
MCP tools/call
      ↓
Capability / Workspace / Policy
      ↓
     ASK
      ↓
ApprovalManager registers exact pending request
      ↓
MCP request asynchronously waits
      ↓
Approval Center
      ↓
approvals.resolve
      ├ AllowOnce
      ├ AllowForSession
      └ Deny
      ↓
wake exact waiting request
      ↓
revalidate Runtime / Capability / Workspace
      ↓
execute or terminate
```

核心目标：

> ASK 不再丢失原始请求。审批必须恢复或终止**同一个 request identity**，而不是要求客户端重新调用一次工具。

## 2. Baseline Facts

当前代码已经具备：

- M3 immutable Policy snapshot。
- typed Policy evidence：effective decision / matched rule IDs / reason。
- ExecutionPipeline 内部 `AwaitingApproval` state。
- ASK 当前返回 `ExecutionError::ApprovalRequired`，不会执行。
- `ApprovalRequest` / `ApprovalStatus` / `ApprovalDecision` protocol DTO。
- Control methods `approvals.list` / `approvals.resolve` 已冻结。
- Tauri / HarnessControlClient 已有 approvals list/resolve transport surface。
- Runtime `approvals.list` 当前固定空集合；`approvals.resolve` 当前固定 `APPROVAL_NOT_FOUND`。
- MCP HTTP 运行在独立 Tokio/Axum thread，但当前 `McpService::call_tool` 内部仍同步执行 pipeline，并通过 ready future立即返回。
- M3 `ASK` tool仍属于 unavailable-by-policy 过渡 exposure。

M4 应接通现有 seams，而不是新建第二套 approval transport 或让 Tauri拥有 execution continuation。

## 3. Scope

M4 实现：

1. Runtime-owned `ApprovalManager`。
2. Pending approval registration/list/filter/resolve。
3. exact request identity + request fingerprint binding。
4. finite expiry。
5. Allow once。
6. Allow for session。
7. Deny。
8. memory-only `SessionGrantStore`。
9. ASK MCP request真正异步等待并恢复同一 request。
10. approval 后重新验证 readiness/capability/workspace。
11. Runtime stop/crash取消 pending approvals并清空 session grants。
12. M4 将 ASK tool升级为 MCP 可发现的 `AvailableWithApproval`。
13. Approval evidence 与 Activity/Audit关联。
14. Approval Center UI。
15. Create Rule handoff到 M3 Policy Editor，但不是 execution decision。
16. M0–M3 regression。

M4 不实现：

- Bash executor / Bash command analyzer。
- Bash-specific grant matcher。
- persistent approvals。
- approvals跨 Runtime restart恢复。
- persistent session grants。
- background/remote approval service。
- arbitrary approval delegation。
- approval绕过 System Guardrails / WorkspaceGuard / explicit DENY。
- Create Rule自动保存或自动批准当前请求。
- schemaVersion v1→v2 migration。
- i18n。

## 4. Ownership Boundary

Approval authority：

```text
Runtime
```

不是：

```text
React
Tauri
Tunnel
ChatGPT
```

Runtime拥有：

- exact request identity。
- validated typed request。
- policy evidence。
- request fingerprint。
- normalized grant scope。
- waiting continuation。
- Runtime generation identity。

Tauri只负责：

```text
Approval Center
→ Tauri command
→ JSONL approvals.list / approvals.resolve
→ Runtime
```

Tauri不得保存 execution continuation，也不得直接“放行” executor。

## 5. Protocol Evolution

### 5.1 ApprovalDecision

现有：

```text
AllowOnce
Deny
```

M4 additive扩展为：

```text
AllowOnce
AllowForSession
Deny
```

`Create Rule` **不是** `ApprovalDecision`。

### 5.2 ApprovalRequest

保留现有：

```text
requestId
tool
sanitizedArguments
createdAt
expiresAt
status
resolvedAt?
decision?
```

并 additive 增加足够的安全/evidence展示字段，至少：

```text
runtimeId / generation identity
matchedRuleIds
policyReason
riskLevel
scopeSummary / normalized display scope
```

具体 DTO 命名可局部调整，但必须满足：

- 不返回 raw file content。
- 不返回 secret。
- 不无必要暴露 absolute Workspace root。
- UI能明确知道自己正在批准什么 scope。
- Runtime internal fingerprint不必直接暴露给 UI。

### 5.3 Existing control methods remain authoritative

继续使用：

```text
approvals.list
approvals.resolve
```

不另建 HTTP approval API。

`ApprovalsListParams.status` 必须真实过滤。

## 6. Approval Identity Contract

一个 pending approval必须内部绑定：

```text
runtime_id
request_id
tool
request_fingerprint
validated request / continuation
policy evidence
approval scope
created_at
expires_at
```

### 6.1 Request ID

继续使用 ExecutionPipeline生成的 request UUID 作为当前 approval lookup identity，并保持 Control `approvals.resolve(requestId,...)` 兼容。

一次 invocation只能有一个当前 pending approval。

### 6.2 Exact request fingerprint

AllowOnce必须绑定 exact validated request。

fingerprint要求：

- Runtime生成。
- deterministic。
- 不由 UI传回/重构。
- 对 exact authorization-relevant request data敏感。
- 不把 raw secret/content写入 Activity/UI。

批准 Request A 绝不能唤醒 Request B。

## 7. Approval Scope Contract

ApprovalRequest向用户展示 Runtime生成的 normalized scope。

M4 file tools：

### read_file

```text
tool = read_file
scope = exact normalized workspace-relative file path
```

### write

```text
tool = write
scope = exact normalized workspace-relative target path
```

content不作为 UI grant scope，不展示 raw content。

### edit

```text
tool = edit
scope = exact normalized workspace-relative target path
```

old/new text不进入 session grant scope。

### glob

```text
tool = glob
scope = exact normalized glob pattern / query scope
```

Workspace generation本身隐含在 grant identity中。

## 8. Approval Lifetime

Pending approvals：

- memory-only。
- current Runtime generation only。
- finite TTL。
- 不写 AppConfig。
- 不跨 restart恢复。

M4冻结默认 TTL：

```text
5 minutes
```

实现应集中在 Runtime constant，例如：

```text
DEFAULT_APPROVAL_TTL
```

测试必须使用 injectable/test clock 或确定性 expiry seam，不得依赖长时间 sleep。

到期：

```text
Pending → Expired
waiting invocation → ApprovalExpired
no execution
```

历史 terminal record可以在当前 generation内继续通过 list观察；Runtime restart后全部消失。

## 9. ApprovalManager Contract

Runtime新增：

```text
ApprovalManager
```

建议内部以 thread-safe shared state持有：

```text
Arc<ApprovalManager>
```

因为：

- MCP request在独立 Tokio HTTP thread等待。
- Control JSONL thread需要同时 list/resolve。
- 两边不能共享 RuntimeCore mutable lock来阻塞彼此。

职责：

- register pending。
- list/filter。
- resolve pending exactly once。
- expire。
- cancel all。
- wake exact waiter。
- preserve bounded sanitized evidence。
- reject stale/duplicate resolutions。

## 10. Resolve Semantics

### 10.1 AllowOnce

```text
Pending
→ Approved(AllowOnce)
→ wake exact waiter
→ revalidate
→ execute exactly once
```

Approval token不可复用到第二次 invocation。

### 10.2 Deny

```text
Pending
→ Denied
→ wake exact waiter
→ terminate request
→ no executor side effect
```

返回稳定 typed error，例如：

```text
APPROVAL_DENIED
```

### 10.3 Duplicate / stale resolve

未知 request：

```text
APPROVAL_NOT_FOUND
```

已 Approved/Denied/Expired/Cancelled：

```text
APPROVAL_ALREADY_RESOLVED
```

并发 resolve只允许一个 winner；不得 double execution。

## 11. SessionGrantStore

AllowForSession创建：

```text
SessionGrantStore
```

性质：

- Runtime-owned。
- memory-only。
- current Runtime generation only。
- stop/crash/restart清空。
- 不写 AppConfig。
- 不能覆盖 explicit DENY。
- 不能覆盖 System Guardrail。
- 不能恢复 disabled/unavailable capability。

Policy execution顺序：

```text
Capability Gate
→ WorkspaceGuard
→ System Guardrails
→ User Policy / Tool Default
   ├ DENY → end
   ├ ALLOW → execute
   └ ASK
       ↓
   matching session grant?
       ├ yes → ALLOW
       └ no  → pending Approval
```

## 12. File-tool Session Grant Scope

M4必须选择不宽于用户看见的 approval scope。

冻结为：

```text
same Runtime generation
+
same tool
+
same normalized approval scope
```

因此：

- allow session for `write foo.txt` 不允许 `write bar.txt`。
- allow session for `edit foo.txt` 不允许 `write foo.txt`。
- allow session for one glob pattern不允许不同 pattern。
- grant不包含 raw file content。

这比“允许整个 write tool本 session”更窄，也更符合 Architecture 的 no-broader-than-displayed 要求。

## 13. Async Execution Contract

M4必须改变当前 ASK“立即返回”的同步路径。

### 13.1 Required behavior

MCP handler：

```text
async call_tool(...)
→ pipeline async-capable execution
→ ASK registers pending
→ await resolution without blocking Tokio thread
→ resume same future
```

禁止：

- `std::sync::Condvar::wait` 阻塞 MCP Tokio event loop。
- polling + sleep等待 UI。
- ASK立即返回错误并要求 ChatGPT重新调用。
- 把 pending request存到 Tauri。

推荐使用：

```text
tokio::sync::oneshot / watch / Notify
```

或等价 async primitive。

### 13.2 File executors

read/glob/write/edit executor内部可继续保留现有同步实现。

M4必须解决的是 approval wait非阻塞；无需为了 M4全面重写文件 I/O。

## 14. Post-Approval Revalidation

批准不是永久 permit。

waiter被唤醒后，执行前至少重新检查：

1. Runtime仍 Ready。
2. same Runtime generation仍有效。
3. Tool仍 capability-eligible / enabled。
4. Workspace target重新验证。
5. persistent Policy的 System/explicit DENY约束仍不能被绕过。
6. request fingerprint仍对应原 validated request。
7. mutation executor继续执行 M1 自己的提交前 TOCTOU revalidation。

任何 revalidation失败：

```text
approved request still fails
→ no unsafe execution
→ evidence explains post-approval rejection
```

## 15. Runtime Stop / Crash Semantics

Runtime stop开始时：

```text
all Pending → Cancelled
all waiting requests wake → ApprovalCancelled / RuntimeNotReady
SessionGrantStore.clear()
```

MCP shutdown不得永远等待 pending approvals。

Runtime crash自然失去 memory-only approvals/grants；新 generation不能恢复它们。

## 16. ASK MCP Exposure in M4

M3过渡：

```text
ALLOW → Available
ASK   → hidden/unavailable-by-policy
DENY  → hidden
```

M4正式升级：

```text
ALLOW → Available
ASK   → AvailableWithApproval
DENY  → UnavailableByPolicy
```

MCP `tools/list`：

```text
Available
AvailableWithApproval
```

都必须暴露。

disabled / runtime unavailable仍隐藏。

direct `tools/call` 每次仍重新评估 Capability + Workspace + Policy + Session Grant。

## 17. Activity / Audit Evidence

M4至少关联：

```text
request ID
tool
policy decision
matched rule IDs
policy reason
approval status
approval decision
created/resolved/expired/cancelled time
execution outcome
```

事件语义至少覆盖：

- ApprovalRequired / Pending。
- ApprovalResolved AllowOnce。
- ApprovalResolved AllowForSession。
- ApprovalDenied。
- ApprovalExpired。
- ApprovalCancelled。
- resumed ExecutionStarted / Completed / Failed。

同一 request必须可串联。

不得记录 raw content/secrets。

## 18. Create Rule Boundary

`Create Rule` 是 UI workflow，不是 Runtime execution decision。

流程：

```text
ApprovalRequest
→ Create Rule
→ open Policies editor draft
→ explicit user review
→ configSave
→ configRevision changes
→ applies next Runtime generation
```

当前 pending request仍保持 Pending，直到用户另行：

```text
AllowOnce
AllowForSession
Deny
```

### M3 matcher limitation

M3 persistent User Rule只有 tool-level matcher。

因此从单个 file approval创建规则时，draft可能比当前 exact file scope更宽：

```text
approval scope: write foo.txt
policy draft: tool = write
```

UI必须明确显示这一 widening，并要求显式保存；不得一键静默永久 ALLOW。

## 19. Approval Center UI

M4新增/启用 Approval Center 页面与导航。

Pending card至少展示：

- Tool。
- sanitized arguments。
- normalized target/pattern scope。
- matched policy IDs。
- policy reason。
- risk level。
- created time。
- expiry。
- Runtime/generation context（用户可理解形式）。

Actions：

```text
Allow once
Allow for session
Deny
Create rule
```

要求：

- resolve后立即刷新真实 Runtime state。
- terminal approval不可重复操作。
- expiry状态明确。
- Runtime stopped时不可继续 approve stale approval。
- Create Rule跳转/打开 M3 Policy editor draft，不直接resolve。

M4文案可继续英文；i18n留 M6。

## 20. Error Contract

建议扩展稳定 execution/MCP错误：

```text
APPROVAL_DENIED
APPROVAL_EXPIRED
APPROVAL_CANCELLED
```

Control已有：

```text
APPROVAL_NOT_FOUND
APPROVAL_ALREADY_RESOLVED
```

保留。

错误必须 sanitized，并与：

```text
POLICY_DENIED
TOOL_DISABLED
RUNTIME_NOT_READY
```

可区分。

## 21. Expected Code Touchpoints

```text
protocol/src/v1/approvals.rs
protocol/src/v1/common.rs / generated TS/schema if needed

runtime/src/approval/*                 # new
runtime/src/session_grant/*            # new or approval-owned module
runtime/src/execution/pipeline.rs
runtime/src/execution/context.rs
runtime/src/execution/state.rs
runtime/src/state.rs
runtime/src/server.rs
runtime/src/mcp/context.rs
runtime/src/mcp/service.rs
runtime/src/mcp/errors.rs
runtime/src/activity/* / audit/*

ui/src-tauri/src/commands/*
ui/src-tauri/src/runtime/*             # existing transport only

ui/src/pages/approvals/*               # new
ui/src/pages/policies/*                # Create Rule handoff only
ui/src/app/* / navigation
ui/src/services/harness/*
related tests
```

Tauri不得成为 execution proxy。

## 22. Compatibility Requirements

M4必须保持：

- M3 Policy precedence/fallback/invalid snapshot semantics。
- M2 Tool Toggle优先级。
- M1 atomic write/edit。
- WorkspaceGuard。
- ExecutionPermit只能由 Pipeline创建。
- AppConfig无新增 persistent approval/session grant state。
- default ALLOW tools无需审批。
- explicit DENY仍直接拒绝。
- old configs无需 migration。
- no protocol major bump。

## 23. Development Verification Requirements

每个 Task必须包含 unit/code-level tests。

完成至少运行：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
frontend tests / typecheck / build commands defined by repository
git diff --check
```

必须自动化覆盖：

```text
ASK exposed as AvailableWithApproval
ASK call remains pending instead of returning immediately
approvals.list returns pending request
AllowOnce resumes exact request once
Deny wakes exact request with no side effect
AllowForSession resumes request + creates narrow grant
matching grant bypasses new approval for same scope
different path/tool does not match grant
explicit DENY ignores grant
System Guardrail ignores grant
expiry wakes request and prevents execution
duplicate resolve has one winner
Runtime stop cancels pending + clears grants
post-approval Workspace revalidation
request identity/fingerprint isolation
Create Rule does not auto-resolve
M0–M3 regression
```

## 24. Post-TASK Verification Documents

SPEC / PLAN / TASKS冻结后再独立生成：

```text
INTEGRATION_TEST_PLAN.md
E2E_TEST_PLAN.md
MANUAL_ACCEPTANCE_CHECKLIST.md
```

Unit / Integration / E2E由 Coding Agent自动完成；Manual Acceptance才需要人工介入。

M4 Local Black-box E2E必须真实：

```text
Runtime subprocess
+ real Control RPC
+ loopback MCP HTTP
+ one MCP call held pending
+ concurrent approvals.list/resolve
+ same call resumes
+ filesystem/activity evidence
```

这是 M4 的核心 E2E，不允许用“先收到 APPROVAL_REQUIRED，再重新 call tool”冒充。

## 25. Final Acceptance Criteria

M4可 ACCEPTED / FROZEN 前必须证明：

- ASK tool在 MCP中可发现。
- ASK invocation保留原 request identity并真实 Pending。
- Approval Center能观察 pending request。
- AllowOnce只恢复精确原请求一次。
- Deny终止精确原请求且无 side effect。
- AllowForSession生成 current-generation narrow grant。
- grant不宽于 displayed approval scope。
- grant不能覆盖 explicit DENY/System Guardrail/disabled capability。
- expiry正确终止等待请求。
- duplicate/stale resolve安全拒绝。
- approval后重新验证 Workspace/readiness/capability。
- Runtime stop/crash不遗留 pending或session grants。
- Activity/Audit能关联 policy→approval→execution。
- Create Rule只创建 M3 policy draft并应用于下代，不自动批准当前请求。
- M0–M3 regression PASS。
- Integration / Local Black-box E2E / Manual Acceptance完成。

## 26. Open Questions

```text
0
```

局部 module/type命名可调整，但不得改变 Runtime ownership、same-request resume、narrow session grant、async wait、post-approval revalidation和 M5边界。
